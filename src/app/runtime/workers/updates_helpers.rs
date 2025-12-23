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

/// What: Check if fakeroot is available on the system.
///
/// Output:
/// - `true` if fakeroot is available, `false` otherwise
///
/// Details:
/// - Fakeroot is required to sync a temporary pacman database without root
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
/// - checkupdates (from pacman-contrib) can check for updates without root
/// - It automatically syncs the database and doesn't require fakeroot
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
/// - `true` if sync succeeds, `false` otherwise
///
/// Details:
/// - Uses fakeroot to run `pacman -Sy` without root privileges
/// - Syncs only the temporary database, not the system database
/// - Uses `--logfile /dev/null` to prevent log file creation
/// - Logs stderr on failure to help diagnose sync issues
#[cfg(not(target_os = "windows"))]
pub fn sync_temp_db(temp_db: &std::path::Path) -> bool {
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
        Ok(o) if o.status.success() => true,
        Ok(o) => {
            // Log stderr to help diagnose sync failures
            let stderr = String::from_utf8_lossy(&o.stderr);
            if !stderr.trim().is_empty() {
                tracing::warn!(
                    "Temp database sync failed (exit code: {:?}): {}",
                    o.status.code(),
                    stderr.trim()
                );
            }
            false
        }
        Err(e) => {
            tracing::warn!("Failed to execute fakeroot pacman -Sy: {}", e);
            false
        }
    }
}
