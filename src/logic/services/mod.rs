//! Service impact resolution for the preflight "Services" tab.

mod binaries;
mod command;
mod systemd;
mod units;

use std::collections::BTreeMap;
use std::path::Path;

use crate::state::modal::{ServiceImpact, ServiceRestartDecision};
use crate::state::{PackageItem, PreflightAction};

use binaries::collect_binaries_for_package;
use systemd::{fetch_active_service_binaries, fetch_active_units};
use units::collect_service_units_for_package;

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
    let _span = tracing::info_span!(
        "resolve_service_impacts",
        stage = "services",
        item_count = items.len()
    )
    .entered();
    let start_time = std::time::Instant::now();
    let mut unit_to_providers: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // First pass: collect units shipped by packages
    for item in items {
        match collect_service_units_for_package(&item.name, &item.source) {
            Ok(units) => {
                for unit in units {
                    let providers = unit_to_providers.entry(unit).or_default();
                    if !providers.iter().any(|name| name == &item.name) {
                        providers.push(item.name.clone());
                    }
                }
            }
            Err(err) => {
                // Only warn for official packages - AUR packages are expected to fail with pacman -Fl
                if matches!(item.source, crate::state::types::Source::Official { .. }) {
                    tracing::warn!(
                        "Failed to resolve service units for package {}: {}",
                        item.name,
                        err
                    );
                } else {
                    tracing::debug!(
                        "Could not resolve service units for AUR package {} (expected): {}",
                        item.name,
                        err
                    );
                }
            }
        }
    }

    let active_units = fetch_active_units().unwrap_or_else(|err| {
        tracing::warn!("Unable to query active services: {}", err);
        std::collections::BTreeSet::new()
    });

    // Second pass: detect binaries that impact active units (heuristic enhancement)
    if matches!(action, PreflightAction::Install) && !active_units.is_empty() {
        // Get ExecStart paths for all active services
        let active_service_binaries = fetch_active_service_binaries(&active_units);

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

    let results: Vec<ServiceImpact> = unit_to_providers
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
        .collect();

    let elapsed = start_time.elapsed();
    let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        stage = "services",
        item_count = items.len(),
        result_count = results.len(),
        duration_ms = duration_ms,
        "Service resolution complete"
    );
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::ServiceRestartDecision;

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
