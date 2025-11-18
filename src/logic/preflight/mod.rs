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
use metadata::fetch_official_metadata;
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

#[cfg(all(test, unix))]
mod tests;
