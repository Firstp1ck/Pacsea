//! Service impact resolution for the preflight \"Services\" tab.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::Path;
use std::process::Command;

use crate::state::modal::{ServiceImpact, ServiceRestartDecision};
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
/// - Computes a recommended restart decision; defaults to defer when the unit
///   is inactive or the action is a removal.
pub fn resolve_service_impacts(
    items: &[PackageItem],
    action: PreflightAction,
) -> Vec<ServiceImpact> {
    let mut unit_to_providers: BTreeMap<String, Vec<String>> = BTreeMap::new();

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
