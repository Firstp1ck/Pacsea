//! Tests for `AppState`.

use crate::state::app_state::AppState;
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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

#[cfg(test)]
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
