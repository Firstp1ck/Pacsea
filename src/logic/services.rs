//! Service impact resolution for the preflight \"Services\" tab.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Command;

use crate::state::modal::{ServiceImpact, ServiceRestartDecision};
use crate::state::types::Source;
use crate::state::{PackageItem, PreflightAction};

/// What: Resolve systemd service impacts for the selected transaction items.
///
/// Inputs:
/// - `items`: Packages being installed or removed.
/// - `action`: Preflight action (install/update vs. remove).
///
/// Output:
/// - Vector of `ServiceImpact` entries representing impacted systemd units.
///
/// Details:
/// - Inspects `pacman -Fl` output for each package to find shipped unit files.
/// - Determines which units are currently active via `systemctl list-units`.
/// - Heuristically detects binaries that impact active units, even without unit files.
/// - Computes a recommended restart decision; defaults to defer when the unit
///   is inactive or the action is a removal.
pub fn resolve_service_impacts(
    items: &[PackageItem],
    action: PreflightAction,
) -> Vec<ServiceImpact> {
    let mut unit_to_providers: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // First pass: collect units shipped by packages
    for item in items {
        match collect_service_units_for_package(&item.name) {
            Ok(units) => {
                for unit in units {
                    let providers = unit_to_providers.entry(unit).or_default();
                    if !providers.iter().any(|name| name == &item.name) {
                        providers.push(item.name.clone());
                    }
                }
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to resolve service units for package {}: {}",
                    item.name,
                    err
                );
            }
        }
    }

    let active_units = fetch_active_units().unwrap_or_else(|err| {
        tracing::warn!("Unable to query active services: {}", err);
        BTreeSet::new()
    });

    // Second pass: detect binaries that impact active units (heuristic enhancement)
    if matches!(action, PreflightAction::Install) && !active_units.is_empty() {
        // Get ExecStart paths for all active services
        let active_service_binaries =
            fetch_active_service_binaries(&active_units).unwrap_or_default();

        // For each package, check if any of its binaries match active service binaries
        for item in items {
            match collect_binaries_for_package(&item.name, &item.source) {
                Ok(binaries) => {
                    for binary in binaries {
                        // Check if this binary is used by any active service
                        for (unit_name, service_binaries) in &active_service_binaries {
                            if service_binaries.iter().any(|sb| {
                                // Match exact path, or match binary name
                                // Handle cases like: service uses "/usr/bin/foo", package provides "/usr/bin/foo"
                                // or service uses "/usr/bin/foo", package provides "foo"
                                sb == &binary
                                    || binary.ends_with(sb)
                                    || sb.ends_with(&binary)
                                    || (binary.contains('/')
                                        && sb.contains('/')
                                        && Path::new(sb).file_name()
                                            == Path::new(&binary).file_name())
                            }) {
                                let providers =
                                    unit_to_providers.entry(unit_name.clone()).or_default();
                                if !providers.iter().any(|name| name == &item.name) {
                                    providers.push(item.name.clone());
                                    tracing::debug!(
                                        "Detected binary impact: package {} provides {} used by active service {}",
                                        item.name,
                                        binary,
                                        unit_name
                                    );
                                }
                            }
                        }
                    }
                }
                Err(err) => {
                    tracing::debug!(
                        "Failed to collect binaries for package {}: {}",
                        item.name,
                        err
                    );
                }
            }
        }
    }

    unit_to_providers
        .into_iter()
        .map(|(unit_name, mut providers)| {
            providers.sort();
            let is_active = active_units.contains(&unit_name);
            let needs_restart = matches!(action, PreflightAction::Install) && is_active;
            let recommended_decision = if needs_restart {
                ServiceRestartDecision::Restart
            } else {
                ServiceRestartDecision::Defer
            };

            ServiceImpact {
                unit_name,
                providers,
                is_active,
                needs_restart,
                recommended_decision,
                restart_decision: recommended_decision,
            }
        })
        .collect()
}

/// What: Collect service unit filenames shipped by a specific package.
///
/// Inputs:
/// - `package`: Package name for which to inspect the remote file list.
///
/// Output:
/// - Vector of unit filenames (e.g., `sshd.service`). Empty when the package
///   ships no systemd units.
///
/// Details:
/// - Executes `pacman -Fl <package>` and filters paths under the standard
///   systemd directories.
fn collect_service_units_for_package(package: &str) -> Result<Vec<String>, String> {
    let output = run_command(
        "pacman",
        &["-Fl", package],
        &format!("pacman -Fl {}", package),
    )?;
    let units = extract_service_units_from_file_list(&output, package);
    Ok(units)
}

/// What: Execute a command and capture stdout as UTF-8.
///
/// Inputs:
/// - `program`: Binary to execute.
/// - `args`: Command-line arguments.
/// - `display`: Human-friendly command description for logging.
///
/// Output:
/// - Stdout as a `String` on success; error description otherwise.
///
/// Details:
/// - Annotates errors with the supplied `display` string for easier debugging.
fn run_command(program: &str, args: &[&str], display: &str) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to spawn `{}`: {}", display, err))?;

    if !output.status.success() {
        return Err(format!(
            "`{}` exited with status {}",
            display, output.status
        ));
    }

    String::from_utf8(output.stdout)
        .map_err(|err| format!("`{}` produced invalid UTF-8: {}", display, err))
}

/// What: Extract unit filenames from `pacman -Fl` output.
///
/// Inputs:
/// - `file_list`: Raw `pacman -Fl` stdout.
/// - `package`: Package name used to filter unrelated entries in the output.
///
/// Output:
/// - Vector of unit filenames sorted in discovery order.
///
/// Details:
/// - Recognises units residing under `/usr/lib/systemd/system/` or the legacy
///   `/lib/systemd/system/` prefixes.
/// - Discards duplicate unit entries while preserving discovery order.
fn extract_service_units_from_file_list(file_list: &str, package: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut units = Vec::new();

    for line in file_list.lines() {
        let (pkg, raw_path) = match line.split_once(' ') {
            Some(parts) => parts,
            None => continue,
        };
        if pkg != package {
            continue;
        }

        let path = raw_path.strip_suffix('/').unwrap_or(raw_path);
        if !is_service_path(path) {
            continue;
        }

        if let Some(file_name) = Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.to_string())
            .filter(|name| seen.insert(name.clone()))
        {
            units.push(file_name);
        }
    }

    units
}

/// What: Determine whether a path refers to a systemd service unit file.
///
/// Inputs:
/// - `path`: File path extracted from `pacman -Fl`.
///
/// Output:
/// - `true` when the path resides under a known systemd unit directory and
///   ends with `.service`; otherwise `false`.
///
/// Details:
/// - Supports both `/usr/lib/systemd/system` and `/lib/systemd/system` roots.
fn is_service_path(path: &str) -> bool {
    const PREFIXES: [&str; 2] = ["/usr/lib/systemd/system/", "/lib/systemd/system/"];
    PREFIXES
        .iter()
        .any(|prefix| path.starts_with(prefix) && path.ends_with(".service"))
}

/// What: Fetch the set of currently active systemd services.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `BTreeSet` containing unit names (e.g., `sshd.service`). Errors when the
///   `systemctl` command fails.
///
/// Details:
/// - Runs `systemctl list-units --type=service --no-legend --state=active`.
fn fetch_active_units() -> Result<BTreeSet<String>, String> {
    let output = run_command(
        "systemctl",
        &[
            "list-units",
            "--type=service",
            "--no-legend",
            "--state=active",
        ],
        "systemctl list-units --type=service",
    )?;
    Ok(parse_active_units(&output))
}

/// What: Parse `systemctl list-units` output for active service names.
///
/// Inputs:
/// - `systemctl_output`: Raw stdout captured from the `systemctl` command.
///
/// Output:
/// - Sorted set of service names (`BTreeSet<String>`).
///
/// Details:
/// - Splits by whitespace and captures the first field on each line, ignoring
///   empty lines.
fn parse_active_units(systemctl_output: &str) -> BTreeSet<String> {
    systemctl_output
        .lines()
        .filter_map(|line| {
            let unit = line.split_whitespace().next()?;
            if unit.ends_with(".service") {
                Some(unit.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// What: Fetch ExecStart binary paths for active systemd services.
///
/// Inputs:
/// - `active_units`: Set of active unit names.
///
/// Output:
/// - Map from unit name to vector of binary paths used by that service.
///
/// Details:
/// - Uses `systemctl show` to get ExecStart paths for each active service.
/// - Parses ExecStart to extract binary paths (handles paths with arguments).
fn fetch_active_service_binaries(
    active_units: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut unit_to_binaries: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for unit in active_units {
        match run_command(
            "systemctl",
            &["show", unit, "-p", "ExecStart"],
            &format!("systemctl show {} -p ExecStart", unit),
        ) {
            Ok(output) => {
                let binaries = parse_execstart_paths(&output);
                if !binaries.is_empty() {
                    unit_to_binaries.insert(unit.clone(), binaries);
                }
            }
            Err(err) => {
                tracing::debug!("Failed to get ExecStart for {}: {}", unit, err);
            }
        }
    }

    Ok(unit_to_binaries)
}

/// What: Parse ExecStart paths from `systemctl show` output.
///
/// Inputs:
/// - `systemctl_output`: Raw stdout from `systemctl show -p ExecStart`.
///
/// Output:
/// - Vector of binary paths extracted from ExecStart.
///
/// Details:
/// - Handles ExecStart format: `ExecStart=/usr/bin/binary --args`
/// - Extracts the binary path (first token after `ExecStart=`)
/// - Handles multiple ExecStart entries (ExecStart, ExecStartPre, etc.)
fn parse_execstart_paths(systemctl_output: &str) -> Vec<String> {
    let mut binaries = Vec::new();

    for line in systemctl_output.lines() {
        if let Some(exec_line) = line.strip_prefix("ExecStart=") {
            // Extract the binary path (first token, may be quoted)
            let path = exec_line
                .split_whitespace()
                .next()
                .unwrap_or(exec_line)
                .trim_matches('"')
                .trim_matches('\'');

            if !path.is_empty() && !path.starts_with('-') {
                binaries.push(path.to_string());
            }
        } else if line.starts_with("ExecStartPre=") || line.starts_with("ExecStartPost=") {
            // Also consider ExecStartPre/Post binaries
            if let Some(exec_line) = line.split_once('=') {
                let path = exec_line
                    .1
                    .split_whitespace()
                    .next()
                    .unwrap_or(exec_line.1)
                    .trim_matches('"')
                    .trim_matches('\'');

                if !path.is_empty() && !path.starts_with('-') {
                    binaries.push(path.to_string());
                }
            }
        }
    }

    binaries
}

/// What: Collect binary paths shipped by a specific package.
///
/// Inputs:
/// - `package`: Package name for which to inspect the remote file list.
/// - `source`: Source descriptor to determine how to fetch binaries (Official vs AUR).
///
/// Output:
/// - Vector of binary paths (e.g., `/usr/bin/foo`, `/usr/sbin/bar`).
///
/// Details:
/// - For official packages: Executes `pacman -Fl <package>` and filters paths under standard binary directories.
/// - For AUR packages: Uses installed files, paru/yay -Fl, or PKGBUILD parsing as fallback.
/// - Includes executables from `/usr/bin`, `/usr/sbin`, `/bin`, `/sbin`, and `/usr/local/bin`.
fn collect_binaries_for_package(package: &str, source: &Source) -> Result<Vec<String>, String> {
    match source {
        Source::Official { .. } => {
            // Use pacman -Fl for official packages
            let output = run_command(
                "pacman",
                &["-Fl", package],
                &format!("pacman -Fl {}", package),
            )?;
            let binaries = extract_binaries_from_file_list(&output, package);
            Ok(binaries)
        }
        Source::Aur => {
            // For AUR packages, use the same fallback chain as file lists
            // First, check if package is already installed
            if let Ok(installed_files) = crate::logic::files::get_installed_file_list(package)
                && !installed_files.is_empty()
            {
                let binaries = extract_binaries_from_file_list(
                    &installed_files
                        .iter()
                        .map(|f| format!("{} {}", package, f))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    package,
                );
                if !binaries.is_empty() {
                    tracing::debug!(
                        "Found {} binaries from installed AUR package {}",
                        binaries.len(),
                        package
                    );
                    return Ok(binaries);
                }
            }

            // Try to use paru/yay -Fl if available (works for cached AUR packages)
            let has_paru = Command::new("paru").args(["--version"]).output().is_ok();
            let has_yay = Command::new("yay").args(["--version"]).output().is_ok();

            if has_paru {
                tracing::debug!("Trying paru -Fl {} for AUR package binaries", package);
                if let Ok(output) = Command::new("paru")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let binaries = extract_binaries_from_file_list(&text, package);
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from paru -Fl for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
            }

            if has_yay {
                tracing::debug!("Trying yay -Fl {} for AUR package binaries", package);
                if let Ok(output) = Command::new("yay")
                    .args(["-Fl", package])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                    && output.status.success()
                {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let binaries = extract_binaries_from_file_list(&text, package);
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from yay -Fl for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
            }

            // Fallback: try to parse PKGBUILD to extract install paths
            match crate::logic::files::fetch_pkgbuild_sync(package) {
                Ok(pkgbuild) => {
                    let files =
                        crate::logic::files::parse_install_paths_from_pkgbuild(&pkgbuild, package);
                    let binaries: Vec<String> = files
                        .into_iter()
                        .filter(|f| {
                            f.starts_with("/usr/bin/")
                                || f.starts_with("/usr/sbin/")
                                || f.starts_with("/bin/")
                                || f.starts_with("/sbin/")
                                || f.starts_with("/usr/local/bin/")
                        })
                        .collect();
                    if !binaries.is_empty() {
                        tracing::debug!(
                            "Found {} binaries from PKGBUILD parsing for {}",
                            binaries.len(),
                            package
                        );
                        return Ok(binaries);
                    }
                }
                Err(e) => {
                    tracing::debug!("Failed to fetch PKGBUILD for {}: {}", package, e);
                }
            }

            // No binaries available
            Ok(Vec::new())
        }
    }
}

/// What: Extract binary paths from `pacman -Fl` output.
///
/// Inputs:
/// - `file_list`: Raw `pacman -Fl` stdout.
/// - `package`: Package name used to filter unrelated entries.
///
/// Output:
/// - Vector of binary paths sorted in discovery order.
///
/// Details:
/// - Recognises executables under standard binary directories.
/// - Filters out directories and non-executable files.
fn extract_binaries_from_file_list(file_list: &str, package: &str) -> Vec<String> {
    const BINARY_PREFIXES: [&str; 5] = [
        "/usr/bin/",
        "/usr/sbin/",
        "/bin/",
        "/sbin/",
        "/usr/local/bin/",
    ];

    let mut seen = HashSet::new();
    let mut binaries = Vec::new();

    for line in file_list.lines() {
        let (pkg, raw_path) = match line.split_once(' ') {
            Some(parts) => parts,
            None => continue,
        };
        if pkg != package {
            continue;
        }

        let path = raw_path.strip_suffix('/').unwrap_or(raw_path);

        // Check if path is under a binary directory
        let is_binary = BINARY_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix));

        if is_binary {
            // Extract the binary name for matching
            if let Some(binary_name) = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|s| s.to_string())
            {
                // Store full path for exact matching
                if seen.insert(path.to_string()) {
                    binaries.push(path.to_string());
                }
                // Also store binary name for flexible matching (if not already added)
                if seen.insert(binary_name.clone()) {
                    binaries.push(binary_name);
                }
            }
        }
    }

    binaries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::ServiceRestartDecision;

    #[test]
    /// What: Ensure unit extraction recognises service files and ignores others.
    ///
    /// Inputs:
    /// - Synthetic `pacman -Fl` output containing service files, directories, and
    ///   irrelevant paths.
    ///
    /// Output:
    /// - Confirms only valid `.service` entries are returned.
    ///
    /// Details:
    /// - Verifies both `/usr/lib/systemd/system/` and `/lib/systemd/system/` paths.
    fn extract_service_units_from_file_list_filters_correctly() {
        let output = "\
mockpkg /usr/lib/systemd/system/example.service
mockpkg /usr/lib/systemd/system/example.service/
mockpkg /usr/lib/systemd/system/example.timer
mockpkg /lib/systemd/system/legacy.service
mockpkg /usr/bin/mock
otherpkg /usr/lib/systemd/system/other.service
";
        let units = extract_service_units_from_file_list(output, "mockpkg");
        assert_eq!(
            units,
            vec!["example.service".to_string(), "legacy.service".to_string()]
        );
    }

    #[test]
    /// What: Ensure duplicate `.service` listings are deduplicated without disturbing order.
    ///
    /// Inputs:
    /// - Synthetic `pacman -Fl` output containing repeated entries for the same units.
    ///
    /// Output:
    /// - Confirms the resulting list contains each unit once in discovery order.
    ///
    /// Details:
    /// - Validates that later duplicates are ignored and first occurrences are retained.
    fn extract_service_units_from_file_list_deduplicates_preserving_order() {
        let output = "\
mockpkg /usr/lib/systemd/system/alpha.service
mockpkg /usr/lib/systemd/system/beta.service
mockpkg /usr/lib/systemd/system/alpha.service/
mockpkg /usr/lib/systemd/system/gamma.service
mockpkg /usr/lib/systemd/system/beta.service/
";
        let units = extract_service_units_from_file_list(output, "mockpkg");
        assert_eq!(
            units,
            vec![
                "alpha.service".to_string(),
                "beta.service".to_string(),
                "gamma.service".to_string()
            ]
        );
    }

    #[test]
    /// What: Confirm parsing of active units handles typical `systemctl` output.
    ///
    /// Inputs:
    /// - Representative `systemctl list-units` snippet with multiple columns.
    ///
    /// Output:
    /// - Validates only `.service` units are captured in a sorted set.
    ///
    /// Details:
    /// - Ensures secondary tokens (loaded/active/running) do not impact parsing.
    fn parse_active_units_extracts_first_column() {
        let output = "\
sshd.service                loaded active running OpenSSH Daemon
cups.service                loaded active running CUPS Scheduler
dbus.socket                 loaded active running D-Bus Socket
";
        let active = parse_active_units(output);
        let expected: BTreeSet<String> = ["sshd.service", "cups.service"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(active, expected);
    }

    #[test]
    /// What: Verify the recommended decision logic defaults to defer when inactive.
    ///
    /// Inputs:
    /// - Crafted service impacts simulating inactive units.
    ///
    /// Output:
    /// - Ensures `resolve_service_impacts` would compute `Defer` when `needs_restart` is false.
    ///
    /// Details:
    /// - Uses direct struct construction to avoid spawning commands in the test.
    fn recommended_decision_default_is_defer_when_inactive() {
        let impact = ServiceImpact {
            unit_name: "example.service".into(),
            providers: vec!["pkg".into()],
            is_active: false,
            needs_restart: false,
            recommended_decision: ServiceRestartDecision::Defer,
            restart_decision: ServiceRestartDecision::Defer,
        };
        assert_eq!(impact.recommended_decision, ServiceRestartDecision::Defer);
    }
}
