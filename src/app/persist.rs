use std::fs;

use super::deps_cache;
use super::files_cache;
use super::sandbox_cache;
use super::services_cache;
use crate::state::AppState;

/// What: Persist the details cache to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state whose `details_cache` and `cache_path` are used
///
/// Output:
/// - Writes `details_cache` JSON to `cache_path` and clears the dirty flag on success.
pub fn maybe_flush_cache(app: &mut AppState) {
    if !app.cache_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.details_cache) {
        tracing::trace!(
            path = %app.cache_path.display(),
            bytes = s.len(),
            "[Persist] Writing details cache to disk"
        );
        match fs::write(&app.cache_path, &s) {
            Ok(()) => {
                tracing::trace!(
                    path = %app.cache_path.display(),
                    "[Persist] Details cache persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.cache_path.display(),
                    error = %e,
                    "[Persist] Failed to write details cache"
                );
            }
        }
        // Preserve existing behaviour: clear dirty flag regardless to avoid repeated writes.
        app.cache_dirty = false;
    }
}

/// What: Persist the recent searches list to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `recent` and `recent_path`
///
/// Output:
/// - Writes `recent` JSON to `recent_path` and clears the dirty flag on success.
pub fn maybe_flush_recent(app: &mut AppState) {
    if !app.recent_dirty {
        return;
    }
    let recent_values = app.recent_values();
    if let Ok(s) = serde_json::to_string(&recent_values) {
        tracing::debug!(
            path = %app.recent_path.display(),
            bytes = s.len(),
            "[Persist] Writing recent searches to disk"
        );
        match fs::write(&app.recent_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.recent_path.display(),
                    "[Persist] Recent searches persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.recent_path.display(),
                    error = %e,
                    "[Persist] Failed to write recent searches"
                );
            }
        }
        app.recent_dirty = false;
    }
}

/// What: Persist the news search history to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `news_recent` and `news_recent_path`
pub fn maybe_flush_news_recent(app: &mut AppState) {
    if !app.news_recent_dirty {
        return;
    }
    let values: Vec<String> = app.news_recent.iter().map(|(_, v)| v.clone()).collect();
    if let Ok(s) = serde_json::to_string(&values) {
        tracing::debug!(
            path = %app.news_recent_path.display(),
            bytes = s.len(),
            "[Persist] Writing news recent searches to disk"
        );
        match fs::write(&app.news_recent_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.news_recent_path.display(),
                    "[Persist] News recent searches persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.news_recent_path.display(),
                    error = %e,
                    "[Persist] Failed to write news recent searches"
                );
            }
        }
        app.news_recent_dirty = false;
    }
}

/// What: Persist news bookmarks to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `news_bookmarks` and `news_bookmarks_path`
pub fn maybe_flush_news_bookmarks(app: &mut AppState) {
    if !app.news_bookmarks_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.news_bookmarks) {
        tracing::debug!(
            path = %app.news_bookmarks_path.display(),
            bytes = s.len(),
            "[Persist] Writing news bookmarks to disk"
        );
        match fs::write(&app.news_bookmarks_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.news_bookmarks_path.display(),
                    "[Persist] News bookmarks persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.news_bookmarks_path.display(),
                    error = %e,
                    "[Persist] Failed to write news bookmarks"
                );
            }
        }
        app.news_bookmarks_dirty = false;
    }
}

/// What: Persist the set of read Arch news URLs to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `news_read_urls` and `news_read_path`
///
/// Output:
/// - Writes `news_read_urls` JSON to `news_read_path` and clears the dirty flag on success.
pub fn maybe_flush_news_read(app: &mut AppState) {
    if !app.news_read_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.news_read_urls) {
        tracing::debug!(
            path = %app.news_read_path.display(),
            bytes = s.len(),
            "[Persist] Writing news read URLs to disk"
        );
        match fs::write(&app.news_read_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.news_read_path.display(),
                    "[Persist] News read URLs persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.news_read_path.display(),
                    error = %e,
                    "[Persist] Failed to write news read URLs"
                );
            }
        }
        app.news_read_dirty = false;
    }
}

/// What: Persist last-seen package versions for news updates if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `news_seen_pkg_versions` and its path.
///
/// Output:
/// - Writes JSON file when dirty, clears the dirty flag on success.
///
/// Details:
/// - No-op when dirty flag is false; logs success/failure.
pub fn maybe_flush_news_seen_versions(app: &mut AppState) {
    if !app.news_seen_pkg_versions_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.news_seen_pkg_versions) {
        tracing::debug!(
            path = %app.news_seen_pkg_versions_path.display(),
            bytes = s.len(),
            "[Persist] Writing news seen package versions to disk"
        );
        match fs::write(&app.news_seen_pkg_versions_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.news_seen_pkg_versions_path.display(),
                    "[Persist] News seen package versions persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.news_seen_pkg_versions_path.display(),
                    error = %e,
                    "[Persist] Failed to write news seen package versions"
                );
            }
        }
        app.news_seen_pkg_versions_dirty = false;
    }
}

/// What: Persist last-seen AUR comments if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `news_seen_aur_comments` and its path.
///
/// Output:
/// - Writes JSON file when dirty, clears the dirty flag on success.
///
/// Details:
/// - No-op when dirty flag is false; logs success/failure.
pub fn maybe_flush_news_seen_aur_comments(app: &mut AppState) {
    if !app.news_seen_aur_comments_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.news_seen_aur_comments) {
        tracing::debug!(
            path = %app.news_seen_aur_comments_path.display(),
            bytes = s.len(),
            "[Persist] Writing news seen AUR comments to disk"
        );
        match fs::write(&app.news_seen_aur_comments_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.news_seen_aur_comments_path.display(),
                    "[Persist] News seen AUR comments persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.news_seen_aur_comments_path.display(),
                    error = %e,
                    "[Persist] Failed to write news seen AUR comments"
                );
            }
        }
        app.news_seen_aur_comments_dirty = false;
    }
}

/// What: Persist the announcement read IDs to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state containing `announcements_read_ids` and `announcement_read_path`
///
/// Output:
/// - Writes `announcements_read_ids` JSON to `announcement_read_path` and clears the dirty flag on success.
///
/// Details:
/// - Saves set of read announcement IDs as JSON array.
pub fn maybe_flush_announcement_read(app: &mut AppState) {
    if !app.announcement_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.announcements_read_ids) {
        tracing::debug!(
            path = %app.announcement_read_path.display(),
            bytes = s.len(),
            "[Persist] Writing announcement read IDs to disk"
        );
        match fs::write(&app.announcement_read_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.announcement_read_path.display(),
                    "[Persist] Announcement read IDs persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.announcement_read_path.display(),
                    error = %e,
                    "[Persist] Failed to write announcement read IDs"
                );
            }
        }
        app.announcement_dirty = false;
    }
}

/// What: Persist the dependency cache to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state with `install_list_deps`, `deps_cache_path`, and `install_list`
///
/// Output:
/// - Writes dependency cache JSON to `deps_cache_path` and clears dirty flag on success.
/// - If install list is empty, ensures an empty cache file exists instead of deleting it.
pub fn maybe_flush_deps_cache(app: &mut AppState) {
    if app.install_list.is_empty() {
        // Write an empty cache file when nothing is queued to keep the path present.
        if app.deps_cache_dirty || !app.deps_cache_path.exists() {
            let empty_signature: Vec<String> = Vec::new();
            tracing::debug!(
                path = %app.deps_cache_path.display(),
                "[Persist] Writing empty dependency cache because install list is empty"
            );
            deps_cache::save_cache(&app.deps_cache_path, &empty_signature, &[]);
        }
        app.deps_cache_dirty = false;
        return;
    }
    if !app.deps_cache_dirty {
        return;
    }
    let signature = deps_cache::compute_signature(&app.install_list);
    tracing::debug!(
        path = %app.deps_cache_path.display(),
        signature_len = signature.len(),
        deps_len = app.install_list_deps.len(),
        "[Persist] Saving dependency cache"
    );
    deps_cache::save_cache(&app.deps_cache_path, &signature, &app.install_list_deps);
    app.deps_cache_dirty = false;
    tracing::debug!(
        path = %app.deps_cache_path.display(),
        deps_len = app.install_list_deps.len(),
        "[Persist] Dependency cache save attempted"
    );
}

/// What: Persist the file cache to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state with `install_list_files`, `files_cache_path`, and `install_list`
///
/// Output:
/// - Writes file cache JSON to `files_cache_path` and clears dirty flag on success.
/// - If install list is empty, ensures an empty cache file exists instead of deleting it.
pub fn maybe_flush_files_cache(app: &mut AppState) {
    if app.install_list.is_empty() {
        // Write an empty cache file when nothing is queued to keep the path present.
        if app.files_cache_dirty || !app.files_cache_path.exists() {
            let empty_signature: Vec<String> = Vec::new();
            tracing::debug!(
                path = %app.files_cache_path.display(),
                "[Persist] Writing empty file cache because install list is empty"
            );
            files_cache::save_cache(&app.files_cache_path, &empty_signature, &[]);
        }
        app.files_cache_dirty = false;
        return;
    }
    if !app.files_cache_dirty {
        return;
    }
    let signature = files_cache::compute_signature(&app.install_list);
    tracing::debug!(
        "[Persist] Saving file cache: {} entries for packages: {:?}, signature: {:?}",
        app.install_list_files.len(),
        app.install_list_files
            .iter()
            .map(|f| &f.name)
            .collect::<Vec<_>>(),
        signature
    );
    files_cache::save_cache(&app.files_cache_path, &signature, &app.install_list_files);
    app.files_cache_dirty = false;
    tracing::debug!(
        "[Persist] File cache saved successfully, install_list_files still has {} entries",
        app.install_list_files.len()
    );
}

/// What: Persist the service cache to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state with `install_list_services`, `services_cache_path`, and `install_list`
///
/// Output:
/// - Writes service cache JSON to `services_cache_path` and clears dirty flag on success.
/// - If install list is empty, ensures an empty cache file exists instead of deleting it.
pub fn maybe_flush_services_cache(app: &mut AppState) {
    if app.install_list.is_empty() {
        // Write an empty cache file when nothing is queued to keep the path present.
        if app.services_cache_dirty || !app.services_cache_path.exists() {
            let empty_signature: Vec<String> = Vec::new();
            tracing::debug!(
                path = %app.services_cache_path.display(),
                "[Persist] Writing empty service cache because install list is empty"
            );
            services_cache::save_cache(&app.services_cache_path, &empty_signature, &[]);
        }
        app.services_cache_dirty = false;
        return;
    }
    if !app.services_cache_dirty {
        return;
    }
    let signature = services_cache::compute_signature(&app.install_list);
    tracing::debug!(
        path = %app.services_cache_path.display(),
        signature_len = signature.len(),
        services_len = app.install_list_services.len(),
        "[Persist] Saving service cache"
    );
    services_cache::save_cache(
        &app.services_cache_path,
        &signature,
        &app.install_list_services,
    );
    app.services_cache_dirty = false;
    tracing::debug!(
        path = %app.services_cache_path.display(),
        services_len = app.install_list_services.len(),
        "[Persist] Service cache save attempted"
    );
}

/// What: Persist the sandbox cache to disk if marked dirty.
///
/// Inputs:
/// - `app`: Application state with `install_list_sandbox`, `sandbox_cache_path`, and `install_list`
///
/// Output:
/// - Writes sandbox cache JSON to `sandbox_cache_path` and clears dirty flag on success.
/// - If install list is empty, ensures an empty cache file exists instead of deleting it.
pub fn maybe_flush_sandbox_cache(app: &mut AppState) {
    if app.install_list.is_empty() {
        // Write an empty cache file when nothing is queued to keep the path present.
        if app.sandbox_cache_dirty || !app.sandbox_cache_path.exists() {
            let empty_signature: Vec<String> = Vec::new();
            tracing::debug!(
                path = %app.sandbox_cache_path.display(),
                "[Persist] Writing empty sandbox cache because install list is empty"
            );
            sandbox_cache::save_cache(&app.sandbox_cache_path, &empty_signature, &[]);
        }
        app.sandbox_cache_dirty = false;
        return;
    }
    if !app.sandbox_cache_dirty {
        return;
    }
    let signature = sandbox_cache::compute_signature(&app.install_list);
    tracing::debug!(
        "[Persist] Saving sandbox cache: {} entries for packages: {:?}, signature: {:?}",
        app.install_list_sandbox.len(),
        app.install_list_sandbox
            .iter()
            .map(|s| &s.package_name)
            .collect::<Vec<_>>(),
        signature
    );
    sandbox_cache::save_cache(
        &app.sandbox_cache_path,
        &signature,
        &app.install_list_sandbox,
    );
    app.sandbox_cache_dirty = false;
    tracing::debug!(
        "[Persist] Sandbox cache saved successfully, install_list_sandbox still has {} entries",
        app.install_list_sandbox.len()
    );
}

/// What: Persist the PKGBUILD parse cache if there are pending updates.
///
/// Inputs: None.
///
/// Output:
/// - Flushes the global PKGBUILD parse cache to disk best-effort.
///
/// Details:
/// - Safe to call frequently; returns immediately when cache is clean.
pub fn maybe_flush_pkgbuild_parse_cache() {
    crate::logic::files::flush_pkgbuild_cache();
}

/// What: Persist the install list to disk if marked dirty, throttled to ~1s.
///
/// Inputs:
/// - `app`: Application state with `install_list`, `install_path`, and throttle timestamps
///
/// Output:
/// - Writes `install_list` JSON to `install_path` and clears dirty flags when written.
pub fn maybe_flush_install(app: &mut AppState) {
    // Throttle disk writes: only flush if dirty and either never written
    // before or the last change is at least 1s ago.
    if !app.install_dirty {
        return;
    }
    if let Some(when) = app.last_install_change
        && when.elapsed() < std::time::Duration::from_millis(1000)
    {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.install_list) {
        tracing::debug!(
            path = %app.install_path.display(),
            bytes = s.len(),
            "[Persist] Writing install list to disk"
        );
        match fs::write(&app.install_path, &s) {
            Ok(()) => {
                tracing::debug!(
                    path = %app.install_path.display(),
                    "[Persist] Install list persisted"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %app.install_path.display(),
                    error = %e,
                    "[Persist] Failed to write install list"
                );
            }
        }
        app.install_dirty = false;
        app.last_install_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::modal::{
        DependencyInfo, DependencySource, DependencyStatus, FileChange, FileChangeType,
        PackageFileInfo,
    };
    use crate::state::{PackageItem, Source};

    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Ensure `maybe_flush_cache` persists the details cache and clears the dirty flag.
    ///
    /// Inputs:
    /// - `AppState` with `cache_dirty = true` pointing to a temporary cache path.
    ///
    /// Output:
    /// - Writes JSON to disk, resets `cache_dirty`, and leaves audit strings in the file.
    ///
    /// Details:
    /// - Validates the helper cleans up after itself by removing the temp file at the end.
    fn flush_cache_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_cache_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.cache_path = path.clone();
        app.details_cache.insert(
            "ripgrep".into(),
            crate::state::PackageDetails {
                name: "ripgrep".into(),
                ..Default::default()
            },
        );
        app.cache_dirty = true;
        maybe_flush_cache(&mut app);
        assert!(!app.cache_dirty);
        let body =
            std::fs::read_to_string(&app.cache_path).expect("Failed to read test cache file");
        assert!(body.contains("ripgrep"));
        let _ = std::fs::remove_file(&app.cache_path);
    }

    #[test]
    /// What: Verify `maybe_flush_recent` serialises the recent list and resets the dirty flag.
    ///
    /// Inputs:
    /// - `AppState` seeded with recent entries, temp path, and `recent_dirty = true`.
    ///
    /// Output:
    /// - JSON file includes both entries and `recent_dirty` becomes `false`.
    ///
    /// Details:
    /// - Cleans up the generated file to avoid cluttering the system temp directory.
    fn flush_recent_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_recent_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.recent_path = path.clone();
        app.load_recent_items(&["rg".to_string(), "fd".to_string()]);
        app.recent_dirty = true;
        maybe_flush_recent(&mut app);
        assert!(!app.recent_dirty);
        let body =
            std::fs::read_to_string(&app.recent_path).expect("Failed to read test recent file");
        assert!(body.contains("rg") && body.contains("fd"));
        let _ = std::fs::remove_file(&app.recent_path);
    }

    #[test]
    /// What: Check `maybe_flush_install` throttles writes then persists once the timer elapses.
    ///
    /// Inputs:
    /// - `AppState` with `install_dirty = true`, a fresh package entry, and `last_install_change` set to now.
    ///
    /// Output:
    /// - First invocation avoids writing; after clearing the timestamp, the file appears with the package name.
    ///
    /// Details:
    /// - Simulates the passage of time by resetting `last_install_change` before invoking the helper again.
    fn flush_install_throttle_and_write() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_install_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.install_path = path.clone();
        app.install_list = vec![crate::state::PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.install_dirty = true;
        app.last_install_change = Some(std::time::Instant::now());
        // First call should be throttled -> no file
        maybe_flush_install(&mut app);
        assert!(std::fs::read_to_string(&app.install_path).is_err());
        // Simulate time passing by clearing last_install_change
        app.last_install_change = None;
        maybe_flush_install(&mut app);
        let body =
            std::fs::read_to_string(&app.install_path).expect("Failed to read test install file");
        assert!(body.contains("rg"));
        let _ = std::fs::remove_file(&app.install_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_deps_cache` persists dependency cache entries and clears the dirty flag.
    ///
    /// Inputs:
    /// - `AppState` with a populated install list, dependency data, and `deps_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file contains dependency information and the dirty flag is reset.
    ///
    /// Details:
    /// - Cleans up the temporary file to keep runs idempotent.
    fn flush_deps_cache_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_deps_cache_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.deps_cache_path = path.clone();
        app.install_list = vec![PackageItem {
            name: "ripgrep".into(),
            version: "14.0.0".into(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.install_list_deps = vec![DependencyInfo {
            name: "gcc-libs".into(),
            version: ">=13".into(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".into(),
            },
            required_by: vec!["ripgrep".into()],
            depends_on: Vec::new(),
            is_core: true,
            is_system: false,
        }];
        app.deps_cache_dirty = true;
        maybe_flush_deps_cache(&mut app);
        assert!(!app.deps_cache_dirty);
        let body = std::fs::read_to_string(&app.deps_cache_path)
            .expect("Failed to read test deps cache file");
        assert!(body.contains("gcc-libs"));
        let _ = std::fs::remove_file(&app.deps_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_deps_cache` writes an empty cache file when the install list is empty.
    ///
    /// Inputs:
    /// - `AppState` with an empty install list, existing cache file, and `deps_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file is replaced with an empty payload and the dirty flag is cleared.
    ///
    /// Details:
    /// - Simulates clearing the install list so persistence helper should clean up stale cache content.
    fn flush_deps_cache_writes_empty_when_install_list_empty() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_deps_cache_remove_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.deps_cache_path = path.clone();
        std::fs::write(&app.deps_cache_path, "stale")
            .expect("Failed to write test deps cache file");
        app.deps_cache_dirty = true;
        app.install_list.clear();

        maybe_flush_deps_cache(&mut app);

        assert!(!app.deps_cache_dirty);
        let body = std::fs::read_to_string(&app.deps_cache_path)
            .expect("Failed to read test deps cache file");
        let cache: crate::app::deps_cache::DependencyCache =
            serde_json::from_str(&body).expect("Failed to parse dependency cache");
        assert!(cache.install_list_signature.is_empty());
        assert!(cache.dependencies.is_empty());
        let _ = std::fs::remove_file(&app.deps_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_files_cache` persists file change metadata and clears the dirty flag.
    ///
    /// Inputs:
    /// - `AppState` with a populated install list, file change data, and `files_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file contains file metadata and the dirty flag is reset.
    ///
    /// Details:
    /// - Removes the temporary cache file after assertions complete.
    fn flush_files_cache_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_files_cache_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.files_cache_path = path.clone();
        app.install_list = vec![PackageItem {
            name: "ripgrep".into(),
            version: "14.0.0".into(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.install_list_files = vec![PackageFileInfo {
            name: "ripgrep".into(),
            files: vec![FileChange {
                path: "/usr/bin/rg".into(),
                change_type: FileChangeType::New,
                package: "ripgrep".into(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            }],
            total_count: 1,
            new_count: 1,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        }];
        app.files_cache_dirty = true;
        maybe_flush_files_cache(&mut app);
        assert!(!app.files_cache_dirty);
        let body = std::fs::read_to_string(&app.files_cache_path)
            .expect("Failed to read test files cache file");
        assert!(body.contains("/usr/bin/rg"));
        let _ = std::fs::remove_file(&app.files_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_files_cache` writes an empty cache file when the install list is empty.
    ///
    /// Inputs:
    /// - `AppState` with an empty install list, an on-disk cache file, and `files_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file is replaced with an empty payload and the dirty flag resets.
    ///
    /// Details:
    /// - Mirrors the behaviour when the user clears the install list to keep disk cache in sync.
    fn flush_files_cache_writes_empty_when_install_list_empty() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_files_cache_remove_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.files_cache_path = path.clone();
        std::fs::write(&app.files_cache_path, "stale")
            .expect("Failed to write test files cache file");
        app.files_cache_dirty = true;
        app.install_list.clear();

        maybe_flush_files_cache(&mut app);

        assert!(!app.files_cache_dirty);
        let body = std::fs::read_to_string(&app.files_cache_path)
            .expect("Failed to read test files cache file");
        let cache: crate::app::files_cache::FileCache =
            serde_json::from_str(&body).expect("Failed to parse file cache");
        assert!(cache.install_list_signature.is_empty());
        assert!(cache.files.is_empty());
        let _ = std::fs::remove_file(&app.files_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_services_cache` writes an empty cache file when the install list is empty.
    ///
    /// Inputs:
    /// - `AppState` with an empty install list, an on-disk cache file, and `services_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file is replaced with an empty payload and the dirty flag resets.
    ///
    /// Details:
    /// - Keeps the cache path present on disk instead of deleting it.
    fn flush_services_cache_writes_empty_when_install_list_empty() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_services_cache_remove_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.services_cache_path = path.clone();
        std::fs::write(&app.services_cache_path, "stale")
            .expect("Failed to write test services cache file");
        app.services_cache_dirty = true;
        app.install_list.clear();

        maybe_flush_services_cache(&mut app);

        assert!(!app.services_cache_dirty);
        let body = std::fs::read_to_string(&app.services_cache_path)
            .expect("Failed to read test services cache file");
        let cache: crate::app::services_cache::ServiceCache =
            serde_json::from_str(&body).expect("Failed to parse service cache");
        assert!(cache.install_list_signature.is_empty());
        assert!(cache.services.is_empty());
        let _ = std::fs::remove_file(&app.services_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_sandbox_cache` writes an empty cache file when the install list is empty.
    ///
    /// Inputs:
    /// - `AppState` with an empty install list, an on-disk cache file, and `sandbox_cache_dirty = true`.
    ///
    /// Output:
    /// - Cache file is replaced with an empty payload and the dirty flag resets.
    ///
    /// Details:
    /// - Keeps the cache path present on disk instead of deleting it.
    fn flush_sandbox_cache_writes_empty_when_install_list_empty() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_sandbox_cache_remove_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.sandbox_cache_path = path.clone();
        std::fs::write(&app.sandbox_cache_path, "stale")
            .expect("Failed to write test sandbox cache file");
        app.sandbox_cache_dirty = true;
        app.install_list.clear();

        maybe_flush_sandbox_cache(&mut app);

        assert!(!app.sandbox_cache_dirty);
        let body = std::fs::read_to_string(&app.sandbox_cache_path)
            .expect("Failed to read test sandbox cache file");
        let cache: crate::app::sandbox_cache::SandboxCache =
            serde_json::from_str(&body).expect("Failed to parse sandbox cache");
        assert!(cache.install_list_signature.is_empty());
        assert!(cache.sandbox_info.is_empty());
        let _ = std::fs::remove_file(&app.sandbox_cache_path);
    }

    #[test]
    /// What: Ensure `maybe_flush_news_read` persists read URLs and clears the dirty flag.
    ///
    /// Inputs:
    /// - `AppState` providing a temp `news_read_path`, a URL in the set, and `news_read_dirty = true`.
    ///
    /// Output:
    /// - File contains the expected URL and `news_read_dirty` flips to `false`.
    ///
    /// Details:
    /// - Removes the temp artifact to keep tests idempotent across runs.
    fn flush_news_read_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_newsread_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        app.news_read_path = path.clone();
        app.news_read_urls
            .insert("https://archlinux.org/news/example/".into());
        app.news_read_dirty = true;
        maybe_flush_news_read(&mut app);
        assert!(!app.news_read_dirty);
        let body = std::fs::read_to_string(&app.news_read_path)
            .expect("Failed to read test news read file");
        assert!(body.contains("archlinux.org/news"));
        let _ = std::fs::remove_file(&app.news_read_path);
    }
}
