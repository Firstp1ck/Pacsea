/// What: Check which AUR helper is available (paru or yay).
///
/// Output:
/// - Tuple of (`has_paru`, `has_yay`, `helper_name`)
pub fn check_aur_helper() -> (bool, bool, &'static str) {
    use std::process::{Command, Stdio};

    let has_paru = Command::new("paru")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok();

    let has_yay = if has_paru {
        false
    } else {
        Command::new("yay")
            .args(["--version"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok()
    };

    let helper = if has_paru { "paru" } else { "yay" };
    if has_paru || has_yay {
        tracing::debug!("Using {} to check for AUR updates", helper);
    }

    (has_paru, has_yay, helper)
}

/// What: Payload from the background package update check worker.
///
/// Inputs:
/// - N/A (constructed by the worker).
///
/// Output:
/// - Carries update count, package name list, whether the official-repo probe was authoritative,
///   and ordered machine-readable reason codes for logs/diagnostics.
///
/// Details:
/// - `authoritative` is `false` when the official repo list came only from a degraded path
///   (e.g. stale system `pacman` db) or when the worker panicked.
#[derive(Debug, Clone)]
pub struct UpdateCheckPayload {
    /// What: Count of packages in `package_names` after deduplication.
    pub count: usize,
    /// What: Sorted package names with available updates (official + AUR combined).
    pub package_names: Vec<String>,
    /// What: When `true`, the official repository list came from a synced or `checkupdates` path.
    pub authoritative: bool,
    /// What: Machine-readable reason codes from non-authoritative or failed sub-steps.
    pub reason_codes: Vec<String>,
    /// What: Which official-repo strategy produced the repo portion (e.g. `checkupdates_db`).
    pub official_strategy: &'static str,
}

impl UpdateCheckPayload {
    /// What: Build an empty payload used when the worker task panics.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Payload with zero updates and `authoritative` false.
    ///
    /// Details:
    /// - Marks `reason_codes` with `worker_panic` for log-based triage.
    pub fn worker_panic() -> Self {
        Self {
            count: 0,
            package_names: Vec::new(),
            authoritative: false,
            reason_codes: vec!["worker_panic".to_string()],
            official_strategy: "none",
        }
    }
}

/// Libalpm / pacman sandbox could not apply Landlock rules (typical pacman stderr).
pub const REASON_LANDLOCK_SANDBOX_FAILURE: &str = "landlock_sandbox_failure";
/// Switching to sandbox user `alpm` failed during pacman operations.
pub const REASON_ALPM_SANDBOX_FAILURE: &str = "alpm_sandbox_failure";
/// Generic permission / operation-not-permitted style failure text.
pub const REASON_PERMISSION_DENIED: &str = "permission_denied";
/// `fakeroot pacman -Sy --dbpath` failed for the temp database.
pub const REASON_TEMP_DB_SYNC_FAILED: &str = "temp_db_sync_failed";
/// `checkupdates` exited with an error (not 0 or 1).
pub const REASON_CHECKUPDATES_FAILED: &str = "checkupdates_failed";
/// Fell back to system `pacman -Qu` without a fresh sync.
pub const REASON_STALE_DB_FALLBACK: &str = "stale_db_fallback";
/// `fakeroot` was missing when the temp-database sync path was needed.
pub const REASON_FAKEROOT_UNAVAILABLE: &str = "fakeroot_unavailable";
/// `checkupdates` was missing when a non-root fresh sync was needed.
pub const REASON_CHECKUPDATES_UNAVAILABLE: &str = "checkupdates_unavailable";

/// What: Map pacman-related stderr to structured reason codes for logs and metrics.
///
/// Inputs:
/// - `stderr`: Raw stderr text from pacman-related subprocesses (including `fakeroot pacman`).
///
/// Output:
/// - Possibly empty vector of reason code strings (no duplicates enforced here).
///
/// Details:
/// - Matching is ASCII-lowercased substring search to tolerate localized prefix text.
pub fn classify_pacman_stderr_for_update_check(stderr: &str) -> Vec<String> {
    let lower = stderr.to_lowercase();
    let mut out = Vec::new();
    if lower.contains("landlock") {
        out.push(REASON_LANDLOCK_SANDBOX_FAILURE.to_string());
    }
    if lower.contains("alpm") && lower.contains("sandbox") {
        out.push(REASON_ALPM_SANDBOX_FAILURE.to_string());
    }
    if lower.contains("operation not permitted")
        || lower.contains("die operation ist nicht erlaubt")
    {
        out.push(REASON_PERMISSION_DENIED.to_string());
    }
    out
}

/// What: Check if `fakeroot` is available on the system.
///
/// Output:
/// - `true` if `fakeroot` is available, `false` otherwise
///
/// Details:
/// - Used only when `checkupdates` is missing, to attempt a non-root temp-database `pacman -Sy`.
#[cfg(not(target_os = "windows"))]
pub fn has_fakeroot() -> bool {
    use std::process::{Command, Stdio};

    Command::new("fakeroot")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Check if checkupdates is available on the system.
///
/// Output:
/// - `true` if checkupdates is available, `false` otherwise
///
/// Details:
/// - `checkupdates` (from pacman-contrib) refreshes a separate sync DB and lists upgrades without root.
#[cfg(not(target_os = "windows"))]
pub fn has_checkupdates() -> bool {
    use std::process::{Command, Stdio};

    Command::new("checkupdates")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok()
}

/// What: Get the current user's UID by reading /proc/self/status.
///
/// Output:
/// - `Some(u32)` with the UID if successful
/// - `None` if unable to read the UID
///
/// Details:
/// - Reads /proc/self/status and parses the Uid line
/// - Returns the real UID (first value on the Uid line)
#[cfg(not(target_os = "windows"))]
pub fn get_uid() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if line.starts_with("Uid:") {
            // Format: "Uid:\treal\teffective\tsaved\tfs"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse().ok();
            }
        }
    }
    None
}

/// What: Set up a temporary pacman database directory for safe update checks.
///
/// Output:
/// - `Some(PathBuf)` with the temp database path if setup succeeds
/// - `None` if setup fails
///
/// Details:
/// - Creates `/tmp/pacsea-db-{UID}/` directory
/// - Creates a symlink from `local` to `/var/lib/pacman/local`
/// - The symlink allows pacman to know which packages are installed
/// - Directory is kept for reuse across subsequent checks
#[cfg(not(target_os = "windows"))]
pub fn setup_temp_db() -> Option<std::path::PathBuf> {
    // Get current user ID
    let uid = get_uid()?;
    let temp_db = std::path::PathBuf::from(format!("/tmp/pacsea-db-{uid}"));

    // Create directory if needed
    if let Err(e) = std::fs::create_dir_all(&temp_db) {
        tracing::warn!("Failed to create temp database directory: {}", e);
        return None;
    }

    // Create symlink to local database (skip if exists)
    let local_link = temp_db.join("local");
    if !local_link.exists()
        && let Err(e) = std::os::unix::fs::symlink("/var/lib/pacman/local", &local_link)
    {
        tracing::warn!("Failed to create symlink to local database: {}", e);
        return None;
    }

    Some(temp_db)
}

/// What: Sync the temporary pacman database with remote repositories.
///
/// Inputs:
/// - `temp_db`: Path to the temporary database directory
///
/// Output:
/// - `Ok(())` on success
/// - `Err(message)` with trimmed stderr or IO message on failure
///
/// Details:
/// - Uses `fakeroot` to run `pacman -Sy` without root privileges (fallback when `checkupdates` is absent or fails).
/// - Syncs only the temporary database, not the system database
/// - Uses `--logfile /dev/null` to prevent log file creation
/// - May fail on pacman 7+ with Landlock / `alpm` sandbox under `fakeroot`; callers should fall back to stale `pacman -Qu`
#[cfg(not(target_os = "windows"))]
pub fn sync_temp_db(temp_db: &std::path::Path) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let output = Command::new("fakeroot")
        .args(["--", "pacman", "-Sy", "--dbpath"])
        .arg(temp_db)
        .args(["--logfile", "/dev/null"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if !stderr.is_empty() {
                tracing::warn!(
                    "Temp database sync failed (exit code: {:?}): {}",
                    o.status.code(),
                    stderr
                );
            }
            Err(stderr)
        }
        Err(e) => {
            tracing::warn!("Failed to execute fakeroot pacman -Sy: {}", e);
            Err(e.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Verify Landlock-related pacman stderr yields the landlock reason code.
    ///
    /// Inputs:
    /// - Sample stderr line from pacman mentioning Landlock.
    ///
    /// Output:
    /// - Classification includes `REASON_LANDLOCK_SANDBOX_FAILURE`.
    ///
    /// Details:
    /// - Regression guard for Landlock/sandbox stderr classification.
    #[test]
    fn classify_stderr_detects_landlock() {
        let msg = "Fehler: restricting filesystem access failed because the Landlock ruleset could not be applied";
        let v = classify_pacman_stderr_for_update_check(msg);
        assert!(v.iter().any(|s| s == REASON_LANDLOCK_SANDBOX_FAILURE));
    }

    /// What: Verify alpm sandbox user failure text yields the alpm reason code.
    ///
    /// Inputs:
    /// - Sample stderr mentioning the `alpm` sandbox user switch failure.
    ///
    /// Output:
    /// - Classification includes `REASON_ALPM_SANDBOX_FAILURE`.
    ///
    /// Details:
    /// - Matches both German and English error prefixes indirectly via key tokens.
    #[test]
    fn classify_stderr_detects_alpm_sandbox() {
        let msg = "Fehler: switching to sandbox user 'alpm' failed!";
        let v = classify_pacman_stderr_for_update_check(msg);
        assert!(v.iter().any(|s| s == REASON_ALPM_SANDBOX_FAILURE));
    }

    /// What: Verify German permission-denied text is classified.
    ///
    /// Inputs:
    /// - Localized permission-denied phrase.
    ///
    /// Output:
    /// - Classification includes `REASON_PERMISSION_DENIED`.
    ///
    /// Details:
    /// - Ensures STDERR triage is not English-only.
    #[test]
    fn classify_stderr_detects_german_permission_denied() {
        let msg = "Die Operation ist nicht erlaubt";
        let v = classify_pacman_stderr_for_update_check(msg);
        assert!(v.iter().any(|s| s == REASON_PERMISSION_DENIED));
    }
}
