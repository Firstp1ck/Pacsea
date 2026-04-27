use std::sync::OnceLock;
use tokio::sync::mpsc;

#[cfg(target_os = "windows")]
use crate::app::runtime::workers::updates_helpers::UpdateCheckPayload;
use crate::app::runtime::workers::updates_helpers::check_aur_helper;
#[cfg(not(target_os = "windows"))]
use crate::app::runtime::workers::updates_helpers::{
    REASON_CHECKUPDATES_FAILED, REASON_CHECKUPDATES_UNAVAILABLE, REASON_FAKEROOT_UNAVAILABLE,
    REASON_STALE_DB_FALLBACK, REASON_TEMP_DB_SYNC_FAILED, UpdateCheckPayload,
    classify_pacman_stderr_for_update_check, has_checkupdates, has_fakeroot, setup_temp_db,
    sync_temp_db,
};
use crate::app::runtime::workers::updates_parsing::{
    get_installed_version, parse_checkupdates, parse_checkupdates_tool, parse_qua,
};

/// What: Merge official-repo command output into package maps when exit status is authoritative.
///
/// Inputs:
/// - `output`: `std::process::Output` result from pacman or checkupdates.
/// - `is_checkupdates_tool`: `true` for `checkupdates` line format; `false` for `pacman -Qu`.
/// - `packages_map` / `packages_set`: Accumulators.
///
/// Output:
/// - `true` if exit code was 0 or 1 (success or no updates).
/// - `false` on IO error or unexpected exit code.
///
/// Details:
/// - Exit code 1 means no upgrades for both pacman and checkupdates in this app’s convention.
fn ingest_official_repo_output(
    output: Result<std::process::Output, std::io::Error>,
    is_checkupdates_tool: bool,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) -> bool {
    match output {
        Ok(output) => {
            let exit_code = output.status.code();
            if output.status.success() {
                if is_checkupdates_tool {
                    let packages = parse_checkupdates_tool(&output.stdout);
                    let count = packages.len();
                    for (name, new_version) in packages {
                        let old_version =
                            get_installed_version(&name).unwrap_or_else(|| "unknown".to_string());
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }
                    tracing::debug!(
                        "checkupdates completed successfully (exit code: {:?}): found {} packages from official repos",
                        exit_code,
                        count
                    );
                } else {
                    let packages = parse_checkupdates(&output.stdout);
                    let count = packages.len();
                    for (name, old_version, new_version) in packages {
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }
                    tracing::debug!(
                        "pacman -Qu completed successfully (exit code: {:?}): found {} packages from official repos",
                        exit_code,
                        count
                    );
                }
                true
            } else if output.status.code() == Some(1) {
                if is_checkupdates_tool {
                    tracing::debug!(
                        "checkupdates returned exit code 1 (no updates available in official repos)"
                    );
                } else {
                    tracing::debug!(
                        "pacman -Qu returned exit code 1 (no updates available in official repos)"
                    );
                }
                true
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if is_checkupdates_tool {
                    tracing::warn!(
                        "checkupdates command failed with exit code: {:?}, stderr: {}",
                        exit_code,
                        stderr.trim()
                    );
                } else {
                    tracing::warn!(
                        "pacman -Qu command failed with exit code: {:?}, stderr: {}",
                        exit_code,
                        stderr.trim()
                    );
                }
                false
            }
        }
        Err(e) => {
            if is_checkupdates_tool {
                tracing::warn!("Failed to execute checkupdates: {}", e);
            } else {
                tracing::warn!("Failed to execute pacman -Qu: {}", e);
            }
            false
        }
    }
}

/// What: Process -Qua output and add packages to collections.
///
/// Inputs:
/// - `result`: Command output result
/// - `helper`: Helper name for logging
/// - `packages_map`: Mutable `HashMap` to store formatted package strings
/// - `packages_set`: Mutable `HashSet` to track unique package names
fn process_qua_output(
    result: Option<Result<std::process::Output, std::io::Error>>,
    helper: &str,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) {
    if let Some(result) = result {
        match result {
            Ok(output) => {
                let exit_code = output.status.code();
                if output.status.success() {
                    let packages = parse_qua(&output.stdout);
                    let count = packages.len();
                    let before_count = packages_set.len();

                    for (name, old_version, new_version) in packages {
                        let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                        packages_map.insert(name.clone(), formatted);
                        packages_set.insert(name);
                    }

                    let after_count = packages_set.len();
                    tracing::debug!(
                        "{} -Qua completed successfully (exit code: {:?}): found {} packages from AUR, {} total ({} new)",
                        helper,
                        exit_code,
                        count,
                        after_count,
                        after_count - before_count
                    );
                } else if output.status.code() == Some(1) {
                    tracing::debug!(
                        "{} -Qua returned exit code 1 (no updates available in AUR)",
                        helper
                    );
                } else {
                    tracing::warn!(
                        "{} -Qua command failed with exit code: {:?}",
                        helper,
                        exit_code
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to execute {} -Qua: {}", helper, e);
            }
        }
    } else {
        tracing::debug!("No AUR helper available, skipping AUR updates check");
    }
}

/// Static mutex to prevent concurrent update checks.
///
/// What: Tracks whether an update check is currently in progress.
///
/// Details:
/// - Uses `OnceLock` for lazy initialization
/// - Uses `tokio::sync::Mutex` for async-safe synchronization
/// - Prevents overlapping file writes to `available_updates.txt`
static UPDATE_CHECK_IN_PROGRESS: OnceLock<tokio::sync::Mutex<bool>> = OnceLock::new();

/// What: Attempt authoritative official-repo listing via fakeroot temp-db sync + `pacman -Qu`.
///
/// Inputs:
/// - `temp_db_path`: Prepared temp DB dir from [`setup_temp_db`].
/// - `packages_map` / `packages_set`: Accumulators.
/// - `reason_codes`: Diagnostic codes appended on failure paths.
///
/// Output:
/// - `Some("temp_db_pacman_qu")` when the probe is authoritative; `None` otherwise.
///
/// Details:
/// - Fallback after `checkupdates` when that tool is missing **or** when it was run but did not
///   yield an authoritative listing (mirrors, parse errors, non-zero exit, etc.).
/// - No-ops when `fakeroot` or temp DB path is unavailable without adding sync-failed reasons beyond
///   `REASON_FAKEROOT_UNAVAILABLE` when `fakeroot` is missing.
#[cfg(not(target_os = "windows"))]
fn unix_try_authoritative_fakeroot(
    temp_db_path: Option<&std::path::PathBuf>,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
    reason_codes: &mut Vec<String>,
) -> Option<&'static str> {
    use std::process::{Command, Stdio};

    if !has_fakeroot() {
        reason_codes.push(REASON_FAKEROOT_UNAVAILABLE.to_string());
        tracing::debug!("fakeroot not available; skipping temp-database sync");
        return None;
    }
    let db = temp_db_path?;
    match sync_temp_db(db) {
        Ok(()) => {
            tracing::debug!(
                "Executing: pacman -Qu --dbpath {:?} (using synced temp database)",
                db
            );
            let output = Command::new("pacman")
                .args(["-Qu", "--dbpath"])
                .arg(db)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            if ingest_official_repo_output(output, false, packages_map, packages_set) {
                Some("temp_db_pacman_qu")
            } else {
                None
            }
        }
        Err(stderr) => {
            tracing::warn!("Temp database sync failed");
            reason_codes.push(REASON_TEMP_DB_SYNC_FAILED.to_string());
            reason_codes.extend(classify_pacman_stderr_for_update_check(&stderr));
            None
        }
    }
}

/// What: Attempt authoritative official-repo listing via `checkupdates` with `CHECKUPDATES_DB`.
///
/// Inputs:
/// - `temp_db_path`: Existing temp DB dir if any.
/// - `packages_map` / `packages_set`: Accumulators.
/// - `reason_codes`: Diagnostic codes (e.g. `checkupdates_failed`).
///
/// Output:
/// - `Some("checkupdates_db")` on success; `None` when skipped or command failed.
///
/// Details:
/// - Preferred when `pacman-contrib` is installed; if absent, callers try [`unix_try_authoritative_fakeroot`].
/// - Call only when [`has_checkupdates`] is true (caller probes once per update check).
#[cfg(not(target_os = "windows"))]
fn unix_try_authoritative_checkupdates(
    temp_db_path: Option<&std::path::PathBuf>,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
    reason_codes: &mut Vec<String>,
) -> Option<&'static str> {
    use std::process::{Command, Stdio};

    let db_for_check = temp_db_path.cloned().or_else(setup_temp_db)?;
    tracing::info!(
        "Executing: checkupdates with CHECKUPDATES_DB={:?} (isolated sync db)",
        db_for_check
    );
    let output = Command::new("checkupdates")
        .env("CHECKUPDATES_DB", db_for_check.as_os_str())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    match output {
        Ok(o) => {
            let stderr_text = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if ingest_official_repo_output(Ok(o), true, packages_map, packages_set) {
                Some("checkupdates_db")
            } else {
                reason_codes.push(REASON_CHECKUPDATES_FAILED.to_string());
                reason_codes.extend(classify_pacman_stderr_for_update_check(&stderr_text));
                None
            }
        }
        Err(e) => {
            tracing::warn!("Failed to execute checkupdates: {}", e);
            reason_codes.push(REASON_CHECKUPDATES_FAILED.to_string());
            None
        }
    }
}

/// What: Apply non-authoritative system `pacman -Qu` as last-resort official listing.
///
/// Inputs:
/// - `packages_map` / `packages_set`: Accumulators.
/// - `reason_codes`: Appends `REASON_STALE_DB_FALLBACK`.
///
/// Output:
/// - Always `"stale_pacman_qu"` for strategy labeling.
///
/// Details:
/// - May reflect a stale sync database; never marks the overall probe as authoritative.
#[cfg(not(target_os = "windows"))]
fn unix_apply_stale_official_pacman_qu(
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
    reason_codes: &mut Vec<String>,
) -> &'static str {
    use std::process::{Command, Stdio};

    reason_codes.push(REASON_STALE_DB_FALLBACK.to_string());
    tracing::warn!("Falling back to pacman -Qu (system database may be stale)");
    let output = Command::new("pacman")
        .args(["-Qu"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();
    let _ = ingest_official_repo_output(output, false, packages_map, packages_set);
    "stale_pacman_qu"
}

/// What: Run the blocking portion of a package update check (Unix).
///
/// Output:
/// - [`UpdateCheckPayload`] with deduplicated package names, file body saved under config.
#[cfg(not(target_os = "windows"))]
fn run_update_check_blocking_unix() -> UpdateCheckPayload {
    use std::collections::{HashMap, HashSet};
    use std::process::{Command, Stdio};

    let mut reason_codes: Vec<String> = Vec::new();
    let mut authoritative_official = false;
    let mut official_strategy: &'static str = "none";

    let (has_paru, has_yay, helper) = check_aur_helper();

    let mut packages_map: HashMap<String, String> = HashMap::new();
    let mut packages_set: HashSet<String> = HashSet::new();

    let temp_db_path = setup_temp_db();
    let have_checkupdates = has_checkupdates();

    if have_checkupdates
        && let Some(s) = unix_try_authoritative_checkupdates(
            temp_db_path.as_ref(),
            &mut packages_map,
            &mut packages_set,
            &mut reason_codes,
        )
    {
        authoritative_official = true;
        official_strategy = s;
    }

    if !authoritative_official
        && let Some(s) = unix_try_authoritative_fakeroot(
            temp_db_path.as_ref(),
            &mut packages_map,
            &mut packages_set,
            &mut reason_codes,
        )
    {
        authoritative_official = true;
        official_strategy = s;
    }

    if !authoritative_official {
        if !have_checkupdates {
            reason_codes.push(REASON_CHECKUPDATES_UNAVAILABLE.to_string());
        }
        official_strategy = unix_apply_stale_official_pacman_qu(
            &mut packages_map,
            &mut packages_set,
            &mut reason_codes,
        );
    }

    let output_qua = if has_paru {
        tracing::debug!("Executing: paru -Qua (AUR updates)");
        Some(
            Command::new("paru")
                .args(["-Qua"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output(),
        )
    } else if has_yay {
        tracing::debug!("Executing: yay -Qua (AUR updates)");
        Some(
            Command::new("yay")
                .args(["-Qua"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output(),
        )
    } else {
        tracing::debug!("No AUR helper available (paru/yay), skipping AUR updates check");
        None
    };

    process_qua_output(output_qua, helper, &mut packages_map, &mut packages_set);

    let mut package_names: Vec<String> = packages_set.into_iter().collect();
    package_names.sort_unstable();

    let packages: Vec<String> = package_names
        .iter()
        .filter_map(|name| packages_map.get(name).cloned())
        .collect();

    let count = packages.len();

    tracing::info!(
        target: "pacsea::update_check",
        authoritative = authoritative_official,
        official_strategy = official_strategy,
        reasons = %reason_codes.join(","),
        package_count = count,
        "update_check_summary"
    );

    tracing::info!(
        "Update check completed: found {} total available updates (after deduplication)",
        count
    );

    let lists_dir = crate::theme::lists_dir();
    let updates_file = lists_dir.join("available_updates.txt");
    if let Err(e) = std::fs::write(&updates_file, packages.join("\n")) {
        tracing::warn!("Failed to save updates list to file: {}", e);
    } else {
        tracing::debug!("Saved updates list to {:?}", updates_file);
    }

    UpdateCheckPayload {
        count,
        package_names,
        authoritative: authoritative_official,
        reason_codes,
        official_strategy,
    }
}

/// What: Run the blocking portion of a package update check (Windows / non-Arch).
#[cfg(target_os = "windows")]
fn run_update_check_blocking_windows() -> UpdateCheckPayload {
    use std::collections::{HashMap, HashSet};
    use std::process::{Command, Stdio};

    let (has_paru, has_yay, helper) = check_aur_helper();
    let mut packages_map: HashMap<String, String> = HashMap::new();
    let mut packages_set: HashSet<String> = HashSet::new();

    tracing::debug!("Executing: pacman -Qu (Windows fallback)");
    let output = Command::new("pacman")
        .args(["-Qu"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let authoritative =
        ingest_official_repo_output(output, false, &mut packages_map, &mut packages_set);

    let output_qua = if has_paru {
        Some(
            Command::new("paru")
                .args(["-Qua"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output(),
        )
    } else if has_yay {
        Some(
            Command::new("yay")
                .args(["-Qua"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output(),
        )
    } else {
        None
    };
    process_qua_output(output_qua, helper, &mut packages_map, &mut packages_set);

    let mut package_names: Vec<String> = packages_set.into_iter().collect();
    package_names.sort_unstable();
    let packages: Vec<String> = package_names
        .iter()
        .filter_map(|name| packages_map.get(name).cloned())
        .collect();
    let count = packages.len();

    tracing::info!(
        target: "pacsea::update_check",
        authoritative = authoritative,
        official_strategy = "windows_pacman_qu",
        reasons = "",
        package_count = count,
        "update_check_summary"
    );

    let lists_dir = crate::theme::lists_dir();
    let updates_file = lists_dir.join("available_updates.txt");
    if let Err(e) = std::fs::write(&updates_file, packages.join("\n")) {
        tracing::warn!("Failed to save updates list to file: {}", e);
    }

    UpdateCheckPayload {
        count,
        package_names,
        authoritative,
        reason_codes: Vec::new(),
        official_strategy: "windows_pacman_qu",
    }
}

/// What: Spawn background worker to check for available package updates.
///
/// Inputs:
/// - `updates_tx`: Channel sender for [`UpdateCheckPayload`]
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Linux: temp db + `pacman -Qu --dbpath`, then `checkupdates` with `CHECKUPDATES_DB`, then stale `pacman -Qu`
/// - Executes `paru -Qua` / `yay -Qua` for AUR
/// - Saves list to `~/.config/pacsea/lists/available_updates.txt`
#[allow(clippy::too_many_lines)]
pub fn spawn_updates_worker(updates_tx: mpsc::UnboundedSender<UpdateCheckPayload>) {
    let updates_tx_once = updates_tx;

    tokio::spawn(async move {
        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));

        let mut in_progress = mutex.lock().await;
        if *in_progress {
            tracing::debug!("Update check already in progress, skipping concurrent call");
            return;
        }

        *in_progress = true;
        drop(in_progress);

        let result = tokio::task::spawn_blocking(move || {
            tracing::debug!("Starting update check");
            #[cfg(not(target_os = "windows"))]
            {
                run_update_check_blocking_unix()
            }
            #[cfg(target_os = "windows")]
            {
                run_update_check_blocking_windows()
            }
        })
        .await;

        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));
        let mut in_progress = mutex.lock().await;
        *in_progress = false;
        drop(in_progress);

        match result {
            Ok(payload) => {
                let _ = updates_tx_once.send(payload);
            }
            Err(e) => {
                tracing::error!("Updates worker task panicked: {:?}", e);
                let _ = updates_tx_once.send(UpdateCheckPayload::worker_panic());
            }
        }
    });
}

/// What: Spawns periodic updates worker that checks for package updates at intervals.
///
/// Inputs:
/// - `updates_tx`: Channel sender for package updates
/// - `updates_refresh_interval`: Refresh interval in seconds
pub fn spawn_periodic_updates_worker(
    updates_tx: &mpsc::UnboundedSender<UpdateCheckPayload>,
    updates_refresh_interval: u64,
) {
    spawn_updates_worker(updates_tx.clone());

    let updates_tx_periodic = updates_tx.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(updates_refresh_interval));
        interval.tick().await;
        loop {
            interval.tick().await;
            spawn_updates_worker(updates_tx_periodic.clone());
        }
    });
}
