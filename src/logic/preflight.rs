//! Preflight summary computation helpers.
//!
//! The routines in this module gather package metadata, estimate download and
//! install deltas, and derive risk heuristics used to populate the preflight
//! modal. All command execution is abstracted behind [`CommandRunner`] so the
//! logic can be exercised in isolation.

use crate::state::modal::{
    PreflightAction, PreflightHeaderChips, PreflightPackageSummary, PreflightSummaryData, RiskLevel,
};
use crate::state::types::{PackageItem, Source};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

/// Packages that contribute additional risk when present in a transaction.
const CORE_CRITICAL_PACKAGES: &[&str] = &[
    "linux",
    "linux-lts",
    "linux-zen",
    "systemd",
    "glibc",
    "openssl",
    "pacman",
    "bash",
    "util-linux",
    "filesystem",
];

/// What: Abstract command execution interface used for spawning helper
/// binaries such as `pacman`.
///
/// Inputs:
/// - `program`: Executable name to run (for example, `"pacman"`).
/// - `args`: Slice of positional arguments passed to the executable.
///
/// Output:
/// - `Ok(String)` containing UTF-8 stdout on success.
/// - `Err(CommandError)` when the invocation fails or stdout is not valid UTF-8.
///
/// Details:
/// - Implementations may stub command results to enable deterministic unit
///   testing.
/// - Production code relies on [`SystemCommandRunner`].
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError>;
}

/// What: Real command runner backed by `std::process::Command`.
///
/// Inputs: Satisfies the [`CommandRunner`] trait without additional parameters.
///
/// Output:
/// - Executes commands on the host system and captures stdout.
///
/// Details:
/// - Errors from `std::process::Command::output` are surfaced as
///   [`CommandError::Io`].
#[derive(Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
        let output = std::process::Command::new(program).args(args).output()?;
        if !output.status.success() {
            return Err(CommandError::Failed {
                program: program.to_string(),
                args: args.iter().map(|s| s.to_string()).collect(),
                status: output.status,
            });
        }
        Ok(String::from_utf8(output.stdout)?)
    }
}

/// What: Error type capturing command spawning, execution, and decoding
/// failures.
///
/// Inputs: Generated internally by helper routines.
///
/// Output: Implements `Display`/`Error` for ergonomic propagation.
///
/// Details:
/// - Wraps I/O errors, UTF-8 conversion failures, parsing issues, and
///   non-success exit statuses.
#[derive(Debug)]
pub enum CommandError {
    Io(std::io::Error),
    Utf8(std::string::FromUtf8Error),
    Failed {
        program: String,
        args: Vec<String>,
        status: std::process::ExitStatus,
    },
    Parse {
        program: String,
        field: String,
    },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::Io(err) => write!(f, "I/O error: {err}"),
            CommandError::Utf8(err) => write!(f, "UTF-8 decoding error: {err}"),
            CommandError::Failed {
                program,
                args,
                status,
            } => {
                write!(f, "{program:?} {args:?} exited with status {status}")
            }
            CommandError::Parse { program, field } => {
                write!(
                    f,
                    "{program} output did not contain expected field \"{field}\""
                )
            }
        }
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CommandError::Io(err) => Some(err),
            CommandError::Utf8(err) => Some(err),
            CommandError::Failed { .. } | CommandError::Parse { .. } => None,
        }
    }
}

impl From<std::io::Error> for CommandError {
    fn from(value: std::io::Error) -> Self {
        CommandError::Io(value)
    }
}

impl From<std::string::FromUtf8Error> for CommandError {
    fn from(value: std::string::FromUtf8Error) -> Self {
        CommandError::Utf8(value)
    }
}

/// What: Aggregated preflight summary payload plus header chip metrics.
///
/// Inputs: Produced by the summary computation helpers.
///
/// Output:
/// - `summary`: Structured data powering the Summary tab.
/// - `header`: Condensed metrics displayed in the modal header and execution
///   sidebar.
///
/// Details:
/// - Bundled together so downstream code can reuse the derived chip data
///   without recomputation.
#[derive(Debug, Clone)]
pub struct PreflightSummaryOutcome {
    pub summary: PreflightSummaryData,
    pub header: PreflightHeaderChips,
}

/// What: Compute preflight summary data using the system command runner.
///
/// Inputs:
/// - `items`: Packages scheduled for install/update/remove.
/// - `action`: Active operation (install vs. remove) shaping the analysis.
///
/// Output:
/// - [`PreflightSummaryOutcome`] combining Summary tab data and header chips.
///
/// Details:
/// - Delegates to [`compute_preflight_summary_with_runner`] with
///   [`SystemCommandRunner`].
/// - Metadata lookups that fail are logged and treated as best-effort.
pub fn compute_preflight_summary(
    items: &[PackageItem],
    action: PreflightAction,
) -> PreflightSummaryOutcome {
    let runner = SystemCommandRunner;
    compute_preflight_summary_with_runner(items, action, &runner)
}

/// What: Compute preflight summary data using a custom command runner.
///
/// Inputs:
/// - `items`: Packages to analyse.
/// - `action`: Install vs. remove context.
/// - `runner`: Command execution abstraction (mockable).
///
/// Output:
/// - [`PreflightSummaryOutcome`] with fully materialised Summary data and
///   header chip metrics.
///
/// Details:
/// - Fetches installed versions/sizes via `pacman` when possible.
/// - Applies the initial risk heuristic outlined in the specification.
/// - Gracefully degrades metrics when metadata is unavailable.
pub fn compute_preflight_summary_with_runner<R: CommandRunner>(
    items: &[PackageItem],
    action: PreflightAction,
    runner: &R,
) -> PreflightSummaryOutcome {
    let _span = tracing::info_span!(
        "compute_preflight_summary",
        stage = "summary",
        item_count = items.len()
    )
    .entered();
    let start_time = std::time::Instant::now();
    let mut packages = Vec::with_capacity(items.len());
    let mut aur_count = 0usize;
    let mut total_download_bytes = 0u64;
    let mut total_install_delta_bytes = 0i64;

    let mut major_bump_packages = Vec::new();
    let mut core_system_updates = Vec::new();
    let mut summary_notes = Vec::new();
    let mut summary_warnings = Vec::new();
    let mut risk_reasons = Vec::new();

    let pacnew_candidates = 0usize;
    let pacsave_candidates = 0usize;
    let config_warning_packages = Vec::new();
    let service_restart_units = Vec::new();

    let mut any_major_bump = false;
    let mut any_core_update = false;
    let mut any_aur = false;

    // Batch fetch installed versions and sizes for all packages
    let installed_versions = batch_fetch_installed_versions(runner, items);
    let installed_sizes = batch_fetch_installed_sizes(runner, items);

    for (idx, item) in items.iter().enumerate() {
        if matches!(item.source, Source::Aur) {
            aur_count += 1;
            any_aur = true;
        }

        // Use batched results
        let installed_version = installed_versions
            .get(idx)
            .and_then(|v| v.as_ref().ok())
            .cloned();
        let installed_size = installed_sizes
            .get(idx)
            .and_then(|s| s.as_ref().ok())
            .copied();

        if installed_version.is_none() {
            tracing::debug!(
                "Preflight summary: failed to fetch installed version for {}",
                item.name
            );
        }
        if installed_size.is_none() {
            tracing::debug!(
                "Preflight summary: failed to fetch installed size for {}",
                item.name
            );
        }

        let mut download_bytes = None;
        let mut install_size_target = None;

        if let Source::Official { repo, .. } = &item.source {
            match fetch_official_metadata(runner, repo, &item.name, item.version.as_str()) {
                Ok(meta) => {
                    download_bytes = meta.download_size;
                    install_size_target = meta.install_size;
                }
                Err(err) => tracing::debug!(
                    "Preflight summary: failed to fetch metadata for {repo}/{pkg}: {err}",
                    pkg = item.name
                ),
            }
        }

        let install_delta_bytes = match action {
            PreflightAction::Install => {
                if let Some(target) = install_size_target {
                    let current = installed_size.unwrap_or(0);
                    Some(target as i64 - current as i64)
                } else {
                    None
                }
            }
            PreflightAction::Remove => installed_size.map(|size| -(size as i64)),
        };

        if let Some(bytes) = download_bytes {
            total_download_bytes = total_download_bytes.saturating_add(bytes);
        }
        if let Some(delta) = install_delta_bytes {
            total_install_delta_bytes = total_install_delta_bytes.saturating_add(delta);
        }

        let target_version = item.version.clone();
        let mut notes = Vec::new();
        let mut is_major_bump = false;
        let mut is_downgrade = false;

        if let Some(ref current) = installed_version {
            match compare_versions(current, &target_version) {
                Ordering::Greater => {
                    if matches!(action, PreflightAction::Install) {
                        is_downgrade = true;
                        notes.push(format!("Downgrade detected: {current} → {target_version}"));
                    }
                }
                Ordering::Less => {
                    if is_major_version_bump(current, &target_version) {
                        is_major_bump = true;
                        any_major_bump = true;
                        major_bump_packages.push(item.name.clone());
                        notes.push(format!("Major version bump: {current} → {target_version}"));
                    }
                }
                Ordering::Equal => {}
            }
        } else if matches!(action, PreflightAction::Install) {
            notes.push("New installation".to_string());
        }

        let normalized_name = item.name.to_ascii_lowercase();
        if CORE_CRITICAL_PACKAGES
            .iter()
            .any(|candidate| normalized_name == *candidate)
        {
            any_core_update = true;
            core_system_updates.push(item.name.clone());
            if matches!(action, PreflightAction::Remove) {
                notes.push("Removing core/system package".to_string());
            } else {
                notes.push("Core/system package update".to_string());
            }
        }

        packages.push(PreflightPackageSummary {
            name: item.name.clone(),
            source: item.source.clone(),
            installed_version,
            target_version,
            is_downgrade,
            is_major_bump,
            download_bytes,
            install_delta_bytes,
            notes,
        });
    }

    if any_core_update {
        risk_reasons.push("Core/system packages involved (+3)".to_string());
    }
    if any_major_bump {
        risk_reasons.push("Major version bump detected (+2)".to_string());
    }
    if any_aur {
        risk_reasons.push("AUR packages included (+2)".to_string());
    }
    if pacnew_candidates > 0 {
        risk_reasons.push("Configuration files may produce .pacnew (+1)".to_string());
    }
    if !service_restart_units.is_empty() {
        risk_reasons.push("Services likely require restart (+1)".to_string());
    }

    let mut risk_score: u8 = 0;
    if any_core_update {
        risk_score = risk_score.saturating_add(3);
    }
    if any_major_bump {
        risk_score = risk_score.saturating_add(2);
    }
    if any_aur {
        risk_score = risk_score.saturating_add(2);
    }
    if pacnew_candidates > 0 {
        risk_score = risk_score.saturating_add(1);
    }
    if !service_restart_units.is_empty() {
        risk_score = risk_score.saturating_add(1);
    }

    let risk_level = match risk_score {
        0 => RiskLevel::Low,
        1..=4 => RiskLevel::Medium,
        _ => RiskLevel::High,
    };

    if any_core_update {
        summary_notes.push("Core/system packages will be modified.".to_string());
    }
    if any_major_bump {
        summary_notes.push("Major version changes detected; review changelogs.".to_string());
    }
    if any_aur {
        summary_notes.push("AUR packages present; build steps may vary.".to_string());
    }

    if summary_warnings.is_empty() {
        summary_warnings.extend(risk_reasons.iter().cloned());
    }

    let summary = PreflightSummaryData {
        packages,
        package_count: items.len(),
        aur_count,
        download_bytes: total_download_bytes,
        install_delta_bytes: total_install_delta_bytes,
        risk_score,
        risk_level,
        risk_reasons: risk_reasons.clone(),
        major_bump_packages,
        core_system_updates,
        pacnew_candidates,
        pacsave_candidates,
        config_warning_packages,
        service_restart_units,
        summary_warnings,
        summary_notes,
    };

    let header = PreflightHeaderChips {
        package_count: items.len(),
        download_bytes: total_download_bytes,
        install_delta_bytes: total_install_delta_bytes,
        aur_count,
        risk_score,
        risk_level,
    };

    let elapsed = start_time.elapsed();
    let duration_ms = elapsed.as_millis() as u64;
    tracing::info!(
        stage = "summary",
        item_count = items.len(),
        duration_ms = duration_ms,
        "Preflight summary computation complete"
    );

    PreflightSummaryOutcome { summary, header }
}

/// What: Extract remote download/install sizes for an official package via
/// `pacman -Si`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `repo`: Repository name (e.g., `"core"`).
/// - `name`: Package identifier.
/// - `expected_version`: Version string to cross-check.
///
/// Output:
/// - `Ok(OfficialMetadata)` containing optional size metrics.
/// - `Err(CommandError)` when the command fails.
///
/// Details:
/// - Performs best-effort verification of the returned version, logging
///   mismatches for diagnostics.
fn fetch_official_metadata<R: CommandRunner>(
    runner: &R,
    repo: &str,
    name: &str,
    expected_version: &str,
) -> Result<OfficialMetadata, CommandError> {
    let spec = format!("{repo}/{name}");
    let output = runner.run("pacman", &["-Si", &spec])?;
    let fields = parse_pacman_key_values(&output);

    if let Some(version) = fields.get("Version")
        && version.trim() != expected_version
    {
        tracing::debug!(
            "Preflight summary: pacman -Si reported version {} for {} (expected {})",
            version.trim(),
            spec,
            expected_version
        );
    }

    let download_size = fields
        .get("Download Size")
        .and_then(|raw| parse_size_to_bytes(raw));
    let install_size = fields
        .get("Installed Size")
        .and_then(|raw| parse_size_to_bytes(raw));

    Ok(OfficialMetadata {
        download_size,
        install_size,
    })
}

/// What: Retrieve installed package version via `pacman -Q`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `name`: Package identifier.
///
/// Output:
/// - `Ok(String)` containing the installed version.
/// - `Err(CommandError)` when fetch fails.
///
/// Details:
/// - Trims stdout and returns the last whitespace-separated token.
fn fetch_installed_version<R: CommandRunner>(
    runner: &R,
    name: &str,
) -> Result<String, CommandError> {
    let output = runner.run("pacman", &["-Q", name])?;
    let mut parts = output.split_whitespace();
    let _pkg_name = parts.next();
    parts
        .next_back()
        .map(|value| value.to_string())
        .ok_or_else(|| CommandError::Parse {
            program: "pacman -Q".to_string(),
            field: "version".to_string(),
        })
}

/// What: Retrieve the installed size of a package via `pacman -Qi`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `name`: Package identifier.
///
/// Output:
/// - `Ok(u64)` representing bytes installed.
/// - `Err(CommandError)` when parsing fails.
///
/// Details:
/// - Parses the `Installed Size` field using [`parse_size_to_bytes`].
fn fetch_installed_size<R: CommandRunner>(runner: &R, name: &str) -> Result<u64, CommandError> {
    let output = runner.run("pacman", &["-Qi", name])?;
    let fields = parse_pacman_key_values(&output);
    fields
        .get("Installed Size")
        .and_then(|raw| parse_size_to_bytes(raw))
        .ok_or_else(|| CommandError::Parse {
            program: "pacman -Qi".to_string(),
            field: "Installed Size".to_string(),
        })
}

/// What: Metadata extracted from `pacman -Si` to inform download/install
/// calculations.
///
/// Inputs: Populated by [`fetch_official_metadata`].
///
/// Output: Holds optional download and install sizes in bytes.
///
/// Details:
/// - Values are `None` when the upstream output omits a field.
#[derive(Default, Debug)]
struct OfficialMetadata {
    download_size: Option<u64>,
    install_size: Option<u64>,
}

/// What: Transform pacman key-value output into a `HashMap`.
///
/// Inputs:
/// - `output`: Raw stdout from `pacman` invocations.
///
/// Output:
/// - `HashMap<String, String>` mapping field names to raw string values.
///
/// Details:
/// - Continuation lines (prefixed with a space) are appended to the previous
///   key's value.
fn parse_pacman_key_values(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut last_key: Option<String> = None;

    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_string();
            let val = value.trim().to_string();
            map.insert(key.clone(), val);
            last_key = Some(key);
        } else if line.starts_with(' ')
            && let Some(key) = &last_key
        {
            map.entry(key.clone())
                .and_modify(|existing| {
                    if !existing.ends_with(' ') {
                        existing.push(' ');
                    }
                    existing.push_str(line.trim());
                })
                .or_insert_with(|| line.trim().to_string());
        }
    }

    map
}

/// What: Convert human-readable pacman size strings to bytes.
///
/// Inputs:
/// - `raw`: String such as `"1.5 MiB"` or `"512 KiB"`.
///
/// Output:
/// - `Some(u64)` with byte representation on success.
/// - `None` when parsing fails.
///
/// Details:
/// - Supports B, KiB, MiB, GiB, and TiB units.
fn parse_size_to_bytes(raw: &str) -> Option<u64> {
    let mut parts = raw.split_whitespace();
    let number = parts.next()?.replace(',', ".");
    let value = number.parse::<f64>().ok()?;
    let unit = parts.next().unwrap_or("B");
    let multiplier = match unit {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((value * multiplier) as u64)
}

/// What: Compare dotted version strings numerically.
///
/// Inputs:
/// - `a`: Left-hand version.
/// - `b`: Right-hand version.
///
/// Output:
/// - `Ordering` indicating which version is greater.
///
/// Details:
/// - Splits on `.` and `-`, comparing numeric segments when possible and
///   falling back to lexicographical comparison.
fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split(['.', '-']).collect();
    let b_parts: Vec<&str> = b.split(['.', '-']).collect();
    let len = a_parts.len().max(b_parts.len());

    for idx in 0..len {
        let a_seg = a_parts.get(idx).copied().unwrap_or("0");
        let b_seg = b_parts.get(idx).copied().unwrap_or("0");

        match (a_seg.parse::<i64>(), b_seg.parse::<i64>()) {
            (Ok(a_num), Ok(b_num)) => match a_num.cmp(&b_num) {
                Ordering::Equal => continue,
                ord => return ord,
            },
            _ => match a_seg.cmp(b_seg) {
                Ordering::Equal => continue,
                ord => return ord,
            },
        }
    }

    Ordering::Equal
}

/// What: Determine whether `new` constitutes a major version bump relative to
/// `old`.
///
/// Inputs:
/// - `old`: Currently installed version.
/// - `new`: Target version.
///
/// Output:
/// - `true` when the major component increased; `false` otherwise.
///
/// Details:
/// - Parses the first numeric segment (before `.`/`-`) for comparison.
fn is_major_version_bump(old: &str, new: &str) -> bool {
    match (extract_major_component(old), extract_major_component(new)) {
        (Some(old_major), Some(new_major)) => new_major > old_major,
        _ => false,
    }
}

/// What: Extract the leading numeric component from a version string.
///
/// Inputs:
/// - `version`: Version string to parse.
///
/// Output:
/// - `Some(u64)` for the first numeric segment.
/// - `None` when parsing fails.
///
/// Details:
/// - Splits on `.` and `-`, treating the first token as the major component.
fn extract_major_component(version: &str) -> Option<u64> {
    let token = version.split(['.', '-']).next()?;
    token.parse::<u64>().ok()
}

/// What: Batch fetch installed versions for multiple packages using `pacman -Q`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `items`: Packages to query.
///
/// Output:
/// - Vector of results, one per package (Ok(version) or Err).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - `pacman -Q` outputs "name version" per line, one per package.
fn batch_fetch_installed_versions<R: CommandRunner>(
    runner: &R,
    items: &[PackageItem],
) -> Vec<Result<String, CommandError>> {
    const BATCH_SIZE: usize = 50;
    let mut results = Vec::with_capacity(items.len());

    for chunk in items.chunks(BATCH_SIZE) {
        let names: Vec<&str> = chunk.iter().map(|i| i.name.as_str()).collect();
        let mut args = vec!["-Q"];
        args.extend(names.iter().copied());
        match runner.run("pacman", &args) {
            Ok(output) => {
                // Parse output: each line is "name version"
                let mut version_map = std::collections::HashMap::new();
                for line in output.lines() {
                    let mut parts = line.split_whitespace();
                    if let (Some(name), Some(version)) = (parts.next(), parts.next_back()) {
                        version_map.insert(name, version.to_string());
                    }
                }
                // Map results back to original order
                for item in chunk {
                    if let Some(version) = version_map.get(item.name.as_str()) {
                        results.push(Ok(version.clone()));
                    } else {
                        results.push(Err(CommandError::Parse {
                            program: "pacman -Q".to_string(),
                            field: format!("version for {}", item.name),
                        }));
                    }
                }
            }
            Err(_) => {
                // If batch fails, fall back to individual queries
                for item in chunk {
                    match fetch_installed_version(runner, &item.name) {
                        Ok(v) => results.push(Ok(v)),
                        Err(err) => results.push(Err(err)),
                    }
                }
            }
        }
    }
    results
}

/// What: Batch fetch installed sizes for multiple packages using `pacman -Qi`.
///
/// Inputs:
/// - `runner`: Command executor.
/// - `items`: Packages to query.
///
/// Output:
/// - Vector of results, one per package (Ok(size_bytes) or Err).
///
/// Details:
/// - Batches queries into chunks of 50 to avoid command-line length limits.
/// - Parses multi-package `pacman -Qi` output (packages separated by blank lines).
fn batch_fetch_installed_sizes<R: CommandRunner>(
    runner: &R,
    items: &[PackageItem],
) -> Vec<Result<u64, CommandError>> {
    const BATCH_SIZE: usize = 50;
    let mut results = Vec::with_capacity(items.len());

    for chunk in items.chunks(BATCH_SIZE) {
        let names: Vec<&str> = chunk.iter().map(|i| i.name.as_str()).collect();
        let mut args = vec!["-Qi"];
        args.extend(names.iter().copied());
        match runner.run("pacman", &args) {
            Ok(output) => {
                // Parse multi-package output: packages are separated by blank lines
                let mut package_blocks = Vec::new();
                let mut current_block = String::new();
                for line in output.lines() {
                    if line.trim().is_empty() {
                        if !current_block.is_empty() {
                            package_blocks.push(current_block.clone());
                            current_block.clear();
                        }
                    } else {
                        current_block.push_str(line);
                        current_block.push('\n');
                    }
                }
                if !current_block.is_empty() {
                    package_blocks.push(current_block);
                }

                // Parse each block to extract package name and size
                let mut size_map = std::collections::HashMap::new();
                for block in package_blocks {
                    let block_fields = parse_pacman_key_values(&block);
                    if let (Some(name), Some(size_str)) = (
                        block_fields.get("Name").map(|s| s.trim()),
                        block_fields.get("Installed Size").map(|s| s.trim()),
                    ) && let Some(size_bytes) = parse_size_to_bytes(size_str)
                    {
                        size_map.insert(name.to_string(), size_bytes);
                    }
                }

                // Map results back to original order
                for item in chunk {
                    if let Some(size) = size_map.get(&item.name) {
                        results.push(Ok(*size));
                    } else {
                        results.push(Err(CommandError::Parse {
                            program: "pacman -Qi".to_string(),
                            field: format!("Installed Size for {}", item.name),
                        }));
                    }
                }
            }
            Err(_) => {
                // If batch fails, fall back to individual queries
                for item in chunk {
                    match fetch_installed_size(runner, &item.name) {
                        Ok(s) => results.push(Ok(s)),
                        Err(err) => results.push(Err(err)),
                    }
                }
            }
        }
    }
    results
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex;

    type MockCommandKey = (String, Vec<String>);
    type MockCommandResult = Result<String, CommandError>;
    type MockResponseMap = HashMap<MockCommandKey, MockCommandResult>;

    #[derive(Default)]
    struct MockRunner {
        responses: Mutex<MockResponseMap>,
    }

    impl MockRunner {
        fn with(responses: MockResponseMap) -> Self {
            Self {
                responses: Mutex::new(responses),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
            let key = (
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            );
            let mut guard = self.responses.lock().expect("poisoned responses mutex");
            guard.remove(&key).unwrap_or_else(|| {
                Err(CommandError::Failed {
                    program: program.to_string(),
                    args: args.iter().map(|s| s.to_string()).collect(),
                    status: std::process::ExitStatus::from_raw(1),
                })
            })
        }
    }

    #[test]
    /// What: Ensure core package major bumps elevate risk and populate notes.
    ///
    /// Inputs:
    /// - Single core package (`systemd`) transitioning from `1.0.0` to `2.0.0`.
    ///
    /// Output:
    /// - Risk score escalates to the "High" bucket with appropriate notes and chip totals.
    fn summary_identifies_core_major_bump() {
        let mut responses = HashMap::new();
        responses.insert(
            ("pacman".into(), vec!["-Q".into(), "systemd".into()]),
            Ok("systemd 1.0.0\n".to_string()),
        );
        responses.insert(
            ("pacman".into(), vec!["-Qi".into(), "systemd".into()]),
            Ok("Name            : systemd\nInstalled Size  : 4.00 MiB\n".to_string()),
        );
        responses.insert(
            ("pacman".into(), vec!["-Si".into(), "extra/systemd".into()]),
            Ok("Repository      : extra\nName            : systemd\nVersion         : 2.0.0\nDownload Size   : 2.00 MiB\nInstalled Size  : 5.00 MiB\n".to_string()),
        );

        let runner = MockRunner::with(responses);
        let item = PackageItem {
            name: "systemd".into(),
            version: "2.0.0".into(),
            description: "system init".into(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        };

        let outcome =
            compute_preflight_summary_with_runner(&[item], PreflightAction::Install, &runner);

        assert_eq!(outcome.summary.package_count, 1);
        assert_eq!(outcome.summary.aur_count, 0);
        assert_eq!(outcome.summary.risk_score, 5);
        assert_eq!(outcome.summary.risk_level, RiskLevel::High);
        assert!(
            outcome
                .summary
                .major_bump_packages
                .iter()
                .any(|name| name == "systemd")
        );
        assert!(
            outcome
                .summary
                .core_system_updates
                .iter()
                .any(|name| name == "systemd")
        );
        assert_eq!(
            outcome.summary.download_bytes,
            2 * 1024 * 1024,
            "Download bytes should match pacman -Si output"
        );
        assert_eq!(
            outcome.summary.install_delta_bytes,
            (5 * 1024 * 1024) as i64 - (4 * 1024 * 1024) as i64,
            "Install delta should reflect target minus current size"
        );
        assert!(
            outcome
                .summary
                .summary_warnings
                .iter()
                .any(|reason| reason.contains("Core/system"))
        );
        assert_eq!(outcome.header.risk_score, 5);
        assert_eq!(outcome.header.package_count, 1);
        assert_eq!(
            outcome.summary.packages[0].install_delta_bytes,
            Some((5 * 1024 * 1024) as i64 - (4 * 1024 * 1024) as i64)
        );
    }

    #[test]
    /// What: Confirm AUR-only transactions contribute to risk heuristics even without metadata.
    ///
    /// Inputs:
    /// - Single AUR package with no pacman metadata responses configured.
    ///
    /// Output:
    /// - Risk score increments by the AUR heuristic and remains within the "Medium" bucket.
    fn summary_handles_aur_without_metadata() {
        let runner = MockRunner::default();
        let item = PackageItem {
            name: "my-aur-tool".into(),
            version: "1.4.0".into(),
            description: "AUR utility".into(),
            source: Source::Aur,
            popularity: Some(42.0),
        };

        let outcome =
            compute_preflight_summary_with_runner(&[item], PreflightAction::Install, &runner);

        assert_eq!(outcome.summary.package_count, 1);
        assert_eq!(outcome.summary.aur_count, 1);
        assert_eq!(outcome.summary.risk_score, 2);
        assert_eq!(outcome.summary.risk_level, RiskLevel::Medium);
        assert!(
            outcome
                .summary
                .risk_reasons
                .iter()
                .any(|reason| reason.contains("AUR"))
        );
        assert_eq!(outcome.header.aur_count, 1);
    }
}
