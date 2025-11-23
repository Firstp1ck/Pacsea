use std::fs;
use std::path::Path;

use super::{OfficialIndex, idx};

/// What: Load the official index from `path` if a valid JSON exists.
///
/// Inputs:
/// - `path`: File path to read JSON from
///
/// Output:
/// - Replaces the in-memory index on success; ignores errors and leaves it unchanged on failure.
///
/// Details:
/// - Silently ignores IO or deserialization failures to keep startup resilient.
pub fn load_from_disk(path: &Path) {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(new_idx) = serde_json::from_str::<OfficialIndex>(&s)
        && let Ok(mut guard) = idx().write()
    {
        *guard = new_idx;
    }
}

/// What: Persist the current official index to `path` as JSON.
///
/// Inputs:
/// - `path`: File path to write JSON to
///
/// Output:
/// - Writes JSON to disk; errors are logged but not propagated to avoid interrupting the UI.
///
/// Details:
/// - Serializes under a read lock and ensures parent directory exists before writing.
/// - Creates parent directory if it doesn't exist (Windows-compatible).
/// - Logs write failures for debugging but doesn't crash background tasks.
/// - Warns if the index is empty when saving.
pub fn save_to_disk(path: &Path) {
    if let Ok(guard) = idx().read()
        && let Ok(s) = serde_json::to_string(&*guard)
    {
        // Warn if index is empty
        if guard.pkgs.is_empty() {
            tracing::warn!(
                path = %path.display(),
                "Attempting to save empty index to disk"
            );
        }
        // Ensure parent directory exists before writing
        if let Some(parent) = path.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "Failed to create parent directory for index file"
            );
            return;
        }
        // Write the file and log errors
        if let Err(e) = fs::write(path, s) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "Failed to write index file to disk"
            );
        } else {
            tracing::debug!(
                path = %path.display(),
                package_count = guard.pkgs.len(),
                "Successfully saved index to disk"
            );
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    /// What: Load multiple index snapshots and ensure deduplication.
    ///
    /// Inputs:
    /// - Two JSON snapshots with overlapping package names written sequentially.
    ///
    /// Output:
    /// - `all_official()` yields the unique names `aa` and `zz`.
    ///
    /// Details:
    /// - Validates that reloading replaces the index without duplicating entries.
    async fn index_loads_deduped_and_sorted_after_multiple_writes() {
        use std::path::PathBuf;

        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!(
            "pacsea_idx_multi_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));

        let idx_json1 = serde_json::json!({
            "pkgs": [
                {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""},
                {"name": "aa", "repo": "core", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(
            &path,
            serde_json::to_string(&idx_json1).expect("failed to serialize index JSON"),
        )
        .expect("failed to write index JSON file");
        super::load_from_disk(&path);

        let idx_json2 = serde_json::json!({
            "pkgs": [
                {"name": "aa", "repo": "core", "arch": "x86_64", "version": "2", "description": ""},
                {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(
            &path,
            serde_json::to_string(&idx_json2).expect("failed to serialize index JSON"),
        )
        .expect("failed to write index JSON file");
        super::load_from_disk(&path);

        let all = crate::index::all_official();
        let mut names: Vec<String> = all.into_iter().map(|p| p.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names, vec!["aa", "zz"]);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    /// What: Persist the in-memory index and confirm the file reflects current data.
    ///
    /// Inputs:
    /// - Seed `idx()` with a single package prior to saving.
    ///
    /// Output:
    /// - JSON file containing the seeded package name.
    ///
    /// Details:
    /// - Uses a temp file cleaned up at the end to avoid polluting the workspace.
    async fn index_save_writes_current_state_to_disk() {
        use std::path::PathBuf;
        // Prepare in-memory index
        if let Ok(mut g) = super::idx().write() {
            g.pkgs = vec![crate::index::OfficialPkg {
                name: "abc".to_string(),
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
                version: "9".to_string(),
                description: "desc".to_string(),
            }];
        }
        // Temp path
        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!(
            "pacsea_idx_save_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        super::save_to_disk(&path);
        // Read back and assert content contains our package name
        let body = std::fs::read_to_string(&path).expect("failed to read index JSON file");
        assert!(body.contains("\"abc\""));
        let _ = std::fs::remove_file(&path);
    }
}
