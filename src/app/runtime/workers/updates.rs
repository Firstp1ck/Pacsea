use std::sync::OnceLock;
use tokio::sync::mpsc;

use crate::app::runtime::workers::updates_helpers::{
    check_aur_helper, has_checkupdates, has_fakeroot, setup_temp_db, sync_temp_db,
};
use crate::app::runtime::workers::updates_parsing::{
    get_installed_version, parse_checkupdates, parse_checkupdates_tool, parse_qua,
};

/// What: Process pacman -Qu or checkupdates output and add packages to collections.
///
/// Inputs:
/// - `output`: Command output result
/// - `is_checkupdates_tool`: `true` if output is from checkupdates tool, `false` if from pacman -Qu
/// - `packages_map`: Mutable `HashMap` to store formatted package strings
/// - `packages_set`: Mutable `HashSet` to track unique package names
fn process_checkupdates_output(
    output: Result<std::process::Output, std::io::Error>,
    is_checkupdates_tool: bool,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) {
    match output {
        Ok(output) => {
            let exit_code = output.status.code();
            if output.status.success() {
                if is_checkupdates_tool {
                    // Parse checkupdates output (package-name version format)
                    let packages = parse_checkupdates_tool(&output.stdout);
                    let count = packages.len();

                    for (name, new_version) in packages {
                        // Get old version from installed packages
                        let old_version =
                            get_installed_version(&name).unwrap_or_else(|| "unknown".to_string());
                        // Format: "name - old_version -> name - new_version"
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
                    // Parse pacman -Qu output (package-name old_version -> new_version format)
                    let packages = parse_checkupdates(&output.stdout);
                    let count = packages.len();

                    for (name, old_version, new_version) in packages {
                        // Format: "name - old_version -> name - new_version"
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
            } else if output.status.code() == Some(1) {
                // Exit code 1 is normal (no updates)
                if is_checkupdates_tool {
                    tracing::debug!(
                        "checkupdates returned exit code 1 (no updates available in official repos)"
                    );
                } else {
                    tracing::debug!(
                        "pacman -Qu returned exit code 1 (no updates available in official repos)"
                    );
                }
            } else {
                // Other exit codes are errors
                let stderr = String::from_utf8_lossy(&output.stderr);
                if is_checkupdates_tool {
                    tracing::warn!(
                        "checkupdates command failed with exit code: {:?}, stderr: {}",
                        exit_code,
                        stderr.trim()
                    );
                } else {
                    tracing::warn!("pacman -Qu command failed with exit code: {:?}", exit_code);
                }
            }
        }
        Err(e) => {
            if is_checkupdates_tool {
                tracing::warn!("Failed to execute checkupdates: {}", e);
            } else {
                tracing::warn!("Failed to execute pacman -Qu: {}", e);
            }
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
                        // Format: "name - old_version -> name - new_version"
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
                    // Exit code 1 is normal (no updates)
                    tracing::debug!(
                        "{} -Qua returned exit code 1 (no updates available in AUR)",
                        helper
                    );
                } else {
                    // Other exit codes are errors
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

/// What: Spawn background worker to check for available package updates.
///
/// Inputs:
/// - `updates_tx`: Channel sender for updates (count, sorted list)
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Uses a temporary database to safely check for updates without modifying the system
/// - Syncs the temp database with `fakeroot pacman -Sy` if fakeroot is available
/// - Falls back to `pacman -Qu` (stale local DB) if fakeroot is not available
/// - Executes `yay -Qua` or `paru -Qua` for AUR updates
/// - Removes duplicates using `HashSet`
/// - Sorts package names alphabetically
/// - Saves list to `~/.config/pacsea/lists/available_updates.txt`
/// - Sends `(count, sorted_list)` via channel
/// - Uses synchronization to prevent concurrent update checks and file writes
#[allow(clippy::too_many_lines)] // Complex function handling multiple update check methods (function has 204 lines)
pub fn spawn_updates_worker(updates_tx: mpsc::UnboundedSender<(usize, Vec<String>)>) {
    let updates_tx_once = updates_tx;

    tokio::spawn(async move {
        // Get mutex reference inside async block
        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));

        // Check if update check is already in progress
        let mut in_progress = mutex.lock().await;
        if *in_progress {
            tracing::debug!("Update check already in progress, skipping concurrent call");
            return;
        }

        // Set flag to indicate update check is in progress
        *in_progress = true;
        drop(in_progress); // Release lock before blocking operation

        let result = tokio::task::spawn_blocking(move || {
            use std::collections::HashSet;
            use std::process::{Command, Stdio};

            tracing::debug!("Starting update check");

            let (has_paru, has_yay, helper) = check_aur_helper();

            // Try safe update check with temp database (non-Windows only)
            #[cfg(not(target_os = "windows"))]
            let (temp_db_path, use_checkupdates_tool) = {
                let db_result = if has_fakeroot() {
                    tracing::debug!("fakeroot is available, setting up temp database");
                    setup_temp_db().and_then(|temp_db| {
                        tracing::debug!("Syncing temporary database at {:?}", temp_db);
                        if sync_temp_db(&temp_db) {
                            tracing::debug!("Temp database sync successful");
                            Some(temp_db)
                        } else {
                            tracing::warn!("Temp database sync failed");
                            None
                        }
                    })
                } else {
                    tracing::debug!("fakeroot not available");
                    None
                };

                // If temp database sync failed, try checkupdates as fallback
                if db_result.is_none() && has_checkupdates() {
                    tracing::info!("Temp database sync failed, trying checkupdates as fallback");
                    (None, true)
                } else if db_result.is_none() {
                    tracing::warn!("Temp database sync failed and checkupdates not available, falling back to pacman -Qu (may show stale results)");
                    (None, false)
                } else {
                    (db_result, false)
                }
            };

            // Execute update check command
            #[cfg(not(target_os = "windows"))]
            let (output_checkupdates, is_checkupdates_tool) = if use_checkupdates_tool {
                tracing::info!("Executing: checkupdates (automatically syncs database)");
                (
                    Command::new("checkupdates")
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output(),
                    true,
                )
            } else if let Some(db_path) = temp_db_path.as_ref() {
                tracing::debug!(
                    "Executing: pacman -Qu --dbpath {:?} (using synced temp database)",
                    db_path
                );
                (
                    Command::new("pacman")
                        .args(["-Qu", "--dbpath"])
                        .arg(db_path)
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            } else {
                tracing::debug!("Executing: pacman -Qu (using system database - may be stale)");
                (
                    Command::new("pacman")
                        .args(["-Qu"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            };

            #[cfg(target_os = "windows")]
            let (output_checkupdates, is_checkupdates_tool) = {
                tracing::debug!("Executing: pacman -Qu (Windows fallback)");
                (
                    Command::new("pacman")
                        .args(["-Qu"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .output(),
                    false,
                )
            };

            // Execute -Qua command (AUR) - only if helper is available
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

            // Collect packages from both commands
            // Use HashMap to store: package_name -> formatted_string
            // Use HashSet to track unique package names for deduplication
            let mut packages_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut packages_set = HashSet::new();

            // Parse pacman -Qu or checkupdates output (official repos)
            #[cfg(target_os = "windows")]
            let is_checkupdates_tool = false;
            process_checkupdates_output(
                output_checkupdates,
                is_checkupdates_tool,
                &mut packages_map,
                &mut packages_set,
            );

            // Parse -Qua output (AUR)
            process_qua_output(output_qua, helper, &mut packages_map, &mut packages_set);

            // Convert to Vec of formatted strings, sorted by package name
            let mut package_names: Vec<String> = packages_set.into_iter().collect();
            package_names.sort_unstable();

            let packages: Vec<String> = package_names
                .iter()
                .filter_map(|name| packages_map.get(name).cloned())
                .collect();

            let count = packages.len();
            tracing::info!(
                "Update check completed: found {} total available updates (after deduplication)",
                count
            );

            // Save to file
            let lists_dir = crate::theme::lists_dir();
            let updates_file = lists_dir.join("available_updates.txt");
            if let Err(e) = std::fs::write(&updates_file, packages.join("\n")) {
                tracing::warn!("Failed to save updates list to file: {}", e);
            } else {
                tracing::debug!("Saved updates list to {:?}", updates_file);
            }

            // Return count and package names (for display) - not the formatted strings
            (count, package_names)
        })
        .await;

        // Reset flag when done (even on error)
        let mutex = UPDATE_CHECK_IN_PROGRESS.get_or_init(|| tokio::sync::Mutex::new(false));
        let mut in_progress = mutex.lock().await;
        *in_progress = false;
        drop(in_progress);

        match result {
            Ok((count, list)) => {
                let _ = updates_tx_once.send((count, list));
            }
            Err(e) => {
                tracing::error!("Updates worker task panicked: {:?}", e);
                let _ = updates_tx_once.send((0, Vec::new()));
            }
        }
    });
}

/// What: Spawns periodic updates worker that checks for package updates at intervals.
///
/// Inputs:
/// - `updates_tx`: Channel sender for package updates
/// - `updates_refresh_interval`: Refresh interval in seconds
///
/// Output:
/// - None (spawns async task)
///
/// Details:
/// - Checks for updates once at startup
/// - Periodically refreshes updates list at configured interval
pub fn spawn_periodic_updates_worker(
    updates_tx: &mpsc::UnboundedSender<(usize, Vec<String>)>,
    updates_refresh_interval: u64,
) {
    spawn_updates_worker(updates_tx.clone());

    let updates_tx_periodic = updates_tx.clone();
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(updates_refresh_interval));
        // Skip the first tick to avoid immediate refresh after startup
        interval.tick().await;
        loop {
            interval.tick().await;
            spawn_updates_worker(updates_tx_periodic.clone());
        }
    });
}
