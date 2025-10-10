//! Core application state types for Pacsea's TUI.
//!
//! This module defines the serializable data structures used across the
//! application: package descriptors, search coordination types, UI focus and
//! modals, and the central [`AppState`] container mutated by the event and UI
//! layers. Many of these types are persisted between runs.
use crate::theme::KeyMap;
use ratatui::widgets::ListState;
use std::{collections::HashMap, path::PathBuf, time::Instant};

/// Package source origin.
///
/// Indicates whether a package originates from the official repositories or
/// the Arch User Repository.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Source {
    /// Official repository package and its associated repository and target
    /// architecture.
    Official { repo: String, arch: String },
    /// AUR package.
    Aur,
}

/// Minimal package summary used in lists and search results.
///
/// This is compact enough to render in lists and panes. For a richer, detailed
/// view, see [`PackageDetails`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageItem {
    /// Canonical package name.
    pub name: String,
    /// Version string as reported by the source.
    pub version: String,
    /// One-line description suitable for list display.
    pub description: String,
    /// Origin of the package (official repo or AUR).
    pub source: Source,
    /// AUR popularity score when available (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f64>,
}

/// Full set of details for a package, suitable for a dedicated information
/// pane.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageDetails {
    /// Repository name (e.g., "extra").
    pub repository: String,
    /// Package name.
    pub name: String,
    /// Full version string.
    pub version: String,
    /// Long description.
    pub description: String,
    /// Target architecture.
    pub architecture: String,
    /// Upstream project URL (may be empty if unknown).
    pub url: String,
    /// SPDX or human-readable license identifiers.
    pub licenses: Vec<String>,
    /// Group memberships.
    pub groups: Vec<String>,
    /// Virtual provisions supplied by this package.
    pub provides: Vec<String>,
    /// Required dependencies.
    pub depends: Vec<String>,
    /// Optional dependencies with annotations.
    pub opt_depends: Vec<String>,
    /// Packages that require this package.
    pub required_by: Vec<String>,
    /// Packages for which this package is optional.
    pub optional_for: Vec<String>,
    /// Conflicting packages.
    pub conflicts: Vec<String>,
    /// Packages that this package replaces.
    pub replaces: Vec<String>,
    /// Download size in bytes, if available.
    pub download_size: Option<u64>,
    /// Installed size in bytes, if available.
    pub install_size: Option<u64>,
    /// Packager or maintainer name.
    pub owner: String, // packager/maintainer
    /// Build or packaging date (string-formatted for display).
    pub build_date: String,
    /// AUR popularity score when available (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f64>,
}

/// Search query sent to the background search worker.
#[derive(Clone, Debug)]
pub struct QueryInput {
    /// Monotonic identifier used to correlate responses.
    pub id: u64,
    /// Raw query text entered by the user.
    pub text: String,
}
/// Results corresponding to a prior [`QueryInput`].
#[derive(Clone, Debug)]
pub struct SearchResults {
    /// Echoed identifier from the originating query.
    pub id: u64,
    /// Matching packages in rank order.
    pub items: Vec<PackageItem>,
}

/// Sorting mode for the Results list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Default: Pacman (core/extra/other official) first, then AUR; name tiebreak.
    RepoThenName,
    /// AUR first (by highest popularity), then official repos; name tiebreak.
    AurPopularityThenOfficial,
    /// Best matches: Relevance by name to current query, then repo order, then name.
    BestMatches,
}

impl SortMode {
    pub fn as_config_key(&self) -> &'static str {
        match self {
            SortMode::RepoThenName => "alphabetical",
            SortMode::AurPopularityThenOfficial => "aur_popularity",
            SortMode::BestMatches => "best_matches",
        }
    }
    pub fn from_config_key(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "alphabetical" | "repo_then_name" | "pacman" => Some(SortMode::RepoThenName),
            "aur_popularity" | "popularity" => Some(SortMode::AurPopularityThenOfficial),
            "best_matches" | "relevance" => Some(SortMode::BestMatches),
            _ => None,
        }
    }
}

/// Modal dialog state for the UI.
#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert { message: String },
    /// Confirmation dialog for installing the given items.
    ConfirmInstall { items: Vec<PackageItem> },
    /// Help overlay with keybindings. Non-interactive; dismissed with Esc/Enter.
    Help,
    /// Confirmation dialog for removing the given items.
    ConfirmRemove { items: Vec<PackageItem> },
    /// System update dialog with multi-select options and optional country.
    SystemUpdate {
        /// Whether to update Arch mirrors using reflector.
        do_mirrors: bool,
        /// Whether to update system packages via pacman.
        do_pacman: bool,
        /// Whether to update AUR packages via paru/yay.
        do_aur: bool,
        /// Whether to remove caches (pacman and AUR helper).
        do_cache: bool,
        /// Index into `countries` for the reflector `--country` argument.
        country_idx: usize,
        /// Available countries to choose from for reflector.
        countries: Vec<String>,
        /// Cursor row in the dialog (0..=4)
        cursor: usize,
    },
}

/// Which UI pane currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Center pane: search input and results.
    Search,
    /// Left pane: recent queries list.
    Recent,
    /// Right pane: pending install list.
    Install,
}

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
    // Persisted install list
    /// Path where the install list is persisted as JSON.
    pub install_path: PathBuf,
    /// Dirty flag indicating `install_list` needs to be saved.
    pub install_dirty: bool,

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
    /// Whether mouse capture is temporarily disabled to allow text selection in details.
    pub mouse_disabled_in_details: bool,
    /// Last observed mouse position (column, row) in terminal cells.
    pub last_mouse_pos: Option<(u16, u16)>,
    /// Whether global terminal mouse capture is currently enabled.
    pub mouse_capture_enabled: bool,

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

    /// Whether Results is currently showing only explicitly installed packages.
    pub installed_only_mode: bool,

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
            // Persisted recent searches (XDG state)
            recent_path: crate::theme::state_dir().join("recent_searches.json"),
            recent_dirty: false,

            latest_query_id: 0,
            next_query_id: 1,
            details_cache: HashMap::new(),
            // Details cache (XDG cache)
            cache_path: crate::theme::cache_dir().join("details_cache.json"),
            cache_dirty: false,

            install_list: Vec::new(),
            install_state: ListState::default(),
            remove_list: Vec::new(),
            remove_state: ListState::default(),
            // Install list (XDG state)
            install_path: crate::theme::state_dir().join("install_list.json"),
            install_dirty: false,

            pane_find: None,

            // Search input mode
            search_normal_mode: false,
            search_caret: 0,
            search_select_anchor: None,

            // Official index (XDG cache)
            official_index_path: crate::theme::cache_dir().join("official_index.json"),

            loading_index: false,

            details_focus: None,

            scroll_moves: 0,
            ring_resume_at: None,
            need_ring_prefetch: false,
            url_button_rect: None,
            pkgb_button_rect: None,
            pkgb_check_button_rect: None,
            pkgb_visible: false,
            pkgb_text: None,
            pkgb_scroll: 0,
            pkgb_rect: None,

            layout_left_pct: 20,
            layout_center_pct: 60,
            layout_right_pct: 20,
            keymap: crate::theme::Settings::default().keymap,

            results_rect: None,
            details_rect: None,
            recent_rect: None,
            install_rect: None,
            mouse_disabled_in_details: false,
            last_mouse_pos: None,
            mouse_capture_enabled: true,

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

            installed_only_mode: false,

            // Filters default to showing everything
            results_filter_show_aur: true,
            results_filter_show_core: true,
            results_filter_show_extra: true,
            results_filter_show_multilib: true,
            results_filter_show_eos: true,
            results_filter_aur_rect: None,
            results_filter_core_rect: None,
            results_filter_extra_rect: None,
            results_filter_multilib_rect: None,
            results_filter_eos_rect: None,
        }
    }
}
