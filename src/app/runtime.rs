use std::collections::HashMap;
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use crossterm::event::Event as CEvent;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::{
    select,
    sync::mpsc,
    time::{Duration, sleep},
};

use crate::index as pkgindex;
use crate::logic::{add_to_install_list, send_query};
use crate::sources;
use crate::sources::fetch_details;
use crate::state::*;
use crate::ui::ui;
use crate::util::{match_rank, repo_order};

use super::deps_cache;
use super::files_cache;
use super::persist::{
    maybe_flush_cache, maybe_flush_deps_cache, maybe_flush_files_cache, maybe_flush_install,
    maybe_flush_news_read, maybe_flush_recent, maybe_flush_sandbox_cache,
    maybe_flush_services_cache,
};
use super::recent::maybe_save_recent;
use super::sandbox_cache;

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
fn initialize_locale_system(
    app: &mut AppState,
    locale_pref: &str,
    _prefs: &crate::theme::Settings,
) {
    // Get paths
    let locales_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    let i18n_config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .join("i18n.yml");

    // Validate i18n config file exists
    if !i18n_config_path.exists() {
        tracing::error!(
            "i18n config file not found: {}. Using default locale 'en-US'.",
            i18n_config_path.display()
        );
        app.locale = "en-US".to_string();
        app.translations = std::collections::HashMap::new();
        app.translations_fallback = std::collections::HashMap::new();
        return;
    }

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
use super::services_cache;
use super::terminal::{restore_terminal, setup_terminal};

/// What: Run the Pacsea TUI application end-to-end: initialize terminal and state, spawn
/// background workers (index, search, details, status/news), drive the event loop, persist
/// caches, and restore the terminal on exit.
///
/// Inputs:
/// - `dry_run_flag`: When `true`, install/remove/downgrade actions are displayed but not executed
///   (overrides the config default for the session).
///
/// Output:
/// - `Ok(())` when the UI exits cleanly; `Err` on unrecoverable terminal or runtime errors.
///
/// Details:
/// - Config/state: Migrates legacy configs, loads settings (layout, keymap, sort), and reads
///   persisted files (details cache, recent queries, install list, on-disk official index).
/// - Background tasks: Spawns channels and tasks for batched details fetch, AUR/official search,
///   PKGBUILD retrieval, official index refresh/enrichment, Arch status text, and Arch news.
/// - Event loop: Renders UI frames and handles keyboard, mouse, tick, and channel messages to
///   update results, details, ring-prefetch, PKGBUILD viewer, installed-only mode, and modals.
/// - Persistence: Debounces and periodically writes recent, details cache, and install list.
/// - Cleanup: Flushes pending writes and restores terminal modes before returning.
pub async fn run(dry_run_flag: bool) -> Result<()> {
    let headless = std::env::var("PACSEA_TEST_HEADLESS").ok().as_deref() == Some("1");
    if !headless {
        setup_terminal()?;
    }
    let mut terminal = if headless {
        None
    } else {
        Some(Terminal::new(CrosstermBackend::new(std::io::stdout()))?)
    };

    let mut app = AppState {
        dry_run: if dry_run_flag {
            true
        } else {
            crate::theme::settings().app_dry_run_default
        },
        last_input_change: Instant::now(),
        ..Default::default()
    };

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
    initialize_locale_system(&mut app, &locale_pref, &prefs);

    // GNOME desktop: prompt to install a GNOME terminal if none present (gnome-terminal or gnome-console/kgx)
    let is_gnome = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .map(|v| v.to_uppercase().contains("GNOME"))
        .unwrap_or(false);
    let has_gterm = crate::install::command_on_path("gnome-terminal");
    let has_gconsole =
        crate::install::command_on_path("gnome-console") || crate::install::command_on_path("kgx");
    if is_gnome && !(has_gterm || has_gconsole) {
        app.modal = crate::state::Modal::GnomeTerminalPrompt;
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

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<CEvent>();
    let (search_result_tx, mut results_rx) = mpsc::unbounded_channel::<SearchResults>();
    let (details_req_tx, mut details_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (details_res_tx, mut details_res_rx) = mpsc::unbounded_channel::<PackageDetails>();
    let (tick_tx, mut tick_rx) = mpsc::unbounded_channel::<()>();
    let (net_err_tx, mut net_err_rx) = mpsc::unbounded_channel::<String>();
    let (preview_tx, mut preview_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (add_tx, mut add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (index_notify_tx, mut index_notify_rx) = mpsc::unbounded_channel::<()>();
    let (pkgb_req_tx, mut pkgb_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_res_tx, mut pkgb_res_rx) = mpsc::unbounded_channel::<(String, String)>();
    let (status_tx, mut status_rx) =
        mpsc::unbounded_channel::<(String, crate::state::ArchStatusColor)>();
    let (news_tx, mut news_rx) = mpsc::unbounded_channel::<Vec<NewsItem>>();
    let (deps_req_tx, mut deps_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (deps_res_tx, mut deps_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::DependencyInfo>>();
    let (files_req_tx, mut files_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (files_res_tx, mut files_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::PackageFileInfo>>();
    let (services_req_tx, mut services_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (services_res_tx, mut services_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::state::modal::ServiceImpact>>();
    let (sandbox_req_tx, mut sandbox_req_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (sandbox_res_tx, mut sandbox_res_rx) =
        mpsc::unbounded_channel::<Vec<crate::logic::sandbox::SandboxInfo>>();

    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_BATCH_WINDOW_MS: u64 = 120;
        loop {
            let first = match details_req_rx.recv().await {
                Some(i) => i,
                None => break,
            };
            let mut batch: Vec<PackageItem> = vec![first];
            loop {
                tokio::select! {
                    Some(next) = details_req_rx.recv() => { batch.push(next); }
                    _ = sleep(Duration::from_millis(DETAILS_BATCH_WINDOW_MS)) => { break; }
                }
            }
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<PackageItem> = Vec::with_capacity(batch.len());
            for it in batch.into_iter() {
                if seen.insert(it.name.clone()) {
                    ordered.push(it);
                }
            }
            for it in ordered.into_iter() {
                if !crate::logic::is_allowed(&it.name) {
                    continue;
                }
                match fetch_details(it.clone()).await {
                    Ok(details) => {
                        let _ = details_res_tx.send(details);
                    }
                    Err(e) => {
                        let msg = match it.source {
                            Source::Official { .. } => format!(
                                "Official package details unavailable for {}: {}",
                                it.name, e
                            ),
                            Source::Aur => {
                                format!("AUR package details unavailable for {}: {}", it.name, e)
                            }
                        };
                        let _ = net_err_tx_details.send(msg);
                    }
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(item) = pkgb_req_rx.recv().await {
            let name = item.name.clone();
            match sources::fetch_pkgbuild_fast(&item).await {
                Ok(txt) => {
                    let _ = pkgb_res_tx.send((name, txt));
                }
                Err(e) => {
                    let _ = pkgb_res_tx.send((name, format!("Failed to fetch PKGBUILD: {e}")));
                }
            }
        }
    });

    // Fetch Arch status text in background once at startup, then occasionally refresh
    let status_tx_once = status_tx.clone();
    tokio::spawn(async move {
        if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
            let _ = status_tx_once.send((txt, color));
        }
    });

    // Periodically refresh Arch status every 120 seconds
    let status_tx_periodic = status_tx.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(120)).await;
            if let Ok((txt, color)) = sources::fetch_arch_status_text().await {
                let _ = status_tx_periodic.send((txt, color));
            }
        }
    });

    // Background dependency resolution worker
    let deps_res_tx_bg = deps_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = deps_req_rx.recv().await {
            // Run blocking dependency resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = deps_res_tx_bg.clone();
            let res_tx_error = deps_res_tx_bg.clone(); // Clone for error handling
            let handle = tokio::task::spawn_blocking(move || {
                let deps = crate::logic::deps::resolve_dependencies(&items_clone);
                let _ = res_tx.send(deps);
            });
            // CRITICAL: Always await and send a result, even if task panics
            // This ensures deps_resolving flag is always reset
            tokio::spawn(async move {
                match handle.await {
                    Ok(_) => {
                        // Task completed successfully, result already sent
                        tracing::debug!("[Runtime] Dependency resolution task completed");
                    }
                    Err(e) => {
                        // Task panicked - send empty result to reset flag
                        tracing::error!("[Runtime] Dependency resolution task panicked: {:?}", e);
                        let _ = res_tx_error.send(Vec::new());
                    }
                }
            });
        }
        tracing::debug!("[Runtime] Dependency resolution worker exiting (channel closed)");
    });

    // Background file resolution worker
    let files_res_tx_bg = files_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = files_req_rx.recv().await {
            // Run blocking file resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = files_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let files = crate::logic::files::resolve_file_changes(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                let _ = res_tx.send(files);
            });
        }
    });

    // Background service impact resolution worker
    let services_res_tx_bg = services_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = services_req_rx.recv().await {
            // Run blocking service resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = services_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let services = crate::logic::services::resolve_service_impacts(
                    &items_clone,
                    crate::state::modal::PreflightAction::Install,
                );
                let _ = res_tx.send(services);
            });
        }
    });

    // Background sandbox resolution worker
    let sandbox_res_tx_bg = sandbox_res_tx.clone();
    tokio::spawn(async move {
        while let Some(items) = sandbox_req_rx.recv().await {
            // Run blocking sandbox resolution in a thread pool
            let items_clone = items.clone();
            let res_tx = sandbox_res_tx_bg.clone();
            tokio::task::spawn_blocking(move || {
                let sandbox_info = crate::logic::sandbox::resolve_sandbox_info(&items_clone);
                let _ = res_tx.send(sandbox_info);
            });
        }
    });

    // Fetch Arch news once at startup; show unread items (by URL) if any
    let news_tx_once = news_tx.clone();
    let read_set = app.news_read_urls.clone();
    tokio::spawn(async move {
        if let Ok(list) = sources::fetch_arch_news(10).await {
            let unread: Vec<NewsItem> = list
                .into_iter()
                .filter(|it| !read_set.contains(&it.url))
                .collect();
            let _ = news_tx_once.send(unread);
        }
    });

    #[cfg(windows)]
    {
        // Save mirrors into the repository directory in the source tree and build the index via Arch API
        let repo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("repository");
        crate::index::refresh_windows_mirrors_and_index(
            app.official_index_path.clone(),
            repo_dir,
            net_err_tx.clone(),
            index_notify_tx.clone(),
        )
        .await;
    }
    #[cfg(not(windows))]
    {
        pkgindex::update_in_background(
            app.official_index_path.clone(),
            net_err_tx.clone(),
            index_notify_tx.clone(),
        )
        .await;
    }

    pkgindex::refresh_installed_cache().await;
    pkgindex::refresh_explicit_cache().await;

    // Trigger background dependency resolution if cache was missing/invalid
    if needs_deps_resolution && !app.install_list.is_empty() {
        app.deps_resolving = true;
        let _ = deps_req_tx.send(app.install_list.clone());
    }

    if needs_files_resolution && !app.install_list.is_empty() {
        app.files_resolving = true;
        let _ = files_req_tx.send(app.install_list.clone());
    }

    if needs_services_resolution && !app.install_list.is_empty() {
        app.services_resolving = true;
        let _ = services_req_tx.send(app.install_list.clone());
    }

    if needs_sandbox_resolution && !app.install_list.is_empty() {
        app.sandbox_resolving = true;
        let _ = sandbox_req_tx.send(app.install_list.clone());
    }

    if !headless {
        std::thread::spawn(move || {
            loop {
                match crossterm::event::read() {
                    Ok(ev) => {
                        let _ = event_tx.send(ev);
                    }
                    Err(_) => {
                        // ignore transient read errors and continue
                    }
                }
            }
        });
    }

    let tick_tx_bg = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            let _ = tick_tx_bg.send(());
        }
    });

    let (query_tx, mut query_rx) = mpsc::unbounded_channel::<QueryInput>();
    let net_err_tx_search = net_err_tx.clone();
    let index_path = app.official_index_path.clone();
    tokio::spawn(async move {
        const DEBOUNCE_MS: u64 = 250;
        const MIN_INTERVAL_MS: u64 = 300;
        let mut last_sent = Instant::now() - Duration::from_millis(MIN_INTERVAL_MS);
        loop {
            let mut latest = match query_rx.recv().await {
                Some(q) => q,
                None => break,
            };
            loop {
                select! { Some(new_q) = query_rx.recv() => { latest = new_q; } _ = sleep(Duration::from_millis(DEBOUNCE_MS)) => { break; } }
            }
            if latest.text.trim().is_empty() {
                let mut items = pkgindex::all_official_or_fetch(&index_path).await;
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                // Deduplicate by package name, preferring earlier entries (core > extra > others)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                let _ = search_result_tx.send(SearchResults {
                    id: latest.id,
                    items,
                });
                continue;
            }
            let elapsed = last_sent.elapsed();
            if elapsed < Duration::from_millis(MIN_INTERVAL_MS) {
                sleep(Duration::from_millis(MIN_INTERVAL_MS) - elapsed).await;
            }
            last_sent = Instant::now();

            let qtext = latest.text.clone();
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                if crate::index::all_official().is_empty() {
                    let _ = crate::index::all_official_or_fetch(&ipath).await;
                }
                let mut items = pkgindex::search_official(&qtext);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = sources::fetch_all_with_errors(q_for_net).await;
                items.extend(aur_items);
                let ql = qtext.trim().to_lowercase();
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    let ra = match_rank(&a.name, &ql);
                    let rb = match_rank(&b.name, &ql);
                    if ra != rb {
                        return ra.cmp(&rb);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                // Deduplicate by package name, preferring earlier entries (official over AUR)
                {
                    use std::collections::HashSet;
                    let mut seen = HashSet::new();
                    items.retain(|p| seen.insert(p.name.to_lowercase()));
                }
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults { id: sid, items });
            });
        }
    });

    send_query(&mut app, &query_tx);

    loop {
        if let Some(t) = terminal.as_mut() {
            let _ = t.draw(|f| ui(f, &mut app));
        }

        select! {
            Some(ev) = event_rx.recv() => { if crate::events::handle_event(ev, &mut app, &query_tx, &details_req_tx, &preview_tx, &add_tx, &pkgb_req_tx) { break; } }
            Some(_) = index_notify_rx.recv() => {
                app.loading_index = false;
                let _ = tick_tx.send(());
            }
            Some(new_results) = results_rx.recv() => {
                if new_results.id != app.latest_query_id { continue; }
                let prev_selected_name = app.results.get(app.selected).map(|p| p.name.clone());
                // Respect installed-only mode: keep results restricted to explicit installs
                let mut incoming = new_results.items;
                if app.installed_only_mode {
                    let explicit = crate::index::explicit_names();
                    if app.input.trim().is_empty() {
                        // For empty query, reconstruct full installed list (official + AUR fallbacks)
                        let mut items: Vec<PackageItem> = crate::index::all_official()
                            .into_iter()
                            .filter(|p| explicit.contains(&p.name))
                            .collect();
                        use std::collections::HashSet;
                        let official_names: HashSet<String> =
                            items.iter().map(|p| p.name.clone()).collect();
                        for name in explicit.into_iter() {
                            if !official_names.contains(&name) {
                                let is_eos = name.to_lowercase().contains("eos-");
                                let src = if is_eos {
                                    Source::Official { repo: "EOS".to_string(), arch: String::new() }
                                } else {
                                    Source::Aur
                                };
                                items.push(PackageItem {
                                    name: name.clone(),
                                    version: String::new(),
                                    description: String::new(),
                                    source: src,
                                    popularity: None,
                                });
                            }
                        }
                        incoming = items;
                    } else {
                        // For non-empty query, just intersect results with explicit installed set
                        incoming.retain(|p| explicit.contains(&p.name));
                    }
                }
                app.all_results = incoming;
                crate::logic::apply_filters_and_sort_preserve_selection(&mut app);
                let new_sel = prev_selected_name
                    .and_then(|name| app.results.iter().position(|p| p.name == name))
                    .unwrap_or(0);
                app.selected = new_sel.min(app.results.len().saturating_sub(1));
                app.list_state.select(if app.results.is_empty(){None}else{Some(app.selected)});
                if let Some(item) = app.results.get(app.selected).cloned() {
                    app.details_focus = Some(item.name.clone());
                    if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item.clone()); }
                }
                crate::logic::set_allowed_ring(&app, 30);
                if app.need_ring_prefetch { /* defer */ } else { crate::logic::ring_prefetch_from_selected(&mut app, &details_req_tx); }
                let len_u = app.results.len();
                let mut enrich_names: Vec<String> = Vec::new();
                if let Some(sel) = app.results.get(app.selected) && matches!(sel.source, Source::Official { .. }) { enrich_names.push(sel.name.clone()); }
                let max_radius: usize = 30; let mut step: usize = 1; while step <= max_radius { if let Some(i) = app.selected.checked_sub(step) && let Some(it) = app.results.get(i) && matches!(it.source, Source::Official { .. }) { enrich_names.push(it.name.clone()); } let below = app.selected + step; if below < len_u && let Some(it) = app.results.get(below) && matches!(it.source, Source::Official { .. }) { enrich_names.push(it.name.clone()); } step += 1; }
                if !enrich_names.is_empty() { crate::index::request_enrich_for(app.official_index_path.clone(), index_notify_tx.clone(), enrich_names); }
            }
            Some(details) = details_res_rx.recv() => {
                if app.details_focus.as_deref() == Some(details.name.as_str()) {
                    app.details = details.clone();
                }
                app.details_cache.insert(details.name.clone(), details.clone());
                app.cache_dirty = true;
                if let Some(pos) = app.results.iter().position(|p| p.name == details.name) {
                    app.results[pos].description = details.description.clone();
                    if !details.version.is_empty() && app.results[pos].version != details.version { app.results[pos].version = details.version.clone(); }
                    if details.popularity.is_some() { app.results[pos].popularity = details.popularity; }
                    if let crate::state::Source::Official { repo, arch } = &mut app.results[pos].source {
                        if repo.is_empty() && !details.repository.is_empty() { *repo = details.repository.clone(); }
                        if arch.is_empty() && !details.architecture.is_empty() { *arch = details.architecture.clone(); }
                    }
                }
                let _ = tick_tx.send(());
            }
            Some(item) = preview_rx.recv() => {
                if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item.clone()); }
                if !app.results.is_empty() && app.selected >= app.results.len() { app.selected = app.results.len() - 1; app.list_state.select(Some(app.selected)); }
            }
            Some(first) = add_rx.recv() => {
                // Batch-drain imported items arriving close together to avoid
                // repeated redraws and disk writes. Limit batch window to ~50ms.
                let mut batch = vec![first];
                loop {
                    match add_rx.try_recv() {
                        Ok(it) => batch.push(it),
                        Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                        Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                    }
                }
                for it in batch.into_iter() {
                    add_to_install_list(&mut app, it);
                }
                // Trigger background dependency resolution for updated install list
                if !app.install_list.is_empty() {
                    app.deps_resolving = true;
                    let _ = deps_req_tx.send(app.install_list.clone());
                    // Trigger background file resolution for updated install list
                    app.files_resolving = true;
                    let _ = files_req_tx.send(app.install_list.clone());
                    // Trigger background service resolution for updated install list
                    app.services_resolving = true;
                    let _ = services_req_tx.send(app.install_list.clone());
                    // Trigger background sandbox resolution for updated install list
                    app.sandbox_resolving = true;
                    let _ = sandbox_req_tx.send(app.install_list.clone());
                }
            }
            Some(deps) = deps_res_rx.recv() => {
                // Update cached dependencies
                // Always reset deps_resolving flag, even if result is empty (which indicates failure)
                tracing::debug!("[Runtime] Received dependency resolution results: {} deps", deps.len());
                app.install_list_deps = deps;
                app.deps_resolving = false; // CRITICAL: Always reset this flag when we receive ANY result
                app.deps_cache_dirty = true; // Mark cache as dirty for persistence
                let _ = tick_tx.send(());
            }
            Some(files) = files_res_rx.recv() => {
                // Update cached files
                app.install_list_files = files;
                app.files_resolving = false;
                app.files_cache_dirty = true; // Mark cache as dirty for persistence
                let _ = tick_tx.send(());
            }
            Some(services) = services_res_rx.recv() => {
                // Update cached services
                app.install_list_services = services;
                app.services_resolving = false;
                app.services_cache_dirty = true; // Mark cache as dirty for persistence
                let _ = tick_tx.send(());
            }
            Some(sandbox_info) = sandbox_res_rx.recv() => {
                // Update cached sandbox info
                app.install_list_sandbox = sandbox_info;
                app.sandbox_resolving = false;
                app.sandbox_cache_dirty = true; // Mark cache as dirty for persistence
                let _ = tick_tx.send(());
            }
            Some((pkgname, text)) = pkgb_res_rx.recv() => {
                if app.details_focus.as_deref() == Some(pkgname.as_str()) || app.results.get(app.selected).map(|i| i.name.as_str()) == Some(pkgname.as_str()) {
                    app.pkgb_text = Some(text);
                    app.pkgb_package_name = Some(pkgname);
                    // Clear any pending debounce request since we've successfully loaded
                    app.pkgb_reload_requested_at = None;
                    app.pkgb_reload_requested_for = None;
                }
                let _ = tick_tx.send(());
            }
            Some(msg) = net_err_rx.recv() => { app.modal = Modal::Alert { message: msg }; }
            Some(_) = tick_rx.recv() => { maybe_save_recent(&mut app); maybe_flush_cache(&mut app); maybe_flush_recent(&mut app); maybe_flush_news_read(&mut app); maybe_flush_install(&mut app); maybe_flush_deps_cache(&mut app); maybe_flush_files_cache(&mut app); maybe_flush_services_cache(&mut app); maybe_flush_sandbox_cache(&mut app);
                // Check for pending PKGBUILD reload request (debounce delay)
                const PKGBUILD_DEBOUNCE_MS: u64 = 250;
                if let (Some(requested_at), Some(requested_for)) = (app.pkgb_reload_requested_at, &app.pkgb_reload_requested_for) {
                    let elapsed = requested_at.elapsed();
                    if elapsed.as_millis() >= PKGBUILD_DEBOUNCE_MS as u128 {
                        // Check if the requested package is still the currently selected one
                        if let Some(current_item) = app.results.get(app.selected)
                            && current_item.name == *requested_for
                        {
                            // Still on the same package, actually send the request
                            let _ = pkgb_req_tx.send(current_item.clone());
                        }
                        // Clear the pending request
                        app.pkgb_reload_requested_at = None;
                        app.pkgb_reload_requested_for = None;
                    }
                }
                // If we recently triggered install/remove, poll installed/explicit caches briefly
                if let Some(deadline) = app.refresh_installed_until {
                    let now = Instant::now();
                    if now >= deadline {
                        app.refresh_installed_until = None;
                        app.next_installed_refresh_at = None;
                        app.pending_install_names = None;
                    } else {
                        let should_poll = app
                            .next_installed_refresh_at
                            .map(|t| now >= t)
                            .unwrap_or(true);
                        if should_poll {
                            let maybe_pending_installs = app.pending_install_names.clone();
                            let maybe_pending_removes = app.pending_remove_names.clone();
                            tokio::spawn(async move {
                                // Refresh caches in background; ignore errors
                                crate::index::refresh_installed_cache().await;
                                crate::index::refresh_explicit_cache().await;
                            });
                            // Schedule next poll ~1s later
                            app.next_installed_refresh_at = Some(now + Duration::from_millis(1000));
                            // If installed-only mode, results depend on explicit set; re-run query soon
                            send_query(&mut app, &query_tx);
                            // If we are tracking pending installs, check if all are installed now
                            if let Some(pending) = maybe_pending_installs {
                                let all_installed = pending
                                    .iter()
                                    .all(|n| crate::index::is_installed(n));
                                if all_installed {
                                    // Clear install list and stop tracking
                                    app.install_list.clear();
                                    app.install_dirty = true;
                                    app.pending_install_names = None;
                                    // Clear dependency cache when install list is cleared
                                    app.install_list_deps.clear();
                                    app.install_list_files.clear();
                                    app.deps_resolving = false;
                                    app.files_resolving = false;
                                    // End polling soon to avoid extra work
                                    app.refresh_installed_until = Some(now + Duration::from_secs(1));
                                }
                            }
                            // If tracking pending removals, log once all are uninstalled
                            if let Some(pending_rm) = maybe_pending_removes {
                                let all_removed = pending_rm
                                    .iter()
                                    .all(|n| !crate::index::is_installed(n));
                                if all_removed {
                                    if let Err(e) = crate::install::log_removed(&pending_rm) {
                                        let _ = e; // ignore logging errors
                                    }
                                    app.pending_remove_names = None;
                                    // End polling soon to avoid extra work
                                    app.refresh_installed_until = Some(now + Duration::from_secs(1));
                                }
                            }
                        }
                    }
                }
                if app.need_ring_prefetch && app.ring_resume_at.map(|t| std::time::Instant::now() >= t).unwrap_or(false) {
                    crate::logic::set_allowed_ring(&app, 30);
                    crate::logic::ring_prefetch_from_selected(&mut app, &details_req_tx);
                    app.need_ring_prefetch = false;
                    app.scroll_moves = 0; app.ring_resume_at = None;
                }
                if app.sort_menu_open && let Some(deadline) = app.sort_menu_auto_close_at && std::time::Instant::now() >= deadline {
                    app.sort_menu_open = false; app.sort_menu_auto_close_at = None;
                }
                if let Some(deadline) = app.toast_expires_at
                    && std::time::Instant::now() >= deadline {
                        app.toast_message = None;
                        app.toast_expires_at = None;
                    }
            }
            Some(todays) = news_rx.recv() => {
                if todays.is_empty() {
                    app.toast_message = Some(crate::i18n::t(&app, "app.toasts.no_new_news"));
                    app.toast_expires_at = Some(Instant::now() + Duration::from_secs(10));
                } else {
                    // Show unread news items; default to first selected
                    app.modal = Modal::News { items: todays.clone(), selected: 0 };
                }
            }
            Some((txt, color)) = status_rx.recv() => {
                app.arch_status_text = txt;
                app.arch_status_color = color;
            }
            else => {}
        }
    }

    // Reset resolution flags on exit to ensure clean shutdown
    // This prevents background tasks from blocking if they're still running
    tracing::debug!("[Runtime] Main loop exited, resetting resolution flags");
    app.deps_resolving = false;
    app.files_resolving = false;
    app.services_resolving = false;
    app.sandbox_resolving = false;

    maybe_flush_cache(&mut app);
    maybe_flush_recent(&mut app);
    maybe_flush_news_read(&mut app);
    maybe_flush_install(&mut app);
    maybe_flush_deps_cache(&mut app);
    maybe_flush_files_cache(&mut app);
    maybe_flush_services_cache(&mut app);
    maybe_flush_sandbox_cache(&mut app);

    if !headless {
        restore_terminal()?;
    }
    Ok(())
}
