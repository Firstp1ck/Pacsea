//! Theme system for Pacsea.
//!
//! Split from a monolithic file into submodules for maintainability. Public
//! re-exports keep the `crate::theme::*` API stable.

mod config;
mod parsing;
mod paths;
mod settings;
mod store;
mod types;

pub use config::{
    ensure_settings_keys_present, maybe_migrate_legacy_confs, save_mirror_count,
    save_scan_do_clamav, save_scan_do_custom, save_scan_do_semgrep, save_scan_do_shellcheck,
    save_scan_do_sleuth, save_scan_do_trivy, save_scan_do_virustotal, save_selected_countries,
    save_show_install_pane, save_show_keybinds_footer, save_show_recent_pane, save_sort_mode,
    save_virustotal_api_key,
};
pub use paths::{config_dir, lists_dir, logs_dir};
pub use settings::settings;
pub use store::{reload_theme, theme};
pub use types::{KeyChord, KeyMap, Settings, Theme};

#[cfg(test)]
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
