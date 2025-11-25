use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use crossterm::event::Event as CEvent;
use tokio::{
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::sources;
use crate::state::{ArchStatusColor, NewsItem};

/// What: Spawn background workers for status, news, and tick events.
///
/// Inputs:
/// - `headless`: When `true`, skip terminal-dependent operations
/// - `status_tx`: Channel sender for Arch status updates
/// - `news_tx`: Channel sender for Arch news updates
/// - `tick_tx`: Channel sender for tick events
/// - `news_read_urls`: Set of already-read news URLs
/// - `official_index_path`: Path to official package index
/// - `net_err_tx`: Channel sender for network errors
/// - `index_notify_tx`: Channel sender for index update notifications
/// - `updates_tx`: Channel sender for package updates
/// - `updates_refresh_interval`: Refresh interval in seconds for checkupdates and AUR helper checks
///
/// Details:
/// - Fetches Arch status text once at startup and periodically every 120 seconds
/// - Fetches Arch news once at startup, filtering out already-read items
/// - Updates package index in background (Windows vs non-Windows handling)
/// - Refreshes pacman caches (installed, explicit)
/// - Spawns tick worker that sends events every 200ms
/// - Checks for available package updates once at startup and periodically at configured interval
#[allow(clippy::too_many_arguments)]
pub fn spawn_auxiliary_workers(
    headless: bool,
    status_tx: &mpsc::UnboundedSender<(String, ArchStatusColor)>,
    news_tx: &mpsc::UnboundedSender<Vec<NewsItem>>,
    tick_tx: &mpsc::UnboundedSender<()>,
    news_read_urls: &std::collections::HashSet<String>,
    official_index_path: &std::path::Path,
    net_err_tx: &mpsc::UnboundedSender<String>,
    index_notify_tx: &mpsc::UnboundedSender<()>,
    updates_tx: &mpsc::UnboundedSender<(usize, Vec<String>)>,
    updates_refresh_interval: u64,
) {
    // Fetch Arch status text once at startup (skip in headless mode to avoid network delays)
    if !headless {
        let status_tx_once = status_tx.clone();
        tokio::spawn(async move {
            if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
                let _ = status_tx_once.send((txt, color));
            }
        });

        // Periodically refresh Arch status every 120 seconds
        let status_tx_periodic = status_tx.clone();
        tokio::spawn(async move {
            loop {
                sleep(Duration::from_secs(120)).await;
                if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
                    let _ = status_tx_periodic.send((txt, color));
                }
            }
        });
    }

    // Fetch Arch news once at startup; show unread items (by URL) if any (skip in headless mode)
    if !headless {
        let news_tx_once = news_tx.clone();
        let read_set = news_read_urls.clone();
        tokio::spawn(async move {
            if let Ok(list) = sources::fetch_arch_news(10).await {
                let unread: Vec<NewsItem> = list
                    .into_iter()
                    .filter(|it| !read_set.contains(&it.url))
                    .collect();
                let _ = news_tx_once.send(unread);
            }
        });
    }

    #[cfg(windows)]
    {
        // Save mirrors into the repository directory in the source tree and build the index via Arch API
        let repo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repository");
        let index_path = official_index_path.to_path_buf();
        let net_err = net_err_tx.clone();
        let index_notify = index_notify_tx.clone();
        tokio::spawn(async move {
            crate::index::refresh_windows_mirrors_and_index(
                index_path,
                repo_dir,
                net_err,
                index_notify,
            )
            .await;
        });
    }
    #[cfg(not(windows))]
    {
        // Skip index update in headless mode to avoid slow network/disk operations
        if !headless {
            let index_path = official_index_path.to_path_buf();
            let net_err = net_err_tx.clone();
            let index_notify = index_notify_tx.clone();
            tokio::spawn(async move {
                pkgindex::update_in_background(index_path, net_err, index_notify).await;
            });
        }
    }

    // Skip pacman cache refreshes in headless mode to avoid slow process spawning
    if !headless {
        tokio::spawn(async move {
            pkgindex::refresh_installed_cache().await;
            // Use default mode (LeafOnly) at startup; periodic refresh will use user's configured mode
            pkgindex::refresh_explicit_cache(crate::state::InstalledPackagesMode::default()).await;
        });
    }

    // Check for available package updates once at startup (skip in headless mode)
    if !headless {
        spawn_updates_worker(updates_tx.clone());

        // Periodically refresh updates list at configured interval
        let updates_tx_periodic = updates_tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(updates_refresh_interval));
            // Skip the first tick to avoid immediate refresh after startup
            interval.tick().await;
            loop {
                interval.tick().await;
                spawn_updates_worker(updates_tx_periodic.clone());
            }
        });
    }

    // Spawn tick worker
    let tick_tx_bg = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            let _ = tick_tx_bg.send(());
        }
    });
}

/// What: Check which AUR helper is available (paru or yay).
///
/// Output:
/// - Tuple of (`has_paru`, `has_yay`, `helper_name`)
fn check_aur_helper() -> (bool, bool, &'static str) {
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

/// What: Parse packages from checkupdates output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses `"package-name old_version -> new_version"` format
fn parse_checkupdates(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package-name old_version -> new_version" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// What: Parse packages from -Qua output.
///
/// Inputs:
/// - `output`: Raw command output bytes
///
/// Output:
/// - Vector of (`package_name`, `old_version`, `new_version`) tuples
///
/// Details:
/// - Parses "package old -> new" format
fn parse_qua(output: &[u8]) -> Vec<(String, String, String)> {
    String::from_utf8_lossy(output)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                // Parse "package old -> new" format
                trimmed.find(" -> ").and_then(|arrow_pos| {
                    let before_arrow = &trimmed[..arrow_pos];
                    let after_arrow = &trimmed[arrow_pos + 4..];
                    let parts: Vec<&str> = before_arrow.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[0].to_string();
                        let old_version = parts[1..].join(" "); // In case version has spaces
                        let new_version = after_arrow.trim().to_string();
                        Some((name, old_version, new_version))
                    } else {
                        None
                    }
                })
            }
        })
        .collect()
}

/// What: Process checkupdates output and add packages to collections.
///
/// Inputs:
/// - `output`: Command output result
/// - `packages_map`: Mutable `HashMap` to store formatted package strings
/// - `packages_set`: Mutable `HashSet` to track unique package names
fn process_checkupdates_output(
    output: Result<std::process::Output, std::io::Error>,
    packages_map: &mut std::collections::HashMap<String, String>,
    packages_set: &mut std::collections::HashSet<String>,
) {
    match output {
        Ok(output) => {
            if output.status.success() {
                let packages = parse_checkupdates(&output.stdout);
                let count = packages.len();

                // Parse checkupdates output which already contains old and new versions
                for (name, old_version, new_version) in packages {
                    // Format: "name - old_version -> name - new_version"
                    let formatted = format!("{name} - {old_version} -> {name} - {new_version}");
                    packages_map.insert(name.clone(), formatted);
                    packages_set.insert(name);
                }

                tracing::debug!(
                    "Found {} packages from official repos (checkupdates)",
                    count
                );
            } else if output.status.code() != Some(1) {
                // Exit code 1 is normal (no updates), other codes are errors
                tracing::warn!(
                    "checkupdates command failed with exit code: {:?}",
                    output.status.code()
                );
            }
        }
        Err(e) => {
            tracing::warn!("Failed to execute checkupdates: {}", e);
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
                        "Found {} packages from AUR (-Qua), {} total ({} new)",
                        count,
                        after_count,
                        after_count - before_count
                    );
                } else if output.status.code() != Some(1) {
                    // Exit code 1 is normal (no updates), other codes are errors
                    tracing::warn!(
                        "-Qua command failed with exit code: {:?}",
                        output.status.code()
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
/// - Executes `checkupdates` (official repos) and `yay -Qua` or `paru -Qua` (AUR)
/// - Checks for paru first, then falls back to yay for AUR updates
/// - Parses output from both commands (one package name per line)
/// - Removes duplicates using `HashSet`
/// - Sorts package names alphabetically
/// - Saves list to `~/.config/pacsea/lists/available_updates.txt`
/// - Sends `(count, sorted_list)` via channel
/// - Uses synchronization to prevent concurrent update checks and file writes
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

            let (has_paru, has_yay, helper) = check_aur_helper();

            // Execute checkupdates command (official repos)
            let output_checkupdates = Command::new("checkupdates")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output();

            // Execute -Qua command (AUR) - only if helper is available
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

            // Collect packages from both commands
            // Use HashMap to store: package_name -> formatted_string
            // Use HashSet to track unique package names for deduplication
            let mut packages_map: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            let mut packages_set = HashSet::new();

            // Parse checkupdates output (official repos)
            process_checkupdates_output(output_checkupdates, &mut packages_map, &mut packages_set);

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
            tracing::debug!(
                "Found {} total available updates (after deduplication)",
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

/// What: Spawn event reading thread for terminal input.
///
/// Inputs:
/// - `headless`: When `true`, skip spawning the thread
/// - `event_tx`: Channel sender for terminal events
/// - `event_thread_cancelled`: Atomic flag to signal thread cancellation
///
/// Details:
/// - Spawns a blocking thread that polls for terminal events
/// - Checks cancellation flag periodically to allow immediate exit
/// - Uses 50ms poll timeout to balance responsiveness and CPU usage
pub fn spawn_event_thread(
    headless: bool,
    event_tx: mpsc::UnboundedSender<CEvent>,
    event_thread_cancelled: Arc<AtomicBool>,
) {
    if !headless {
        let event_tx_for_thread = event_tx;
        let cancelled = event_thread_cancelled;
        std::thread::spawn(move || {
            loop {
                // Check cancellation flag first for immediate exit
                if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                // Use poll with timeout to allow periodic cancellation checks
                // This prevents blocking indefinitely when exit is requested
                match crossterm::event::poll(std::time::Duration::from_millis(50)) {
                    Ok(true) => {
                        // Event available, read it
                        if let Ok(ev) = crossterm::event::read() {
                            // Check cancellation again before sending
                            if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                                break;
                            }
                            // Check if channel is still open before sending
                            // When receiver is dropped (on exit), send will fail
                            if event_tx_for_thread.send(ev).is_err() {
                                // Channel closed, exit thread
                                break;
                            }
                        }
                        // ignore transient read errors and continue
                    }
                    Ok(false) => {
                        // No event available, check cancellation flag
                        // This allows the thread to exit promptly when exit is requested
                        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                    }
                    Err(_) => {
                        // Poll error, check cancellation before continuing
                        if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::parse_checkupdates;

    /// What: Test that checkupdates parsing correctly extracts old and new versions.
    ///
    /// Inputs:
    /// - Sample checkupdates output with format `"package-name old_version -> new_version"`
    ///
    /// Output:
    /// - Verifies that `old_version` and `new_version` are correctly parsed and different
    ///
    /// Details:
    /// - Tests parsing of checkupdates output format
    #[test]
    fn test_parse_checkupdates_extracts_correct_versions() {
        let test_cases = vec![
            ("bat 0.26.0-1 -> 0.26.0-2", "bat", "0.26.0-1", "0.26.0-2"),
            (
                "comgr 2:6.4.4-2 -> 2:7.1.0-1",
                "comgr",
                "2:6.4.4-2",
                "2:7.1.0-1",
            ),
            (
                "composable-kernel 6.4.4-1 -> 7.1.0-1",
                "composable-kernel",
                "6.4.4-1",
                "7.1.0-1",
            ),
        ];

        for (input, expected_name, expected_old, expected_new) in test_cases {
            let output = input.as_bytes();
            let entries = parse_checkupdates(output);

            assert_eq!(entries.len(), 1, "Failed to parse: {input}");
            let (name, old_version, new_version) = &entries[0];
            assert_eq!(name, expected_name, "Wrong name for: {input}");
            assert_eq!(old_version, expected_old, "Wrong old_version for: {input}");
            assert_eq!(new_version, expected_new, "Wrong new_version for: {input}");
        }
    }

    /// What: Test that checkupdates parsing handles multiple packages.
    ///
    /// Inputs:
    /// - Multi-line checkupdates output
    ///
    /// Output:
    /// - Verifies that all packages are parsed correctly
    #[test]
    fn test_parse_checkupdates_multiple_packages() {
        let input = "bat 0.26.0-1 -> 0.26.0-2\ncomgr 2:6.4.4-2 -> 2:7.1.0-1\n";
        let output = input.as_bytes();
        let entries = parse_checkupdates(output);

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0],
            (
                "bat".to_string(),
                "0.26.0-1".to_string(),
                "0.26.0-2".to_string()
            )
        );
        assert_eq!(
            entries[1],
            (
                "comgr".to_string(),
                "2:6.4.4-2".to_string(),
                "2:7.1.0-1".to_string()
            )
        );
    }
}
