//! Pre-transaction guardrails: pacman db-lock detection, disk-space checks, and
//! sync-database freshness checks with actionable guidance.
//!
//! These checks run before install/remove/update transactions (CLI and TUI) and are
//! read-only: they never mutate the system and are therefore safe in dry-run mode.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// The kind of transaction a guardrail check runs for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardrailOperation {
    /// Packages will be installed (downloads into the pacman cache).
    Install,
    /// Packages will be removed (no downloads).
    Remove,
    /// Full system update (downloads into the pacman cache).
    Update,
}

/// A single guardrail finding carrying the data needed to render an actionable message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailIssue {
    /// The pacman database lock file exists.
    DbLocked {
        /// Path of the lock file (usually `/var/lib/pacman/db.lck`).
        lock_path: PathBuf,
        /// Whether a package manager process (pacman/paru/yay) appears to be running.
        pacman_running: bool,
    },
    /// Free space on the filesystem holding `path` is below the configured minimum.
    LowDiskSpace {
        /// Directory whose filesystem was checked (usually the pacman package cache).
        path: PathBuf,
        /// Free space in MiB.
        available_mib: u64,
        /// Configured minimum in MiB.
        min_free_mib: u64,
    },
    /// The pacman sync databases have not been refreshed for `age_days` days.
    SyncDbStale {
        /// Age of the newest sync database in whole days.
        age_days: u64,
    },
}

/// Filesystem locations and thresholds consulted by the guardrail checks.
///
/// Details: Injectable so tests can point the checks at temporary directories.
pub struct GuardrailContext {
    /// Path of the pacman database lock file.
    pub db_lock_path: PathBuf,
    /// Pacman package cache directory (download target for installs/updates).
    pub pacman_cache_dir: PathBuf,
    /// Directory holding the pacman sync databases (`*.db`).
    pub sync_db_dir: PathBuf,
    /// Root of the `/proc` filesystem used to detect running package managers.
    pub proc_dir: PathBuf,
    /// Minimum free space (MiB) below which a warning is raised.
    pub min_free_mib: u64,
    /// Age in days after which the sync databases are considered stale.
    pub stale_sync_days: u64,
}

impl Default for GuardrailContext {
    /// What: Build the standard Arch Linux locations with default thresholds.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - Context pointing at `/var/lib/pacman`, `/var/cache/pacman/pkg`, and `/proc`,
    ///   with a 1 GiB free-space minimum and a 14-day staleness window.
    ///
    /// Details:
    /// - Paths intentionally match pacman defaults; systems with a relocated `DBPath`
    ///   can construct the context manually.
    fn default() -> Self {
        Self {
            db_lock_path: PathBuf::from("/var/lib/pacman/db.lck"),
            pacman_cache_dir: PathBuf::from("/var/cache/pacman/pkg"),
            sync_db_dir: PathBuf::from("/var/lib/pacman/sync"),
            proc_dir: PathBuf::from("/proc"),
            min_free_mib: 1024,
            stale_sync_days: 14,
        }
    }
}

/// What: Determine whether a package manager process appears to be running.
///
/// Inputs:
/// - `proc_dir`: Root of the `/proc` filesystem (injectable for tests).
///
/// Output:
/// - `true` when a process named pacman/paru/yay (or pacman-key) is found.
///
/// Details:
/// - Scans numeric `/proc/<pid>/comm` entries; unreadable entries are skipped.
/// - Returns `false` on platforms or systems without a proc filesystem.
fn package_manager_running(proc_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(proc_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        let comm = comm.trim();
        if matches!(comm, "pacman" | "paru" | "yay" | "pacman-key") {
            return true;
        }
    }
    false
}

/// What: Check whether the pacman database is locked.
///
/// Inputs:
/// - `ctx`: Guardrail context providing the lock path and proc dir.
///
/// Output:
/// - `Some(GuardrailIssue::DbLocked)` when the lock file exists; `None` otherwise.
///
/// Details:
/// - Also reports whether a package manager process is running so callers can
///   distinguish "wait for it to finish" from "remove the stale lock".
#[must_use]
pub fn check_db_lock(ctx: &GuardrailContext) -> Option<GuardrailIssue> {
    if !ctx.db_lock_path.exists() {
        return None;
    }
    Some(GuardrailIssue::DbLocked {
        lock_path: ctx.db_lock_path.clone(),
        pacman_running: package_manager_running(&ctx.proc_dir),
    })
}

/// What: Report the free space in MiB on the filesystem containing `path`.
///
/// Inputs:
/// - `path`: Directory to inspect (must exist).
///
/// Output:
/// - `Some(free MiB)` on success; `None` when the query fails or is unsupported.
///
/// Details:
/// - Uses `statvfs` (unprivileged block counts) on Unix; no-op elsewhere.
#[cfg(unix)]
fn available_mib(path: &Path) -> Option<u64> {
    let stat = nix::sys::statvfs::statvfs(path).ok()?;
    // fsblkcnt_t/c_ulong are u32 on some 32-bit targets, so the conversion is not
    // always redundant even though it is on 64-bit Linux.
    #[allow(clippy::useless_conversion)]
    let bytes = u64::from(stat.blocks_available()) * u64::from(stat.fragment_size());
    Some(bytes / (1024 * 1024))
}

/// What: Report the free space in MiB on the filesystem containing `path`.
///
/// Inputs:
/// - `path`: Directory to inspect (unused).
///
/// Output:
/// - Always `None`; disk-space checks are unsupported on this platform.
///
/// Details:
/// - Non-Unix fallback keeping callers platform-agnostic.
#[cfg(not(unix))]
fn available_mib(_path: &Path) -> Option<u64> {
    None
}

/// What: Check whether the pacman cache filesystem has enough free space.
///
/// Inputs:
/// - `ctx`: Guardrail context providing cache dir and minimum threshold.
/// - `op`: Transaction kind; removals skip the check (they free space).
///
/// Output:
/// - `Some(GuardrailIssue::LowDiskSpace)` when free space is below the minimum.
///
/// Details:
/// - Falls back to the filesystem root when the cache directory is missing.
#[must_use]
pub fn check_disk_space(ctx: &GuardrailContext, op: GuardrailOperation) -> Option<GuardrailIssue> {
    if op == GuardrailOperation::Remove {
        return None;
    }
    let path: &Path = if ctx.pacman_cache_dir.exists() {
        &ctx.pacman_cache_dir
    } else {
        Path::new("/")
    };
    let available = available_mib(path)?;
    if available >= ctx.min_free_mib {
        return None;
    }
    Some(GuardrailIssue::LowDiskSpace {
        path: path.to_path_buf(),
        available_mib: available,
        min_free_mib: ctx.min_free_mib,
    })
}

/// What: Check whether the pacman sync databases are stale.
///
/// Inputs:
/// - `ctx`: Guardrail context providing the sync directory and staleness window.
/// - `now`: Reference time (injectable for tests).
///
/// Output:
/// - `Some(GuardrailIssue::SyncDbStale)` when the newest `*.db` is older than the window.
///
/// Details:
/// - Stale databases correlate with mirror/database drift ("partial upgrade" errors,
///   404s during download); the guidance is to refresh before mutating.
/// - Returns `None` when the directory or usable mtimes are unavailable.
#[must_use]
pub fn check_sync_db_freshness(ctx: &GuardrailContext, now: SystemTime) -> Option<GuardrailIssue> {
    let entries = std::fs::read_dir(&ctx.sync_db_dir).ok()?;
    let newest = entries
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("db"))
        })
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()?;
    let age = now.duration_since(newest).ok()?;
    let age_days = age.as_secs() / 86_400;
    if age_days < ctx.stale_sync_days {
        return None;
    }
    Some(GuardrailIssue::SyncDbStale { age_days })
}

/// What: Run all guardrail checks for a transaction.
///
/// Inputs:
/// - `ctx`: Guardrail context (paths and thresholds).
/// - `op`: Transaction kind about to run.
///
/// Output:
/// - All findings, most severe first (db lock, disk space, staleness).
///
/// Details:
/// - Read-only; safe to call in dry-run mode and from both CLI and TUI paths.
#[must_use]
pub fn run_guardrails(ctx: &GuardrailContext, op: GuardrailOperation) -> Vec<GuardrailIssue> {
    let mut issues = Vec::new();
    if let Some(issue) = check_db_lock(ctx) {
        issues.push(issue);
    }
    if let Some(issue) = check_disk_space(ctx, op) {
        issues.push(issue);
    }
    if op != GuardrailOperation::Remove
        && let Some(issue) = check_sync_db_freshness(ctx, SystemTime::now())
    {
        issues.push(issue);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// What: Build a unique temp directory for guardrail tests.
    ///
    /// Inputs:
    /// - `label`: Short label distinguishing the calling test.
    ///
    /// Output:
    /// - Created directory path under the system temp dir.
    ///
    /// Details:
    /// - Combines process id and a nanosecond timestamp for uniqueness.
    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pacsea_guardrails_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    /// What: Build a context rooted in a temp directory with no lock and fresh dbs.
    ///
    /// Inputs:
    /// - `root`: Temp directory to root all paths under.
    ///
    /// Output:
    /// - Context whose paths point below `root`.
    ///
    /// Details:
    /// - `proc_dir` points at an empty directory so no package manager is detected.
    fn context_under(root: &Path) -> GuardrailContext {
        let proc_dir = root.join("proc");
        let sync_dir = root.join("sync");
        let cache_dir = root.join("cache");
        std::fs::create_dir_all(&proc_dir).expect("create proc dir");
        std::fs::create_dir_all(&sync_dir).expect("create sync dir");
        std::fs::create_dir_all(&cache_dir).expect("create cache dir");
        GuardrailContext {
            db_lock_path: root.join("db.lck"),
            pacman_cache_dir: cache_dir,
            sync_db_dir: sync_dir,
            proc_dir,
            min_free_mib: 0,
            stale_sync_days: 14,
        }
    }

    #[test]
    /// What: Verify the db-lock check reports the lock file and stale-vs-running state.
    ///
    /// Inputs:
    /// - Context without a lock, then with a lock file and a fake running pacman.
    ///
    /// Output:
    /// - `None` without the lock; `DbLocked` with correct `pacman_running` afterwards.
    ///
    /// Details:
    /// - Fakes `/proc/<pid>/comm` entries to drive the process detection.
    fn db_lock_check_detects_lock_and_running_process() {
        let root = temp_dir("dblock");
        let ctx = context_under(&root);

        assert!(check_db_lock(&ctx).is_none());

        std::fs::write(&ctx.db_lock_path, "").expect("create lock");
        match check_db_lock(&ctx) {
            Some(GuardrailIssue::DbLocked { pacman_running, .. }) => {
                assert!(!pacman_running, "no fake process created yet");
            }
            other => panic!("expected DbLocked, got {other:?}"),
        }

        let fake_pid = ctx.proc_dir.join("4242");
        std::fs::create_dir_all(&fake_pid).expect("create fake pid dir");
        std::fs::write(fake_pid.join("comm"), "pacman\n").expect("write comm");
        match check_db_lock(&ctx) {
            Some(GuardrailIssue::DbLocked { pacman_running, .. }) => {
                assert!(pacman_running, "fake pacman process should be detected");
            }
            other => panic!("expected DbLocked, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    /// What: Verify disk-space checks respect the operation kind and threshold.
    ///
    /// Inputs:
    /// - Context with a huge threshold (always low) and a zero threshold (never low).
    ///
    /// Output:
    /// - Install reports low space with a huge threshold; Remove never reports;
    ///   zero threshold reports nothing.
    ///
    /// Details:
    /// - Uses the real filesystem via statvfs on the temp directory.
    fn disk_space_check_respects_threshold_and_operation() {
        let root = temp_dir("disk");
        let mut ctx = context_under(&root);

        ctx.min_free_mib = u64::MAX;
        assert!(matches!(
            check_disk_space(&ctx, GuardrailOperation::Install),
            Some(GuardrailIssue::LowDiskSpace { .. })
        ));
        assert!(check_disk_space(&ctx, GuardrailOperation::Remove).is_none());

        ctx.min_free_mib = 0;
        assert!(check_disk_space(&ctx, GuardrailOperation::Update).is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    /// What: Verify sync-db staleness uses the newest `*.db` mtime against the window.
    ///
    /// Inputs:
    /// - A fresh `core.db`, compared against "now" and a time 20 days ahead.
    ///
    /// Output:
    /// - Fresh comparison yields `None`; the 20-day-later comparison yields `SyncDbStale`.
    ///
    /// Details:
    /// - Advances the reference time instead of back-dating the file to stay portable.
    fn sync_db_freshness_reports_stale_databases() {
        let root = temp_dir("sync");
        let ctx = context_under(&root);

        // No databases at all: cannot judge, no finding.
        assert!(check_sync_db_freshness(&ctx, SystemTime::now()).is_none());

        std::fs::write(ctx.sync_db_dir.join("core.db"), "x").expect("write db");
        assert!(check_sync_db_freshness(&ctx, SystemTime::now()).is_none());

        let later = SystemTime::now() + Duration::from_hours(20 * 24);
        match check_sync_db_freshness(&ctx, later) {
            Some(GuardrailIssue::SyncDbStale { age_days }) => {
                assert!(
                    age_days >= 19,
                    "age should be about 20 days, got {age_days}"
                );
            }
            other => panic!("expected SyncDbStale, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    /// What: Verify `run_guardrails` aggregates findings in severity order.
    ///
    /// Inputs:
    /// - Context with a lock file present and an impossible free-space threshold.
    ///
    /// Output:
    /// - Db lock first, then low disk space.
    ///
    /// Details:
    /// - Sync staleness is absent because the databases are fresh.
    fn run_guardrails_aggregates_findings() {
        let root = temp_dir("aggregate");
        let mut ctx = context_under(&root);
        std::fs::write(&ctx.db_lock_path, "").expect("create lock");
        std::fs::write(ctx.sync_db_dir.join("core.db"), "x").expect("write db");
        ctx.min_free_mib = u64::MAX;

        let issues = run_guardrails(&ctx, GuardrailOperation::Install);
        assert_eq!(issues.len(), 2);
        assert!(matches!(issues[0], GuardrailIssue::DbLocked { .. }));
        assert!(matches!(issues[1], GuardrailIssue::LowDiskSpace { .. }));

        let _ = std::fs::remove_dir_all(&root);
    }
}
