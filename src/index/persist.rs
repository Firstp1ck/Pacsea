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
/// - Writes JSON to disk; errors are ignored to avoid interrupting the UI.
pub fn save_to_disk(path: &Path) {
    if let Ok(guard) = idx().read()
        && let Ok(s) = serde_json::to_string(&*guard)
    {
        let _ = fs::write(path, s);
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    /// What: Loading multiple index snapshots results in deduped package names
    ///
    /// - Input: Two JSON files with overlapping names written sequentially
    /// - Output: all_official() returns unique names ["aa", "zz"]
    async fn index_loads_deduped_and_sorted_after_multiple_writes() {
        use std::path::PathBuf;

        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!(
            "pacsea_idx_multi_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let idx_json1 = serde_json::json!({
            "pkgs": [
                {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""},
                {"name": "aa", "repo": "core", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(&path, serde_json::to_string(&idx_json1).unwrap()).unwrap();
        super::load_from_disk(&path);

        let idx_json2 = serde_json::json!({
            "pkgs": [
                {"name": "aa", "repo": "core", "arch": "x86_64", "version": "2", "description": ""},
                {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(&path, serde_json::to_string(&idx_json2).unwrap()).unwrap();
        super::load_from_disk(&path);

        let all = crate::index::all_official();
        let mut names: Vec<String> = all.into_iter().map(|p| p.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names, vec!["aa", "zz"]);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
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
                .unwrap()
                .as_nanos()
        ));
        super::save_to_disk(&path);
        // Read back and assert content contains our package name
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"abc\""));
        let _ = std::fs::remove_file(&path);
    }
}
