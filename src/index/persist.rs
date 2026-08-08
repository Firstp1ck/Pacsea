//! Official package index persistence through arch-toolkit.

use std::path::Path;

use super::idx;

/// What: Load the official index from disk through arch-toolkit.
///
/// Inputs:
/// - `path`: Existing Pacsea index JSON path.
///
/// Output:
/// - Replaces the process-wide index when loading and conversion succeed.
///
/// Details:
/// - Invalid or corrupt cache data is ignored so startup remains resilient; name lookup is rebuilt.
pub fn load_from_disk(path: &Path) {
    match crate::integrations::arch_toolkit::index::load(path) {
        Ok(index) => {
            if let Ok(mut guard) = idx().write() {
                *guard = index;
            }
        }
        Err(error) => {
            tracing::debug!(path = %path.display(), error = %error, "official index cache unavailable");
        }
    }
}

/// What: Persist the current official index through arch-toolkit.
///
/// Inputs:
/// - `path`: Destination JSON path.
///
/// Output:
/// - Writes a Pacsea-compatible index snapshot; failures are logged and remain nonfatal.
///
/// Details:
/// - arch-toolkit handles parent creation and serialization while Pacsea retains process-wide state.
pub fn save_to_disk(path: &Path) {
    let Ok(guard) = idx().read() else {
        tracing::warn!(path = %path.display(), "official index lock is poisoned; cache not saved");
        return;
    };
    if guard.pkgs.is_empty() {
        tracing::warn!(path = %path.display(), "attempting to save empty official index");
    }
    if let Err(error) = crate::integrations::arch_toolkit::index::save(&guard, path) {
        tracing::warn!(path = %path.display(), error = %error, "failed to save official index");
    } else {
        tracing::info!(
            path = %path.display(),
            package_count = guard.pkgs.len(),
            "saved official index"
        );
    }
}

#[cfg(test)]
mod tests {
    /// What: Verify Pacsea index snapshots persist through the toolkit boundary.
    ///
    /// Inputs:
    /// - One in-memory package and a temporary destination.
    ///
    /// Output:
    /// - Reloaded process state contains the package and rebuilt lookup.
    ///
    /// Details:
    /// - Uses a deterministic local file and removes it afterward.
    #[test]
    fn index_persistence_round_trip_uses_toolkit() {
        let _guard = crate::global_test_mutex_lock();
        if let Ok(mut index) = super::idx().write() {
            index.pkgs = vec![crate::index::OfficialPkg {
                name: "pacsea-fixture".to_string(),
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
                version: "1".to_string(),
                description: "fixture".to_string(),
            }];
            index.rebuild_name_index();
        }
        let path = std::env::temp_dir().join(format!(
            "pacsea-toolkit-index-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should follow epoch")
                .as_nanos()
        ));
        super::save_to_disk(&path);
        if let Ok(mut index) = super::idx().write() {
            *index = crate::index::OfficialIndex::default();
        }
        super::load_from_disk(&path);
        assert!(crate::index::find_package_by_name("pacsea-fixture").is_some());
        let _ = std::fs::remove_file(path);
    }
}
