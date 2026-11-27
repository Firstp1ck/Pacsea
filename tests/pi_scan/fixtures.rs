//! Shared constants and paths for Pi scanner Wave 0 tests.

use std::path::PathBuf;

/// Marker emitted by the no-model Pi probe extension.
pub const ACTIVE_TOOLS_MARKER: &str = "PACSEA_ACTIVE_TOOLS:";

/// Exact restricted tool allowlist expected from the probe session.
pub const RESTRICTED_TOOL_NAMES: [&str; 4] = [
    "pacsea_scan_find",
    "pacsea_scan_grep",
    "pacsea_scan_ls",
    "pacsea_scan_read",
];

/// Required Pi CLI flags for fail-closed scanner startup.
pub const REQUIRED_PI_FLAGS: [&str; 12] = [
    "--mode <mode>",
    "--no-session",
    "--no-builtin-tools",
    "--tools",
    "--extension",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--no-themes",
    "--no-context-files",
    "--no-approve",
    "--offline",
];

/// What: Return the absolute path to the checked-in no-model Pi probe extension.
///
/// Inputs: None.
///
/// Output:
/// - Absolute path under the repository test fixtures.
///
/// Details:
/// - Uses `CARGO_MANIFEST_DIR` so the ignored live probe is independent of the caller's cwd.
pub fn probe_extension_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("pi_scan")
        .join("assets")
        .join("pacsea-probe.ts")
}

/// What: Return the comma-delimited restricted tool allowlist used by Pi CLI startup.
///
/// Inputs: None.
///
/// Output:
/// - Stable comma-delimited tool names.
///
/// Details:
/// - Names remain sorted so extension output and command arguments compare deterministically.
pub fn restricted_tool_csv() -> String {
    RESTRICTED_TOOL_NAMES.join(",")
}
