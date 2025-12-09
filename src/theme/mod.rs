//! Theme system for Pacsea.
//!
//! Split from a monolithic file into submodules for maintainability. Public
//! re-exports keep the `crate::theme::*` API stable.

/// Configuration file management and migration.
mod config;
/// Configuration parsing utilities.
mod parsing;
/// Path resolution for config directories.
mod paths;
/// Settings access and management.
mod settings;
/// Theme store and caching.
mod store;
/// Theme type definitions.
mod types;

pub use config::{
    ensure_settings_keys_present, maybe_migrate_legacy_confs, save_app_start_mode,
    save_fuzzy_search, save_mirror_count, save_news_filter_installed_only,
    save_news_filter_show_advisories, save_news_filter_show_arch_news,
    save_news_filter_show_aur_comments, save_news_filter_show_aur_updates,
    save_news_filter_show_pkg_updates, save_news_max_age_days, save_scan_do_clamav,
    save_scan_do_custom, save_scan_do_semgrep, save_scan_do_shellcheck, save_scan_do_sleuth,
    save_scan_do_trivy, save_scan_do_virustotal, save_selected_countries, save_show_install_pane,
    save_show_keybinds_footer, save_show_recent_pane, save_sort_mode, save_startup_news_configured,
    save_startup_news_max_age_days, save_startup_news_show_advisories,
    save_startup_news_show_arch_news, save_startup_news_show_aur_comments,
    save_startup_news_show_aur_updates, save_startup_news_show_pkg_updates,
    save_virustotal_api_key,
};
pub use paths::{config_dir, lists_dir, logs_dir};
pub use settings::settings;
pub use store::{reload_theme, theme};
pub use types::{KeyChord, KeyMap, PackageMarker, Settings, Theme};

#[cfg(test)]
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
/// What: Provide a process-wide mutex to serialize filesystem-mutating tests in this module.
///
/// Inputs:
/// - None
///
/// Output:
/// - Shared reference to a lazily-initialized `Mutex<()>`.
///
/// Details:
/// - Uses `OnceLock` to ensure the mutex is constructed exactly once per process.
/// - Callers should lock the mutex to guard environment-variable or disk state changes.
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
