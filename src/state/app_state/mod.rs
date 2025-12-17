//! Central `AppState` container, split out from the monolithic module.

use lru::LruCache;
use ratatui::widgets::ListState;
use std::{
    collections::HashMap, collections::HashSet, num::NonZeroUsize, path::PathBuf, time::Instant,
};

use crate::state::modal::{CascadeMode, Modal, PreflightAction, ServiceImpact};
use crate::state::types::{
    AppMode, ArchStatusColor, Focus, InstalledPackagesMode, NewsBookmark, NewsFeedItem,
    NewsReadFilter, NewsSortMode, PackageDetails, PackageItem, RightPaneFocus, SortMode,
    severity_rank,
};
use crate::theme::KeyMap;
use chrono::{NaiveDate, Utc};

mod default_impl;
mod defaults;
mod defaults_cache;

/// Maximum number of recent searches to retain (most-recent-first).
pub const RECENT_CAPACITY: usize = 20;

/// What: Provide the non-zero capacity used by the LRU recent cache.
///
/// Inputs: None.
///
/// Output:
/// - Non-zero capacity for the recent LRU cache.
///
/// Details:
/// - Uses a const unchecked constructor because the capacity constant is guaranteed
///   to be greater than zero.
#[must_use]
pub const fn recent_capacity() -> NonZeroUsize {
    // SAFETY: `RECENT_CAPACITY` is a non-zero constant.
    unsafe { NonZeroUsize::new_unchecked(RECENT_CAPACITY) }
}

/// Global application state shared by the event, networking, and UI layers.
///
/// This structure is mutated frequently in response to input and background
/// updates. Certain subsets are persisted to disk to preserve user context
/// across runs (e.g., recent searches, details cache, install list).
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    /// Current top-level mode (package management vs news feed).
    pub app_mode: AppMode,
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
    /// Previously executed queries stored as an LRU cache (keyed case-insensitively).
    pub recent: LruCache<String, String>,
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
    // Search result cache
    /// Cached search query text (None if cache is empty or invalid).
    pub search_cache_query: Option<String>,
    /// Whether fuzzy mode was used for cached query.
    pub search_cache_fuzzy: bool,
    /// Cached search results (None if cache is empty or invalid).
    pub search_cache_results: Option<Vec<PackageItem>>,
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
    /// Set of news feed item IDs the user has marked as read.
    pub news_read_ids: std::collections::HashSet<String>,
    /// Path where the read news IDs are persisted as JSON.
    pub news_read_ids_path: PathBuf,
    /// Dirty flag indicating `news_read_ids` needs to be saved.
    pub news_read_ids_dirty: bool,
    /// News feed items currently loaded.
    pub news_items: Vec<NewsFeedItem>,
    /// Filtered/sorted news results shown in the UI.
    pub news_results: Vec<NewsFeedItem>,
    /// Whether the news feed is currently loading.
    pub news_loading: bool,
    /// Whether news are ready to be viewed (loading complete and news available).
    pub news_ready: bool,
    /// Selected index within news results.
    pub news_selected: usize,
    /// List state for news results pane.
    pub news_list_state: ListState,
    /// News search input text.
    pub news_search_input: String,
    /// Caret position within news search input.
    pub news_search_caret: usize,
    /// Selection anchor within news search input.
    pub news_search_select_anchor: Option<usize>,
    /// LRU cache of recent news searches (case-insensitive key).
    pub news_recent: LruCache<String, String>,
    /// Path where news recent searches are persisted.
    pub news_recent_path: PathBuf,
    /// Dirty flag indicating `news_recent` needs to be saved.
    pub news_recent_dirty: bool,
    /// Pending news search awaiting debounce before saving to history.
    pub news_history_pending: Option<String>,
    /// Timestamp when the pending news search was last updated.
    pub news_history_pending_at: Option<std::time::Instant>,
    /// Last news search saved to history (prevents duplicate saves).
    pub news_history_last_saved: Option<String>,
    /// Whether to show Arch news items.
    pub news_filter_show_arch_news: bool,
    /// Whether to show security advisories.
    pub news_filter_show_advisories: bool,
    /// Whether to show installed package update items.
    pub news_filter_show_pkg_updates: bool,
    /// Whether to show AUR package update items.
    pub news_filter_show_aur_updates: bool,
    /// Whether to show AUR comment items.
    pub news_filter_show_aur_comments: bool,
    /// Whether to restrict advisories to installed packages.
    pub news_filter_installed_only: bool,
    /// Read/unread filter for the News Feed list.
    pub news_filter_read_status: NewsReadFilter,
    /// Clickable rectangle for Arch news filter chip in news title.
    pub news_filter_arch_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for security advisory filter chip in news title.
    pub news_filter_advisory_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for installed-only advisory filter chip in news title.
    pub news_filter_installed_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for installed update filter chip in news title.
    pub news_filter_updates_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for AUR update filter chip in news title.
    pub news_filter_aur_updates_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for AUR comment filter chip in news title.
    pub news_filter_aur_comments_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for read/unread filter chip in news title.
    pub news_filter_read_rect: Option<(u16, u16, u16, u16)>,
    /// Maximum age of news items in days (None = unlimited).
    pub news_max_age_days: Option<u32>,
    /// Whether to show the news history pane in News mode.
    pub show_news_history_pane: bool,
    /// Whether to show the news bookmarks pane in News mode.
    pub show_news_bookmarks_pane: bool,
    /// Sort mode for news results.
    pub news_sort_mode: NewsSortMode,
    /// Saved news/bookmarked items with cached content.
    pub news_bookmarks: Vec<crate::state::types::NewsBookmark>,
    /// Path where news bookmarks are persisted.
    pub news_bookmarks_path: PathBuf,
    /// Dirty flag indicating `news_bookmarks` needs to be saved.
    pub news_bookmarks_dirty: bool,
    /// Cache of fetched news article content (URL -> content).
    pub news_content_cache: std::collections::HashMap<String, String>,
    /// Path where the news content cache is persisted.
    pub news_content_cache_path: PathBuf,
    /// Dirty flag indicating `news_content_cache` needs to be saved.
    pub news_content_cache_dirty: bool,
    /// Currently displayed news content (for the selected item).
    pub news_content: Option<String>,
    /// Whether news content is currently being fetched.
    pub news_content_loading: bool,
    /// When the current news content load started (for timeout/logging).
    pub news_content_loading_since: Option<std::time::Instant>,
    /// Debounce timer for news content requests - tracks when user selected current item.
    /// Only requests content after 0.5 seconds of staying on the same item.
    pub news_content_debounce_timer: Option<std::time::Instant>,
    /// Scroll offset for news content details.
    pub news_content_scroll: u16,
    /// Path where the cached news feed is persisted.
    pub news_feed_path: PathBuf,
    /// Last-seen versions for installed packages (dedup for update feed items).
    pub news_seen_pkg_versions: HashMap<String, String>,
    /// Path where last-seen package versions are persisted.
    pub news_seen_pkg_versions_path: PathBuf,
    /// Dirty flag indicating `news_seen_pkg_versions` needs to be saved.
    pub news_seen_pkg_versions_dirty: bool,
    /// Last-seen AUR comment identifiers per installed package.
    pub news_seen_aur_comments: HashMap<String, String>,
    /// Path where last-seen AUR comments are persisted.
    pub news_seen_aur_comments_path: PathBuf,
    /// Dirty flag indicating `news_seen_aur_comments` needs to be saved.
    pub news_seen_aur_comments_dirty: bool,

    // Announcement read tracking (persisted)
    /// Set of announcement IDs the user has marked as read.
    /// Tracks both version strings (e.g., "v0.6.0") and remote announcement IDs.
    pub announcements_read_ids: std::collections::HashSet<String>,
    /// Path where the read announcement IDs are persisted as JSON.
    pub announcement_read_path: PathBuf,
    /// Dirty flag indicating `announcements_read_ids` needs to be saved.
    pub announcement_dirty: bool,

    // Last startup tracking (for incremental updates)
    /// Timestamp of the previous TUI startup (format: `YYYYMMDD:HHMMSS`).
    /// Used to determine what news/updates need fresh fetching vs cached data.
    pub last_startup_timestamp: Option<String>,
    /// Path where the last startup timestamp is persisted.
    pub last_startup_path: PathBuf,

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
    /// `HashSet` of package names in install list for O(1) membership checking.
    pub install_list_names: HashSet<String>,
    /// `HashSet` of package names in remove list for O(1) membership checking.
    pub remove_list_names: HashSet<String>,
    /// `HashSet` of package names in downgrade list for O(1) membership checking.
    pub downgrade_list_names: HashSet<String>,

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

    /// Whether fuzzy search is enabled (fzf-style matching) instead of normal substring search.
    pub fuzzy_search_enabled: bool,

    /// Caret position (in characters) within the `Search` input.
    /// Always clamped to the range 0..=`input.chars().count()`.
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

    // Track which package's details the UI is focused on
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
    /// Rectangle of the clickable `VirusTotal` API URL in the setup modal (x, y, w, h).
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

    // Package updates available
    /// Number of available package updates, if checked.
    pub updates_count: Option<usize>,
    /// Sorted list of package names with available updates.
    pub updates_list: Vec<String>,
    /// Clickable rectangle for the updates button (x, y, w, h).
    pub updates_button_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the news button in News mode (x, y, w, h).
    pub news_button_rect: Option<(u16, u16, u16, u16)>,
    /// Whether updates check is currently in progress.
    pub updates_loading: bool,
    /// Flag to trigger refresh of updates list after package installation/update.
    pub refresh_updates: bool,
    /// Flag to indicate that Updates modal should open after refresh completes.
    pub pending_updates_modal: bool,

    // Faillock lockout status
    /// Whether the user account is currently locked out.
    pub faillock_locked: bool,
    /// Timestamp when the lockout will expire (if locked).
    pub faillock_lockout_until: Option<std::time::SystemTime>,
    /// Remaining lockout time in minutes (if locked).
    pub faillock_remaining_minutes: Option<u32>,

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

    // AUR comments viewer state
    /// Rectangle of the clickable "Show comments" / "Hide comments" button in terminal cell coordinates.
    pub comments_button_rect: Option<(u16, u16, u16, u16)>,
    /// Whether the comments viewer is visible (details pane split).
    pub comments_visible: bool,
    /// The fetched comments data when available.
    pub comments: Vec<crate::state::types::AurComment>,
    /// Name of the package that the comments are currently for.
    pub comments_package_name: Option<String>,
    /// Timestamp when comments were last fetched (for cache invalidation).
    pub comments_fetched_at: Option<Instant>,
    /// Scroll offset (lines) for the comments viewer.
    pub comments_scroll: u16,
    /// Content rectangle of the comments viewer (x, y, w, h) when visible.
    pub comments_rect: Option<(u16, u16, u16, u16)>,
    /// Whether comments are currently being fetched.
    pub comments_loading: bool,
    /// Error message if comments fetch failed.
    pub comments_error: Option<String>,
    /// URLs in comments with their screen positions for click detection.
    /// Vector of (`x`, `y`, `width`, `url_string`) tuples.
    pub comments_urls: Vec<(u16, u16, u16, String)>,
    /// Author names in comments with their screen positions for click detection.
    /// Vector of (`x`, `y`, `width`, `username`) tuples.
    pub comments_authors: Vec<(u16, u16, u16, String)>,
    /// Dates in comments with their screen positions and URLs for click detection.
    /// Vector of (`x`, `y`, `width`, `url_string`) tuples.
    pub comments_dates: Vec<(u16, u16, u16, String)>,

    // Transient toast message (bottom-right)
    /// Optional short-lived info message rendered at the bottom-right corner.
    pub toast_message: Option<String>,
    /// Deadline (Instant) after which the toast is automatically hidden.
    pub toast_expires_at: Option<Instant>,

    // User settings loaded at startup
    /// Left pane width percentage.
    pub layout_left_pct: u16,
    /// Center pane width percentage.
    pub layout_center_pct: u16,
    /// Right pane width percentage.
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

    // Announcement modal mouse hit-testing
    /// Outer rectangle of the Announcement modal (including borders) when visible.
    pub announcement_rect: Option<(u16, u16, u16, u16)>,
    /// URLs in announcement content with their screen positions for click detection.
    /// Vector of (`x`, `y`, `width`, `url_string`) tuples.
    pub announcement_urls: Vec<(u16, u16, u16, String)>,
    /// Pending remote announcements to show after current announcement is dismissed.
    pub pending_announcements: Vec<crate::announcements::RemoteAnnouncement>,
    /// Pending news to show after all announcements are dismissed.
    pub pending_news: Option<Vec<crate::state::NewsItem>>,
    /// Flag to trigger startup news fetch after `NewsSetup` is completed.
    pub trigger_startup_news_fetch: bool,

    // Updates modal mouse hit-testing
    /// Outer rectangle of the Updates modal (including borders) when visible.
    pub updates_modal_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle for scrollable updates list.
    pub updates_modal_content_rect: Option<(u16, u16, u16, u16)>,

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
    /// Filter mode for installed packages (leaf only vs all explicit).
    pub installed_packages_mode: InstalledPackagesMode,
    /// Whether the sort dropdown is currently visible.
    pub sort_menu_open: bool,
    /// Clickable rectangle for the sort button in the Results title (x, y, w, h).
    pub sort_button_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the news age toggle button (x, y, w, h).
    pub news_age_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the sort dropdown menu when visible (x, y, w, h).
    pub sort_menu_rect: Option<(u16, u16, u16, u16)>,
    /// Deadline after which the sort dropdown auto-closes.
    pub sort_menu_auto_close_at: Option<Instant>,
    // Sort result caching for O(1) sort mode switching
    /// Cached sort order for `RepoThenName` mode (indices into `results`).
    pub sort_cache_repo_name: Option<Vec<usize>>,
    /// Cached sort order for `AurPopularityThenOfficial` mode (indices into `results`).
    pub sort_cache_aur_popularity: Option<Vec<usize>>,
    /// Signature of results used to validate caches (order-insensitive hash of names).
    pub sort_cache_signature: Option<u64>,

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

    // Artix filter dropdown UI (when specific repo filters are hidden)
    /// Whether the Artix filter dropdown is currently visible.
    pub artix_filter_menu_open: bool,
    /// Inner content rectangle of the Artix filter dropdown menu when visible (x, y, w, h).
    pub artix_filter_menu_rect: Option<(u16, u16, u16, u16)>,

    // Collapsed menu dropdown UI (when window is too narrow for all three buttons)
    /// Whether the collapsed menu dropdown is currently visible.
    pub collapsed_menu_open: bool,
    /// Clickable rectangle for the collapsed menu button in the Results title (x, y, w, h).
    pub collapsed_menu_button_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle of the collapsed menu dropdown when visible (x, y, w, h).
    pub collapsed_menu_rect: Option<(u16, u16, u16, u16)>,

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
    /// Whether to include packages from Artix omniverse repo in the Results view.
    pub results_filter_show_artix_omniverse: bool,
    /// Whether to include packages from Artix universe repo in the Results view.
    pub results_filter_show_artix_universe: bool,
    /// Whether to include packages from Artix lib32 repo in the Results view.
    pub results_filter_show_artix_lib32: bool,
    /// Whether to include packages from Artix galaxy repo in the Results view.
    pub results_filter_show_artix_galaxy: bool,
    /// Whether to include packages from Artix world repo in the Results view.
    pub results_filter_show_artix_world: bool,
    /// Whether to include packages from Artix system repo in the Results view.
    pub results_filter_show_artix_system: bool,
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
    /// Clickable rectangle for the `CachyOS` filter toggle in the Results title (x, y, w, h).
    pub results_filter_cachyos_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix omniverse filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_omniverse_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix universe filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_universe_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix lib32 filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_lib32_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix galaxy filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_galaxy_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix world filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_world_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Artix system filter toggle in the Results title (x, y, w, h).
    pub results_filter_artix_system_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Manjaro filter toggle in the Results title (x, y, w, h).
    pub results_filter_manjaro_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the fuzzy search mode indicator in the Search title (x, y, w, h).
    pub fuzzy_indicator_rect: Option<(u16, u16, u16, u16)>,

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
    /// Packages to resolve for preflight dependency analysis (with action for forward/reverse).
    pub preflight_deps_items: Option<(Vec<PackageItem>, crate::state::modal::PreflightAction)>,
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
    /// Last preflight dependency log state to suppress duplicate tick logs.
    pub last_logged_preflight_deps_state: Option<(usize, bool, bool)>,
    /// Cancellation flag for preflight operations (set to true when modal closes).
    pub preflight_cancelled: std::sync::Arc<std::sync::atomic::AtomicBool>,

    // Executor integration
    /// Pending executor request to be sent when `PreflightExec` modal is ready.
    pub pending_executor_request: Option<crate::install::ExecutorRequest>,
    /// Pending post-summary computation request (items and success flag to compute summary for).
    pub pending_post_summary_items: Option<(Vec<PackageItem>, Option<bool>)>,
    /// Header chips to use when transitioning to `PreflightExec` modal.
    pub pending_exec_header_chips: Option<crate::state::modal::PreflightHeaderChips>,
    /// Custom command to execute after password prompt (for special packages like paru/yay/semgrep-bin).
    pub pending_custom_command: Option<String>,
    /// Update commands to execute after password prompt (for system update).
    pub pending_update_commands: Option<Vec<String>>,
    /// AUR update command to execute conditionally if pacman fails (for system update).
    pub pending_aur_update_command: Option<String>,
    /// Password obtained from password prompt, stored temporarily for reinstall confirmation flow.
    pub pending_executor_password: Option<String>,
    /// File database sync result from background thread (checked in tick handler).
    pub pending_file_sync_result: Option<FileSyncResult>,
}

/// File database sync result type.
pub type FileSyncResult = std::sync::Arc<std::sync::Mutex<Option<Result<bool, String>>>>;

impl AppState {
    /// What: Return recent searches in most-recent-first order.
    ///
    /// Inputs:
    /// - `self`: Application state containing the recent LRU cache.
    ///
    /// Output:
    /// - Vector of recent search strings ordered from most to least recent.
    ///
    /// Details:
    /// - Clones stored values; limited to `RECENT_CAPACITY`.
    #[must_use]
    pub fn recent_values(&self) -> Vec<String> {
        self.recent.iter().map(|(_, v)| v.clone()).collect()
    }

    /// What: Fetch a recent search by positional index.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` when the index is valid; `None` otherwise.
    ///
    /// Details:
    /// - Uses the LRU iterator, so `index == 0` is the most recent entry.
    #[must_use]
    pub fn recent_value_at(&self, index: usize) -> Option<String> {
        self.recent.iter().nth(index).map(|(_, v)| v.clone())
    }

    /// What: Remove a recent search at the provided position.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` containing the removed value when found; `None` otherwise.
    ///
    /// Details:
    /// - Resolves the cache key via iteration, then pops it to maintain LRU invariants.
    pub fn remove_recent_at(&mut self, index: usize) -> Option<String> {
        let key = self.recent.iter().nth(index).map(|(k, _)| k.clone())?;
        self.recent.pop(&key)
    }

    /// What: Add or replace a news bookmark, marking state dirty.
    ///
    /// Inputs:
    /// - `bookmark`: Bookmark to insert (deduped by `item.id`).
    ///
    /// Output:
    /// - None (mutates bookmarks and dirty flag).
    pub fn add_news_bookmark(&mut self, bookmark: NewsBookmark) {
        if let Some(pos) = self
            .news_bookmarks
            .iter()
            .position(|b| b.item.id == bookmark.item.id)
        {
            self.news_bookmarks[pos] = bookmark;
        } else {
            self.news_bookmarks.push(bookmark);
        }
        self.news_bookmarks_dirty = true;
    }

    /// What: Remove a news bookmark at a position.
    ///
    /// Inputs:
    /// - `index`: Zero-based index into bookmarks vector.
    ///
    /// Output:
    /// - Removed bookmark if present.
    pub fn remove_news_bookmark_at(&mut self, index: usize) -> Option<NewsBookmark> {
        if index >= self.news_bookmarks.len() {
            return None;
        }
        let removed = self.news_bookmarks.remove(index);
        self.news_bookmarks_dirty = true;
        Some(removed)
    }

    /// What: Return recent news searches in most-recent-first order.
    ///
    /// Inputs:
    /// - `self`: Application state containing the news recent LRU cache.
    ///
    /// Output:
    /// - Vector of recent news search strings ordered from most to least recent.
    ///
    /// Details:
    /// - Clones stored values; limited by the configured recent capacity.
    #[must_use]
    pub fn news_recent_values(&self) -> Vec<String> {
        self.news_recent.iter().map(|(_, v)| v.clone()).collect()
    }

    /// What: Fetch a recent news search by positional index.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` when the index is valid; `None` otherwise.
    ///
    /// Details:
    /// - Uses the LRU iterator, so `index == 0` is the most recent entry.
    #[must_use]
    pub fn news_recent_value_at(&self, index: usize) -> Option<String> {
        self.news_recent.iter().nth(index).map(|(_, v)| v.clone())
    }

    /// What: Replace the news recent cache with the provided most-recent-first entries.
    ///
    /// Inputs:
    /// - `items`: Slice of recent news search strings ordered from most to least recent.
    ///
    /// Output:
    /// - None (mutates `self.news_recent`).
    ///
    /// Details:
    /// - Clears existing entries, enforces configured capacity, and preserves ordering by
    ///   inserting from least-recent to most-recent.
    pub fn load_news_recent_items(&mut self, items: &[String]) {
        self.news_recent.clear();
        self.news_recent.resize(recent_capacity());
        for value in items.iter().rev() {
            let stored = value.clone();
            let key = stored.to_ascii_lowercase();
            self.news_recent.put(key, stored);
        }
    }

    /// What: Remove a recent news search at the provided position.
    ///
    /// Inputs:
    /// - `index`: Zero-based position in most-recent-first ordering.
    ///
    /// Output:
    /// - `Some(String)` containing the removed value when found; `None` otherwise.
    ///
    /// Details:
    /// - Resolves the cache key via iteration, then pops it to maintain LRU invariants.
    pub fn remove_news_recent_at(&mut self, index: usize) -> Option<String> {
        let key = self.news_recent.iter().nth(index).map(|(k, _)| k.clone())?;
        self.news_recent.pop(&key)
    }

    /// What: Replace the recent cache with the provided most-recent-first entries.
    ///
    /// Inputs:
    /// - `items`: Slice of recent search strings ordered from most to least recent.
    ///
    /// Output:
    /// - None (mutates `self.recent`).
    ///
    /// Details:
    /// - Clears existing entries, enforces configured capacity, and preserves ordering by
    ///   inserting from least-recent to most-recent.
    pub fn load_recent_items(&mut self, items: &[String]) {
        self.recent.clear();
        self.recent.resize(recent_capacity());
        for value in items.iter().rev() {
            let stored = value.clone();
            let key = stored.to_ascii_lowercase();
            self.recent.put(key, stored);
        }
    }

    /// What: Recompute news results applying filters, search, age cutoff, and sorting.
    ///
    /// Inputs:
    /// - `self`: Mutable application state containing news items and filter fields.
    ///
    /// Output:
    /// - Updates `news_results`, selection state, and recent news searches.
    pub fn refresh_news_results(&mut self) {
        let query = self.news_search_input.to_lowercase();
        if query.is_empty() {
            self.news_history_pending = None;
            self.news_history_pending_at = None;
        } else {
            self.news_history_pending = Some(self.news_search_input.clone());
            self.news_history_pending_at = Some(std::time::Instant::now());
        }
        let mut filtered: Vec<NewsFeedItem> = self
            .news_items
            .iter()
            .filter(|it| match it.source {
                crate::state::types::NewsFeedSource::ArchNews => self.news_filter_show_arch_news,
                crate::state::types::NewsFeedSource::SecurityAdvisory => {
                    self.news_filter_show_advisories
                }
                crate::state::types::NewsFeedSource::InstalledPackageUpdate => {
                    self.news_filter_show_pkg_updates
                }
                crate::state::types::NewsFeedSource::AurPackageUpdate => {
                    self.news_filter_show_aur_updates
                }
                crate::state::types::NewsFeedSource::AurComment => {
                    self.news_filter_show_aur_comments
                }
            })
            .cloned()
            .collect();

        // Apply installed-only filter for advisories when enabled.
        // When "[Advisories All]" is active (news_filter_show_advisories = true,
        // news_filter_installed_only = false), this block does not run, allowing
        // all advisories to be shown regardless of installed status.
        if self.news_filter_installed_only {
            let installed: std::collections::HashSet<String> =
                crate::index::explicit_names().into_iter().collect();
            filtered.retain(|it| {
                !matches!(
                    it.source,
                    crate::state::types::NewsFeedSource::SecurityAdvisory
                ) || it.packages.iter().any(|pkg| installed.contains(pkg))
            });
        }

        if !matches!(self.news_filter_read_status, NewsReadFilter::All) {
            filtered.retain(|it| {
                let is_read = self.news_read_ids.contains(&it.id)
                    || it
                        .url
                        .as_ref()
                        .is_some_and(|u| self.news_read_urls.contains(u));
                matches!(self.news_filter_read_status, NewsReadFilter::Read) && is_read
                    || matches!(self.news_filter_read_status, NewsReadFilter::Unread) && !is_read
            });
        }

        if !query.is_empty() {
            filtered.retain(|it| {
                let hay = format!(
                    "{} {} {}",
                    it.title,
                    it.summary.clone().unwrap_or_default(),
                    it.packages.join(" ")
                )
                .to_lowercase();
                hay.contains(&query)
            });
        }

        if let Some(max_days) = self.news_max_age_days
            && let Some(cutoff_date) = Utc::now()
                .date_naive()
                .checked_sub_days(chrono::Days::new(u64::from(max_days)))
        {
            filtered.retain(|it| {
                NaiveDate::parse_from_str(&it.date, "%Y-%m-%d").map_or(true, |d| d >= cutoff_date)
            });
        }

        let is_read = |it: &NewsFeedItem| {
            self.news_read_ids.contains(&it.id)
                || it
                    .url
                    .as_ref()
                    .is_some_and(|u| self.news_read_urls.contains(u))
        };

        match self.news_sort_mode {
            NewsSortMode::DateDesc => filtered.sort_by(|a, b| b.date.cmp(&a.date)),
            NewsSortMode::DateAsc => filtered.sort_by(|a, b| a.date.cmp(&b.date)),
            NewsSortMode::Title => {
                filtered.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            }
            NewsSortMode::SourceThenTitle => filtered.sort_by(|a, b| {
                a.source
                    .cmp(&b.source)
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
            NewsSortMode::SeverityThenDate => filtered.sort_by(|a, b| {
                let sa = severity_rank(a.severity);
                let sb = severity_rank(b.severity);
                sb.cmp(&sa)
                    .then(b.date.cmp(&a.date))
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
            NewsSortMode::UnreadThenDate => filtered.sort_by(|a, b| {
                let ra = is_read(a);
                let rb = is_read(b);
                ra.cmp(&rb)
                    .then(b.date.cmp(&a.date))
                    .then(a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }),
        }

        self.news_results = filtered;
        if self.news_results.is_empty() {
            self.news_selected = 0;
            self.news_list_state.select(None);
        } else {
            self.news_selected = self
                .news_selected
                .min(self.news_results.len().saturating_sub(1));
            self.news_list_state.select(Some(self.news_selected));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::state::types::{
        AdvisorySeverity, NewsFeedItem, NewsFeedSource, NewsReadFilter, NewsSortMode,
    };

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
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        // Shim HOME so lists_dir() resolves under a temp dir
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_state_default_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let app = AppState::default();
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
        assert!(app.news_read_ids_path.starts_with(&lists));

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    /// What: Ensure news filtering respects per-source toggles for updates and comments.
    ///
    /// Inputs:
    /// - Five news items spanning Arch, advisory, official update, AUR update, and AUR comment.
    /// - Filters that disable Arch/advisory/update sources while leaving AUR comments enabled.
    ///
    /// Output:
    /// - `news_results` retains only the enabled source after applying filters.
    ///
    /// Details:
    /// - Uses the global test mutex and HOME shim to avoid path collisions with other tests.
    fn refresh_news_results_applies_all_source_filters() {
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_news_filters_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let mut app = AppState::default();
        app.news_items = vec![
            NewsFeedItem {
                id: "arch".into(),
                date: "2025-01-01".into(),
                title: "Arch".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "adv".into(),
                date: "2025-01-01".into(),
                title: "ADV".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::High),
                packages: vec!["openssl".into()],
            },
            NewsFeedItem {
                id: "upd-official".into(),
                date: "2025-01-01".into(),
                title: "Official update".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::InstalledPackageUpdate,
                severity: None,
                packages: vec!["pacman".into()],
            },
            NewsFeedItem {
                id: "upd-aur".into(),
                date: "2025-01-01".into(),
                title: "AUR update".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::AurPackageUpdate,
                severity: None,
                packages: vec!["yay".into()],
            },
            NewsFeedItem {
                id: "comment".into(),
                date: "2025-01-01".into(),
                title: "New comment".into(),
                summary: Some("hello".into()),
                url: None,
                source: NewsFeedSource::AurComment,
                severity: None,
                packages: vec!["yay".into()],
            },
        ];
        app.news_filter_show_arch_news = false;
        app.news_filter_show_advisories = false;
        app.news_filter_show_pkg_updates = false;
        app.news_filter_show_aur_updates = false;
        app.news_filter_show_aur_comments = true;
        app.news_filter_installed_only = false;
        app.news_max_age_days = None;

        app.refresh_news_results();
        assert_eq!(app.news_results.len(), 1);
        assert_eq!(app.news_results[0].id, "comment");

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    /// What: Ensure news read filter respects read/unread selections.
    ///
    /// Inputs:
    /// - Two news items with distinct IDs and URLs.
    /// - `news_read_ids` containing one of the items.
    ///
    /// Output:
    /// - `news_results` reflect the selected read filter (All/Unread/Read).
    ///
    /// Details:
    /// - Uses HOME shim to avoid collisions with persisted paths.
    fn refresh_news_results_applies_read_filter() {
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_news_read_filter_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let mut app = AppState::default();
        app.news_items = vec![
            NewsFeedItem {
                id: "read".into(),
                date: "2025-01-01".into(),
                title: "Read item".into(),
                summary: None,
                url: Some("https://example.com/read".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "unread".into(),
                date: "2025-01-02".into(),
                title: "Unread item".into(),
                summary: None,
                url: Some("https://example.com/unread".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
        ];
        app.news_read_ids.insert("read".into());
        app.news_filter_read_status = NewsReadFilter::Unread;
        app.news_max_age_days = None;

        app.refresh_news_results();
        assert_eq!(app.news_results.len(), 1);
        assert_eq!(app.news_results[0].id, "unread");

        app.news_filter_read_status = NewsReadFilter::Read;
        app.refresh_news_results();
        assert_eq!(app.news_results.len(), 1);
        assert_eq!(app.news_results[0].id, "read");

        app.news_filter_read_status = NewsReadFilter::All;
        app.refresh_news_results();
        assert_eq!(app.news_results.len(), 2);

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    /// What: Ensure "[Advisories All]" filter shows all advisories regardless of installed status.
    ///
    /// Inputs:
    /// - Advisories for both installed and non-installed packages.
    /// - `news_filter_show_advisories = true` and `news_filter_installed_only = false`.
    ///
    /// Output:
    /// - All advisories are shown in `news_results`.
    ///
    /// Details:
    /// - Verifies that "[Advisories All]" behaves as if [Installed only] filter was off
    ///   and [Advisories] filter was on.
    /// - When `news_filter_installed_only = false`, the installed-only filtering block
    ///   (lines 914-923) should not run, allowing all advisories to pass through.
    /// - Uses HOME shim to avoid collisions with persisted paths.
    fn refresh_news_results_advisories_all_shows_all() {
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_advisories_all_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let mut app = AppState::default();

        app.news_items = vec![
            NewsFeedItem {
                id: "adv-1".into(),
                date: "2025-01-01".into(),
                title: "Advisory 1".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::High),
                packages: vec!["package1".into()],
            },
            NewsFeedItem {
                id: "adv-2".into(),
                date: "2025-01-02".into(),
                title: "Advisory 2".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::Medium),
                packages: vec!["package2".into()],
            },
            NewsFeedItem {
                id: "adv-3".into(),
                date: "2025-01-03".into(),
                title: "Advisory 3".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::Critical),
                packages: vec!["package3".into(), "package4".into()],
            },
        ];

        // Set up "[Advisories All]" state: advisories on, installed_only off
        // This should show all advisories regardless of whether packages are installed
        app.news_filter_show_advisories = true;
        app.news_filter_installed_only = false;
        app.news_filter_show_arch_news = false;
        app.news_filter_show_pkg_updates = false;
        app.news_filter_show_aur_updates = false;
        app.news_filter_show_aur_comments = false;
        app.news_max_age_days = None;

        app.refresh_news_results();

        // All advisories should be shown when [Advisories All] is active
        // (news_filter_show_advisories = true, news_filter_installed_only = false)
        assert_eq!(
            app.news_results.len(),
            3,
            "All advisories should be shown when [Advisories All] is active (advisories on, installed_only off)"
        );
        assert!(app.news_results.iter().any(|it| it.id == "adv-1"));
        assert!(app.news_results.iter().any(|it| it.id == "adv-2"));
        assert!(app.news_results.iter().any(|it| it.id == "adv-3"));

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    /// What: Verify severity-first news sort orders higher severities before date and title tiebreaks.
    ///
    /// Inputs:
    /// - Mixed advisory severities with overlapping dates.
    ///
    /// Output:
    /// - `news_results` starts with Critical, then High (newest first), then Medium/Unknown.
    ///
    /// Details:
    /// - Uses HOME shim to avoid touching real persisted files.
    fn refresh_news_results_sorts_by_severity_then_date() {
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_news_sort_severity_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let mut app = AppState::default();
        app.news_items = vec![
            NewsFeedItem {
                id: "crit".into(),
                date: "2025-01-01".into(),
                title: "critical".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::Critical),
                packages: vec![],
            },
            NewsFeedItem {
                id: "high-new".into(),
                date: "2025-01-03".into(),
                title: "high-new".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::High),
                packages: vec![],
            },
            NewsFeedItem {
                id: "high-old".into(),
                date: "2025-01-02".into(),
                title: "high-old".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::High),
                packages: vec![],
            },
            NewsFeedItem {
                id: "unknown".into(),
                date: "2025-01-04".into(),
                title: "unknown".into(),
                summary: None,
                url: None,
                source: NewsFeedSource::SecurityAdvisory,
                severity: Some(AdvisorySeverity::Unknown),
                packages: vec![],
            },
        ];
        app.news_filter_show_advisories = true;
        app.news_filter_installed_only = false;
        app.news_filter_show_arch_news = false;
        app.news_filter_show_pkg_updates = false;
        app.news_filter_show_aur_updates = false;
        app.news_filter_show_aur_comments = false;
        app.news_max_age_days = None;
        app.news_sort_mode = NewsSortMode::SeverityThenDate;
        app.refresh_news_results();
        let ids: Vec<String> = app.news_results.iter().map(|it| it.id.clone()).collect();
        assert_eq!(ids, vec!["crit", "high-new", "high-old", "unknown"]);

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[test]
    #[allow(clippy::field_reassign_with_default)]
    /// What: Verify unread-first sorting promotes unread items ahead of read ones, then newest-first.
    ///
    /// Inputs:
    /// - Mixed read/unread items with different dates.
    ///
    /// Output:
    /// - Unread entries appear before read entries; newest unread first.
    ///
    /// Details:
    /// - Uses URL-based read markers to ensure both id/url markers are honoured.
    fn refresh_news_results_sorts_unread_first_then_date() {
        let _guard = crate::state::test_mutex()
            .lock()
            .expect("Test mutex poisoned");
        let orig_home = std::env::var_os("HOME");
        let dir = std::env::temp_dir().join(format!(
            "pacsea_test_news_sort_unread_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        unsafe { std::env::set_var("HOME", dir.display().to_string()) };

        let mut app = AppState::default();
        app.news_items = vec![
            NewsFeedItem {
                id: "read-old".into(),
                date: "2025-01-01".into(),
                title: "read-old".into(),
                summary: None,
                url: Some("https://example.com/read-old".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "read-new".into(),
                date: "2025-01-04".into(),
                title: "read-new".into(),
                summary: None,
                url: Some("https://example.com/read-new".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "unread-old".into(),
                date: "2025-01-02".into(),
                title: "unread-old".into(),
                summary: None,
                url: Some("https://example.com/unread-old".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
            NewsFeedItem {
                id: "unread-new".into(),
                date: "2025-01-05".into(),
                title: "unread-new".into(),
                summary: None,
                url: Some("https://example.com/unread-new".into()),
                source: NewsFeedSource::ArchNews,
                severity: None,
                packages: vec![],
            },
        ];
        app.news_filter_show_arch_news = true;
        app.news_filter_show_advisories = false;
        app.news_filter_show_pkg_updates = false;
        app.news_filter_show_aur_updates = false;
        app.news_filter_show_aur_comments = false;
        app.news_filter_installed_only = false;
        app.news_max_age_days = None;
        app.news_read_urls
            .insert("https://example.com/read-old".into());
        app.news_read_ids.insert("read-new".into());
        app.news_sort_mode = NewsSortMode::UnreadThenDate;

        app.refresh_news_results();
        let ids: Vec<String> = app.news_results.iter().map(|it| it.id.clone()).collect();
        assert_eq!(
            ids,
            vec![
                "unread-new".to_string(),
                "unread-old".to_string(),
                "read-new".to_string(),
                "read-old".to_string()
            ]
        );

        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}
