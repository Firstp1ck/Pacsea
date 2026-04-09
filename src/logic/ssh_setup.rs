//! SSH setup workflow helpers for AUR voting.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// Fixed host used by AUR SSH voting.
const AUR_HOST: &str = "aur.archlinux.org";
/// Fixed SSH key file name for the guided setup flow.
const AUR_KEY_NAME: &str = "aur_key";
/// Account URL shown in the setup flow.
pub const AUR_ACCOUNT_URL: &str = "https://aur.archlinux.org/account";

/// What: Check whether `openssh` is installed on the system.
///
/// Inputs: None.
///
/// Output:
/// - `true` when `openssh` is detected as installed.
///
/// Details:
/// - Uses Pacsea installed-package index (`openssh` package name).
#[must_use]
pub fn is_openssh_installed() -> bool {
    #[cfg(test)]
    if let Ok(v) = std::env::var("PACSEA_TEST_OPENSSH_INSTALLED") {
        return v == "1";
    }
    crate::index::is_installed("openssh")
}

/// What: Workflow result for attempting SSH setup actions.
///
/// Inputs:
/// - Produced by `run_aur_ssh_setup`.
///
/// Output:
/// - Either a completed report or an overwrite-confirmation request.
///
/// Details:
/// - `NeedsOverwrite` includes the currently detected host block and progress lines.
pub enum AurSshSetupResult {
    /// Setup finished (success or failure details are in `report.success` + `report.lines`).
    Completed(AurSshSetupReport),
    /// Existing host block requires explicit user overwrite confirmation.
    NeedsOverwrite {
        /// Existing host block text from `~/.ssh/config`.
        existing_block: String,
        /// Status lines generated before the overwrite decision point.
        lines: Vec<String>,
    },
}

/// What: Final setup report for modal rendering.
///
/// Inputs:
/// - Built by `run_aur_ssh_setup`.
///
/// Output:
/// - `success` flag and human-readable status lines.
pub struct AurSshSetupReport {
    /// Whether the full workflow completed successfully.
    pub success: bool,
    /// Human-readable step/result lines for UI display.
    pub lines: Vec<String>,
}

/// What: Detect whether AUR SSH setup appears configured locally.
///
/// Inputs: None.
///
/// Output:
/// - `true` when key exists and `~/.ssh/config` has required AUR host directives.
///
/// Details:
/// - This check is local-only and does not validate remote SSH auth/network.
#[must_use]
pub fn is_aur_ssh_setup_configured() -> bool {
    let Some(home) = home_dir() else {
        return false;
    };
    let ssh_dir = home.join(".ssh");
    let key_path = ssh_dir.join(AUR_KEY_NAME);
    if !key_path.exists() {
        return false;
    }
    let config_path = ssh_dir.join("config");
    let Ok(content) = fs::read_to_string(config_path) else {
        return false;
    };
    find_host_block(&content, AUR_HOST)
        .is_some_and(|(_, _, block)| block_has_required_directives(&block))
}

/// What: Run the guided AUR SSH setup flow.
///
/// Inputs:
/// - `overwrite_existing_host`: Whether to overwrite a conflicting existing host block.
/// - `ssh_command`: SSH binary path or name used for validation command execution.
///
/// Output:
/// - `AurSshSetupResult` with either completion report or explicit overwrite request.
///
/// Details:
/// - Creates `~/.ssh` if missing.
/// - Generates `~/.ssh/aur_key` via `ssh-keygen` if not present.
/// - Writes/updates minimal `Host aur.archlinux.org` block.
/// - Validates with `ssh -o BatchMode=yes -o ConnectTimeout=10 aur@aur.archlinux.org help`.
#[must_use]
pub fn run_aur_ssh_setup(overwrite_existing_host: bool, ssh_command: &str) -> AurSshSetupResult {
    let mut lines = Vec::new();
    let Some(home) = home_dir() else {
        return AurSshSetupResult::Completed(AurSshSetupReport {
            success: false,
            lines: vec!["Failed: HOME environment is not set.".to_string()],
        });
    };
    let ssh_dir = home.join(".ssh");
    if let Err(err) = fs::create_dir_all(&ssh_dir) {
        return AurSshSetupResult::Completed(AurSshSetupReport {
            success: false,
            lines: vec![format!("Failed to create '{}': {err}", ssh_dir.display())],
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(&ssh_dir, fs::Permissions::from_mode(0o700)) {
            lines.push(format!(
                "Warning: could not set '{}' permissions to 700: {err}",
                ssh_dir.display()
            ));
        }
    }

    let key_path = ssh_dir.join(AUR_KEY_NAME);
    if key_path.exists() {
        lines.push(format!("Key exists: '{}'", key_path.display()));
    } else {
        let output = Command::new("ssh-keygen")
            .args(["-t", "ed25519", "-f"])
            .arg(&key_path)
            .args(["-N", ""])
            .output();
        match output {
            Ok(out) if out.status.success() => {
                lines.push(format!("Created key pair: '{}'", key_path.display()));
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                lines.push(format!(
                    "Failed to generate SSH key (exit {}): {}",
                    out.status.code().unwrap_or(-1),
                    if stderr.is_empty() {
                        "no stderr output".to_string()
                    } else {
                        stderr
                    }
                ));
                return AurSshSetupResult::Completed(AurSshSetupReport {
                    success: false,
                    lines,
                });
            }
            Err(err) => {
                lines.push(format!("Failed to run ssh-keygen: {err}"));
                return AurSshSetupResult::Completed(AurSshSetupReport {
                    success: false,
                    lines,
                });
            }
        }
    }

    let config_path = ssh_dir.join("config");
    match write_or_update_aur_host_config(&config_path, overwrite_existing_host, &mut lines) {
        Ok(Some(existing_block)) => {
            return AurSshSetupResult::NeedsOverwrite {
                existing_block,
                lines,
            };
        }
        Ok(None) => {}
        Err(err) => {
            lines.push(format!(
                "Failed to update '{}': {err}",
                config_path.display()
            ));
            return AurSshSetupResult::Completed(AurSshSetupReport {
                success: false,
                lines,
            });
        }
    }

    let validation = Command::new(ssh_command)
        .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=10"])
        .arg("aur@aur.archlinux.org")
        .arg("help")
        .output();
    match validation {
        Ok(out) if out.status.success() => {
            lines.push("Validation OK: 'ssh aur@aur.archlinux.org help' succeeded.".to_string());
            AurSshSetupResult::Completed(AurSshSetupReport {
                success: true,
                lines,
            })
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            lines.push(format!(
                "Validation failed (exit {}): {}",
                out.status.code().unwrap_or(-1),
                if detail.is_empty() {
                    "no output".to_string()
                } else {
                    detail
                }
            ));
            lines.push(format!(
                "Next step: upload public key '{}' to {}",
                key_path.with_extension("pub").display(),
                AUR_ACCOUNT_URL
            ));
            AurSshSetupResult::Completed(AurSshSetupReport {
                success: false,
                lines,
            })
        }
        Err(err) => {
            lines.push(format!("Failed to run SSH validation: {err}"));
            AurSshSetupResult::Completed(AurSshSetupReport {
                success: false,
                lines,
            })
        }
    }
}

/// What: Spawn a background SSH validation check for AUR endpoint readiness.
///
/// Inputs:
/// - `ssh_command`: SSH binary path or name used for the endpoint check.
///
/// Output:
/// - Shared handle containing `Some(true/false)` when finished, or `None` while running.
///
/// Details:
/// - Runs `{ssh_command} -o BatchMode=yes -o ConnectTimeout=8 aur@aur.archlinux.org help`
///   on a worker thread.
#[must_use]
pub fn spawn_aur_ssh_help_check(ssh_command: String) -> Arc<Mutex<Option<bool>>> {
    let result = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&result);
    std::thread::spawn(move || {
        let ok = Command::new(&ssh_command)
            .args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"])
            .arg("aur@aur.archlinux.org")
            .arg("help")
            .output()
            .is_ok_and(|out| out.status.success());
        if let Ok(mut slot) = result_clone.lock() {
            *slot = Some(ok);
        }
    });
    result
}

/// What: Resolve current user's home directory.
fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(PathBuf::from)
}

/// What: Build the target host block text for AUR SSH voting.
fn desired_aur_host_block() -> String {
    "Host aur.archlinux.org\n  User aur\n  IdentityFile ~/.ssh/aur_key\n  IdentitiesOnly yes\n"
        .to_string()
}

/// What: Find one host block range by host token.
///
/// Output:
/// - `(start_byte, end_byte, block_text)` when found.
fn find_host_block(content: &str, host: &str) -> Option<(usize, usize, String)> {
    let mut entries: Vec<(usize, &str)> = Vec::new();
    let mut start = 0usize;
    for line in content.lines() {
        entries.push((start, line));
        start = start.saturating_add(line.len()).saturating_add(1);
    }
    let mut block_start: Option<usize> = None;
    let mut end = content.len();
    for (line_start, line) in entries {
        let trimmed = line.trim();
        if !trimmed.starts_with("Host ") {
            continue;
        }
        if block_start.is_none() {
            let hosts = trimmed.trim_start_matches("Host ").split_whitespace();
            if hosts.into_iter().any(|entry| entry == host) {
                block_start = Some(line_start);
            }
            continue;
        }
        end = line_start;
        break;
    }
    let start = block_start?;
    Some((start, end, content[start..end].trim_end().to_string()))
}

/// What: Determine whether a host block contains required directives.
fn block_has_required_directives(block: &str) -> bool {
    let mut user_ok = false;
    let mut id_ok = false;
    let mut only_ok = false;
    for line in block.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("User aur") {
            user_ok = true;
        } else if trimmed.eq_ignore_ascii_case("IdentityFile ~/.ssh/aur_key") {
            id_ok = true;
        } else if trimmed.eq_ignore_ascii_case("IdentitiesOnly yes") {
            only_ok = true;
        }
    }
    user_ok && id_ok && only_ok
}

/// What: Update `~/.ssh/config` with desired host block.
///
/// Output:
/// - `Ok(Some(existing_block))` when overwrite confirmation is required.
/// - `Ok(None)` when write/update succeeded or file already compliant.
fn write_or_update_aur_host_config(
    config_path: &Path,
    overwrite_existing_host: bool,
    lines: &mut Vec<String>,
) -> Result<Option<String>, String> {
    let desired = desired_aur_host_block();
    let mut content = fs::read_to_string(config_path).unwrap_or_default();
    if let Some((start, end, existing)) = find_host_block(&content, AUR_HOST) {
        if block_has_required_directives(&existing) {
            lines.push(format!(
                "SSH config already contains required '{AUR_HOST}'."
            ));
            return Ok(None);
        }
        if !overwrite_existing_host {
            lines.push(format!(
                "Existing '{AUR_HOST}' block detected. Confirmation required to overwrite."
            ));
            return Ok(Some(existing));
        }
        content.replace_range(start..end, &desired);
        lines.push(format!("Overwrote existing '{AUR_HOST}' host block."));
    } else {
        if !content.is_empty() && !content.ends_with('\n') {
            content.push('\n');
        }
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(&desired);
        lines.push(format!("Added new '{AUR_HOST}' host block."));
    }
    fs::write(config_path, content).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = fs::set_permissions(config_path, fs::Permissions::from_mode(0o600)) {
            lines.push(format!(
                "Warning: could not set '{}' permissions to 600: {err}",
                config_path.display()
            ));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_config_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        std::env::temp_dir().join(format!(
            "pacsea_{name}_{}_{}.conf",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn block_directives_detected() {
        let block = "Host aur.archlinux.org\n  User aur\n  IdentityFile ~/.ssh/aur_key\n  IdentitiesOnly yes\n";
        assert!(block_has_required_directives(block));
    }

    #[test]
    fn block_directives_missing_detected() {
        let block = "Host aur.archlinux.org\n  User aur\n  IdentityFile ~/.ssh/id_ed25519\n";
        assert!(!block_has_required_directives(block));
    }

    #[test]
    fn find_host_block_returns_range() {
        let content = "Host github.com\n  User git\n\nHost aur.archlinux.org\n  User aur\n";
        let found = find_host_block(content, "aur.archlinux.org")
            .expect("expected aur host block to be found");
        assert!(found.2.contains("Host aur.archlinux.org"));
    }

    #[test]
    fn write_or_update_requests_overwrite_for_conflicting_block() {
        let path = temp_config_path("ssh_setup_conflict");
        let original = "Host aur.archlinux.org\n  User aur\n  IdentityFile ~/.ssh/id_ed25519\n";
        fs::write(&path, original).expect("should write temp config");
        let mut lines = Vec::new();
        let result =
            write_or_update_aur_host_config(&path, false, &mut lines).expect("should not error");
        assert!(
            result.is_some(),
            "conflicting block should request overwrite"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn write_or_update_writes_expected_block_when_missing() {
        let path = temp_config_path("ssh_setup_missing");
        let _ = fs::remove_file(&path);
        let mut lines = Vec::new();
        let result =
            write_or_update_aur_host_config(&path, false, &mut lines).expect("should not error");
        assert!(result.is_none(), "missing block should be written directly");
        let body = fs::read_to_string(&path).expect("config should be created");
        assert!(body.contains("Host aur.archlinux.org"));
        assert!(body.contains("IdentityFile ~/.ssh/aur_key"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn openssh_check_honors_test_override() {
        unsafe {
            std::env::set_var("PACSEA_TEST_OPENSSH_INSTALLED", "1");
        }
        assert!(is_openssh_installed());
        unsafe {
            std::env::set_var("PACSEA_TEST_OPENSSH_INSTALLED", "0");
        }
        assert!(!is_openssh_installed());
        unsafe {
            std::env::remove_var("PACSEA_TEST_OPENSSH_INSTALLED");
        }
    }
}
