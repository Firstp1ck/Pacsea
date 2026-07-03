//! JSON envelope output for CLI integrators.
//!
//! All machine-readable CLI output goes through this module: a single JSON envelope with a
//! top-level `schema_version`, printed on stdout. Diagnostics stay on stderr so integrators
//! can parse stdout unconditionally.

/// Version of the CLI JSON envelope schema; bump on breaking changes to the envelope
/// or to any command payload shape.
pub const SCHEMA_VERSION: u32 = 1;

/// What: Build the standard JSON envelope around a command payload.
///
/// Inputs:
/// - `command`: Stable command identifier (e.g. "search", "list", "news").
/// - `data`: Command-specific payload.
///
/// Output:
/// - JSON value `{"schema_version": N, "command": ..., "data": ...}`.
///
/// Details:
/// - `schema_version` is top-level so integrators can dispatch before reading payloads.
pub fn envelope(command: &str, data: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "data": data,
    })
}

/// What: Print the JSON envelope for a command on stdout.
///
/// Inputs:
/// - `command`: Stable command identifier.
/// - `data`: Command-specific payload.
///
/// Output:
/// - One JSON document on stdout; exits with code 1 if serialization fails.
///
/// Details:
/// - Serialization failure is reported on stderr, keeping stdout parseable.
pub fn print_envelope(command: &str, data: &serde_json::Value) {
    match serde_json::to_string(&envelope(command, data)) {
        Ok(line) => println!("{line}"),
        Err(e) => {
            eprintln!("Error: failed to serialize JSON output: {e}");
            tracing::error!(error = %e, "Failed to serialize JSON envelope");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Verify the envelope carries schema version, command, and payload.
    ///
    /// Inputs:
    /// - Command "list" with a small payload object.
    ///
    /// Output:
    /// - Envelope exposes `schema_version`, `command`, and the payload under `data`.
    ///
    /// Details:
    /// - Guards the integrator contract for the CLI `--json` flag.
    fn envelope_carries_schema_version_and_payload() {
        let env = envelope("list", &serde_json::json!({"packages": ["ripgrep"]}));
        assert_eq!(env["schema_version"], SCHEMA_VERSION);
        assert_eq!(env["command"], "list");
        assert_eq!(env["data"]["packages"][0], "ripgrep");
    }
}
