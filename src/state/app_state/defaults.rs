//! Default initialization helpers for `AppState`.

use lru::LruCache;
use ratatui::widgets::ListState;
use serde_json;
use std::fs;
use std::{collections::HashMap, collections::HashSet, path::PathBuf, time::Instant};

use crate::state::modal::Modal;
use crate::state::types::{
    AppMode, ArchStatusColor, Focus, NewsFeedItem, NewsReadFilter, NewsSortMode, PackageDetails,
    PackageItem, SortMode,
};
use crate::theme::KeyMap;

/// What: Create default paths for persisted data.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of default paths for recent searches, cache, news, install list, and various caches.
///
/// Details:
/// - All paths are under the lists directory from theme configuration.
pub(super) fn default_paths() -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let lists_dir = crate::theme::lists_dir();
    (
        lists_dir.join("recent_searches.json"),
        lists_dir.join("details_cache.json"),
        lists_dir.join("news_read_urls.json"),
        lists_dir.join("news_read_ids.json"),
        lists_dir.join("install_list.json"),
        lists_dir.join("official_index.json"),
        lists_dir.join("install_deps_cache.json"),
        lists_dir.join("file_cache.json"),
        lists_dir.join("services_cache.json"),
        lists_dir.join("announcement_read.json"),
        lists_dir.join("news_recent_searches.json"),
        lists_dir.join("news_bookmarks.json"),
    )
}

/// Type alias for default filter state tuple.
///
/// Contains 13 boolean flags for repository filters and an array of 13 optional rects.
pub(super) type DefaultFilters = (
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    bool,
    [Option<(u16, u16, u16, u16)>; 13],
);

/// Type alias for default search state tuple.
pub(super) type DefaultSearchState = (
    String,
    Vec<PackageItem>,
    Vec<PackageItem>,
    Option<Vec<PackageItem>>,
    usize,
    PackageDetails,
    ListState,
    Modal,
    Option<Modal>,
    bool,
    Focus,
    Instant,
    Option<String>,
    u64,
    u64,
    Option<String>,
    bool,
    Option<Vec<PackageItem>>,
);

/// Type alias for default news feed state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultNewsFeedState = (
    Vec<NewsFeedItem>,
    Vec<NewsFeedItem>,
    bool,
    usize,
    ListState,
    String,
    usize,
    Option<usize>,
    LruCache<String, String>,
    PathBuf,
    bool, // news_recent_dirty
    bool, // news_filter_show_arch_news
    bool, // news_filter_show_advisories
    bool, // news_filter_show_pkg_updates
    bool, // news_filter_show_aur_updates
    bool, // news_filter_show_aur_comments
    bool, // news_filter_installed_only
    NewsReadFilter,
    Option<(u16, u16, u16, u16)>, // arch rect
    Option<(u16, u16, u16, u16)>, // advisory rect
    Option<(u16, u16, u16, u16)>, // installed rect
    Option<(u16, u16, u16, u16)>, // updates rect
    Option<(u16, u16, u16, u16)>, // aur updates rect
    Option<(u16, u16, u16, u16)>, // aur comments rect
    Option<(u16, u16, u16, u16)>, // read rect
    Option<u32>,                  // max age days
    bool,                         // show history pane
    bool,                         // show bookmarks pane
    NewsSortMode,                 // sort mode
    Vec<crate::state::types::NewsBookmark>,
    PathBuf,                                   // bookmarks path
    bool,                                      // bookmarks dirty
    std::collections::HashMap<String, String>, // news_content_cache
    Option<String>,                            // news_content
    bool,                                      // news_content_loading
    Option<Instant>,                           // news_content_loading_since
    u16,                                       // news_content_scroll
    Option<String>,                            // news_history_pending
    Option<Instant>,                           // news_history_pending_at
    Option<String>,                            // news_history_last_saved
);

/// What: Default application mode.
///
/// Inputs: None
///
/// Output: `AppMode::Package`
#[must_use]
pub(super) const fn default_app_mode() -> AppMode {
    AppMode::Package
}

/// What: Create default state for the news feed.
///
/// Inputs:
/// - `news_recent_path`: Path to persist news recent searches
/// - `news_bookmarks_path`: Path to persist news bookmarks
///
/// Output:
/// - Tuple containing news feed data, UI state, and persistence flags.
pub(super) fn default_news_feed_state(
    news_recent_path: PathBuf,
    news_bookmarks_path: PathBuf,
    news_feed_path: &PathBuf,
) -> DefaultNewsFeedState {
    let recent_capacity = super::recent_capacity();
    let mut news_recent = LruCache::unbounded();
    news_recent.resize(recent_capacity);
    if let Ok(s) = fs::read_to_string(&news_recent_path)
        && let Ok(values) = serde_json::from_str::<Vec<String>>(&s)
    {
        for v in values.into_iter().rev() {
            let key = v.to_ascii_lowercase();
            news_recent.put(key, v);
        }
    }
    let news_bookmarks: Vec<crate::state::types::NewsBookmark> =
        fs::read_to_string(&news_bookmarks_path)
            .ok()
            .and_then(|s| serde_json::from_str::<Vec<crate::state::types::NewsBookmark>>(&s).ok())
            .or_else(|| {
                // Backward compatibility: load old format Vec<NewsFeedItem>
                fs::read_to_string(&news_bookmarks_path)
                    .ok()
                    .and_then(|s| serde_json::from_str::<Vec<NewsFeedItem>>(&s).ok())
                    .map(|items| {
                        items
                            .into_iter()
                            .map(|item| crate::state::types::NewsBookmark {
                                item,
                                content: None,
                                html_path: None,
                            })
                            .collect()
                    })
            })
            .unwrap_or_default();
    let cached_items: Vec<crate::state::types::NewsFeedItem> = fs::read_to_string(news_feed_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let news_loading = cached_items.is_empty();
    (
        cached_items.clone(), // news_items
        cached_items,         // news_results (filtered later)
        news_loading,         // news_loading
        0,                    // news_selected
        ListState::default(), // news_list_state
        String::new(),        // news_search_input
        0,                    // news_search_caret
        None,                 // news_search_select_anchor
        news_recent,
        news_recent_path,
        false,               // news_recent_dirty
        true,                // news_filter_show_arch_news
        true,                // news_filter_show_advisories
        true,                // news_filter_show_pkg_updates
        true,                // news_filter_show_aur_updates
        true,                // news_filter_show_aur_comments
        false,               // news_filter_installed_only
        NewsReadFilter::All, // news_filter_read_status
        None,                // news_filter_arch_rect
        None,                // news_filter_advisory_rect
        None,                // news_filter_installed_rect
        None,                // news_filter_updates_rect
        None,                // news_filter_aur_updates_rect
        None,                // news_filter_aur_comments_rect
        None,                // news_filter_read_rect
        Some(30),
        true, // show_news_history_pane
        true, // show_news_bookmarks_pane
        NewsSortMode::DateDesc,
        news_bookmarks, // news_bookmarks
        news_bookmarks_path,
        false,                            // news_bookmarks_dirty
        std::collections::HashMap::new(), // news_content_cache
        None,                             // news_content
        false,                            // news_content_loading
        None,                             // news_content_loading_since
        0,                                // news_content_scroll
        None,                             // news_history_pending
        None,                             // news_history_pending_at
        None,                             // news_history_last_saved
    )
}

/// Type alias for default install lists state tuple.
pub(super) type DefaultInstallListsState = (
    Vec<PackageItem>,
    ListState,
    Vec<PackageItem>,
    ListState,
    Vec<PackageItem>,
    ListState,
    PathBuf,
    bool,
    Option<Instant>,
    HashSet<String>,
    HashSet<String>,
    HashSet<String>,
);

/// Type alias for default clickable rectangles state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultClickableRectsState = (
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    String,
    Option<(u16, u16, u16, u16)>,
    ArchStatusColor,
    Option<usize>,
    Vec<String>,
    Option<(u16, u16, u16, u16)>,
    bool,
    bool,
    bool,
    bool,
    Option<std::time::SystemTime>,
    Option<u32>,
);

/// Type alias for default PKGBUILD state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultPkgbuildState = (
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    bool,
    Option<String>,
    Option<String>,
    Option<Instant>,
    Option<String>,
    u16,
    Option<(u16, u16, u16, u16)>,
);

/// Type alias for default comments state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultCommentsState = (
    Option<(u16, u16, u16, u16)>,
    bool,
    Vec<crate::state::types::AurComment>,
    Option<String>,
    Option<Instant>,
    u16,
    Option<(u16, u16, u16, u16)>,
    bool,
    Option<String>,
    Vec<(u16, u16, u16, String)>,
    Vec<(u16, u16, u16, String)>,
    Vec<(u16, u16, u16, String)>,
);

/// Type alias for default mouse hit-test state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultMouseHitTestState = (
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    u16,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    bool,
    Option<(u16, u16)>,
    bool,
);

/// Type alias for default modal rectangles state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultModalRectsState = (
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Vec<(u16, u16, u16, String)>,
    Vec<crate::announcements::RemoteAnnouncement>,
    Option<Vec<crate::state::NewsItem>>,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    u16,
    Option<(u16, u16, u16, u16)>,
    [Option<(u16, u16, u16, u16)>; 5],
    Option<(u16, u16, u16, u16)>,
);

/// Type alias for default sorting menus state tuple.
#[allow(clippy::type_complexity)]
pub(super) type DefaultSortingMenusState = (
    SortMode,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<Instant>,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    bool,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    bool,
    Option<(u16, u16, u16, u16)>,
    Option<(u16, u16, u16, u16)>,
    // Sort result caching fields
    Option<Vec<usize>>,
    Option<Vec<usize>>,
    Option<u64>,
);

/// What: Create default filter state (all filters enabled).
///
/// Inputs: None.
///
/// Output:
/// - Tuple of filter boolean flags and rect options.
///
/// Details:
/// - All repository filters default to showing everything.
pub(super) const fn default_filters() -> DefaultFilters {
    (
        true,       // show_aur
        true,       // show_core
        true,       // show_extra
        true,       // show_multilib
        true,       // show_eos
        true,       // show_cachyos
        true,       // show_artix
        true,       // show_artix_omniverse
        true,       // show_artix_universe
        true,       // show_artix_lib32
        true,       // show_artix_galaxy
        true,       // show_artix_world
        true,       // show_artix_system
        [None; 13], // filter rects
    )
}

/// What: Create default search state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of search-related fields: `input`, `results`, `all_results`, `results_backup_for_toggle`, `selected`, `details`, `list_state`, `modal`, `previous_modal`, `dry_run`, `focus`, `last_input_change`, `last_saved_value`, `latest_query_id`, `next_query_id`.
///
/// Details:
/// - Initializes search input, results, selection state, modal state, and query coordination.
#[allow(clippy::type_complexity)]
pub(super) fn default_search_state() -> DefaultSearchState {
    (
        String::new(),
        Vec::new(),
        Vec::new(),
        None,
        0,
        PackageDetails::default(),
        ListState::default(),
        Modal::None,
        None,
        false,
        Focus::Search,
        Instant::now(),
        None,
        0,
        1,
        None,
        false,
        None,
    )
}

/// What: Create default recent searches state.
///
/// Inputs:
/// - `recent_path`: Path where recent searches are persisted.
///
/// Output:
/// - Tuple of recent searches fields: `recent`, `history_state`, `recent_path`, `recent_dirty`.
///
/// Details:
/// - Initializes empty recent searches list and selection state.
pub(super) fn default_recent_state(
    recent_path: PathBuf,
) -> (LruCache<String, String>, ListState, PathBuf, bool) {
    (
        LruCache::new(super::recent_capacity()),
        ListState::default(),
        recent_path,
        false,
    )
}

/// What: Create default details cache state.
///
/// Inputs:
/// - `cache_path`: Path where the details cache is persisted.
///
/// Output:
/// - Tuple of details cache fields: `details_cache`, `cache_path`, `cache_dirty`.
///
/// Details:
/// - Initializes empty details cache.
pub(super) fn default_details_cache_state(
    cache_path: PathBuf,
) -> (HashMap<String, PackageDetails>, PathBuf, bool) {
    (HashMap::new(), cache_path, false)
}

/// What: Create default news state.
///
/// Inputs:
/// - `news_read_path`: Path where read news URLs are persisted.
///
/// Output:
/// - Tuple of news fields: `news_read_urls`, `news_read_path`, `news_read_dirty`.
///
/// Details:
/// - Initializes empty set of read news URLs.
pub(super) fn default_news_state(
    news_read_path: PathBuf,
) -> (std::collections::HashSet<String>, PathBuf, bool) {
    (std::collections::HashSet::new(), news_read_path, false)
}

/// What: Create default read-IDs state for news feed items.
///
/// Inputs:
/// - `news_read_ids_path`: Path where read news IDs are persisted.
///
/// Output:
/// - Tuple of news read-id fields: `news_read_ids`, `news_read_ids_path`, `news_read_ids_dirty`.
///
/// Details:
/// - Initializes empty set of read news IDs.
pub(super) fn default_news_read_ids_state(
    news_read_ids_path: PathBuf,
) -> (std::collections::HashSet<String>, PathBuf, bool) {
    (std::collections::HashSet::new(), news_read_ids_path, false)
}

/// What: Create default announcement state.
///
/// Inputs:
/// - `announcement_read_path`: Path where the read announcement IDs are persisted.
///
/// Output:
/// - Tuple of announcement fields: `announcements_read_ids`, `announcement_read_path`, `announcement_dirty`.
///
/// Details:
/// - Initializes empty set of read announcement IDs.
pub(super) fn default_announcement_state(
    announcement_read_path: PathBuf,
) -> (std::collections::HashSet<String>, PathBuf, bool) {
    (
        std::collections::HashSet::new(),
        announcement_read_path,
        false,
    )
}

/// What: Create default install lists state.
///
/// Inputs:
/// - `install_path`: Path where the install list is persisted.
///
/// Output:
/// - Tuple of install/remove/downgrade list fields: `install_list`, `install_state`, `remove_list`, `remove_state`, `downgrade_list`, `downgrade_state`, `install_path`, `install_dirty`, `last_install_change`.
///
/// Details:
/// - Initializes empty install, remove, and downgrade lists with their selection states.
#[allow(clippy::type_complexity)]
pub(super) fn default_install_lists_state(install_path: PathBuf) -> DefaultInstallListsState {
    (
        Vec::new(),
        ListState::default(),
        Vec::new(),
        ListState::default(),
        Vec::new(),
        ListState::default(),
        install_path,
        false,
        None,
        HashSet::new(),
        HashSet::new(),
        HashSet::new(),
    )
}

/// What: Create default UI visibility state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of UI visibility fields: `show_recent_pane`, `show_install_pane`, `show_keybinds_footer`, `pane_find`.
///
/// Details:
/// - Middle row panes are visible by default.
pub(super) const fn default_ui_visibility_state() -> (bool, bool, bool, Option<String>) {
    (true, true, true, None)
}

/// What: Create default search input state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of search input mode fields: `search_normal_mode`, `fuzzy_search_enabled`, `search_caret`, `search_select_anchor`.
///
/// Details:
/// - Search starts in Insert mode with caret at position 0, fuzzy search disabled.
pub(super) const fn default_search_input_state() -> (bool, bool, usize, Option<usize>) {
    (false, false, 0, None)
}

/// What: Create default index state.
///
/// Inputs:
/// - `official_index_path`: Path to the persisted official package index.
///
/// Output:
/// - Tuple of index fields: `official_index_path`, `loading_index`, `details_focus`.
///
/// Details:
/// - Index is not loading by default, no package details are focused.
#[allow(clippy::missing_const_for_fn)]
pub(super) fn default_index_state(official_index_path: PathBuf) -> (PathBuf, bool, Option<String>) {
    (official_index_path, false, None)
}

/// What: Create default scroll and prefetch state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of scroll/prefetch fields: `scroll_moves`, `ring_resume_at`, `need_ring_prefetch`.
///
/// Details:
/// - Ring prefetch is not needed initially.
pub(super) const fn default_scroll_prefetch_state() -> (u32, Option<Instant>, bool) {
    (0, None, false)
}

/// What: Create default clickable rectangles state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of clickable rectangle fields: `url_button_rect`, `vt_url_rect`, `install_import_rect`, `install_export_rect`, `arch_status_text`, `arch_status_rect`, `arch_status_color`, `updates_count`, `updates_list`, `updates_button_rect`, `updates_loading`, `refresh_updates`, `pending_updates_modal`, `faillock_locked`, `faillock_lockout_until`, `faillock_remaining_minutes`.
///
/// Details:
/// - All rectangles start as None, updates check is loading by default.
/// - Faillock status defaults to not locked.
pub(super) fn default_clickable_rects_state() -> DefaultClickableRectsState {
    (
        None,
        None,
        None,
        None,
        "Arch Status: loading…".to_string(),
        None,
        ArchStatusColor::None,
        None,
        Vec::new(),
        None,
        true,
        false,
        false,
        false,
        None,
        None,
    )
}

/// What: Create default PKGBUILD viewer state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of PKGBUILD fields: `pkgb_button_rect`, `pkgb_check_button_rect`, `pkgb_reload_button_rect`, `pkgb_visible`, `pkgb_text`, `pkgb_package_name`, `pkgb_reload_requested_at`, `pkgb_reload_requested_for`, `pkgb_scroll`, `pkgb_rect`.
///
/// Details:
/// - PKGBUILD viewer is hidden by default, all rectangles start as None.
pub(super) const fn default_pkgbuild_state() -> DefaultPkgbuildState {
    (None, None, None, false, None, None, None, None, 0, None)
}

/// What: Create default comments state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of comments fields: `comments_button_rect`, `comments_visible`, `comments`, `comments_package_name`, `comments_fetched_at`, `comments_scroll`, `comments_rect`, `comments_loading`, `comments_error`, `comments_urls`, `comments_authors`.
///
/// Details:
/// - Comments viewer is hidden by default, all rectangles start as None.
pub(super) const fn default_comments_state() -> DefaultCommentsState {
    (
        None,
        false,
        Vec::new(),
        None,
        None,
        0,
        None,
        false,
        None,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

/// What: Create default toast message state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of toast fields: `toast_message`, `toast_expires_at`.
///
/// Details:
/// - No toast message is shown by default.
pub(super) const fn default_toast_state() -> (Option<String>, Option<Instant>) {
    (None, None)
}

/// What: Create default settings state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of settings fields: `layout_left_pct`, `layout_center_pct`, `layout_right_pct`, `keymap`, `locale`, `translations`, `translations_fallback`.
///
/// Details:
/// - Default layout percentages, keymap from settings, English locale, empty translation maps.
pub(super) fn default_settings_state() -> (
    u16,
    u16,
    u16,
    KeyMap,
    String,
    crate::i18n::translations::TranslationMap,
    crate::i18n::translations::TranslationMap,
) {
    (
        20,
        60,
        20,
        crate::theme::Settings::default().keymap,
        "en-US".to_string(),
        std::collections::HashMap::new(),
        std::collections::HashMap::new(),
    )
}

/// What: Create default mouse hit-test state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of mouse hit-test fields: `results_rect`, `details_rect`, `details_scroll`, `recent_rect`, `install_rect`, `downgrade_rect`, `mouse_disabled_in_details`, `last_mouse_pos`, `mouse_capture_enabled`.
///
/// Details:
/// - All rectangles start as None, mouse capture is enabled by default.
pub(super) const fn default_mouse_hit_test_state() -> DefaultMouseHitTestState {
    (None, None, 0, None, None, None, false, None, true)
}

/// What: Create default modal rectangles state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of modal rectangle fields: `news_rect`, `news_list_rect`, `announcement_rect`, `announcement_urls`, `pending_announcements`, `pending_news`, `trigger_startup_news_fetch`, `updates_modal_rect`, `updates_modal_content_rect`, `help_scroll`, `help_rect`, `preflight_tab_rects`, `preflight_content_rect`.
///
/// Details:
/// - All modal rectangles start as None, help scroll starts at 0, `announcement_urls` and `pending_announcements` start as empty Vec, `pending_news` starts as None, `trigger_startup_news_fetch` starts as false.
pub(super) const fn default_modal_rects_state() -> DefaultModalRectsState {
    (
        None,
        None,
        None,
        Vec::new(),
        Vec::new(),
        None,
        false,
        None,
        None,
        0,
        None,
        [None; 5],
        None,
    )
}

/// What: Create default sorting and menus state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of sorting/menu fields: `sort_mode`, `sort_menu_open`, `sort_button_rect`, `news_age_button_rect`, `sort_menu_rect`, `sort_menu_auto_close_at`, `options_menu_open`, `options_button_rect`, `options_menu_rect`, `panels_menu_open`, `panels_button_rect`, `panels_menu_rect`, `config_menu_open`, `artix_filter_menu_open`, `artix_filter_menu_rect`, `config_button_rect`, `config_menu_rect`, `collapsed_menu_open`, `collapsed_menu_button_rect`, `collapsed_menu_rect`, `sort_cache_repo_name`, `sort_cache_aur_popularity`, `sort_cache_signature`.
///
/// Details:
/// - All menus are closed by default, sort mode is `SortMode::RepoThenName`.
/// - Sort caches are empty (`None`) by default.
pub(super) const fn default_sorting_menus_state() -> DefaultSortingMenusState {
    (
        SortMode::RepoThenName,
        false,
        None,
        None,
        None,
        None,
        false,
        None,
        None,
        false,
        None,
        None,
        false,
        false,
        None,
        None,
        None,
        false,
        None,
        None,
        // Sort result caching fields
        None, // sort_cache_repo_name
        None, // sort_cache_aur_popularity
        None, // sort_cache_signature
    )
}

/// What: Create default results mode state.
///
/// Inputs: None.
///
/// Output:
/// - Tuple of results mode fields: `installed_only_mode`, `right_pane_focus`, `package_marker`.
///
/// Details:
/// - Not in installed-only mode by default, right pane focuses on Install, marker is at front.
pub(super) const fn default_results_mode_state() -> (
    bool,
    crate::state::types::RightPaneFocus,
    crate::theme::PackageMarker,
) {
    (
        false,
        crate::state::types::RightPaneFocus::Install,
        crate::theme::PackageMarker::Front,
    )
}
