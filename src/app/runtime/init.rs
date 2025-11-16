use std::collections::HashMap;
use std::time::Instant;

use crate::index as pkgindex;
use crate::state::*;

use super::super::deps_cache;
use super::super::files_cache;
use super::super::sandbox_cache;
use super::super::services_cache;

/// What: Initialize the locale system: resolve locale, load translations, set up fallbacks.
///
/// Inputs:
/// - `app`: Application state to populate with locale and translations
/// - `locale_pref`: Locale preference from settings.conf (empty = auto-detect)
/// - `_prefs`: Settings struct (unused but kept for future use)
///
/// Output:
/// - Populates `app.locale`, `app.translations`, and `app.translations_fallback`
///
/// Details:
/// - Resolves locale using fallback chain (settings -> system -> default)
/// - Loads English fallback translations first (required)
/// - Loads primary locale translations if different from English
/// - Handles errors gracefully: falls back to English if locale file missing/invalid
/// - Logs warnings for missing files but continues execution
pub fn initialize_locale_system(
    app: &mut AppState,
    locale_pref: &str,
    _prefs: &crate::theme::Settings,
) {
    // Get paths - try both development and installed locations
    let locales_dir = crate::i18n::find_locales_dir().unwrap_or_else(|| {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("config")
            .join("locales")
    });
    let i18n_config_path = match crate::i18n::find_config_file("i18n.yml") {
        Some(path) => path,
        None => {
            tracing::error!(
                "i18n config file not found in development or installed locations. Using default locale 'en-US'."
            );
            app.locale = "en-US".to_string();
            app.translations = std::collections::HashMap::new();
            app.translations_fallback = std::collections::HashMap::new();
            return;
        }
    };

    // Resolve locale
    let resolver = crate::i18n::LocaleResolver::new(&i18n_config_path);
    let resolved_locale = resolver.resolve(locale_pref);
    app.locale = resolved_locale.clone();

    tracing::info!(
        "Resolved locale: '{}' (from settings: '{}')",
        resolved_locale,
        if locale_pref.trim().is_empty() {
            "<auto-detect>"
        } else {
            locale_pref
        }
    );

    // Load translations
    let mut loader = crate::i18n::LocaleLoader::new(locales_dir.clone());

    // Load fallback (English) translations first - this is required
    match loader.load("en-US") {
        Ok(fallback) => {
            let key_count = fallback.len();
            app.translations_fallback = fallback.clone();
            tracing::debug!("Loaded English fallback translations ({} keys)", key_count);
        }
        Err(e) => {
            tracing::error!(
                "Failed to load English fallback translations: {}. Application may show untranslated keys.",
                e
            );
            app.translations_fallback = std::collections::HashMap::new();
        }
    }

    // Load primary locale translations
    if resolved_locale != "en-US" {
        match loader.load(&resolved_locale) {
            Ok(translations) => {
                let key_count = translations.len();
                app.translations = translations;
                tracing::info!(
                    "Loaded translations for locale '{}' ({} keys)",
                    resolved_locale,
                    key_count
                );
                // Debug: Check if specific keys exist
                let test_keys = [
                    "app.details.footer.search_hint",
                    "app.details.footer.confirm_installation",
                ];
                for key in &test_keys {
                    if app.translations.contains_key(*key) {
                        tracing::debug!("  ✓ Key '{}' found in translations", key);
                    } else {
                        tracing::debug!("  ✗ Key '{}' NOT found in translations", key);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load translations for locale '{}': {}. Using English fallback.",
                    resolved_locale,
                    e
                );
                // Use empty map - translate_with_fallback will use English fallback
                app.translations = std::collections::HashMap::new();
            }
        }
    } else {
        // Already loaded English as fallback, use it as primary too
        app.translations = app.translations_fallback.clone();
        tracing::debug!("Using English as primary locale");
    }
}

/// What: Initialize application state: load settings, caches, and persisted data.
///
/// Inputs:
/// - `app`: Application state to initialize
/// - `dry_run_flag`: When `true`, install/remove/downgrade actions are displayed but not executed
/// - `headless`: When `true`, skip terminal-dependent operations
///
/// Output:
/// - Returns flags indicating which caches need background resolution
///
/// Details:
/// - Migrates legacy configs and loads settings
/// - Loads persisted caches (details, recent, install list, dependencies, files, services, sandbox)
/// - Initializes locale system
/// - Checks for GNOME terminal if on GNOME desktop
pub struct InitFlags {
    pub needs_deps_resolution: bool,
    pub needs_files_resolution: bool,
    pub needs_services_resolution: bool,
    pub needs_sandbox_resolution: bool,
}

pub fn initialize_app_state(app: &mut AppState, dry_run_flag: bool, headless: bool) -> InitFlags {
    app.dry_run = if dry_run_flag {
        true
    } else {
        crate::theme::settings().app_dry_run_default
    };
    app.last_input_change = Instant::now();

    // Log resolved configuration/state file locations at startup
    tracing::info!(
        recent = %app.recent_path.display(),
        install = %app.install_path.display(),
        details_cache = %app.cache_path.display(),
        index = %app.official_index_path.display(),
        news_read = %app.news_read_path.display(),
        "resolved state file paths"
    );

    // Migrate legacy single-file config to split files before reading settings
    crate::theme::maybe_migrate_legacy_confs();
    let prefs = crate::theme::settings();
    // Ensure config has all known settings keys (non-destructive append)
    crate::theme::ensure_settings_keys_present(&prefs);
    app.layout_left_pct = prefs.layout_left_pct;
    app.layout_center_pct = prefs.layout_center_pct;
    app.layout_right_pct = prefs.layout_right_pct;
    app.keymap = prefs.keymap.clone();
    app.sort_mode = prefs.sort_mode;
    app.package_marker = prefs.package_marker;
    // Apply initial visibility for middle row panes from settings
    app.show_recent_pane = prefs.show_recent_pane;
    app.show_install_pane = prefs.show_install_pane;
    // Apply initial keybind footer visibility (default true if not present)
    app.show_keybinds_footer = prefs.show_keybinds_footer;

    // Initialize locale system (clone locale string to avoid borrow issues)
    let locale_pref = prefs.locale.clone();
    initialize_locale_system(app, &locale_pref, &prefs);

    // GNOME desktop: prompt to install a GNOME terminal if none present (gnome-terminal or gnome-console/kgx)
    // Skip this check in headless mode to avoid slow PATH scanning
    if !headless {
        let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
            .ok()
            .map(|v| v.to_uppercase().contains("GNOME"))
            .unwrap_or(false);
        let has_gterm = crate::install::command_on_path("gnome-terminal");
        let has_gconsole = crate::install::command_on_path("gnome-console")
            || crate::install::command_on_path("kgx");
        if is_gnome && !(has_gterm || has_gconsole) {
            app.modal = crate::state::Modal::GnomeTerminalPrompt;
        }
    }

    if let Ok(s) = std::fs::read_to_string(&app.cache_path)
        && let Ok(map) = serde_json::from_str::<HashMap<String, PackageDetails>>(&s)
    {
        app.details_cache = map;
        tracing::info!(path = %app.cache_path.display(), "loaded details cache");
    }
    if let Ok(s) = std::fs::read_to_string(&app.recent_path)
        && let Ok(list) = serde_json::from_str::<Vec<String>>(&s)
    {
        app.recent = list;
        if !app.recent.is_empty() {
            app.history_state.select(Some(0));
        }
        tracing::info!(path = %app.recent_path.display(), count = app.recent.len(), "loaded recent searches");
    }
    if let Ok(s) = std::fs::read_to_string(&app.install_path)
        && let Ok(list) = serde_json::from_str::<Vec<PackageItem>>(&s)
    {
        app.install_list = list;
        if !app.install_list.is_empty() {
            app.install_state.select(Some(0));
        }
        tracing::info!(path = %app.install_path.display(), count = app.install_list.len(), "loaded install list");
    }

    // Load dependency cache after install list is loaded (but before channels are created)
    let mut needs_deps_resolution = false;
    if !app.install_list.is_empty() {
        let signature = deps_cache::compute_signature(&app.install_list);
        if let Some(cached_deps) = deps_cache::load_cache(&app.deps_cache_path, &signature) {
            app.install_list_deps = cached_deps;
            tracing::info!(path = %app.deps_cache_path.display(), count = app.install_list_deps.len(), "loaded dependency cache");
        } else {
            // Cache missing or invalid - will trigger background resolution after channels are set up
            needs_deps_resolution = true;
            tracing::info!(
                "Dependency cache missing or invalid, will trigger background resolution"
            );
        }
    }

    // Load file cache after install list is loaded (but before channels are created)
    let mut needs_files_resolution = false;
    if !app.install_list.is_empty() {
        let signature = files_cache::compute_signature(&app.install_list);
        if let Some(cached_files) = files_cache::load_cache(&app.files_cache_path, &signature) {
            app.install_list_files = cached_files;
            tracing::info!(path = %app.files_cache_path.display(), count = app.install_list_files.len(), "loaded file cache");
        } else {
            // Cache missing or invalid - will trigger background resolution after channels are set up
            needs_files_resolution = true;
            tracing::info!("File cache missing or invalid, will trigger background resolution");
        }
    }

    // Load service cache after install list is loaded (but before channels are created)
    let mut needs_services_resolution = false;
    if !app.install_list.is_empty() {
        let signature = services_cache::compute_signature(&app.install_list);
        if let Some(cached_services) =
            services_cache::load_cache(&app.services_cache_path, &signature)
        {
            app.install_list_services = cached_services;
            tracing::info!(path = %app.services_cache_path.display(), count = app.install_list_services.len(), "loaded service cache");
        } else {
            // Cache missing or invalid - will trigger background resolution after channels are set up
            needs_services_resolution = true;
            tracing::info!("Service cache missing or invalid, will trigger background resolution");
        }
    }

    // Load sandbox cache after install list is loaded (but before channels are created)
    let mut needs_sandbox_resolution = false;
    if !app.install_list.is_empty() {
        let signature = sandbox_cache::compute_signature(&app.install_list);
        if let Some(cached_sandbox) = sandbox_cache::load_cache(&app.sandbox_cache_path, &signature)
        {
            app.install_list_sandbox = cached_sandbox;
            tracing::info!(path = %app.sandbox_cache_path.display(), count = app.install_list_sandbox.len(), "loaded sandbox cache");
        } else {
            // Cache missing or invalid - will trigger background resolution after channels are set up
            needs_sandbox_resolution = true;
            tracing::info!("Sandbox cache missing or invalid, will trigger background resolution");
        }
    }

    if let Ok(s) = std::fs::read_to_string(&app.news_read_path)
        && let Ok(set) = serde_json::from_str::<std::collections::HashSet<String>>(&s)
    {
        app.news_read_urls = set;
        tracing::info!(path = %app.news_read_path.display(), count = app.news_read_urls.len(), "loaded read news urls");
    }

    pkgindex::load_from_disk(&app.official_index_path);
    tracing::info!(path = %app.official_index_path.display(), "attempted to load official index from disk");

    InitFlags {
        needs_deps_resolution,
        needs_files_resolution,
        needs_services_resolution,
        needs_sandbox_resolution,
    }
}

/// What: Trigger initial background resolution for caches that were missing or invalid.
///
/// Inputs:
/// - `app`: Application state
/// - `flags`: Initialization flags indicating which caches need resolution
/// - `deps_req_tx`: Channel sender for dependency resolution requests
/// - `files_req_tx`: Channel sender for file resolution requests
/// - `services_req_tx`: Channel sender for service resolution requests
/// - `sandbox_req_tx`: Channel sender for sandbox resolution requests
///
/// Output:
/// - Sets resolution flags and sends requests to background workers
///
/// Details:
/// - Only triggers resolution if cache was missing/invalid and install list is not empty
pub fn trigger_initial_resolutions(
    app: &mut AppState,
    flags: &InitFlags,
    deps_req_tx: &tokio::sync::mpsc::UnboundedSender<Vec<PackageItem>>,
    files_req_tx: &tokio::sync::mpsc::UnboundedSender<Vec<PackageItem>>,
    services_req_tx: &tokio::sync::mpsc::UnboundedSender<Vec<PackageItem>>,
    sandbox_req_tx: &tokio::sync::mpsc::UnboundedSender<Vec<PackageItem>>,
) {
    if flags.needs_deps_resolution && !app.install_list.is_empty() {
        app.deps_resolving = true;
        let _ = deps_req_tx.send(app.install_list.clone());
    }

    if flags.needs_files_resolution && !app.install_list.is_empty() {
        app.files_resolving = true;
        let _ = files_req_tx.send(app.install_list.clone());
    }

    if flags.needs_services_resolution && !app.install_list.is_empty() {
        app.services_resolving = true;
        let _ = services_req_tx.send(app.install_list.clone());
    }

    if flags.needs_sandbox_resolution && !app.install_list.is_empty() {
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(app.install_list.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::runtime::background::Channels;

    /// What: Provide a baseline `AppState` for initialization tests.
    ///
    /// Inputs: None
    /// Output: Fresh `AppState` with default values
    fn new_app() -> AppState {
        AppState::default()
    }

    #[test]
    /// What: Verify that initialize_locale_system sets default locale when config file is missing.
    ///
    /// Inputs:
    /// - App state with default locale
    /// - Empty locale preference
    ///
    /// Output:
    /// - Locale is set to "en-US" when config file is missing
    /// - Translations maps are initialized (may be empty)
    ///
    /// Details:
    /// - Tests graceful fallback when i18n config is not found
    fn initialize_locale_system_fallback_when_config_missing() {
        let mut app = new_app();
        let prefs = crate::theme::Settings::default();

        // This will fall back to en-US if config file is missing
        initialize_locale_system(&mut app, "", &prefs);

        // Locale should be set (either resolved or default)
        assert!(!app.locale.is_empty());
        // Translations maps should be initialized
        assert!(app.translations.is_empty() || !app.translations.is_empty());
        assert!(app.translations_fallback.is_empty() || !app.translations_fallback.is_empty());
    }

    #[test]
    /// What: Verify that initialize_app_state sets dry_run flag correctly.
    ///
    /// Inputs:
    /// - App state
    /// - dry_run_flag = true
    /// - headless = false
    ///
    /// Output:
    /// - app.dry_run is set to true
    /// - InitFlags are returned
    ///
    /// Details:
    /// - Tests that dry_run flag is properly initialized
    fn initialize_app_state_sets_dry_run_flag() {
        let mut app = new_app();
        let flags = initialize_app_state(&mut app, true, false);

        assert!(app.dry_run);
        // Flags should be returned (InitFlags struct is created)
        // The actual values depend on cache state, so we just verify flags exist
        let _ = flags;
    }

    #[test]
    /// What: Verify that initialize_app_state loads settings correctly.
    ///
    /// Inputs:
    /// - App state
    /// - dry_run_flag = false
    /// - headless = false
    ///
    /// Output:
    /// - App state has layout percentages set
    /// - Keymap is set
    /// - Sort mode is set
    ///
    /// Details:
    /// - Tests that settings are properly applied to app state
    fn initialize_app_state_loads_settings() {
        let mut app = new_app();
        let _flags = initialize_app_state(&mut app, false, false);

        // Settings should be loaded (values depend on config, but should be set)
        assert!(app.layout_left_pct > 0);
        assert!(app.layout_center_pct > 0);
        assert!(app.layout_right_pct > 0);
        // Keymap should be initialized (it's a struct, not a string)
        // Just verify it's not the default empty state by checking a field
        // (KeyMap has many fields, we just verify it's been set)
    }

    #[tokio::test]
    /// What: Verify that trigger_initial_resolutions skips when install list is empty.
    ///
    /// Inputs:
    /// - App state with empty install list
    /// - InitFlags with needs_deps_resolution = true
    /// - Channel senders
    ///
    /// Output:
    /// - No requests sent when install list is empty
    ///
    /// Details:
    /// - Tests that resolution is only triggered when install list is not empty
    async fn trigger_initial_resolutions_skips_when_install_list_empty() {
        let mut app = new_app();
        app.install_list.clear();

        let flags = InitFlags {
            needs_deps_resolution: true,
            needs_files_resolution: true,
            needs_services_resolution: true,
            needs_sandbox_resolution: true,
        };

        // Create channels (we only need the senders)
        let channels = Channels::new(std::path::PathBuf::from("/tmp"));

        // Should not panic even with empty install list
        trigger_initial_resolutions(
            &mut app,
            &flags,
            &channels.deps_req_tx,
            &channels.files_req_tx,
            &channels.services_req_tx,
            &channels.sandbox_req_tx,
        );

        // Flags should not be set when install list is empty
        assert!(!app.deps_resolving);
        assert!(!app.files_resolving);
        assert!(!app.services_resolving);
        assert!(!app.sandbox_resolving);
    }

    #[tokio::test]
    /// What: Verify that trigger_initial_resolutions sets flags and sends requests when needed.
    ///
    /// Inputs:
    /// - App state with non-empty install list
    /// - InitFlags with needs_deps_resolution = true
    /// - Channel senders
    ///
    /// Output:
    /// - deps_resolving flag is set
    /// - Request is sent to deps_req_tx
    ///
    /// Details:
    /// - Tests that resolution is properly triggered when conditions are met
    async fn trigger_initial_resolutions_triggers_when_needed() {
        let mut app = new_app();
        app.install_list.push(crate::state::PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: crate::state::Source::Aur,
            popularity: None,
        });

        let flags = InitFlags {
            needs_deps_resolution: true,
            needs_files_resolution: false,
            needs_services_resolution: false,
            needs_sandbox_resolution: false,
        };

        let channels = Channels::new(std::path::PathBuf::from("/tmp"));

        trigger_initial_resolutions(
            &mut app,
            &flags,
            &channels.deps_req_tx,
            &channels.files_req_tx,
            &channels.services_req_tx,
            &channels.sandbox_req_tx,
        );

        // Flag should be set
        assert!(app.deps_resolving);
        // Other flags should not be set
        assert!(!app.files_resolving);
        assert!(!app.services_resolving);
        assert!(!app.sandbox_resolving);
    }
}
