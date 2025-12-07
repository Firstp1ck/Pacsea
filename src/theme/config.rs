//! Configuration module for Pacsea theme, settings, and keybinds.
//!
//! This module is split into submodules for maintainability:
//! - `skeletons`: Default configuration file templates
//! - `theme_loader`: Theme loading and parsing
//! - `settings_save`: Functions to persist settings changes
//! - `settings_ensure`: Settings initialization and migration
//! - `tests`: Test module

/// Settings initialization and migration module.
mod settings_ensure;
/// Settings persistence module.
mod settings_save;
/// Default configuration file templates module.
mod skeletons;
/// Theme loading and parsing module.
pub mod theme_loader;

#[cfg(test)]
mod tests;

// Re-export skeleton constants (only THEME_SKELETON_CONTENT is used externally)
pub use skeletons::THEME_SKELETON_CONTENT;

// Re-export theme loading functions
pub use theme_loader::{load_theme_from_file, try_load_theme_with_diagnostics};

// Re-export settings save functions
pub use settings_save::{
    save_fuzzy_search, save_mirror_count, save_scan_do_clamav, save_scan_do_custom,
    save_scan_do_semgrep, save_scan_do_shellcheck, save_scan_do_sleuth, save_scan_do_trivy,
    save_scan_do_virustotal, save_selected_countries, save_show_install_pane,
    save_show_keybinds_footer, save_show_recent_pane, save_sort_mode, save_virustotal_api_key,
};

// Re-export settings ensure/migration functions
pub use settings_ensure::{ensure_settings_keys_present, maybe_migrate_legacy_confs};
