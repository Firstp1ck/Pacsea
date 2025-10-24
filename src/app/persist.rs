use std::fs;

use crate::state::AppState;

pub fn maybe_flush_cache(app: &mut AppState) {
    if !app.cache_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.details_cache) {
        let _ = fs::write(&app.cache_path, s);
        app.cache_dirty = false;
    }
}

pub fn maybe_flush_recent(app: &mut AppState) {
    if !app.recent_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.recent) {
        let _ = fs::write(&app.recent_path, s);
        app.recent_dirty = false;
    }
}

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
        let _ = fs::write(&app.install_path, s);
        app.install_dirty = false;
        app.last_install_change = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
    }

    #[test]
    /// What: maybe_flush_cache writes details_cache JSON and clears dirty flag
    ///
    /// - Input: AppState with cache_dirty=true and temp cache_path
    /// - Output: File written with JSON; cache_dirty=false
    fn flush_cache_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_cache_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
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
        let body = std::fs::read_to_string(&app.cache_path).unwrap();
        assert!(body.contains("ripgrep"));
        let _ = std::fs::remove_file(&app.cache_path);
    }

    #[test]
    /// What: maybe_flush_recent writes recent JSON and clears dirty flag
    ///
    /// - Input: AppState with recent_dirty=true and temp recent_path
    /// - Output: File contains the recent strings; recent_dirty=false
    fn flush_recent_writes_and_clears_flag() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_recent_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.recent_path = path.clone();
        app.recent = vec!["rg".into(), "fd".into()];
        app.recent_dirty = true;
        maybe_flush_recent(&mut app);
        assert!(!app.recent_dirty);
        let body = std::fs::read_to_string(&app.recent_path).unwrap();
        assert!(body.contains("rg") && body.contains("fd"));
        let _ = std::fs::remove_file(&app.recent_path);
    }

    #[test]
    /// What: maybe_flush_install throttles writes within 1s, then writes after delay
    ///
    /// - Input: AppState with install_dirty=true and last_install_change just now
    /// - Output: First call no write; after advancing timestamp, file written
    fn flush_install_throttle_and_write() {
        let mut app = new_app();
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_install_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        app.install_path = path.clone();
        app.install_list = vec![crate::state::PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.install_dirty = true;
        app.last_install_change = Some(std::time::Instant::now());
        // First call should be throttled -> no file
        maybe_flush_install(&mut app);
        assert!(std::fs::read_to_string(&app.install_path).is_err());
        // Simulate time passing by clearing last_install_change
        app.last_install_change = None;
        maybe_flush_install(&mut app);
        let body = std::fs::read_to_string(&app.install_path).unwrap();
        assert!(body.contains("rg"));
        let _ = std::fs::remove_file(&app.install_path);
    }
}
