//! Systemd querying and parsing functions.

use std::collections::{BTreeMap, BTreeSet};

use super::command::run_command;

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
pub(super) fn fetch_active_units() -> Result<BTreeSet<String>, String> {
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
pub(super) fn parse_active_units(systemctl_output: &str) -> BTreeSet<String> {
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

/// What: Fetch `ExecStart` binary paths for active systemd services.
///
/// Inputs:
/// - `active_units`: Set of active unit names.
///
/// Output:
/// - Map from unit name to vector of binary paths used by that service.
///
/// Details:
/// - Uses `systemctl show` to get `ExecStart` paths for each active service.
/// - Parses `ExecStart` to extract binary paths (handles paths with arguments).
pub(super) fn fetch_active_service_binaries(
    active_units: &BTreeSet<String>,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut unit_to_binaries: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for unit in active_units {
        match run_command(
            "systemctl",
            &["show", unit, "-p", "ExecStart"],
            &format!("systemctl show {unit} -p ExecStart"),
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

/// What: Parse `ExecStart` paths from `systemctl show` output.
///
/// Inputs:
/// - `systemctl_output`: Raw stdout from `systemctl show -p ExecStart`.
///
/// Output:
/// - Vector of binary paths extracted from `ExecStart`.
///
/// Details:
/// - Handles `ExecStart` format: `ExecStart=/usr/bin/binary --args`
/// - Extracts the binary path (first token after `ExecStart=`)
/// - Handles multiple `ExecStart` entries (`ExecStart`, `ExecStartPre`, etc.)
pub(super) fn parse_execstart_paths(systemctl_output: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
