//! Central `AppState` container, split out from the monolithic module.

use ratatui::widgets::ListState;
use std::{collections::HashMap, path::PathBuf, time::Instant};

use crate::state::modal::Modal;
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

    // Install pane bottom action (Import)
    /// Clickable rectangle for the Install pane bottom "Import" button (x, y, w, h).
    pub install_import_rect: Option<(u16, u16, u16, u16)>,

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
    /// Rectangle of the clickable "Check Package Build" button in PKGBUILD title.
    pub pkgb_check_button_rect: Option<(u16, u16, u16, u16)>,
    /// Whether the PKGBUILD viewer is visible (details pane split in half).
    pub pkgb_visible: bool,
    /// The fetched PKGBUILD text when available.
    pub pkgb_text: Option<String>,
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

    // Mouse hit-test rectangles for panes
    /// Inner content rectangle of the Results list (x, y, w, h).
    pub results_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the Package Info details pane (x, y, w, h).
    pub details_rect: Option<(u16, u16, u16, u16)>,
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

            install_list: Vec::new(),
            install_state: ListState::default(),
            remove_list: Vec::new(),
            remove_state: ListState::default(),
            downgrade_list: Vec::new(),
            downgrade_state: ListState::default(),
            // Install list (lists dir under config)
            install_path: crate::theme::lists_dir().join("install_list.json"),
            install_dirty: false,

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
            install_import_rect: None,
            arch_status_text: "Arch Status: loading…".to_string(),
            arch_status_rect: None,
            arch_status_color: ArchStatusColor::None,
            pkgb_button_rect: None,
            pkgb_check_button_rect: None,
            pkgb_visible: false,
            pkgb_text: None,
            pkgb_scroll: 0,
            pkgb_rect: None,

            toast_message: None,
            toast_expires_at: None,

            layout_left_pct: 20,
            layout_center_pct: 60,
            layout_right_pct: 20,
            keymap: crate::theme::Settings::default().keymap,

            results_rect: None,
            details_rect: None,
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

            // Filters default to showing everything
            results_filter_show_aur: true,
            results_filter_show_core: true,
            results_filter_show_extra: true,
            results_filter_show_multilib: true,
            results_filter_show_eos: true,
            results_filter_show_cachyos: true,
            results_filter_aur_rect: None,
            results_filter_core_rect: None,
            results_filter_extra_rect: None,
            results_filter_multilib_rect: None,
            results_filter_eos_rect: None,
            results_filter_cachyos_rect: None,

            // Package mutation cache refresh state (inactive by default)
            refresh_installed_until: None,
            next_installed_refresh_at: None,

            // Pending install tracking
            pending_install_names: None,
            pending_remove_names: None,
        }
    }
}
