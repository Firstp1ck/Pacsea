use std::fs;
use std::path::Path;

use super::{OfficialIndex, idx};

/// Load the official index from `path` if a valid JSON exists.
///
/// Silently ignores errors and leaves the index unchanged on failure.
pub fn load_from_disk(path: &Path) {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(new_idx) = serde_json::from_str::<OfficialIndex>(&s)
        && let Ok(mut guard) = idx().write()
    {
        *guard = new_idx;
    }
}

/// Persist the current official index to `path` as JSON.
///
/// Silently ignores errors to avoid interrupting the UI.
pub fn save_to_disk(path: &Path) {
    if let Ok(guard) = idx().read()
        && let Ok(s) = serde_json::to_string(&*guard)
    {
        let _ = fs::write(path, s);
    }
}
