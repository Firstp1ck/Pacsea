use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use crate::theme::paths::{resolve_keybinds_config_path, resolve_settings_config_path};
use crate::theme::types::Settings;
use tracing::{debug, warn};

mod normalize;
mod parse_keybinds;
mod parse_settings;

use normalize::normalize;
use parse_keybinds::parse_keybinds;
use parse_settings::parse_settings;

struct SettingsCache {
    settings: Settings,
    settings_mtime: Option<SystemTime>,
    keybinds_mtime: Option<SystemTime>,
    initialized: bool,
}

impl SettingsCache {
    fn new() -> Self {
        Self {
            settings: Settings::default(),
            settings_mtime: None,
            keybinds_mtime: None,
            initialized: false,
        }
    }
}

static SETTINGS_CACHE: OnceLock<Mutex<SettingsCache>> = OnceLock::new();

/// What: Load user settings and keybinds from config files under HOME/XDG.
///
/// Inputs:
/// - None (reads `settings.conf` and `keybinds.conf` if present)
///
/// Output:
/// - A `Settings` value; falls back to `Settings::default()` when missing or invalid.
///
/// # Panics
/// - If the internal settings cache mutex is poisoned (unexpected).
#[must_use]
pub fn settings() -> Settings {
    let mut cache = SETTINGS_CACHE
        .get_or_init(|| Mutex::new(SettingsCache::new()))
        .lock()
        .expect("Settings cache mutex poisoned");

    let mut out = Settings::default();
    // Load settings from settings.conf (or legacy pacsea.conf)
    let settings_path = resolve_settings_config_path().or_else(|| {
        env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")))
            .map(|base| base.join("pacsea").join("settings.conf"))
    });

    let settings_mtime = settings_path
        .as_ref()
        .and_then(|p| fs::metadata(p).and_then(|m| m.modified()).ok());
    let keybinds_path = resolve_keybinds_config_path();
    let keybinds_mtime = keybinds_path
        .as_ref()
        .and_then(|p| fs::metadata(p).and_then(|m| m.modified()).ok());

    let cache_initialized = cache.initialized;
    let mtimes_match = cache_initialized
        && cache.settings_mtime == settings_mtime
        && cache.keybinds_mtime == keybinds_mtime;
    if mtimes_match {
        if tracing::enabled!(tracing::Level::TRACE) {
            debug!("[Config] Using cached settings (unchanged files)");
        }
        return cache.settings.clone();
    }

    if let Some(p) = settings_path.as_ref()
        && let Ok(content) = fs::read_to_string(p)
    {
        debug!(path = %p.display(), bytes = content.len(), "[Config] Loaded settings.conf");
        parse_settings(&content, p, &mut out);
    } else if let Some(p) = settings_path.as_ref() {
        warn!(
            path = %p.display(),
            "[Config] settings.conf missing or unreadable, using defaults"
        );
    }

    // Normalize settings
    normalize(&mut out);

    // Load keybinds from keybinds.conf if available; otherwise fall back to legacy keys in settings file
    if let Some(kp) = keybinds_path.as_ref() {
        if let Ok(content) = fs::read_to_string(kp) {
            debug!(path = %kp.display(), bytes = content.len(), "[Config] Loaded keybinds.conf");
            parse_keybinds(&content, &mut out);
            // Done; keybinds loaded from dedicated file, so we can return now after validation
        }
    } else if let Some(p) = settings_path.as_ref() {
        // Fallback: parse legacy keybind_* from settings file if keybinds.conf not present
        if let Ok(content) = fs::read_to_string(p) {
            debug!(
                path = %p.display(),
                bytes = content.len(),
                "[Config] Loaded legacy keybinds from settings.conf"
            );
            parse_keybinds(&content, &mut out);
        }
    }

    // Validate sum; if invalid, revert layout to defaults but preserve keybinds
    let sum = out
        .layout_left_pct
        .saturating_add(out.layout_center_pct)
        .saturating_add(out.layout_right_pct);
    if sum != 100
        || out.layout_left_pct == 0
        || out.layout_center_pct == 0
        || out.layout_right_pct == 0
    {
        // Preserve keybinds when resetting layout defaults
        let keymap = out.keymap.clone();
        out = Settings::default();
        out.keymap = keymap;
        debug!(
            layout_left = out.layout_left_pct,
            layout_center = out.layout_center_pct,
            layout_right = out.layout_right_pct,
            "[Config] Layout percentages invalid, reset to defaults while preserving keybinds"
        );
    }
    cache.settings_mtime = settings_mtime;
    cache.keybinds_mtime = keybinds_mtime;
    cache.settings = out.clone();
    cache.initialized = true;
    out
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Ensure settings parsing applies defaults when layout percentages sum incorrectly while still loading keybinds.
    ///
    /// Inputs:
    /// - Temporary configuration directory containing `settings.conf` with an invalid layout sum and `keybinds.conf` with overrides.
    ///
    /// Output:
    /// - Resulting `Settings` fall back to default layout percentages yet pick up configured keybinds.
    ///
    /// Details:
    /// - Overrides `HOME` to a temp dir and restores it afterwards to avoid polluting the user environment.
    fn settings_parse_values_and_keybinds_with_defaults_on_invalid_sum() {
        let _guard = crate::theme::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_settings_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let cfg = base.join(".config").join("pacsea");
        let _ = std::fs::create_dir_all(&cfg);
        unsafe { std::env::set_var("HOME", base.display().to_string()) };

        // Write settings.conf with values and bad sum (should reset to defaults)
        let settings_path = cfg.join("settings.conf");
        std::fs::write(
            &settings_path,
            "layout_left_pct=10\nlayout_center_pct=10\nlayout_right_pct=10\nsort_mode=aur_popularity\nclipboard_suffix=OK\nshow_search_history_pane=true\nshow_install_pane=false\nshow_keybinds_footer=true\n",
        )
        .expect("failed to write test settings file");
        // Write keybinds.conf
        let keybinds_path = cfg.join("keybinds.conf");
        std::fs::write(&keybinds_path, "keybind_exit = Ctrl+Q\nkeybind_help = F1\n")
            .expect("Failed to write test keybinds file");

        let s = super::settings();
        // Invalid layout sum -> defaults
        assert_eq!(
            s.layout_left_pct + s.layout_center_pct + s.layout_right_pct,
            100
        );
        // Keybinds parsed
        assert!(!s.keymap.exit.is_empty());
        assert!(!s.keymap.help_overlay.is_empty());

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
