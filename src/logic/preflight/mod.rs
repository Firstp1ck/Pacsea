//! Preflight summary computation helpers.
//!
//! The routines in this module gather package metadata, estimate download and
//! install deltas, and derive risk heuristics used to populate the preflight
//! modal. All command execution is abstracted behind [`CommandRunner`] so the
//! logic can be exercised in isolation.

mod batch;
mod command;
mod metadata;
mod version;

use crate::state::modal::{
    PreflightAction, PreflightHeaderChips, PreflightPackageSummary, PreflightSummaryData, RiskLevel,
};
use crate::state::types::{PackageItem, Source};
use std::cmp::Ordering;

pub use command::{CommandError, CommandRunner, SystemCommandRunner};

use batch::{batch_fetch_installed_sizes, batch_fetch_installed_versions};
use version::{compare_versions, is_major_version_bump};

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

/// What: Intermediate state accumulated during package processing.
///
/// Inputs: Built incrementally while iterating packages.
///
/// Output: Used to construct the final summary and risk calculations.
///
/// Details: Groups related mutable state to reduce parameter passing.
struct ProcessingState {
    packages: Vec<PreflightPackageSummary>,
    aur_count: usize,
    total_download_bytes: u64,
    total_install_delta_bytes: i64,
    major_bump_packages: Vec<String>,
    core_system_updates: Vec<String>,
    any_major_bump: bool,
    any_core_update: bool,
    any_aur: bool,
}

impl ProcessingState {
    fn new(capacity: usize) -> Self {
        Self {
            packages: Vec::with_capacity(capacity),
            aur_count: 0,
            total_download_bytes: 0,
            total_install_delta_bytes: 0,
            major_bump_packages: Vec::new(),
            core_system_updates: Vec::new(),
            any_major_bump: false,
            any_core_update: false,
            any_aur: false,
        }
    }
}

/// What: Process a single package item and update processing state.
///
/// Inputs:
/// - `item`: Package to process.
/// - `action`: Install vs. remove context.
/// - `runner`: Command execution abstraction.
/// - `installed_version`: Previously fetched installed version (if any).
/// - `installed_size`: Previously fetched installed size (if any).
/// - `state`: Mutable state accumulator.
///
/// Output: Updates `state` in place.
///
/// Details:
/// - Fetches metadata for official packages.
/// - Computes version comparisons and notes.
/// - Detects core packages and major version bumps.
fn process_package_item<R: CommandRunner>(
    item: &PackageItem,
    action: PreflightAction,
    runner: &R,
    installed_version: Option<String>,
    installed_size: Option<u64>,
    state: &mut ProcessingState,
) {
    if matches!(item.source, Source::Aur) {
        state.aur_count += 1;
        state.any_aur = true;
    }

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

    let (download_bytes, install_size_target) = fetch_package_metadata(runner, item);

    let install_delta_bytes = calculate_install_delta(action, install_size_target, installed_size);

    if let Some(bytes) = download_bytes {
        state.total_download_bytes = state.total_download_bytes.saturating_add(bytes);
    }
    if let Some(delta) = install_delta_bytes {
        state.total_install_delta_bytes = state.total_install_delta_bytes.saturating_add(delta);
    }

    let (notes, is_major_bump, is_downgrade) = analyze_version_changes(
        &installed_version,
        &item.version,
        action,
        item.name.clone(),
        &mut state.major_bump_packages,
        &mut state.any_major_bump,
    );

    let core_note = check_core_package(
        item,
        action,
        &mut state.core_system_updates,
        &mut state.any_core_update,
    );
    let mut all_notes = notes;
    if let Some(note) = core_note {
        all_notes.push(note);
    }

    state.packages.push(PreflightPackageSummary {
        name: item.name.clone(),
        source: item.source.clone(),
        installed_version,
        target_version: item.version.clone(),
        is_downgrade,
        is_major_bump,
        download_bytes,
        install_delta_bytes,
        notes: all_notes,
    });
}

/// What: Fetch metadata for official and AUR packages.
///
/// Inputs:
/// - `runner`: Command execution abstraction.
/// - `item`: Package item to fetch metadata for.
///
/// Output: Tuple of (`download_bytes`, `install_size_target`), both `Option`.
///
/// Details:
/// - For official packages: uses `pacman -Si`.
/// - For AUR packages: checks local caches (pacman cache, AUR helper caches) for built package files.
fn fetch_package_metadata<R: CommandRunner>(
    runner: &R,
    item: &PackageItem,
) -> (Option<u64>, Option<u64>) {
    match &item.source {
        Source::Official { repo, .. } => {
            match metadata::fetch_official_metadata(runner, repo, &item.name, item.version.as_str())
            {
                Ok(meta) => (meta.download_size, meta.install_size),
                Err(err) => {
                    tracing::debug!(
                        "Preflight summary: failed to fetch metadata for {repo}/{pkg}: {err}",
                        pkg = item.name
                    );
                    (None, None)
                }
            }
        }
        Source::Aur => {
            match metadata::fetch_aur_metadata(runner, &item.name, Some(item.version.as_str())) {
                Ok(meta) => {
                    if meta.download_size.is_some() || meta.install_size.is_some() {
                        tracing::debug!(
                            "Preflight summary: found AUR package sizes for {}: DL={:?}, Install={:?}",
                            item.name,
                            meta.download_size,
                            meta.install_size
                        );
                    }
                    (meta.download_size, meta.install_size)
                }
                Err(err) => {
                    tracing::debug!(
                        "Preflight summary: failed to fetch AUR metadata for {}: {}",
                        item.name,
                        err
                    );
                    (None, None)
                }
            }
        }
    }
}

/// What: Calculate install size delta based on action type.
///
/// Inputs:
/// - `action`: Install vs. remove context.
/// - `install_size_target`: Target install size (for installs).
/// - `installed_size`: Current installed size.
///
/// Output: Delta in bytes (positive for installs, negative for removes).
///
/// Details: Returns None if metadata is unavailable.
fn calculate_install_delta(
    action: PreflightAction,
    install_size_target: Option<u64>,
    installed_size: Option<u64>,
) -> Option<i64> {
    match action {
        PreflightAction::Install => install_size_target.map(|target| {
            let current = installed_size.unwrap_or(0);
            target as i64 - current as i64
        }),
        PreflightAction::Remove => installed_size.map(|size| -(size as i64)),
    }
}

/// What: Analyze version changes and generate notes.
///
/// Inputs:
/// - `installed_version`: Current installed version (if any).
/// - `target_version`: Target version.
/// - `action`: Install vs. remove context.
/// - `package_name`: Name of the package.
/// - `major_bump_packages`: Mutable list to append to if major bump detected.
/// - `any_major_bump`: Mutable flag to set if major bump detected.
///
/// Output: Tuple of (`notes`, `is_major_bump`, `is_downgrade`).
///
/// Details: Detects downgrades, major version bumps, and new installations.
fn analyze_version_changes(
    installed_version: &Option<String>,
    target_version: &str,
    action: PreflightAction,
    package_name: String,
    major_bump_packages: &mut Vec<String>,
    any_major_bump: &mut bool,
) -> (Vec<String>, bool, bool) {
    let mut notes = Vec::new();
    let mut is_major_bump = false;
    let mut is_downgrade = false;

    if let Some(current) = installed_version {
        match compare_versions(current, target_version) {
            Ordering::Greater => {
                if matches!(action, PreflightAction::Install) {
                    is_downgrade = true;
                    notes.push(format!("Downgrade detected: {current} → {target_version}"));
                }
            }
            Ordering::Less => {
                if is_major_version_bump(current, target_version) {
                    is_major_bump = true;
                    *any_major_bump = true;
                    major_bump_packages.push(package_name);
                    notes.push(format!("Major version bump: {current} → {target_version}"));
                }
            }
            Ordering::Equal => {}
        }
    } else if matches!(action, PreflightAction::Install) {
        notes.push("New installation".to_string());
    }

    (notes, is_major_bump, is_downgrade)
}

/// What: Check if package is a core/system package and generate note.
///
/// Inputs:
/// - `item`: Package item to check.
/// - `action`: Install vs. remove context.
/// - `core_system_updates`: Mutable list to append to if core package.
/// - `any_core_update`: Mutable flag to set if core package.
///
/// Output: Optional note string if core package detected.
///
/// Details: Normalizes package name for comparison against critical packages list.
fn check_core_package(
    item: &PackageItem,
    action: PreflightAction,
    core_system_updates: &mut Vec<String>,
    any_core_update: &mut bool,
) -> Option<String> {
    let normalized_name = item.name.to_ascii_lowercase();
    if CORE_CRITICAL_PACKAGES
        .iter()
        .any(|candidate| normalized_name == *candidate)
    {
        *any_core_update = true;
        core_system_updates.push(item.name.clone());
        Some(if matches!(action, PreflightAction::Remove) {
            "Removing core/system package".to_string()
        } else {
            "Core/system package update".to_string()
        })
    } else {
        None
    }
}

/// What: Calculate risk reasons and score from processing state.
///
/// Inputs:
/// - `state`: Processing state with accumulated flags.
/// - `pacnew_candidates`: Count of packages that may produce .pacnew files.
/// - `service_restart_units`: List of services that need restart.
///
/// Output: Tuple of (`risk_reasons`, `risk_score`, `risk_level`).
///
/// Details: Applies the risk heuristic scoring system.
fn calculate_risk_metrics(
    state: &ProcessingState,
    pacnew_candidates: usize,
    service_restart_units: &[String],
) -> (Vec<String>, u8, RiskLevel) {
    let mut risk_reasons = Vec::new();
    let mut risk_score: u8 = 0;

    if state.any_core_update {
        risk_reasons.push("Core/system packages involved (+3)".to_string());
        risk_score = risk_score.saturating_add(3);
    }
    if state.any_major_bump {
        risk_reasons.push("Major version bump detected (+2)".to_string());
        risk_score = risk_score.saturating_add(2);
    }
    if state.any_aur {
        risk_reasons.push("AUR packages included (+2)".to_string());
        risk_score = risk_score.saturating_add(2);
    }
    if pacnew_candidates > 0 {
        risk_reasons.push("Configuration files may produce .pacnew (+1)".to_string());
        risk_score = risk_score.saturating_add(1);
    }
    if !service_restart_units.is_empty() {
        risk_reasons.push("Services likely require restart (+1)".to_string());
        risk_score = risk_score.saturating_add(1);
    }

    let risk_level = match risk_score {
        0 => RiskLevel::Low,
        1..=4 => RiskLevel::Medium,
        _ => RiskLevel::High,
    };

    (risk_reasons, risk_score, risk_level)
}

/// What: Build summary notes from processing state.
///
/// Inputs:
/// - `state`: Processing state with accumulated flags.
///
/// Output: Vector of summary note strings.
///
/// Details: Generates informational notes for the summary tab.
fn build_summary_notes(state: &ProcessingState) -> Vec<String> {
    let mut notes = Vec::new();
    if state.any_core_update {
        notes.push("Core/system packages will be modified.".to_string());
    }
    if state.any_major_bump {
        notes.push("Major version changes detected; review changelogs.".to_string());
    }
    if state.any_aur {
        notes.push("AUR packages present; build steps may vary.".to_string());
    }
    notes
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

    let mut state = ProcessingState::new(items.len());

    let pacnew_candidates = 0usize;
    let pacsave_candidates = 0usize;
    let config_warning_packages = Vec::new();
    let service_restart_units = Vec::new();

    // Batch fetch installed versions and sizes for all packages
    let installed_versions = batch_fetch_installed_versions(runner, items);
    let installed_sizes = batch_fetch_installed_sizes(runner, items);

    for (idx, item) in items.iter().enumerate() {
        let installed_version = installed_versions
            .get(idx)
            .and_then(|v| v.as_ref().ok())
            .cloned();
        let installed_size = installed_sizes
            .get(idx)
            .and_then(|s| s.as_ref().ok())
            .copied();

        process_package_item(
            item,
            action,
            runner,
            installed_version,
            installed_size,
            &mut state,
        );
    }

    let (risk_reasons, risk_score, risk_level) =
        calculate_risk_metrics(&state, pacnew_candidates, &service_restart_units);

    let summary_notes = build_summary_notes(&state);
    let mut summary_warnings = Vec::new();
    if summary_warnings.is_empty() {
        summary_warnings.extend(risk_reasons.iter().cloned());
    }

    let summary = PreflightSummaryData {
        packages: state.packages,
        package_count: items.len(),
        aur_count: state.aur_count,
        download_bytes: state.total_download_bytes,
        install_delta_bytes: state.total_install_delta_bytes,
        risk_score,
        risk_level,
        risk_reasons: risk_reasons.clone(),
        major_bump_packages: state.major_bump_packages,
        core_system_updates: state.core_system_updates,
        pacnew_candidates,
        pacsave_candidates,
        config_warning_packages,
        service_restart_units,
        summary_warnings,
        summary_notes,
    };

    let header = PreflightHeaderChips {
        package_count: items.len(),
        download_bytes: state.total_download_bytes,
        install_delta_bytes: state.total_install_delta_bytes,
        aur_count: state.aur_count,
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

#[cfg(all(test, unix))]
mod tests;
