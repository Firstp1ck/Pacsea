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
    if let Some(when) = app.last_install_change {
        if when.elapsed() < std::time::Duration::from_millis(1000) {
            return;
        }
    }
    if let Ok(s) = serde_json::to_string(&app.install_list) {
        let _ = fs::write(&app.install_path, s);
        app.install_dirty = false;
        app.last_install_change = None;
    }
}
