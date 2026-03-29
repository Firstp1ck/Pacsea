//! Privilege escalation abstraction for sudo/doas support.
//!
//! # doas capability spike (Phase 0)
//!
//! **Target package:** `opendoas` (Arch: `extra/opendoas`)
//! **Minimum supported behavior:** `OpenDoas` as packaged in Arch Linux repos.
//!
//! ## Supported patterns
//!
//! | Pattern | sudo | doas |
//! |---|---|---|
//! | Non-interactive check | `sudo -n true` | `doas -n true` |
//! | Direct command execution | `sudo <cmd>` | `doas <cmd>` |
//! | Passwordless execution | sudoers `NOPASSWD` | `permit nopass` in `/etc/doas.conf` |
//! | Password via stdin | `sudo -S` reads stdin | **NOT supported** |
//! | Credential refresh | `sudo -v` | **NOT supported** |
//! | Credential invalidation | `sudo -k` | **NOT supported** |
//! | Askpass env var | `SUDO_ASKPASS` | **NOT supported** |
//!
//! ## Implications for Pacsea
//!
//! - When doas requires a password, it prompts via its own terminal interaction.
//! - The in-app password modal **cannot** be used with doas (no stdin pipe support).
//! - Pacsea skips the password modal for doas and lets the spawned terminal handle prompting.
//! - Credential warm-up (`sudo -S -v`) is unavailable for doas.
//! - `doas -n true` works identically to `sudo -n true` for passwordless detection.

use std::fmt;
use std::process::Command;

/// What: Privilege escalation tool supported by Pacsea.
///
/// Inputs: None (enum variant selection).
///
/// Output: Identifies which privilege tool to invoke.
///
/// Details:
/// - `Sudo` uses the standard sudo binary with full feature support
///   (stdin password, credential caching, askpass).
/// - `Doas` uses the `OpenDoas` binary with limited feature support
///   (no stdin password, no credential caching).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivilegeTool {
    /// Standard sudo — full feature support.
    Sudo,
    /// `OpenDoas` — limited feature support (no stdin password pipe, no credential caching).
    Doas,
}

/// What: User-configured privilege tool selection mode parsed from `settings.conf`.
///
/// Inputs: None (enum variant selection).
///
/// Output: Controls how Pacsea selects the privilege escalation tool.
///
/// Details:
/// - `Auto` (default): prefer doas if available on `$PATH`, fall back to sudo.
/// - `Sudo`: always use sudo; fail with actionable error if unavailable.
/// - `Doas`: always use doas; fail with actionable error if unavailable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrivilegeMode {
    /// Auto-detect: prefer doas if available, fall back to sudo.
    #[default]
    Auto,
    /// Always use sudo.
    Sudo,
    /// Always use doas.
    Doas,
}

/// What: Capability flags describing which features a privilege tool supports.
///
/// Inputs: None (populated per tool via [`PrivilegeTool::capabilities`]).
///
/// Output: Boolean flags for each optional capability.
///
/// Details:
/// - sudo supports all capabilities.
/// - doas supports none of these optional capabilities.
/// - Used to route behavior: e.g. skip password modal when stdin pipe is unsupported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PrivilegeCapabilities {
    /// Tool supports reading password from stdin (`sudo -S`).
    pub supports_stdin_password: bool,
    /// Tool supports credential validation/refresh without running a command (`sudo -v`).
    pub supports_credential_refresh: bool,
    /// Tool supports credential invalidation (`sudo -k`).
    pub supports_credential_invalidation: bool,
    /// Tool supports the `ASKPASS` environment variable (`SUDO_ASKPASS`).
    pub supports_askpass: bool,
}

// ---------------------------------------------------------------------------
// PrivilegeTool implementation
// ---------------------------------------------------------------------------

impl PrivilegeTool {
    /// What: Return the shell binary name for this tool.
    ///
    /// Inputs: None.
    ///
    /// Output: `"sudo"` or `"doas"`.
    ///
    /// Details: Used in command construction and `which` lookups.
    #[must_use]
    pub const fn binary_name(self) -> &'static str {
        match self {
            Self::Sudo => "sudo",
            Self::Doas => "doas",
        }
    }

    /// What: Return the capability flags for this tool.
    ///
    /// Inputs: None.
    ///
    /// Output: [`PrivilegeCapabilities`] with tool-specific flags.
    ///
    /// Details:
    /// - sudo: all capabilities enabled.
    /// - doas: all capabilities disabled (see module-level docs for rationale).
    #[must_use]
    pub const fn capabilities(self) -> PrivilegeCapabilities {
        match self {
            Self::Sudo => PrivilegeCapabilities {
                supports_stdin_password: true,
                supports_credential_refresh: true,
                supports_credential_invalidation: true,
                supports_askpass: true,
            },
            Self::Doas => PrivilegeCapabilities {
                supports_stdin_password: false,
                supports_credential_refresh: false,
                supports_credential_invalidation: false,
                supports_askpass: false,
            },
        }
    }

    /// What: Check whether this tool's binary exists on `$PATH`.
    ///
    /// Inputs: None.
    ///
    /// Output: `true` if the binary is found.
    ///
    /// Details:
    /// - In integration test context (`PACSEA_INTEGRATION_TEST=1`), honors
    ///   `PACSEA_TEST_PRIVILEGE_AVAILABLE` (comma-separated list, or `"none"`).
    /// - Production: delegates to `which::which`.
    #[must_use]
    pub fn is_available(self) -> bool {
        if is_integration_test_context()
            && let Ok(val) = std::env::var("PACSEA_TEST_PRIVILEGE_AVAILABLE")
        {
            if val == "none" {
                return false;
            }
            return val.split(',').any(|t| t.trim() == self.binary_name());
        }
        which::which(self.binary_name()).is_ok()
    }

    /// What: Check whether passwordless privilege escalation is available.
    ///
    /// Inputs: None.
    ///
    /// Output: `Ok(true)` if non-interactive execution succeeds, `Err` if the check itself fails.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the tool binary cannot be executed (e.g. not installed).
    ///
    /// Details:
    /// - Runs `<tool> -n true` (`-n` = non-interactive, `true` = no-op command).
    /// - Both sudo and doas support the `-n` flag.
    /// - In integration test context, honors `PACSEA_TEST_SUDO_PASSWORDLESS`.
    pub fn check_passwordless(self) -> Result<bool, String> {
        if is_integration_test_context()
            && let Ok(val) = std::env::var("PACSEA_TEST_SUDO_PASSWORDLESS")
        {
            tracing::debug!(
                tool = self.binary_name(),
                val = %val,
                "Using test override for passwordless check"
            );
            return Ok(val == "1");
        }

        let status = Command::new(self.binary_name())
            .args(["-n", "true"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|e| format!("Failed to check passwordless {}: {e}", self.binary_name()))?;

        Ok(status.success())
    }
}

impl fmt::Display for PrivilegeTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.binary_name())
    }
}

// ---------------------------------------------------------------------------
// PrivilegeMode implementation
// ---------------------------------------------------------------------------

impl PrivilegeMode {
    /// What: Parse a config file value into a `PrivilegeMode`.
    ///
    /// Inputs:
    /// - `val`: Raw config string (e.g. `"auto"`, `"sudo"`, `"doas"`).
    ///
    /// Output: `Some(mode)` on recognized value, `None` otherwise.
    ///
    /// Details: Case-insensitive matching after trim.
    #[must_use]
    pub fn from_config_key(val: &str) -> Option<Self> {
        match val.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "sudo" => Some(Self::Sudo),
            "doas" => Some(Self::Doas),
            _ => None,
        }
    }

    /// What: Return the canonical config key string for this mode.
    ///
    /// Inputs: None.
    ///
    /// Output: `"auto"`, `"sudo"`, or `"doas"`.
    ///
    /// Details: Inverse of [`from_config_key`](Self::from_config_key).
    #[must_use]
    pub const fn as_config_key(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Sudo => "sudo",
            Self::Doas => "doas",
        }
    }
}

impl fmt::Display for PrivilegeMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_key())
    }
}

// ---------------------------------------------------------------------------
// AuthMode
// ---------------------------------------------------------------------------

/// What: Authentication strategy for privilege escalation.
///
/// Inputs: None (enum variant selection).
///
/// Output: Controls how Pacsea handles authentication before privileged operations.
///
/// Details:
/// - `Prompt` (default): Pacsea shows its own password modal/prompt, then pipes the
///   password to the privilege tool (`sudo -S` or doas PTY injection).
/// - `PasswordlessOnly`: Skip password prompt only when `{tool} -n true` succeeds;
///   fall back to `Prompt` otherwise.
/// - `Interactive`: Skip Pacsea's password capture entirely and let the privilege
///   tool handle authentication directly (fingerprint via PAM, terminal password, etc.).
///   Works with both sudo and doas when PAM is configured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AuthMode {
    /// Pacsea captures the password and pipes it to the privilege tool.
    #[default]
    Prompt,
    /// Skip password prompt only when passwordless escalation is available.
    PasswordlessOnly,
    /// Let the privilege tool handle authentication interactively (PAM fingerprint, etc.).
    Interactive,
}

impl AuthMode {
    /// What: Parse a config file value into an `AuthMode`.
    ///
    /// Inputs:
    /// - `val`: Raw config string (e.g. `"prompt"`, `"passwordless_only"`, `"interactive"`).
    ///
    /// Output: `Some(mode)` on recognized value, `None` otherwise.
    ///
    /// Details: Case-insensitive matching after trim; accepts underscores and hyphens.
    #[must_use]
    pub fn from_config_key(val: &str) -> Option<Self> {
        match val.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "prompt" => Some(Self::Prompt),
            "passwordless_only" | "passwordless" => Some(Self::PasswordlessOnly),
            "interactive" => Some(Self::Interactive),
            _ => None,
        }
    }

    /// What: Return the canonical config key string for this mode.
    ///
    /// Inputs: None.
    ///
    /// Output: `"prompt"`, `"passwordless_only"`, or `"interactive"`.
    ///
    /// Details: Inverse of [`from_config_key`](Self::from_config_key).
    #[must_use]
    pub const fn as_config_key(self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::PasswordlessOnly => "passwordless_only",
            Self::Interactive => "interactive",
        }
    }

    /// What: Whether this mode skips Pacsea's password modal entirely.
    ///
    /// Inputs: None.
    ///
    /// Output: `true` for `Interactive`, `false` for `Prompt` and `PasswordlessOnly`.
    ///
    /// Details:
    /// - `PasswordlessOnly` does not unconditionally skip — it still needs a runtime
    ///   `{tool} -n true` check before skipping.
    /// - `Interactive` always skips the modal because the tool handles auth directly.
    #[must_use]
    pub const fn always_skips_password_modal(self) -> bool {
        matches!(self, Self::Interactive)
    }
}

impl fmt::Display for AuthMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_config_key())
    }
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

/// What: Resolve which privilege tool to use based on the configured mode.
///
/// Inputs:
/// - `mode`: User-configured [`PrivilegeMode`].
///
/// Output: `Ok(tool)` on success, `Err` with actionable message on failure.
///
/// # Errors
///
/// - `Auto`: neither doas nor sudo found on `$PATH`.
/// - `Sudo`/`Doas`: the explicitly requested tool is not on `$PATH`.
///
/// Details:
/// - `Auto` prefers doas over sudo when both are available.
/// - Explicit modes fail fast with a message suggesting config changes.
pub fn resolve_privilege_tool(mode: PrivilegeMode) -> Result<PrivilegeTool, String> {
    match mode {
        PrivilegeMode::Auto => {
            let doas_ok = PrivilegeTool::Doas.is_available();
            let sudo_ok = PrivilegeTool::Sudo.is_available();
            tracing::debug!(
                mode = %mode,
                doas_available = doas_ok,
                sudo_available = sudo_ok,
                "Resolving privilege tool"
            );
            if doas_ok {
                tracing::info!(
                    tool = "doas",
                    reason = "auto: doas preferred when available",
                    "Selected privilege tool"
                );
                Ok(PrivilegeTool::Doas)
            } else if sudo_ok {
                tracing::info!(
                    tool = "sudo",
                    reason = "auto: doas unavailable, falling back",
                    "Selected privilege tool"
                );
                Ok(PrivilegeTool::Sudo)
            } else {
                Err("Neither doas nor sudo found on $PATH. \
                     Install one to perform privileged operations: \
                     `pacman -S sudo` or `pacman -S opendoas`."
                    .to_string())
            }
        }
        PrivilegeMode::Sudo => {
            if PrivilegeTool::Sudo.is_available() {
                tracing::info!(
                    tool = "sudo",
                    reason = "explicit config",
                    "Selected privilege tool"
                );
                Ok(PrivilegeTool::Sudo)
            } else {
                Err(
                    "sudo is not available on $PATH. Install sudo (`pacman -S sudo`) \
                     or change privilege_tool to 'auto' or 'doas' in settings.conf."
                        .to_string(),
                )
            }
        }
        PrivilegeMode::Doas => {
            if PrivilegeTool::Doas.is_available() {
                tracing::info!(
                    tool = "doas",
                    reason = "explicit config",
                    "Selected privilege tool"
                );
                Ok(PrivilegeTool::Doas)
            } else {
                Err(
                    "doas is not available on $PATH. Install opendoas (`pacman -S opendoas`) \
                     or change privilege_tool to 'auto' or 'sudo' in settings.conf."
                        .to_string(),
                )
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Convenience resolver
// ---------------------------------------------------------------------------

/// What: Resolve the privilege tool from the current settings, with sudo fallback.
///
/// Inputs: None (reads `crate::theme::settings().privilege_mode`).
///
/// Output: The active [`PrivilegeTool`].
///
/// Details:
/// - Reads the cached application settings and resolves the configured mode.
/// - Falls back to `Sudo` if resolution fails (e.g. neither tool is on `$PATH`).
/// - Logs a warning on fallback so the user can diagnose missing tools.
#[must_use]
pub fn active_tool() -> PrivilegeTool {
    let settings = crate::theme::settings();
    resolve_privilege_tool(settings.privilege_mode).unwrap_or_else(|e| {
        tracing::warn!(
            configured_mode = %settings.privilege_mode,
            error = %e,
            fallback = "sudo",
            "Privilege tool resolution failed — commands may fail if sudo is also missing"
        );
        PrivilegeTool::Sudo
    })
}

// ---------------------------------------------------------------------------
// Interactive authentication
// ---------------------------------------------------------------------------

/// What: Run the privilege tool interactively to let the user authenticate.
///
/// Inputs:
/// - `tool`: Resolved privilege tool (sudo or doas).
///
/// Output:
/// - `Ok(true)` if authentication succeeded, `Ok(false)` if it failed.
///
/// # Errors
///
/// Returns `Err` if the tool binary cannot be executed.
///
/// Details:
/// - For sudo: runs `sudo -v` which validates credentials without executing a command.
///   On success, the credential cache is refreshed so subsequent `sudo` calls don't re-prompt.
/// - For doas: runs `doas true` (a no-op command) to trigger authentication.
///   If `persist` is configured in `doas.conf`, subsequent `doas` calls won't re-prompt.
///   Without `persist`, each `doas` invocation will re-prompt (known limitation).
/// - The caller is responsible for ensuring the terminal is in a state where the user
///   can interact with the prompt (e.g. not in TUI raw mode).
pub fn run_interactive_auth(tool: PrivilegeTool) -> Result<bool, String> {
    if is_integration_test_context() {
        tracing::debug!(tool = %tool, "Skipping interactive auth in integration test context");
        return Ok(true);
    }

    let mut cmd = Command::new(tool.binary_name());
    if tool.capabilities().supports_credential_refresh {
        cmd.arg("-v");
    } else {
        cmd.arg("true");
    }

    let status = cmd.status().map_err(|e| {
        format!(
            "Failed to run {} for interactive authentication: {e}",
            tool.binary_name()
        )
    })?;

    Ok(status.success())
}

// ---------------------------------------------------------------------------
// Fingerprint / PAM detection
// ---------------------------------------------------------------------------

/// What: Detect whether the active privilege tool's PAM configuration includes `pam_fprintd`.
///
/// Inputs:
/// - `tool`: Resolved privilege tool (sudo or doas).
///
/// Output:
/// - `true` if `/etc/pam.d/{tool}` exists and contains a reference to `pam_fprintd`.
///
/// Details:
/// - Reads `/etc/pam.d/sudo` or `/etc/pam.d/doas` and checks for `pam_fprintd.so`.
/// - Also checks `/etc/pam.d/system-auth` and `/etc/pam.d/system-local-login` as common
///   include targets where `pam_fprintd` may be configured instead of the tool-specific file.
/// - Informational only — never blocks execution.
/// - Returns `false` on any I/O error (missing file, permission denied).
pub fn detect_pam_fingerprint(tool: PrivilegeTool) -> bool {
    use std::fs;

    let tool_pam_path = format!("/etc/pam.d/{}", tool.binary_name());
    let pam_files = [
        tool_pam_path.as_str(),
        "/etc/pam.d/system-auth",
        "/etc/pam.d/system-local-login",
    ];

    for path in &pam_files {
        if let Ok(contents) = fs::read_to_string(path)
            && contents.contains("pam_fprintd")
        {
            tracing::debug!(path = %path, "Detected pam_fprintd in PAM configuration");
            return true;
        }
    }

    false
}

/// What: Check whether a fingerprint reader is enrolled via `fprintd-list`.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `true` if `fprintd-list` reports at least one enrolled finger.
///
/// Details:
/// - Runs `fprintd-list $USER` and checks the output for enrolled fingerprint entries.
/// - Returns `false` if `fprintd-list` is not installed, the command fails, or no fingers
///   are enrolled.
/// - Does not require root; `fprintd-list` reads enrollment data via D-Bus.
/// - Informational only — never blocks execution.
pub fn detect_fprintd_enrolled() -> bool {
    let username = std::env::var("USER").unwrap_or_default();
    if username.is_empty() {
        return false;
    }

    let output = Command::new("fprintd-list").arg(&username).output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let has_finger = stdout.lines().any(|line| {
                let trimmed = line.trim().to_lowercase();
                trimmed.contains("left-") || trimmed.contains("right-")
            });
            if has_finger {
                tracing::debug!("fprintd-list reports enrolled fingerprint(s)");
            }
            has_finger
        }
        _ => false,
    }
}

/// What: Cached result of fingerprint availability detection.
///
/// Details:
/// - Combines PAM configuration check and `fprintd-list` enrollment check.
/// - Cached via `OnceLock` since fingerprint availability doesn't change during a session.
static FINGERPRINT_AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// What: Check whether fingerprint authentication appears to be available.
///
/// Inputs:
/// - None (uses the active privilege tool from settings).
///
/// Output:
/// - `true` if both PAM fingerprint integration and an enrolled finger are detected.
///
/// Details:
/// - Result is cached for the lifetime of the process (checked once, never re-checked).
/// - Requires both `pam_fprintd` in the tool's PAM stack AND at least one enrolled finger.
/// - Informational only — used to show a hint in the password modal, never gates execution.
#[must_use]
pub fn is_fingerprint_available() -> bool {
    *FINGERPRINT_AVAILABLE.get_or_init(|| {
        let Ok(tool) = active_tool() else {
            return false;
        };

        let pam_configured = detect_pam_fingerprint(tool);
        if !pam_configured {
            tracing::debug!(tool = %tool, "No pam_fprintd in PAM config for tool");
            return false;
        }

        let enrolled = detect_fprintd_enrolled();
        if !enrolled {
            tracing::debug!("pam_fprintd configured but no enrolled fingerprints found");
        }
        enrolled
    })
}

// ---------------------------------------------------------------------------
// Command builders
// ---------------------------------------------------------------------------

/// What: Build a privilege-escalated command string.
///
/// Inputs:
/// - `tool`: Resolved privilege tool.
/// - `command`: The unprivileged command to wrap.
///
/// Output: Shell string like `"sudo pacman -S foo"` or `"doas pacman -S foo"`.
///
/// Details: Simple prefix — does not handle password piping.
#[must_use]
pub fn build_privilege_command(tool: PrivilegeTool, command: &str) -> String {
    format!("{} {command}", tool.binary_name())
}

/// What: Build a command that pipes a password to the privilege tool via stdin.
///
/// Inputs:
/// - `tool`: Resolved privilege tool.
/// - `password`: Cleartext password.
/// - `command`: The unprivileged command to wrap.
///
/// Output:
/// - `Some(cmd)` for tools that support stdin password (sudo).
/// - `None` for tools that do not (doas).
///
/// Details:
/// - Uses `shell_single_quote` for safe password escaping.
/// - Only sudo supports `-S` (read password from stdin).
#[must_use]
pub fn build_password_pipe(tool: PrivilegeTool, password: &str, command: &str) -> Option<String> {
    if !tool.capabilities().supports_stdin_password {
        return None;
    }
    let escaped = crate::install::shell_single_quote(password);
    Some(format!(
        "printf '%s\\n' {escaped} | {} -S {command}",
        tool.binary_name()
    ))
}

/// What: Build a credential warm-up command that caches the password.
///
/// Inputs:
/// - `tool`: Resolved privilege tool.
/// - `password`: Cleartext password.
///
/// Output:
/// - `Some(cmd)` for tools that support credential refresh (sudo).
/// - `None` for tools that do not (doas).
///
/// Details:
/// - For sudo: `printf '%s\n' '<pass>' | sudo -S -v 2>/dev/null`
/// - Warms up the credential cache so subsequent sudo calls don't re-prompt.
#[must_use]
pub fn build_credential_warmup(tool: PrivilegeTool, password: &str) -> Option<String> {
    if !tool.capabilities().supports_credential_refresh {
        return None;
    }
    let escaped = crate::install::shell_single_quote(password);
    Some(format!(
        "printf '%s\\n' {escaped} | {} -S -v 2>/dev/null",
        tool.binary_name()
    ))
}

/// What: Build a credential invalidation command.
///
/// Inputs:
/// - `tool`: Resolved privilege tool.
///
/// Output:
/// - `Some(cmd)` for tools that support credential invalidation (sudo).
/// - `None` for tools that do not (doas).
///
/// Details:
/// - For sudo: `sudo -k` invalidates cached credentials.
/// - doas has no credential cache to invalidate.
#[must_use]
pub fn build_credential_invalidation(tool: PrivilegeTool) -> Option<String> {
    if !tool.capabilities().supports_credential_invalidation {
        return None;
    }
    Some(format!("{} -k", tool.binary_name()))
}

/// What: Validate a password against the privilege tool.
///
/// Inputs:
/// - `tool`: Resolved privilege tool.
/// - `password`: Password to validate.
///
/// Output:
/// - `Ok(true)` if valid, `Ok(false)` if invalid.
/// - `Err` if the tool doesn't support stdin password or the check fails.
///
/// # Errors
///
/// - Returns `Err` if the tool does not support stdin password validation.
/// - Returns `Err` if the validation command cannot be executed.
///
/// Details:
/// - Only works for tools with `supports_stdin_password` (currently sudo only).
/// - First invalidates cached credentials, then tests the password.
pub fn validate_password(tool: PrivilegeTool, password: &str) -> Result<bool, String> {
    if !tool.capabilities().supports_stdin_password {
        return Err(format!(
            "{tool} does not support password validation via stdin. \
             Configure passwordless {tool} or switch to sudo in settings.conf."
        ));
    }

    let escaped = crate::install::shell_single_quote(password);
    let bin = tool.binary_name();
    let cmd = format!("{bin} -k ; printf '%s\\n' {escaped} | {bin} -S -v 2>&1");

    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .output()
        .map_err(|e| format!("Failed to execute {bin} validation: {e}"))?;

    Ok(output.status.success())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// What: Returns true only when running in integration test context.
///
/// Inputs: None (reads env var `PACSEA_INTEGRATION_TEST`).
///
/// Output: `true` if `PACSEA_INTEGRATION_TEST=1` is set, `false` otherwise.
///
/// Details: Guards test-only env overrides so production never honors them.
fn is_integration_test_context() -> bool {
    std::env::var("PACSEA_INTEGRATION_TEST").is_ok_and(|v| v == "1")
}

/// What: Public wrapper for [`is_integration_test_context`].
///
/// Inputs: None.
///
/// Output: `true` when the process is running inside the integration test harness.
///
/// Details: Exposed so `password.rs` can gate test overrides.
#[must_use]
pub fn is_integration_test() -> bool {
    is_integration_test_context()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PrivilegeTool -------------------------------------------------------

    #[test]
    fn tool_binary_name_sudo() {
        assert_eq!(PrivilegeTool::Sudo.binary_name(), "sudo");
    }

    #[test]
    fn tool_binary_name_doas() {
        assert_eq!(PrivilegeTool::Doas.binary_name(), "doas");
    }

    #[test]
    fn tool_display_matches_binary_name() {
        assert_eq!(format!("{}", PrivilegeTool::Sudo), "sudo");
        assert_eq!(format!("{}", PrivilegeTool::Doas), "doas");
    }

    #[test]
    fn sudo_capabilities_all_enabled() {
        let caps = PrivilegeTool::Sudo.capabilities();
        assert!(caps.supports_stdin_password);
        assert!(caps.supports_credential_refresh);
        assert!(caps.supports_credential_invalidation);
        assert!(caps.supports_askpass);
    }

    #[test]
    fn doas_capabilities_all_disabled() {
        let caps = PrivilegeTool::Doas.capabilities();
        assert!(!caps.supports_stdin_password);
        assert!(!caps.supports_credential_refresh);
        assert!(!caps.supports_credential_invalidation);
        assert!(!caps.supports_askpass);
    }

    // -- PrivilegeMode -------------------------------------------------------

    #[test]
    fn mode_from_config_key_valid() {
        assert_eq!(
            PrivilegeMode::from_config_key("auto"),
            Some(PrivilegeMode::Auto)
        );
        assert_eq!(
            PrivilegeMode::from_config_key("sudo"),
            Some(PrivilegeMode::Sudo)
        );
        assert_eq!(
            PrivilegeMode::from_config_key("doas"),
            Some(PrivilegeMode::Doas)
        );
    }

    #[test]
    fn mode_from_config_key_case_insensitive() {
        assert_eq!(
            PrivilegeMode::from_config_key("AUTO"),
            Some(PrivilegeMode::Auto)
        );
        assert_eq!(
            PrivilegeMode::from_config_key("Sudo"),
            Some(PrivilegeMode::Sudo)
        );
        assert_eq!(
            PrivilegeMode::from_config_key("DOAS"),
            Some(PrivilegeMode::Doas)
        );
    }

    #[test]
    fn mode_from_config_key_with_whitespace() {
        assert_eq!(
            PrivilegeMode::from_config_key("  auto  "),
            Some(PrivilegeMode::Auto)
        );
    }

    #[test]
    fn mode_from_config_key_invalid() {
        assert_eq!(PrivilegeMode::from_config_key(""), None);
        assert_eq!(PrivilegeMode::from_config_key("su"), None);
        assert_eq!(PrivilegeMode::from_config_key("runas"), None);
    }

    #[test]
    fn mode_as_config_key_roundtrip() {
        for mode in [
            PrivilegeMode::Auto,
            PrivilegeMode::Sudo,
            PrivilegeMode::Doas,
        ] {
            let key = mode.as_config_key();
            assert_eq!(PrivilegeMode::from_config_key(key), Some(mode));
        }
    }

    #[test]
    fn mode_default_is_auto() {
        assert_eq!(PrivilegeMode::default(), PrivilegeMode::Auto);
    }

    #[test]
    fn mode_display() {
        assert_eq!(format!("{}", PrivilegeMode::Auto), "auto");
        assert_eq!(format!("{}", PrivilegeMode::Sudo), "sudo");
        assert_eq!(format!("{}", PrivilegeMode::Doas), "doas");
    }

    // -- AuthMode -------------------------------------------------------

    #[test]
    fn auth_mode_from_config_key_valid() {
        assert_eq!(AuthMode::from_config_key("prompt"), Some(AuthMode::Prompt));
        assert_eq!(
            AuthMode::from_config_key("passwordless_only"),
            Some(AuthMode::PasswordlessOnly)
        );
        assert_eq!(
            AuthMode::from_config_key("interactive"),
            Some(AuthMode::Interactive)
        );
    }

    #[test]
    fn auth_mode_from_config_key_case_insensitive() {
        assert_eq!(AuthMode::from_config_key("PROMPT"), Some(AuthMode::Prompt));
        assert_eq!(
            AuthMode::from_config_key("Interactive"),
            Some(AuthMode::Interactive)
        );
        assert_eq!(
            AuthMode::from_config_key("PASSWORDLESS_ONLY"),
            Some(AuthMode::PasswordlessOnly)
        );
    }

    #[test]
    fn auth_mode_from_config_key_hyphen_alias() {
        assert_eq!(
            AuthMode::from_config_key("passwordless-only"),
            Some(AuthMode::PasswordlessOnly)
        );
    }

    #[test]
    fn auth_mode_from_config_key_short_alias() {
        assert_eq!(
            AuthMode::from_config_key("passwordless"),
            Some(AuthMode::PasswordlessOnly)
        );
    }

    #[test]
    fn auth_mode_from_config_key_with_whitespace() {
        assert_eq!(
            AuthMode::from_config_key("  interactive  "),
            Some(AuthMode::Interactive)
        );
    }

    #[test]
    fn auth_mode_from_config_key_invalid() {
        assert_eq!(AuthMode::from_config_key(""), None);
        assert_eq!(AuthMode::from_config_key("fingerprint"), None);
        assert_eq!(AuthMode::from_config_key("password"), None);
    }

    #[test]
    fn auth_mode_as_config_key_roundtrip() {
        for mode in [
            AuthMode::Prompt,
            AuthMode::PasswordlessOnly,
            AuthMode::Interactive,
        ] {
            let key = mode.as_config_key();
            assert_eq!(AuthMode::from_config_key(key), Some(mode));
        }
    }

    #[test]
    fn auth_mode_default_is_prompt() {
        assert_eq!(AuthMode::default(), AuthMode::Prompt);
    }

    #[test]
    fn auth_mode_display() {
        assert_eq!(format!("{}", AuthMode::Prompt), "prompt");
        assert_eq!(
            format!("{}", AuthMode::PasswordlessOnly),
            "passwordless_only"
        );
        assert_eq!(format!("{}", AuthMode::Interactive), "interactive");
    }

    #[test]
    fn auth_mode_always_skips_password_modal() {
        assert!(!AuthMode::Prompt.always_skips_password_modal());
        assert!(!AuthMode::PasswordlessOnly.always_skips_password_modal());
        assert!(AuthMode::Interactive.always_skips_password_modal());
    }

    // -- Resolver (env-controlled) -------------------------------------------

    #[test]
    fn resolve_auto_prefers_doas_when_both_available() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo,doas");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Auto);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(PrivilegeTool::Doas));
    }

    #[test]
    fn resolve_auto_falls_back_to_sudo() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Auto);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(PrivilegeTool::Sudo));
    }

    #[test]
    fn resolve_auto_fails_when_none_available() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "none");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Auto);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        let err = result.expect_err("should fail when no tool available");
        assert!(err.contains("Neither doas nor sudo found"));
    }

    #[test]
    fn resolve_explicit_sudo_succeeds() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Sudo);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(PrivilegeTool::Sudo));
    }

    #[test]
    fn resolve_explicit_sudo_fails_when_missing() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Sudo);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert!(result.is_err());
        let err = result.expect_err("should fail when sudo unavailable");
        assert!(err.contains("sudo is not available"));
    }

    #[test]
    fn resolve_explicit_doas_succeeds() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Doas);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(PrivilegeTool::Doas));
    }

    #[test]
    fn resolve_explicit_doas_fails_when_missing() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
        }
        let result = resolve_privilege_tool(PrivilegeMode::Doas);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert!(result.is_err());
        let err = result.expect_err("should fail when doas unavailable");
        assert!(err.contains("doas is not available"));
    }

    // -- Command builders ----------------------------------------------------

    #[test]
    fn build_privilege_command_sudo() {
        let cmd = build_privilege_command(PrivilegeTool::Sudo, "pacman -S foo");
        assert_eq!(cmd, "sudo pacman -S foo");
    }

    #[test]
    fn build_privilege_command_doas() {
        let cmd = build_privilege_command(PrivilegeTool::Doas, "pacman -S foo");
        assert_eq!(cmd, "doas pacman -S foo");
    }

    #[test]
    fn build_password_pipe_sudo_returns_some() {
        let result = build_password_pipe(PrivilegeTool::Sudo, "secret", "pacman -S foo");
        let cmd = result.expect("sudo should support password pipe");
        assert!(cmd.contains("printf "));
        assert!(cmd.contains("sudo -S pacman -S foo"));
    }

    #[test]
    fn build_password_pipe_doas_returns_none() {
        let result = build_password_pipe(PrivilegeTool::Doas, "secret", "pacman -S foo");
        assert!(result.is_none());
    }

    #[test]
    fn build_credential_warmup_sudo_returns_some() {
        let result = build_credential_warmup(PrivilegeTool::Sudo, "secret");
        let cmd = result.expect("sudo should support credential warmup");
        assert!(cmd.contains("sudo -S -v"));
    }

    #[test]
    fn build_credential_warmup_doas_returns_none() {
        let result = build_credential_warmup(PrivilegeTool::Doas, "secret");
        assert!(result.is_none());
    }

    #[test]
    fn build_credential_invalidation_sudo_returns_some() {
        let cmd = build_credential_invalidation(PrivilegeTool::Sudo)
            .expect("sudo should support credential invalidation");
        assert_eq!(cmd, "sudo -k");
    }

    #[test]
    fn build_credential_invalidation_doas_returns_none() {
        let result = build_credential_invalidation(PrivilegeTool::Doas);
        assert!(result.is_none());
    }

    // -- Password validation -------------------------------------------------

    #[test]
    fn validate_password_doas_returns_err() {
        let result = validate_password(PrivilegeTool::Doas, "any");
        let err = result.expect_err("doas should not support password validation");
        assert!(err.contains("does not support"));
    }

    // -- Availability (env-controlled) ---------------------------------------

    #[test]
    fn is_available_test_override_none() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "none");
        }
        assert!(!PrivilegeTool::Sudo.is_available());
        assert!(!PrivilegeTool::Doas.is_available());
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    #[test]
    fn is_available_test_override_sudo_only() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
        }
        assert!(PrivilegeTool::Sudo.is_available());
        assert!(!PrivilegeTool::Doas.is_available());
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    #[test]
    fn is_available_test_override_both() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo,doas");
        }
        assert!(PrivilegeTool::Sudo.is_available());
        assert!(PrivilegeTool::Doas.is_available());
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    // -- is_integration_test -------------------------------------------------

    #[test]
    fn is_integration_test_when_set() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
        }
        assert!(is_integration_test());
        unsafe {
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    #[test]
    fn is_integration_test_when_unset() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert!(!is_integration_test());
    }

    #[test]
    fn is_integration_test_wrong_value() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "0");
        }
        assert!(!is_integration_test());
        unsafe {
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }

    // -- active_tool ---------------------------------------------------------

    #[test]
    fn active_tool_returns_valid_tool() {
        let tool = active_tool();
        assert!(
            tool == PrivilegeTool::Sudo || tool == PrivilegeTool::Doas,
            "active_tool should return Sudo or Doas"
        );
    }

    // -- Password pipe format ------------------------------------------------

    #[test]
    fn build_password_pipe_uses_printf_not_echo() {
        let result = build_password_pipe(PrivilegeTool::Sudo, "pw", "cmd");
        let cmd = result.expect("sudo pipe should return Some");
        assert!(cmd.starts_with("printf "), "should use printf, not echo");
        assert!(cmd.contains("%s\\n"), "should use %s\\n format");
    }

    #[test]
    fn build_password_pipe_escapes_special_chars() {
        let result = build_password_pipe(PrivilegeTool::Sudo, "pa's$word", "pacman -S foo");
        let cmd = result.expect("sudo pipe should return Some");
        assert!(cmd.contains("sudo -S pacman -S foo"));
        assert!(!cmd.contains("pa's$word"), "password must be shell-escaped");
    }

    #[test]
    fn build_credential_warmup_uses_printf() {
        let result = build_credential_warmup(PrivilegeTool::Sudo, "pw");
        let cmd = result.expect("sudo warmup should return Some");
        assert!(cmd.starts_with("printf "), "warmup should use printf");
        assert!(cmd.contains("sudo -S -v"));
        assert!(cmd.contains("2>/dev/null"));
    }

    // -- Doas command builders -----------------------------------------------

    #[test]
    fn build_privilege_command_doas_format() {
        let cmd = build_privilege_command(PrivilegeTool::Doas, "pacman -Syu --noconfirm");
        assert_eq!(cmd, "doas pacman -Syu --noconfirm");
    }

    #[test]
    fn doas_password_pipe_returns_none() {
        assert!(build_password_pipe(PrivilegeTool::Doas, "pw", "cmd").is_none());
    }

    #[test]
    fn doas_credential_warmup_returns_none() {
        assert!(build_credential_warmup(PrivilegeTool::Doas, "pw").is_none());
    }

    #[test]
    fn doas_credential_invalidation_returns_none() {
        assert!(build_credential_invalidation(PrivilegeTool::Doas).is_none());
    }

    // -- Passwordless check --------------------------------------------------

    #[test]
    fn check_passwordless_test_override_true() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "1");
        }
        let result = PrivilegeTool::Sudo.check_passwordless();
        unsafe {
            std::env::remove_var("PACSEA_TEST_SUDO_PASSWORDLESS");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(true));
    }

    #[test]
    fn check_passwordless_test_override_false() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "0");
        }
        let result = PrivilegeTool::Sudo.check_passwordless();
        unsafe {
            std::env::remove_var("PACSEA_TEST_SUDO_PASSWORDLESS");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(false));
    }

    #[test]
    fn check_passwordless_doas_test_override() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_SUDO_PASSWORDLESS", "1");
        }
        let result = PrivilegeTool::Doas.check_passwordless();
        unsafe {
            std::env::remove_var("PACSEA_TEST_SUDO_PASSWORDLESS");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(result, Ok(true));
    }

    // -- Validate password ---------------------------------------------------

    #[test]
    fn validate_password_doas_returns_unsupported() {
        let result = validate_password(PrivilegeTool::Doas, "any");
        let err = result.expect_err("doas should not support password validation");
        assert!(
            err.contains("does not support"),
            "unexpected error message: {err}"
        );
    }

    // -- Tool symmetry -------------------------------------------------------

    #[test]
    fn both_tools_produce_distinct_commands() {
        let sudo_cmd = build_privilege_command(PrivilegeTool::Sudo, "pacman -S foo");
        let doas_cmd = build_privilege_command(PrivilegeTool::Doas, "pacman -S foo");
        assert_ne!(sudo_cmd, doas_cmd, "sudo and doas commands must differ");
        assert!(sudo_cmd.starts_with("sudo "));
        assert!(doas_cmd.starts_with("doas "));
    }

    #[test]
    fn capabilities_are_complementary() {
        let sudo_caps = PrivilegeTool::Sudo.capabilities();
        let doas_caps = PrivilegeTool::Doas.capabilities();
        assert_ne!(
            sudo_caps.supports_stdin_password, doas_caps.supports_stdin_password,
            "sudo and doas should differ on stdin password support"
        );
    }

    // -- Fingerprint detection ------------------------------------------------

    #[test]
    fn detect_pam_fingerprint_sudo_does_not_panic() {
        // Returns true only if /etc/pam.d/sudo (or system-auth) has pam_fprintd.
        // In CI or most test machines this is false; we verify no panic.
        let _result = detect_pam_fingerprint(PrivilegeTool::Sudo);
    }

    #[test]
    fn detect_pam_fingerprint_doas_does_not_panic() {
        let _result = detect_pam_fingerprint(PrivilegeTool::Doas);
    }

    #[test]
    fn detect_fprintd_enrolled_does_not_panic() {
        let _result = detect_fprintd_enrolled();
    }

    #[test]
    fn is_fingerprint_available_does_not_panic() {
        let _result = is_fingerprint_available();
    }
}
