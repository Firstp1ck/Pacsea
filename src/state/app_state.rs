//! Central `AppState` container, split out from the monolithic module.

use ratatui::widgets::ListState;
use std::{collections::HashMap, path::PathBuf, time::Instant};

use crate::state::modal::{CascadeMode, Modal, PreflightAction, ServiceImpact};
use crate::state::types::{
    ArchStatusColor, Focus, PackageDetails, PackageItem, RightPaneFocus, SortMode,
};
use crate::theme::KeyMap;

/// Global application state shared by the event, networking, and UI layers.
///
/// This structure is mutated frequently in response to input and background
/// updates. Certain subsets are persisted to disk to preserve user context
/// across runs (e.g., recent searches, details cache, install list).
#[derive(Debug)]
pub struct AppState {
    /// Current search input text.
    pub input: String,
    /// Current search results, most relevant first.
    pub results: Vec<PackageItem>,
    /// Unfiltered results as last received from the search worker.
    pub all_results: Vec<PackageItem>,
    /// Backup of results when toggling to installed-only view.
    pub results_backup_for_toggle: Option<Vec<PackageItem>>,
    /// Index into `results` that is currently highlighted.
    pub selected: usize,
    /// Details for the currently highlighted result.
    pub details: PackageDetails,
    /// List selection state for the search results list.
    pub list_state: ListState,
    /// Active modal dialog, if any.
    pub modal: Modal,
    /// Previous modal state (used to restore when closing help/alert modals).
    pub previous_modal: Option<Modal>,
    /// If `true`, show install steps without executing side effects.
    pub dry_run: bool,
    // Recent searches
    /// Previously executed queries.
    pub recent: Vec<String>,
    /// List selection state for the Recent pane.
    pub history_state: ListState,
    /// Which pane is currently focused.
    pub focus: Focus,
    /// Timestamp of the last input edit, used for debouncing or throttling.
    pub last_input_change: Instant,
    /// Last value persisted for the input field, to avoid redundant writes.
    pub last_saved_value: Option<String>,
    // Persisted recent searches
    /// Path where recent searches are persisted as JSON.
    pub recent_path: PathBuf,
    /// Dirty flag indicating `recent` needs to be saved.
    pub recent_dirty: bool,

    // Search coordination
    /// Identifier of the latest query whose results are being displayed.
    pub latest_query_id: u64,
    /// Next query identifier to allocate.
    pub next_query_id: u64,
    // Details cache
    /// Cache of details keyed by package name.
    pub details_cache: HashMap<String, PackageDetails>,
    /// Path where the details cache is persisted as JSON.
    pub cache_path: PathBuf,
    /// Dirty flag indicating `details_cache` needs to be saved.
    pub cache_dirty: bool,

    // News read/unread tracking (persisted)
    /// Set of Arch news item URLs the user has marked as read.
    pub news_read_urls: std::collections::HashSet<String>,
    /// Path where the read news URLs are persisted as JSON.
    pub news_read_path: PathBuf,
    /// Dirty flag indicating `news_read_urls` needs to be saved.
    pub news_read_dirty: bool,

    // Install list pane
    /// Packages selected for installation.
    pub install_list: Vec<PackageItem>,
    /// List selection state for the Install pane.
    pub install_state: ListState,
    /// Separate list of packages selected for removal (active in installed-only mode).
    pub remove_list: Vec<PackageItem>,
    /// List selection state for the Remove pane.
    pub remove_state: ListState,
    /// Separate list of packages selected for downgrade (shown in installed-only mode).
    pub downgrade_list: Vec<PackageItem>,
    /// List selection state for the Downgrade pane.
    pub downgrade_state: ListState,
    // Persisted install list
    /// Path where the install list is persisted as JSON.
    pub install_path: PathBuf,
    /// Dirty flag indicating `install_list` needs to be saved.
    pub install_dirty: bool,
    /// Timestamp of the most recent change to the install list for throttling disk writes.
    pub last_install_change: Option<Instant>,

    // Visibility toggles for middle row panes
    /// Whether the Recent pane is visible in the middle row.
    pub show_recent_pane: bool,
    /// Whether the Install/Remove pane is visible in the middle row.
    pub show_install_pane: bool,
    /// Whether to show the keybindings footer in the details pane.
    pub show_keybinds_footer: bool,

    // In-pane search (for Recent/Install panes)
    /// Optional, transient find pattern used by pane-local search ("/").
    pub pane_find: Option<String>,

    /// Whether Search pane is in Normal mode (Vim-like navigation) instead of Insert mode.
    pub search_normal_mode: bool,

    /// Caret position (in characters) within the Search input.
    /// Always clamped to the range 0..=input.chars().count().
    pub search_caret: usize,
    /// Selection anchor (in characters) for the Search input when selecting text.
    /// When `None`, no selection is active. When `Some(i)`, the selected range is
    /// between `min(i, search_caret)` and `max(i, search_caret)` (exclusive upper bound).
    pub search_select_anchor: Option<usize>,

    // Official package index persistence
    /// Path to the persisted official package index used for fast offline lookups.
    pub official_index_path: PathBuf,

    // Loading indicator for official index generation
    /// Whether the application is currently generating the official index.
    pub loading_index: bool,

    // Track which package’s details the UI is focused on
    /// Name of the package whose details are being emphasized in the UI, if any.
    pub details_focus: Option<String>,

    // Ring prefetch debounce state
    /// Smooth scrolling accumulator for prefetch heuristics.
    pub scroll_moves: u32,
    /// Timestamp at which to resume ring prefetching, if paused.
    pub ring_resume_at: Option<Instant>,
    /// Whether a ring prefetch is needed soon.
    pub need_ring_prefetch: bool,

    // Clickable URL button rectangle (x, y, w, h) in terminal cells
    /// Rectangle of the clickable URL button in terminal cell coordinates.
    pub url_button_rect: Option<(u16, u16, u16, u16)>,

    // VirusTotal API setup modal clickable URL rectangle
    /// Rectangle of the clickable VirusTotal API URL in the setup modal (x, y, w, h).
    pub vt_url_rect: Option<(u16, u16, u16, u16)>,

    // Install pane bottom action (Import)
    /// Clickable rectangle for the Install pane bottom "Import" button (x, y, w, h).
    pub install_import_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Install pane bottom "Export" button (x, y, w, h).
    pub install_export_rect: Option<(u16, u16, u16, u16)>,

    // Arch status label (middle row footer)
    /// Latest fetched status message from `status.archlinux.org`.
    pub arch_status_text: String,
    /// Clickable rectangle for the status label (x, y, w, h).
    pub arch_status_rect: Option<(u16, u16, u16, u16)>,
    /// Optional status color indicator (e.g., operational vs. current incident).
    pub arch_status_color: ArchStatusColor,

    // Clickable PKGBUILD button rectangle and viewer state
    /// Rectangle of the clickable "Show PKGBUILD" in terminal cell coordinates.
    pub pkgb_button_rect: Option<(u16, u16, u16, u16)>,
    /// Rectangle of the clickable "Copy PKGBUILD" button in PKGBUILD title.
    pub pkgb_check_button_rect: Option<(u16, u16, u16, u16)>,
    /// Rectangle of the clickable "Reload PKGBUILD" button in PKGBUILD title.
    pub pkgb_reload_button_rect: Option<(u16, u16, u16, u16)>,
    /// Whether the PKGBUILD viewer is visible (details pane split in half).
    pub pkgb_visible: bool,
    /// The fetched PKGBUILD text when available.
    pub pkgb_text: Option<String>,
    /// Name of the package that the PKGBUILD is currently for.
    pub pkgb_package_name: Option<String>,
    /// Timestamp when PKGBUILD reload was last requested (for debouncing).
    pub pkgb_reload_requested_at: Option<Instant>,
    /// Name of the package for which PKGBUILD reload was requested (for debouncing).
    pub pkgb_reload_requested_for: Option<String>,
    /// Scroll offset (lines) for the PKGBUILD viewer.
    pub pkgb_scroll: u16,
    /// Content rectangle of the PKGBUILD viewer (x, y, w, h) when visible.
    pub pkgb_rect: Option<(u16, u16, u16, u16)>,

    // Transient toast message (bottom-right)
    /// Optional short-lived info message rendered at the bottom-right corner.
    pub toast_message: Option<String>,
    /// Deadline (Instant) after which the toast is automatically hidden.
    pub toast_expires_at: Option<Instant>,

    // User settings loaded at startup
    pub layout_left_pct: u16,
    pub layout_center_pct: u16,
    pub layout_right_pct: u16,
    /// Resolved key bindings from user settings
    pub keymap: KeyMap,
    // Internationalization (i18n)
    /// Resolved locale code (e.g., "de-DE", "en-US")
    pub locale: String,
    /// Translation map for the current locale
    pub translations: crate::i18n::translations::TranslationMap,
    /// Fallback translation map (English) for missing keys
    pub translations_fallback: crate::i18n::translations::TranslationMap,

    // Mouse hit-test rectangles for panes
    /// Inner content rectangle of the Results list (x, y, w, h).
    pub results_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the Package Info details pane (x, y, w, h).
    pub details_rect: Option<(u16, u16, u16, u16)>,
    /// Scroll offset (lines) for the Package Info details pane.
    pub details_scroll: u16,
    /// Inner content rectangle of the Recent pane list (x, y, w, h).
    pub recent_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the Install pane list (x, y, w, h).
    pub install_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the Downgrade subpane when visible.
    pub downgrade_rect: Option<(u16, u16, u16, u16)>,
    /// Whether mouse capture is temporarily disabled to allow text selection in details.
    pub mouse_disabled_in_details: bool,
    /// Last observed mouse position (column, row) in terminal cells.
    pub last_mouse_pos: Option<(u16, u16)>,
    /// Whether global terminal mouse capture is currently enabled.
    pub mouse_capture_enabled: bool,

    // News modal mouse hit-testing
    /// Outer rectangle of the News modal (including borders) when visible.
    pub news_rect: Option<(u16, u16, u16, u16)>,
    /// Inner list rectangle for clickable news rows.
    pub news_list_rect: Option<(u16, u16, u16, u16)>,

    // Help modal scroll and hit-testing
    /// Scroll offset (lines) for the Help modal content.
    pub help_scroll: u16,
    /// Inner content rectangle of the Help modal (x, y, w, h) for hit-testing.
    pub help_rect: Option<(u16, u16, u16, u16)>,

    // Preflight modal mouse hit-testing
    /// Clickable rectangles for preflight tabs (x, y, w, h) - Summary, Deps, Files, Services, Sandbox.
    pub preflight_tab_rects: [Option<(u16, u16, u16, u16)>; 5],
    /// Inner content rectangle of the preflight modal (x, y, w, h) for hit-testing package groups.
    pub preflight_content_rect: Option<(u16, u16, u16, u16)>,

    // Results sorting UI
    /// Current sort mode for results.
    pub sort_mode: SortMode,
    /// Whether the sort dropdown is currently visible.
    pub sort_menu_open: bool,
    /// Clickable rectangle for the sort button in the Results title (x, y, w, h).
    pub sort_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the sort dropdown menu when visible (x, y, w, h).
    pub sort_menu_rect: Option<(u16, u16, u16, u16)>,
    /// Deadline after which the sort dropdown auto-closes.
    pub sort_menu_auto_close_at: Option<Instant>,

    // Results options UI (top-right dropdown)
    /// Whether the options dropdown is currently visible.
    pub options_menu_open: bool,
    /// Clickable rectangle for the options button in the Results title (x, y, w, h).
    pub options_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the options dropdown menu when visible (x, y, w, h).
    pub options_menu_rect: Option<(u16, u16, u16, u16)>,

    // Panels dropdown UI (left of Options)
    /// Whether the panels dropdown is currently visible.
    pub panels_menu_open: bool,
    /// Clickable rectangle for the panels button in the Results title (x, y, w, h).
    pub panels_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the panels dropdown menu when visible (x, y, w, h).
    pub panels_menu_rect: Option<(u16, u16, u16, u16)>,

    // Config/Lists dropdown UI (left of Panels)
    /// Whether the Config/Lists dropdown is currently visible.
    pub config_menu_open: bool,
    /// Clickable rectangle for the Config/Lists button in the Results title (x, y, w, h).
    pub config_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the Config/Lists dropdown menu when visible (x, y, w, h).
    pub config_menu_rect: Option<(u16, u16, u16, u16)>,

    /// Whether Results is currently showing only explicitly installed packages.
    pub installed_only_mode: bool,
    /// Which right subpane is focused when installed-only mode splits the pane.
    pub right_pane_focus: RightPaneFocus,
    /// Visual marker style for packages added to lists (user preference cached at startup).
    pub package_marker: crate::theme::PackageMarker,

    // Results filters UI
    /// Whether to include AUR packages in the Results view.
    pub results_filter_show_aur: bool,
    /// Whether to include packages from the `core` repo in the Results view.
    pub results_filter_show_core: bool,
    /// Whether to include packages from the `extra` repo in the Results view.
    pub results_filter_show_extra: bool,
    /// Whether to include packages from the `multilib` repo in the Results view.
    pub results_filter_show_multilib: bool,
    /// Whether to include packages from the `eos` repo in the Results view.
    pub results_filter_show_eos: bool,
    /// Whether to include packages from `cachyos*` repos in the Results view.
    pub results_filter_show_cachyos: bool,
    /// Whether to include packages from Artix Linux repos in the Results view.
    pub results_filter_show_artix: bool,
    /// Whether to include packages labeled as `manjaro` in the Results view.
    pub results_filter_show_manjaro: bool,
    /// Clickable rectangle for the AUR filter toggle in the Results title (x, y, w, h).
    pub results_filter_aur_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the core filter toggle in the Results title (x, y, w, h).
    pub results_filter_core_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the extra filter toggle in the Results title (x, y, w, h).
    pub results_filter_extra_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the multilib filter toggle in the Results title (x, y, w, h).
    pub results_filter_multilib_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the EOS filter toggle in the Results title (x, y, w, h).
    pub results_filter_eos_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the CachyOS filter toggle in the Results title (x, y, w, h).
    pub results_filter_cachyos_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Manjaro filter toggle in the Results title (x, y, w, h).
    pub results_filter_manjaro_rect: Option<(u16, u16, u16, u16)>,

    // Background refresh of installed/explicit caches after package mutations
    /// If `Some`, keep polling pacman/yay to refresh installed/explicit caches until this time.
    pub refresh_installed_until: Option<Instant>,
    /// Next scheduled time to poll caches while `refresh_installed_until` is active.
    pub next_installed_refresh_at: Option<Instant>,

    // Pending installs to detect completion and clear Install list
    /// Names of packages we just triggered to install; when all appear installed, clear Install list.
    pub pending_install_names: Option<Vec<String>>,

    // Pending removals to detect completion and log
    /// Names of packages we just triggered to remove; when all disappear, append to removed log.
    pub pending_remove_names: Option<Vec<String>>,

    // Dependency resolution cache for install list
    /// Cached resolved dependencies for the current install list (updated in background).
    pub install_list_deps: Vec<crate::state::modal::DependencyInfo>,
    /// Reverse dependency summary for the current remove preflight modal (populated on demand).
    pub remove_preflight_summary: Vec<crate::state::modal::ReverseRootSummary>,
    /// Selected cascade removal mode for upcoming removals.
    pub remove_cascade_mode: CascadeMode,
    /// Whether dependency resolution is currently in progress.
    pub deps_resolving: bool,
    /// Path where the dependency cache is persisted as JSON.
    pub deps_cache_path: PathBuf,
    /// Dirty flag indicating `install_list_deps` needs to be saved.
    pub deps_cache_dirty: bool,

    // File resolution cache for install list
    /// Cached resolved file changes for the current install list (updated in background).
    pub install_list_files: Vec<crate::state::modal::PackageFileInfo>,
    /// Whether file resolution is currently in progress.
    pub files_resolving: bool,
    /// Path where the file cache is persisted as JSON.
    pub files_cache_path: PathBuf,
    /// Dirty flag indicating `install_list_files` needs to be saved.
    pub files_cache_dirty: bool,

    // Service impact cache for install list
    /// Cached resolved service impacts for the current install list (updated in background).
    pub install_list_services: Vec<crate::state::modal::ServiceImpact>,
    /// Whether service impact resolution is currently in progress.
    pub services_resolving: bool,
    /// Path where the service cache is persisted as JSON.
    pub services_cache_path: PathBuf,
    /// Dirty flag indicating `install_list_services` needs to be saved.
    pub services_cache_dirty: bool,
    /// Flag requesting that the runtime schedule service impact resolution for the active Preflight modal.
    pub service_resolve_now: bool,
    /// Identifier of the active service impact resolution request, if any.
    pub active_service_request: Option<u64>,
    /// Monotonic counter used to tag service impact resolution requests.
    pub next_service_request_id: u64,
    /// Signature of the package set currently queued for service impact resolution.
    pub services_pending_signature: Option<(PreflightAction, Vec<String>)>,
    /// Service restart decisions captured during the Preflight Services tab.
    pub pending_service_plan: Vec<ServiceImpact>,

    // Sandbox analysis cache for install list
    /// Cached resolved sandbox information for the current install list (updated in background).
    pub install_list_sandbox: Vec<crate::logic::sandbox::SandboxInfo>,
    /// Whether sandbox resolution is currently in progress.
    pub sandbox_resolving: bool,
    /// Path where the sandbox cache is persisted as JSON.
    pub sandbox_cache_path: PathBuf,
    /// Dirty flag indicating `install_list_sandbox` needs to be saved.
    pub sandbox_cache_dirty: bool,

    // Preflight modal background resolution requests
    /// Packages to resolve for preflight summary computation.
    pub preflight_summary_items: Option<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
    /// Packages to resolve for preflight dependency analysis.
    pub preflight_deps_items: Option<Vec<PackageItem>>,
    /// Packages to resolve for preflight file analysis.
    pub preflight_files_items: Option<Vec<PackageItem>>,
    /// Packages to resolve for preflight service analysis.
    pub preflight_services_items: Option<Vec<PackageItem>>,
    /// AUR packages to resolve for preflight sandbox analysis (subset only).
    pub preflight_sandbox_items: Option<Vec<PackageItem>>,
    /// Whether preflight summary computation is in progress.
    pub preflight_summary_resolving: bool,
    /// Whether preflight dependency resolution is in progress.
    pub preflight_deps_resolving: bool,
    /// Whether preflight file resolution is in progress.
    pub preflight_files_resolving: bool,
    /// Whether preflight service resolution is in progress.
    pub preflight_services_resolving: bool,
    /// Whether preflight sandbox resolution is in progress.
    pub preflight_sandbox_resolving: bool,
    /// Cancellation flag for preflight operations (set to true when modal closes).
    pub preflight_cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for AppState {
    /// Construct a default, empty [`AppState`], initializing paths, selection
    /// states, and timers with sensible defaults.
    fn default() -> Self {
        Self {
            input: String::new(),
            results: Vec::new(),
            all_results: Vec::new(),
            results_backup_for_toggle: None,
            selected: 0,
            details: PackageDetails::default(),
            list_state: ListState::default(),
            modal: Modal::None,
            previous_modal: None,
            dry_run: false,
            recent: Vec::new(),
            history_state: ListState::default(),
            focus: Focus::Search,
            last_input_change: Instant::now(),
            last_saved_value: None,
            // Persisted recent searches (lists dir under config)
            recent_path: crate::theme::lists_dir().join("recent_searches.json"),
            recent_dirty: false,

            latest_query_id: 0,
            next_query_id: 1,
            details_cache: HashMap::new(),
            // Details cache (lists dir under config)
            cache_path: crate::theme::lists_dir().join("details_cache.json"),
            cache_dirty: false,

            // News read/unread tracking (lists dir under config)
            news_read_urls: std::collections::HashSet::new(),
            news_read_path: crate::theme::lists_dir().join("news_read_urls.json"),
            news_read_dirty: false,

            install_list: Vec::new(),
            install_state: ListState::default(),
            remove_list: Vec::new(),
            remove_state: ListState::default(),
            downgrade_list: Vec::new(),
            downgrade_state: ListState::default(),
            // Install list (lists dir under config)
            install_path: crate::theme::lists_dir().join("install_list.json"),
            install_dirty: false,
            last_install_change: None,

            // Middle row panes visible by default
            show_recent_pane: true,
            show_install_pane: true,
            show_keybinds_footer: true,

            pane_find: None,

            // Search input mode
            search_normal_mode: false,
            search_caret: 0,
            search_select_anchor: None,

            // Official index (lists dir under config)
            official_index_path: crate::theme::lists_dir().join("official_index.json"),

            loading_index: false,

            details_focus: None,

            scroll_moves: 0,
            ring_resume_at: None,
            need_ring_prefetch: false,
            url_button_rect: None,
            vt_url_rect: None,
            install_import_rect: None,
            install_export_rect: None,
            arch_status_text: "Arch Status: loading…".to_string(),
            arch_status_rect: None,
            arch_status_color: ArchStatusColor::None,
            pkgb_button_rect: None,
            pkgb_check_button_rect: None,
            pkgb_reload_button_rect: None,
            pkgb_visible: false,
            pkgb_text: None,
            pkgb_package_name: None,
            pkgb_reload_requested_at: None,
            pkgb_reload_requested_for: None,
            pkgb_scroll: 0,
            pkgb_rect: None,

            toast_message: None,
            toast_expires_at: None,

            layout_left_pct: 20,
            layout_center_pct: 60,
            layout_right_pct: 20,
            keymap: crate::theme::Settings::default().keymap,
            locale: "en-US".to_string(),
            translations: std::collections::HashMap::new(),
            translations_fallback: std::collections::HashMap::new(),

            results_rect: None,
            details_rect: None,
            details_scroll: 0,
            recent_rect: None,
            install_rect: None,
            downgrade_rect: None,
            mouse_disabled_in_details: false,
            last_mouse_pos: None,
            mouse_capture_enabled: true,

            news_rect: None,
            news_list_rect: None,

            help_scroll: 0,
            help_rect: None,

            // Preflight modal mouse hit-testing
            preflight_tab_rects: [None; 5],
            preflight_content_rect: None,

            // Sorting
            sort_mode: SortMode::RepoThenName,
            sort_menu_open: false,
            sort_button_rect: None,
            sort_menu_rect: None,
            sort_menu_auto_close_at: None,

            // Options dropdown (top-right of Results)
            options_menu_open: false,
            options_button_rect: None,
            options_menu_rect: None,

            // Panels dropdown (top-right of Results)
            panels_menu_open: false,
            panels_button_rect: None,
            panels_menu_rect: None,

            // Config/Lists dropdown (top-right of Results)
            config_menu_open: false,
            config_button_rect: None,
            config_menu_rect: None,

            installed_only_mode: false,
            right_pane_focus: RightPaneFocus::Install,
            package_marker: crate::theme::PackageMarker::Front,

            // Filters default to showing everything
            results_filter_show_aur: true,
            results_filter_show_core: true,
            results_filter_show_extra: true,
            results_filter_show_multilib: true,
            results_filter_show_eos: true,
            results_filter_show_cachyos: true,
            results_filter_show_artix: true,
            results_filter_show_manjaro: true,
            results_filter_aur_rect: None,
            results_filter_core_rect: None,
            results_filter_extra_rect: None,
            results_filter_multilib_rect: None,
            results_filter_eos_rect: None,
            results_filter_cachyos_rect: None,
            results_filter_artix_rect: None,
            results_filter_manjaro_rect: None,

            // Package mutation cache refresh state (inactive by default)
            refresh_installed_until: None,
            next_installed_refresh_at: None,

            // Pending install tracking
            pending_install_names: None,
            pending_remove_names: None,
            install_list_deps: Vec::new(),
            remove_preflight_summary: Vec::new(),
            remove_cascade_mode: CascadeMode::Basic,
            deps_resolving: false,
            // Dependency cache (lists dir under config)
            deps_cache_path: crate::theme::lists_dir().join("install_deps_cache.json"),
            deps_cache_dirty: false,

            install_list_files: Vec::new(),
            files_resolving: false,
            // File cache (lists dir under config)
            files_cache_path: crate::theme::lists_dir().join("file_cache.json"),
            files_cache_dirty: false,

            install_list_services: Vec::new(),
            services_resolving: false,
            // Service cache (lists dir under config)
            services_cache_path: crate::theme::lists_dir().join("services_cache.json"),
            services_cache_dirty: false,
            service_resolve_now: false,
            active_service_request: None,
            next_service_request_id: 1,
            services_pending_signature: None,
            pending_service_plan: Vec::new(),

            install_list_sandbox: Vec::new(),
            sandbox_resolving: false,
            // Sandbox cache (lists dir under config)
            sandbox_cache_path: crate::theme::lists_dir().join("sandbox_cache.json"),
            sandbox_cache_dirty: false,
            preflight_summary_items: None,
            preflight_deps_items: None,
            preflight_files_items: None,
            preflight_services_items: None,
            preflight_sandbox_items: None,
            preflight_summary_resolving: false,
            preflight_deps_resolving: false,
            preflight_files_resolving: false,
            preflight_services_resolving: false,
            preflight_sandbox_resolving: false,
            preflight_cancelled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Verify `AppState::default` initialises UI flags and filesystem paths under the configured lists directory.
    ///
    /// Inputs:
    /// - No direct inputs; shims the `HOME` environment variable to a temporary directory before constructing `AppState`.
    ///
    /// Output:
    /// - Ensures selection indices reset to zero, result buffers start empty, and cached path values live under `lists_dir`.
    ///
    /// Details:
    /// - Uses a mutex guard to serialise environment mutations and restores `HOME` at the end to avoid cross-test interference.
    fn app_state_default_initializes_paths_and_flags() {
        let _guard = crate::state::test_mutex().lock().unwrap();
        // Shim HOME so lists_dir() resolves under a temp dir
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_state_default_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let app = super::AppState::default();
        assert_eq!(app.selected, 0);
        assert!(app.results.is_empty());
        assert!(app.all_results.is_empty());
        assert!(!app.loading_index);
        assert!(!app.dry_run);
        // Paths should point under lists_dir
        let lists = crate::theme::lists_dir();
        assert!(app.recent_path.starts_with(&lists));
        assert!(app.cache_path.starts_with(&lists));
        assert!(app.install_path.starts_with(&lists));
        assert!(app.official_index_path.starts_with(&lists));

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}
