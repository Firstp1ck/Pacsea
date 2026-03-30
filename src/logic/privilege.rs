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
//! | Password via stdin | `sudo -S` reads stdin | **NOT supported** (uses PTY validation path) |
//! | Credential refresh | `sudo -v` | **NOT supported** |
//! | Credential invalidation | `sudo -k` | **NOT supported** |
//! | Askpass env var | `SUDO_ASKPASS` | **NOT supported** |
//!
//! ## Implications for Pacsea
//!
//! - `OpenDoas` reads the password from `/dev/tty`, not stdin — it has no `-S` flag.
//! - Pacsea validates doas credentials through a native PTY session (`portable-pty`)
//!   and then executes plain `doas <cmd>`.
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
/// - `Doas` uses the `OpenDoas` binary with partial feature support
///   (PTY-based validation, no stdin password flag, no credential cache APIs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrivilegeTool {
    /// Standard sudo — full feature support.
    Sudo,
    /// `OpenDoas` — partial feature support (PTY validation, no `-S`, no credential cache APIs).
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
/// - doas does not support stdin password piping (`-S`), and lacks credential cache APIs.
/// - Used to route behavior: e.g. disable warm-up paths for tools without cache refresh support.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct PrivilegeCapabilities {
    /// Tool supports direct stdin password entry (`sudo -S`).
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
    /// - doas: stdin password disabled, credential cache features disabled.
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

/// What: Resolve the privilege tool for a given mode, applying Auto-only fallback policy.
///
/// Inputs:
/// - `mode`: Privilege selection mode (from settings, tests, or callers).
///
/// Output:
/// - `Ok(PrivilegeTool)` when resolution succeeds, including `Ok(Sudo)` after `Auto`
///   resolution failure (with warning logged).
/// - `Err(String)` with an actionable message when `Sudo` or `Doas` mode is set but that
///   binary is missing from `$PATH`.
///
/// Details:
/// - Explicit modes never substitute the other tool; `Auto` may fall back to sudo so
///   behaviour stays lenient when no privilege binary is installed.
fn active_tool_for_mode(mode: PrivilegeMode) -> Result<PrivilegeTool, String> {
    match resolve_privilege_tool(mode) {
        Ok(tool) => Ok(tool),
        Err(err) if mode == PrivilegeMode::Auto => {
            tracing::warn!(
                configured_mode = %mode,
                error = %err,
                fallback = "sudo",
                "Privilege tool auto-resolution failed — falling back to sudo; privileged commands may fail if sudo is missing"
            );
            Ok(PrivilegeTool::Sudo)
        }
        Err(err) => Err(err),
    }
}

/// What: Resolve the privilege tool using the cached application settings.
///
/// Inputs: None (reads `crate::theme::settings().privilege_mode`).
///
/// Output:
/// - `Ok(PrivilegeTool)` for successful resolution, or `Ok(Sudo)` after `Auto` failure
///   (see [`active_tool_for_mode`]).
/// - `Err(String)` when explicit `sudo` or `doas` mode cannot be satisfied.
///
/// Details:
/// - Callers should propagate or display `Err` so misconfiguration is visible.
/// - Same fallback rules as [`active_tool_for_mode`].
///
/// # Errors
///
/// Returns `Err` when [`PrivilegeMode::Sudo`] or [`PrivilegeMode::Doas`] is configured but
/// that tool is not available on `$PATH`. [`PrivilegeMode::Auto`] never errors here: it falls
/// back to [`PrivilegeTool::Sudo`] after logging a warning.
#[must_use = "caller should handle missing explicit privilege tools"]
pub fn active_tool() -> Result<PrivilegeTool, String> {
    let settings = crate::theme::settings();
    active_tool_for_mode(settings.privilege_mode)
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
/// - `Some(cmd)` for tools that support stdin password entry.
/// - `None` for tools that do not.
///
/// Details:
/// - Uses `shell_single_quote` for safe password escaping.
/// - sudo: pipes password via `-S` flag (`printf … | sudo -S <cmd>`).
/// - doas: returns `None` because `OpenDoas` has no stdin password flag (`-S`).
#[must_use]
pub fn build_password_pipe(tool: PrivilegeTool, password: &str, command: &str) -> Option<String> {
    if !tool.capabilities().supports_stdin_password {
        return None;
    }
    let escaped = crate::install::shell_single_quote(password);
    let bin = tool.binary_name();
    Some(format!("printf '%s\\n' {escaped} | {bin} -S {command}"))
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
/// - `Err` if the tool doesn't support password validation or the check fails.
///
/// # Errors
///
/// - Returns `Err` if the tool does not support password validation.
/// - Returns `Err` if the validation command cannot be executed.
/// - Returns `Err` if doas policy denies all probe commands.
///
/// Details:
/// - sudo: invalidates cached credentials and validates with `sudo -S -v`.
/// - doas: uses native PTY probes (`doas true`, fallback `doas pacman -V`)
///   to distinguish auth failure from policy denial (see [`validate_doas_password`]).
pub fn validate_password(tool: PrivilegeTool, password: &str) -> Result<bool, String> {
    if tool == PrivilegeTool::Doas {
        return validate_doas_password(password, tool.binary_name());
    }

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

/// `OpenDoas` stderr marker emitted on authentication failure (wrong password).
const DOAS_AUTH_FAILED: &str = "Authentication failed";
/// `OpenDoas` stderr marker for policy denial.
const DOAS_POLICY_DENIED: &str = "Operation not permitted";

/// What: Validate a password specifically for doas using native PTY probes.
///
/// Inputs:
/// - `password`: Plain password input from the modal.
/// - `bin`: Binary name (`"doas"`).
///
/// Output:
/// - `Ok(true)` if the password is valid.
/// - `Ok(false)` if the password is wrong (doas emitted "Authentication failed").
/// - `Err` if neither probe command is permitted by policy.
///
/// Details:
/// - doas has no `sudo -v` equivalent — every invocation requires a real command.
/// - doas has no `-S` flag — it reads from `/dev/tty`, not stdin.
/// - We spawn doas in a pseudo-terminal and write `password\n` to the PTY master.
/// - `OpenDoas` checks policy **before** authentication, so a policy denial means
///   the password was never tested and we must try a different command.
/// - Primary probe: `doas true` (minimal, no side effects).
/// - Fallback probe: `doas pacman -V` (read-only, safe for a pacman frontend).
fn validate_doas_password(password: &str, bin: &str) -> Result<bool, String> {
    if let Some(result) = run_doas_pty_probe(password, bin, &["true"])? {
        return Ok(result);
    }

    if let Some(result) = run_doas_pty_probe(password, bin, &["pacman", "-V"])? {
        return Ok(result);
    }

    Err(format!(
        "Cannot validate password: {bin} policy does not permit probe commands (true, pacman). \
         Add a matching rule in /etc/doas.conf or configure passwordless doas."
    ))
}

/// What: Execute a single doas probe command in a pseudo-terminal and classify outcome.
///
/// Inputs:
/// - `password`: Plain password to write to the doas prompt.
/// - `bin`: Binary name for error messages (`"doas"`).
/// - `args`: Command arguments to pass to doas (e.g. `["true"]`).
///
/// Output:
/// - `Ok(Some(true))` — command succeeded, password is valid.
/// - `Ok(Some(false))` — authentication failed (wrong password).
/// - `Ok(None)` — policy denied the command (password was never checked).
/// - `Err` — the shell command could not be spawned at all.
///
/// Details:
/// - Uses `portable-pty` so doas can read from `/dev/tty` as designed.
/// - Writes `password\n` to the PTY master, then reads output for diagnostics.
/// - Distinguishes auth failure from policy denial via [`DOAS_AUTH_FAILED`].
fn run_doas_pty_probe(password: &str, bin: &str, args: &[&str]) -> Result<Option<bool>, String> {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::{Read, Write};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    let pty_system = native_pty_system();
    let pty = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to create PTY for {bin} validation: {e}"))?;

    let mut cmd_builder = CommandBuilder::new(bin);
    for arg in args {
        cmd_builder.arg(arg);
    }

    let mut child = pty
        .slave
        .spawn_command(cmd_builder)
        .map_err(|e| format!("Failed to execute {bin} validation: {e}"))?;

    let reader = pty
        .master
        .try_clone_reader()
        .map_err(|e| format!("Failed to open PTY reader for {bin} validation: {e}"))?;

    let mut writer = pty
        .master
        .take_writer()
        .map_err(|e| format!("Failed to open PTY writer for {bin} validation: {e}"))?;

    let wait_timeout = Duration::from_secs(5);
    let poll_interval = Duration::from_millis(25);
    let started = Instant::now();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut local_reader = reader;
        let mut buf = [0_u8; 512];
        loop {
            match local_reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut combined = String::new();
    let mut password_sent = false;

    loop {
        while let Ok(chunk) = rx.try_recv() {
            combined.push_str(&String::from_utf8_lossy(&chunk));
        }

        if !password_sent && contains_password_prompt(&combined) {
            writer
                .write_all(password.as_bytes())
                .map_err(|e| format!("Failed to write password to {bin} PTY: {e}"))?;
            writer
                .write_all(b"\n")
                .map_err(|e| format!("Failed to write newline to {bin} PTY: {e}"))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush {bin} PTY writer: {e}"))?;
            password_sent = true;
        }

        if combined.contains(DOAS_AUTH_FAILED) {
            let _ = child.kill();
            return Ok(Some(false));
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return Ok(Some(true));
                }
                if combined.contains(DOAS_POLICY_DENIED) {
                    return Ok(None);
                }
                if combined.contains(DOAS_AUTH_FAILED) {
                    return Ok(Some(false));
                }
                return Ok(None);
            }
            Ok(None) => {}
            Err(e) => return Err(format!("Failed while waiting for {bin} validation: {e}")),
        }

        if started.elapsed() >= wait_timeout {
            let _ = child.kill();
            let timeout_hint = if password_sent {
                "doas did not complete after password input"
            } else {
                "no password prompt detected"
            };
            return Err(format!(
                "{bin} validation timed out after {}s ({timeout_hint}). \
                 Check /etc/doas.conf and retry.",
                wait_timeout.as_secs()
            ));
        }

        thread::sleep(poll_interval);
    }
}

/// What: Detect whether the accumulated doas PTY output contains a password prompt.
///
/// Inputs:
/// - `output`: Accumulated PTY output as UTF-8 string.
///
/// Output:
/// - `true` if a known password prompt marker is present.
///
/// Details:
/// - Matches common prompt fragments case-insensitively (`password`, `passwort`).
fn contains_password_prompt(output: &str) -> bool {
    let lowercase = output.to_ascii_lowercase();
    lowercase.contains("password") || lowercase.contains("passwort")
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
    fn doas_capabilities_partial_support() {
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
    fn validate_password_doas_runs_probe_or_reports_policy() {
        let result = validate_password(PrivilegeTool::Doas, "any");
        // Depending on system: Ok(bool) if doas is installed and policy permits,
        // Err if doas is missing, PTY setup fails, or policy blocks all probes.
        if let Err(err) = result {
            assert!(
                err.contains("doas validation")
                    || err.contains("policy does not permit probe commands"),
                "unexpected error message: {err}"
            );
        }
    }

    #[test]
    fn run_doas_probe_auth_failed_detection() {
        let combined_output = "doas: Authentication failed\n";
        assert!(combined_output.contains(DOAS_AUTH_FAILED));
    }

    #[test]
    fn run_doas_probe_policy_denied_detection() {
        let combined_output = "doas: Operation not permitted\n";
        assert!(!combined_output.contains(DOAS_AUTH_FAILED));
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
    fn active_tool_returns_ok_with_sudo_or_doas() {
        let tool = active_tool().expect("active_tool should resolve in this environment");
        assert!(
            tool == PrivilegeTool::Sudo || tool == PrivilegeTool::Doas,
            "active_tool should return Sudo or Doas"
        );
    }

    #[test]
    fn active_tool_for_mode_explicit_doas_errors_when_missing() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo");
        }
        let result = super::active_tool_for_mode(PrivilegeMode::Doas);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        let err = result.expect_err("explicit doas without doas on PATH should error");
        assert!(err.contains("doas is not available"));
    }

    #[test]
    fn active_tool_for_mode_explicit_sudo_errors_when_missing() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "doas");
        }
        let result = super::active_tool_for_mode(PrivilegeMode::Sudo);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        let err = result.expect_err("explicit sudo without sudo on PATH should error");
        assert!(err.contains("sudo is not available"));
    }

    #[test]
    fn active_tool_for_mode_auto_none_falls_back_to_sudo() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "none");
        }
        let result = super::active_tool_for_mode(PrivilegeMode::Auto);
        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
        assert_eq!(
            result,
            Ok(PrivilegeTool::Sudo),
            "Auto with no tools should still return Ok(Sudo) after fallback"
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
        let result = build_password_pipe(PrivilegeTool::Doas, "pw", "cmd");
        assert!(result.is_none());
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
}
