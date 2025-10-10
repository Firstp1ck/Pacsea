use Pacsea as crate_root; // alias for clarity in imports

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
