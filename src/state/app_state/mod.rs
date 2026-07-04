//! Central `AppState` container, split out from the monolithic module.

use lru::LruCache;
use ratatui::widgets::ListState;
use std::{
    collections::HashMap, collections::HashSet, collections::VecDeque, path::PathBuf, time::Instant,
};

use crate::sources::VoteAction;
use crate::state::config_editor::ConfigEditorState;
use crate::state::modal::{
    CascadeMode, Modal, PreflightAction, RepoOverlapApplyPending, RepositoriesModalResume,
    ServiceImpact,
};
use crate::state::types::{
    AppMode, ArchStatusColor, Focus, InstalledPackagesMode, NewsFeedItem, NewsReadFilter,
    NewsSortMode, PackageDetails, PackageItem, RightPaneFocus, SortMode,
};
use crate::theme::KeyMap;

mod constants;
mod default_impl;
mod defaults;
mod defaults_cache;
mod methods;

#[cfg(test)]
mod tests;

pub use constants::{FileSyncResult, RECENT_CAPACITY, recent_capacity};

/// What: UI-facing live vote-state for an AUR package.
///
/// Details:
/// - `Unknown`: no live check requested yet.
/// - `Loading`: background check is currently in flight.
/// - `Voted`: current user has voted for the package.
/// - `NotVoted`: current user has not voted for the package.
/// - `Error`: last check failed with a short user-facing reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AurVoteStateUi {
    /// No vote-state has been requested yet.
    Unknown,
    /// Vote-state request is currently running in background.
    Loading,
    /// Current user has voted for the package.
    Voted,
    /// Current user has not voted for the package.
    NotVoted,
    /// Vote-state request failed.
    Error(String),
}

/// What: Execution status for PKGBUILD static checks in the preview panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgbuildCheckStatus {
    /// No checks have been requested yet.
    Idle,
    /// Checks are currently running in a background worker.
    Running,
    /// Checks completed and data is available for rendering.
    Complete,
}

/// What: Supported static checker tool names for PKGBUILD preview checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgbuildCheckTool {
    /// `shellcheck` output.
    Shellcheck,
    /// `namcap` output.
    Namcap,
}

/// What: Severity of a parsed PKGBUILD check finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PkgbuildCheckSeverity {
    /// High-confidence error that should be fixed.
    Error,
    /// Warning that likely needs manual review.
    Warning,
    /// Informational note from checker output.
    Info,
}

/// What: Parsed finding line for PKGBUILD static check output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkgbuildCheckFinding {
    /// Tool that produced the finding.
    pub tool: PkgbuildCheckTool,
    /// Parsed severity level.
    pub severity: PkgbuildCheckSeverity,
    /// Optional line number in PKGBUILD, if parseable.
    pub line: Option<u32>,
    /// User-facing message to show in the findings list.
    pub message: String,
}

/// What: Raw execution result for an individual PKGBUILD checker tool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PkgbuildToolRawResult {
    /// Tool name.
    pub tool: PkgbuildCheckTool,
    /// Whether the tool binary was available on `PATH`.
    pub available: bool,
    /// Exit code when executed.
    pub exit_code: Option<i32>,
    /// Whether execution timed out.
    pub timed_out: bool,
    /// Exact command string executed (or would be executed in dry-run).
    pub command: String,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
}

/// Global application state shared by the event, networking, and UI layers.
///
/// This structure is mutated frequently in response to input and background
/// updates. Certain subsets are persisted to disk to preserve user context
/// across runs (e.g., recent searches, details cache, install list).
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct AppState {
    /// Current top-level mode (package management, news feed, or config editor).
    pub app_mode: AppMode,
    /// Persistent integrated config editor state used while in `AppMode::ConfigEditor`.
    pub config_editor_state: Box<ConfigEditorState>,
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
    /// Whether the last completed update check used an authoritative official-repo source (`None` before first result).
    pub updates_last_check_authoritative: Option<bool>,
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
    /// Active subsection for `Ctrl+D` rotation: 0 = PKGBUILD body, 1 = `ShellCheck`, 2 = `Namcap`.
    pub pkgb_section_cycle: u8,
    /// Content rectangle of the PKGBUILD viewer (x, y, w, h) when visible.
    pub pkgb_rect: Option<(u16, u16, u16, u16)>,
    /// Rectangle of the clickable "Run checks" button in PKGBUILD title.
    pub pkgb_run_checks_button_rect: Option<(u16, u16, u16, u16)>,
    /// Current status of PKGBUILD checks in preview panel.
    pub pkgb_check_status: PkgbuildCheckStatus,
    /// Parsed findings from latest PKGBUILD check run.
    pub pkgb_check_findings: Vec<PkgbuildCheckFinding>,
    /// Raw per-tool outputs from latest PKGBUILD check run.
    pub pkgb_check_raw_results: Vec<PkgbuildToolRawResult>,
    /// Missing tool hints shown when ShellCheck/namcap are unavailable.
    pub pkgb_check_missing_tools: Vec<String>,
    /// Whether raw output panel is expanded in PKGBUILD preview.
    pub pkgb_check_show_raw_output: bool,
    /// Scroll offset for parsed findings list.
    pub pkgb_check_scroll: u16,
    /// Scroll offset for raw output panel.
    pub pkgb_check_raw_scroll: u16,
    /// Last package name for which checks were run.
    pub pkgb_check_last_package_name: Option<String>,
    /// Last completion timestamp for checks.
    pub pkgb_check_last_run_at: Option<Instant>,
    /// Last error text for check execution path.
    pub pkgb_check_last_error: Option<String>,

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
    /// Top-to-bottom order of the main vertical stack (results, middle, package info).
    pub main_pane_order: [crate::state::MainVerticalPane; 3],
    /// Min/max row counts for vertical layout (semantic per pane, not screen slot).
    pub vertical_layout_limits: crate::state::VerticalLayoutLimits,
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
    /// Startup setup steps queued from first-run setup selector.
    pub pending_startup_setup_steps: VecDeque<crate::state::modal::StartupSetupTask>,
    /// Flag to trigger startup news fetch after `NewsSetup` is completed.
    pub trigger_startup_news_fetch: bool,
    /// Session-scoped latch to avoid repeatedly showing long-run auth preflight warning text.
    pub long_run_auth_preflight_warned: bool,

    // Updates modal mouse hit-testing
    /// Outer rectangle of the Updates modal (including borders) when visible.
    pub updates_modal_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the `Wizard` button in the Optional Deps modal.
    pub optional_deps_wizard_rect: Option<(u16, u16, u16, u16)>,
    /// Outer rectangle of the Optional Deps modal for wheel hit-testing.
    pub optional_deps_modal_rect: Option<(u16, u16, u16, u16)>,
    /// Outer rectangle of the System Update modal for wheel hit-testing.
    pub system_update_modal_rect: Option<(u16, u16, u16, u16)>,
    /// Outer rectangle of the Repositories modal for wheel hit-testing.
    pub repositories_modal_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable row for copying the SSH public key in the AUR SSH setup modal.
    pub ssh_setup_copy_key_rect: Option<(u16, u16, u16, u16)>,
    /// Inner content rectangle for scrollable updates list.
    pub updates_modal_content_rect: Option<(u16, u16, u16, u16)>,
    /// Per-entry starting rendered line indices for the updates modal content.
    ///
    /// Each value maps an entry index to its first rendered line in the wrapped pane output.
    pub updates_modal_entry_line_starts: Vec<u16>,
    /// Total rendered line count across all updates entries after wrapping.
    pub updates_modal_total_lines: u16,
    /// Timestamp when `g` was pressed in Updates modal awaiting chord completion.
    pub updates_modal_pending_g_at: Option<Instant>,

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

    /// Whether the custom `repos.conf` results-filter dropdown is visible.
    pub custom_repos_filter_menu_open: bool,
    /// Inner hit-test rect for the custom repos filter dropdown when visible.
    pub custom_repos_filter_menu_rect: Option<(u16, u16, u16, u16)>,

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
    /// Whether to include packages from the `blackarch` repo in the Results view.
    pub results_filter_show_blackarch: bool,
    /// Whether to include packages labeled as `manjaro` in the Results view.
    pub results_filter_show_manjaro: bool,
    /// Lowercase pacman `[repo]` name → canonical `results_filter` id from `repos.conf`.
    pub repo_results_filter_by_name: HashMap<String, String>,
    /// Per dynamic filter id (canonical), whether search results include packages from mapped repos.
    pub results_filter_dynamic: HashMap<String, bool>,
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
    /// Clickable rectangle for the `BlackArch` filter toggle in the Results title (x, y, w, h).
    pub results_filter_blackarch_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the Manjaro filter toggle in the Results title (x, y, w, h).
    pub results_filter_manjaro_rect: Option<(u16, u16, u16, u16)>,
    /// Clickable rectangle for the custom `repos.conf` filter dropdown chip (x, y, w, h).
    pub results_filter_custom_repos_rect: Option<(u16, u16, u16, u16)>,
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
    /// Pending AUR vote intent (pkgbase and action) awaiting user confirmation.
    pub pending_aur_vote_intent: Option<(String, VoteAction)>,
    /// Pending AUR vote request (pkgbase and action) to be sent by the runtime tick handler.
    pub pending_aur_vote_request: Option<(String, VoteAction)>,
    /// Live AUR vote-state cache keyed by pkgbase/package name.
    pub aur_vote_state_by_pkgbase: HashMap<String, AurVoteStateUi>,
    /// Path where persisted AUR vote-state cache is stored as JSON.
    pub aur_vote_state_path: PathBuf,
    /// Dirty flag indicating `aur_vote_state_by_pkgbase` needs to be saved.
    pub aur_vote_state_dirty: bool,
    /// Whether live AUR vote-state lookup is available in current runtime session.
    ///
    /// Details:
    /// - Set to `false` after first unsupported `list-votes` response to avoid repeatedly
    ///   replacing stable cached states with transient `Loading`/`Unknown`.
    pub aur_vote_state_lookup_supported: bool,
    /// Pending AUR vote-state check request (pkgbase) to be sent by the runtime tick handler.
    pub pending_aur_vote_state_request: Option<String>,
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
    /// Repo apply commands after password prompt (custom `repos.conf` apply).
    pub pending_repo_apply_commands: Option<Vec<String>>,
    /// Summary lines to seed `PreflightExec` when starting a repo apply.
    pub pending_repo_apply_summary: Option<Vec<String>>,
    /// Pending foreign∩sync overlap check after a successful full repository apply.
    pub pending_repo_apply_overlap_check: Option<RepoOverlapApplyPending>,
    /// Reopen the Repositories modal with a rescanned pacman view after repo apply completes.
    pub pending_repositories_modal_resume: Option<RepositoriesModalResume>,
    /// Privileged shell commands for foreign→sync migration (`PasswordPurpose::RepoForeignMigrate`).
    pub pending_foreign_migrate_commands: Option<Vec<String>>,
    /// Summary lines for foreign→sync migration preflight log.
    pub pending_foreign_migrate_summary: Option<Vec<String>>,
    /// Skips the next AUR-vs-repo duplicate-results warning (after user continues once).
    pub skip_aur_repo_dup_warning_once: bool,
    /// AUR update command to execute conditionally if pacman fails (for system update).
    pub pending_aur_update_command: Option<String>,
    /// Password obtained from password prompt, stored temporarily for reinstall confirmation flow.
    pub pending_executor_password: Option<crate::state::SecureString>,
    /// File database sync result from background thread (checked in tick handler).
    pub pending_file_sync_result: Option<FileSyncResult>,
    /// Background AUR SSH validation result handle for Optional Deps status refresh.
    pub pending_aur_ssh_help_check_result: Option<std::sync::Arc<std::sync::Mutex<Option<bool>>>>,
    /// Latest AUR SSH help validation result (`Some(true/false)`) from the background check.
    pub aur_ssh_help_ready: Option<bool>,
}
