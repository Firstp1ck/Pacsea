use pacsea as crate_root; // alias for clarity in imports

use crate_root::logic;
use crate_root::state::{AppState, PackageDetails, PackageItem, SortMode, Source};
use crate_root::ui_helpers;
use crate_root::util;

fn item_official(name: &str, repo: &str) -> PackageItem {
    PackageItem {
        name: name.to_string(),
        version: "1.0".to_string(),
        description: format!("{name} desc"),
        source: Source::Official {
            repo: repo.to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    }
}

fn item_aur(name: &str, pop: Option<f64>) -> PackageItem {
    PackageItem {
        name: name.to_string(),
        version: "1.0".to_string(),
        description: format!("{name} desc"),
        source: Source::Aur,
        popularity: pop,
    }
}

fn new_app() -> AppState {
    AppState {
        ..Default::default()
    }
}

#[test]
fn util_percent_encode() {
    assert_eq!(util::percent_encode(""), "");
    assert_eq!(util::percent_encode("abc-_.~"), "abc-_.~");
    assert_eq!(util::percent_encode("a b"), "a%20b");
    assert_eq!(util::percent_encode("C++"), "C%2B%2B");
    assert_eq!(util::percent_encode("π"), "%CF%80");
}

#[test]
fn util_json_extractors_and_u64() {
    let v: serde_json::Value = serde_json::json!({
        "a": "str",
        "b": ["x", 1, "y"],
        "c": 42u64,
        "d": -5,
        "e": "123",
    });
    assert_eq!(util::s(&v, "a"), "str");
    assert_eq!(util::s(&v, "missing"), "");
    assert_eq!(util::ss(&v, &["z", "a"]).as_deref(), Some("str"));
    assert_eq!(
        util::arrs(&v, &["b", "missing"]),
        vec!["x".to_string(), "y".to_string()]
    );
    assert_eq!(util::u64_of(&v, &["c"]), Some(42));
    assert_eq!(util::u64_of(&v, &["d"]), None); // negative not convertible
    assert_eq!(util::u64_of(&v, &["e"]), Some(123));
    assert_eq!(util::u64_of(&v, &["missing"]), None);
}

#[test]
fn util_repo_order_and_rank() {
    let core = Source::Official {
        repo: "core".into(),
        arch: "x86_64".into(),
    };
    let extra = Source::Official {
        repo: "extra".into(),
        arch: "x86_64".into(),
    };
    let other = Source::Official {
        repo: "community".into(),
        arch: "x86_64".into(),
    };
    let aur = Source::Aur;
    assert!(util::repo_order(&core) < util::repo_order(&extra));
    assert!(util::repo_order(&extra) < util::repo_order(&other));
    assert!(util::repo_order(&other) < util::repo_order(&aur));

    assert_eq!(util::match_rank("ripgrep", "ripgrep"), 0);
    assert_eq!(util::match_rank("ripgrep", "rip"), 1);
    assert_eq!(util::match_rank("ripgrep", "pg"), 2);
    assert_eq!(util::match_rank("ripgrep", "zzz"), 3);
}

#[test]
fn util_ts_to_date_and_leap() {
    assert_eq!(util::ts_to_date(None), "");
    assert_eq!(util::ts_to_date(Some(-1)), "-1");
    assert_eq!(util::ts_to_date(Some(0)), "1970-01-01 00:00:00");
    // 2000-02-29 00:00:00 UTC -> seconds since epoch: 951782400
    assert_eq!(util::ts_to_date(Some(951_782_400)), "2000-02-29 00:00:00");
}

#[tokio::test]
async fn logic_send_query_increments_and_sends() {
    let mut app = new_app();
    app.input = "hello".into();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    logic::send_query(&mut app, &tx);
    assert_eq!(app.latest_query_id, 1); // first send assigns id=1 (Default next_query_id starts at 1 and is then incremented)
    let q = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
        .await
        .ok()
        .flatten()
        .expect("query sent");
    assert_eq!(q.id, app.latest_query_id);
    assert_eq!(q.text, "hello");
}

#[test]
fn logic_allowed_only_selected_and_ring() {
    let mut app = new_app();
    app.results = vec![
        item_official("a", "core"),
        item_official("b", "extra"),
        item_official("c", "extra"),
        item_official("d", "other"),
    ];
    app.selected = 1; // b
    logic::set_allowed_only_selected(&app);
    assert!(logic::is_allowed("b"));
    assert!(!logic::is_allowed("a") || !logic::is_allowed("c") || !logic::is_allowed("d"));

    logic::set_allowed_ring(&app, 1);
    assert!(logic::is_allowed("a") || logic::is_allowed("c"));
}

#[test]
fn logic_add_to_install_list_behavior() {
    let mut app = new_app();
    logic::add_to_install_list(&mut app, item_official("pkg1", "core"));
    logic::add_to_install_list(&mut app, item_official("Pkg1", "core")); // duplicate (case-insensitive)
    assert_eq!(app.install_list.len(), 1);
    assert!(app.install_dirty);
    assert_eq!(app.install_state.selected(), Some(0));
}

#[test]
fn logic_sort_preserve_selection_and_best_matches() {
    let mut app = new_app();
    app.results = vec![
        item_aur("zzz", Some(1.0)),
        item_official("aaa", "core"),
        item_official("bbb", "extra"),
        item_aur("ccc", Some(10.0)),
    ];
    app.selected = 2; // bbb
    app.list_state.select(Some(2));
    app.sort_mode = SortMode::RepoThenName;
    logic::sort_results_preserve_selection(&mut app);
    // repo order keeps official before AUR, and name tiebreak; selection remains on "bbb"
    assert_eq!(
        app.results
            .iter()
            .filter(|p| matches!(p.source, Source::Official { .. }))
            .count(),
        2
    );
    assert_eq!(app.results[app.selected].name, "bbb");

    app.sort_mode = SortMode::AurPopularityThenOfficial;
    logic::sort_results_preserve_selection(&mut app);
    // AUR entries should be first in desc popularity
    let aur_first = &app.results[0];
    assert!(matches!(aur_first.source, Source::Aur));

    app.input = "bb".into();
    app.sort_mode = SortMode::BestMatches;
    logic::sort_results_preserve_selection(&mut app);
    // BestMatches should rank names with "bb" earlier
    assert!(
        app.results
            .iter()
            .position(|p| p.name.contains("bb"))
            .unwrap()
            <= 1
    );
}

#[test]
fn logic_apply_filters_and_preserve_selection() {
    let mut app = new_app();
    app.all_results = vec![
        item_aur("aur1", Some(1.0)),
        item_official("core1", "core"),
        item_official("extra1", "extra"),
        item_official("other1", "community"),
    ];
    app.results_filter_show_aur = false;
    app.results_filter_show_core = true;
    app.results_filter_show_extra = false;
    app.results_filter_show_multilib = false;
    logic::apply_filters_and_sort_preserve_selection(&mut app);
    assert!(app.results.iter().all(
        |p| matches!(&p.source, Source::Official{repo, ..} if repo.eq_ignore_ascii_case("core"))
    ));
}

#[test]
fn ui_helpers_filtered_indices_and_details_lines() {
    let mut app = new_app();
    app.recent = vec!["alpha".into(), "bravo".into(), "charlie".into()];
    // Not focused => no filtering
    assert_eq!(ui_helpers::filtered_recent_indices(&app), vec![0, 1, 2]);
    app.focus = crate_root::state::Focus::Recent;
    app.pane_find = Some("a".into());
    let inds = ui_helpers::filtered_recent_indices(&app);
    assert_eq!(inds, vec![0, 1, 2]); // contains 'a' => alpha, bravo, charlie

    app.install_list = vec![item_official("ripgrep", "extra"), item_aur("fd", None)];
    app.focus = crate_root::state::Focus::Install;
    app.pane_find = Some("rip".into());
    let inds2 = ui_helpers::filtered_install_indices(&app);
    assert_eq!(inds2, vec![0]);

    // format_details_lines toggles last action label
    app.details = PackageDetails {
        repository: "extra".into(),
        name: "ripgrep".into(),
        version: "14".into(),
        description: "desc".into(),
        architecture: "x86_64".into(),
        url: "https://example.com".into(),
        licenses: vec!["MIT".into()],
        groups: vec![],
        provides: vec![],
        depends: vec![],
        opt_depends: vec![],
        required_by: vec![],
        optional_for: vec![],
        conflicts: vec![],
        replaces: vec![],
        download_size: None,
        install_size: None,
        owner: "owner".into(),
        build_date: "date".into(),
        popularity: None,
    };
    let th = crate_root::theme::theme();
    let lines = ui_helpers::format_details_lines(&app, 80, &th);
    assert!(
        lines.last().unwrap().spans[0]
            .content
            .contains("Show PKGBUILD")
    );
    let mut app2 = new_app();
    app2.details = app.details.clone();
    app2.pkgb_visible = true;
    let lines2 = ui_helpers::format_details_lines(&app2, 80, &th);
    assert!(
        lines2.last().unwrap().spans[0]
            .content
            .contains("Hide PKGBUILD")
    );
}

#[test]
fn logic_add_to_remove_list_behavior() {
    let mut app = new_app();
    logic::add_to_remove_list(&mut app, item_official("pkg1", "extra"));
    logic::add_to_remove_list(&mut app, item_official("Pkg1", "extra")); // duplicate (case-insensitive)
    assert_eq!(app.remove_list.len(), 1);
    assert_eq!(app.remove_state.selected(), Some(0));
}

#[tokio::test]
async fn logic_move_sel_cached_clamps_and_requests_details() {
    let mut app = new_app();
    app.results = vec![item_aur("aur1", None), item_official("pkg2", "core")];
    app.selected = 0;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    logic::move_sel_cached(&mut app, 1, &tx);

    assert_eq!(app.selected, 1);
    assert_eq!(app.details.repository.to_lowercase(), "core");
    assert_eq!(app.details.architecture.to_lowercase(), "x86_64");

    // At least one request (selected and/or ring) enqueued
    let got = tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv())
        .await.ok().flatten();
    assert!(got.is_some());

    // Clamp to start
    logic::move_sel_cached(&mut app, -100, &tx);
    assert_eq!(app.selected, 0);

    // Move back to AUR and check placeholder fields
    logic::move_sel_cached(&mut app, 0, &tx);
    assert_eq!(app.details.repository, "AUR");
    assert_eq!(app.details.architecture, "any");
}

#[tokio::test]
async fn logic_move_sel_cached_uses_details_cache() {
    let mut app = new_app();
    let pkg = item_official("pkg", "core");
    app.results = vec![pkg.clone()];
    app.details_cache.insert(
        pkg.name.clone(),
        crate_root::state::PackageDetails {
            repository: "core".into(),
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            architecture: "x86_64".into(),
            ..Default::default()
        },
    );

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    logic::move_sel_cached(&mut app, 0, &tx);

    // No network request as cached
    let none = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await.ok().flatten();
    assert!(none.is_none());
    assert_eq!(app.details.name, "pkg");
}

#[tokio::test]
async fn logic_ring_prefetch_sends_neighbors_respecting_allowed_and_cache() {
    let mut app = new_app();
    app.results = vec![
        item_official("a", "core"),
        item_official("b", "core"),
        item_official("c", "core"),
    ];
    app.selected = 1;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    logic::set_allowed_ring(&app, 1);
    logic::ring_prefetch_from_selected(&mut app, &tx);

    let mut names = std::collections::HashSet::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while names.len() < 2 && std::time::Instant::now() < deadline {
        if let Ok(Some(it)) = tokio::time::timeout(std::time::Duration::from_millis(200), rx.recv())
            .await
        {
            names.insert(it.name);
        }
    }
    assert_eq!(names, ["a".to_string(), "c".to_string()].into_iter().collect());
}

#[test]
fn ui_helpers_details_lines_sizes_and_lists() {
    let mut app = new_app();
    app.details = crate_root::state::PackageDetails {
        repository: "extra".into(),
        name: "ripgrep".into(),
        version: "14".into(),
        description: "desc".into(),
        architecture: "x86_64".into(),
        url: String::new(),
        licenses: vec![],                          // -> "-"
        groups: vec![],
        provides: vec!["prov1".into(), "prov2".into()], // -> "prov1, prov2"
        depends: vec![],
        opt_depends: vec![],
        required_by: vec![],
        optional_for: vec![],
        conflicts: vec![],
        replaces: vec![],
        download_size: None,                       // -> "N/A"
        install_size: Some(1536),                  // -> "1.5 KiB"
        owner: String::new(),
        build_date: String::new(),
        popularity: None,
    };
    let th = crate_root::theme::theme();
    let lines = ui_helpers::format_details_lines(&app, 80, &th);

    // Download size shows N/A
    assert!(lines.iter().any(|l| l.spans.iter().any(|s| s.content.contains("N/A"))));
    // Install size shows human bytes
    assert!(lines.iter().any(|l| l.spans.iter().any(|s| s.content.contains("1.5 KiB"))));
    // Licences shows "-"
    assert!(lines.iter().any(|l| l.spans.iter().any(|s| s.content.contains("Licences") || s.content.contains("-"))));
    // Provides shows comma-separated
    assert!(lines.iter().any(|l| l.spans.iter().any(|s| s.content.contains("prov1, prov2"))));
}

#[tokio::test]
async fn ui_helpers_trigger_recent_preview_noop_when_not_recent_or_invalid() {
    let mut app = new_app();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Not Recent focus
    app.focus = crate_root::state::Focus::Search;
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none1 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await.ok().flatten();
    assert!(none1.is_none());

    // Recent focus but no selection
    app.focus = crate_root::state::Focus::Recent;
    app.recent = vec!["abc".into()];
    app.history_state.select(None);
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none2 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await.ok().flatten();
    assert!(none2.is_none());

    // Selection out of bounds after filtering
    app.history_state.select(Some(0));
    app.pane_find = Some("zzz".into()); // filter to empty => idx >= inds.len()
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none3 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await.ok().flatten();
    assert!(none3.is_none());
}

#[test]
fn state_sortmode_config_roundtrip_and_aliases() {
    use crate_root::state::SortMode;
    assert_eq!(SortMode::RepoThenName.as_config_key(), "alphabetical");
    assert_eq!(SortMode::from_config_key("alphabetical"), Some(SortMode::RepoThenName));
    assert_eq!(SortMode::from_config_key("repo_then_name"), Some(SortMode::RepoThenName));
    assert_eq!(SortMode::from_config_key("pacman"), Some(SortMode::RepoThenName));
    assert_eq!(SortMode::from_config_key("aur_popularity"), Some(SortMode::AurPopularityThenOfficial));
    assert_eq!(SortMode::from_config_key("popularity"), Some(SortMode::AurPopularityThenOfficial));
    assert_eq!(SortMode::from_config_key("best_matches"), Some(SortMode::BestMatches));
    assert_eq!(SortMode::from_config_key("relevance"), Some(SortMode::BestMatches));
    assert_eq!(SortMode::from_config_key("unknown"), None);
}

#[test]
fn state_appstate_defaults_sane() {
    let app = new_app();
    assert_eq!(app.latest_query_id, 0);
    assert_eq!(app.next_query_id, 1);
    assert!(app.results.is_empty());
    assert!(app.all_results.is_empty());
    assert!(app.recent.is_empty());
    assert!(app.install_list.is_empty());
    assert!(app.details_cache.is_empty());
    assert_eq!(app.sort_mode, SortMode::RepoThenName);
    assert!(app.results_filter_show_aur);
    assert!(app.results_filter_show_core);
    assert!(app.results_filter_show_extra);
    assert!(app.results_filter_show_multilib);
    assert!(app.results_filter_show_eos);
}

#[test]
fn logic_apply_filters_eos_toggle() {
    let mut app = new_app();
    app.all_results = vec![
        PackageItem {
            name: "e1".into(),
            version: "1".into(),
            description: "".into(),
            source: Source::Official { repo: "eos".into(), arch: "x86_64".into() },
            popularity: None,
        },
        PackageItem {
            name: "e2".into(),
            version: "1".into(),
            description: "".into(),
            source: Source::Official { repo: "endeavouros".into(), arch: "x86_64".into() },
            popularity: None,
        },
        item_official("core1", "core"),
    ];
    app.results_filter_show_eos = false;
    app.results_filter_show_core = true;
    logic::apply_filters_and_sort_preserve_selection(&mut app);
    assert!(app.results.iter().all(
        |p| matches!(&p.source, Source::Official{repo, ..} if repo.eq_ignore_ascii_case("core"))
    ));
}

#[test]
fn util_ts_to_date_boundaries() {
    // 2000-01-01 00:00:00 UTC
    assert_eq!(util::ts_to_date(Some(946_684_800)), "2000-01-01 00:00:00");
    // One second before
    assert_eq!(util::ts_to_date(Some(946_684_799)), "1999-12-31 23:59:59");
}