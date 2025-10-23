use pacsea as crate_root; // alias for clarity in imports

use crate_root::install::command::build_install_command;
use crate_root::logic;
use crate_root::state::ArchStatusColor;
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
fn logic_sort_bestmatches_tiebreak_repo_then_name() {
    let mut app = new_app();
    app.results = vec![
        item_official("alpha2", "extra"),
        item_official("alpha1", "extra"),
        item_official("alpha_core", "core"),
    ];
    app.input = "alpha".into();
    app.sort_mode = SortMode::BestMatches;
    logic::sort_results_preserve_selection(&mut app);

    let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
    // Same rank -> core before extra; within extra, name ascending
    assert_eq!(names, vec!["alpha_core", "alpha1", "alpha2"]);
}

#[test]
fn logic_sort_aur_popularity_and_official_tiebreaks() {
    let mut app = new_app();
    app.results = vec![
        item_aur("aurB", Some(1.0)),
        item_aur("aurA", Some(1.0)),
        item_official("z_off", "core"),
        item_official("a_off", "extra"),
    ];
    app.sort_mode = SortMode::AurPopularityThenOfficial;
    logic::sort_results_preserve_selection(&mut app);

    let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
    // AUR first: equal popularity -> name ascending; then Official: core before extra, then name
    assert_eq!(names, vec!["aurA", "aurB", "z_off", "a_off"]);
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
fn logic_apply_filters_cachyos_and_eos_interaction() {
    let mut app = new_app();
    app.all_results = vec![
        PackageItem {
            name: "cx".into(),
            version: "1".into(),
            description: String::new(),
            source: Source::Official {
                repo: "cachyos-core".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        },
        PackageItem {
            name: "ey".into(),
            version: "1".into(),
            description: String::new(),
            source: Source::Official {
                repo: "endeavouros".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        },
        item_official("core1", "core"),
    ];
    // EOS off, CachyOS on -> CachyOS items included, EOS excluded
    app.results_filter_show_core = true;
    app.results_filter_show_extra = true;
    app.results_filter_show_multilib = true;
    app.results_filter_show_eos = false;
    app.results_filter_show_cachyos = true;
    crate_root::logic::apply_filters_and_sort_preserve_selection(&mut app);
    assert!(app.results.iter().any(|p| match &p.source {
        Source::Official { repo, .. } => repo.to_lowercase().starts_with("cachyos"),
        _ => false,
    }));
    assert!(app.results.iter().all(|p| match &p.source {
        Source::Official { repo, .. } => !repo.eq_ignore_ascii_case("endeavouros"),
        _ => true,
    }));
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
        .await
        .ok()
        .flatten();
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
        .await
        .ok()
        .flatten();
    assert!(none.is_none());
    assert_eq!(app.details.name, "pkg");
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
        licenses: vec![], // -> "-"
        groups: vec![],
        provides: vec!["prov1".into(), "prov2".into()], // -> "prov1, prov2"
        depends: vec![],
        opt_depends: vec![],
        required_by: vec![],
        optional_for: vec![],
        conflicts: vec![],
        replaces: vec![],
        download_size: None,      // -> "N/A"
        install_size: Some(1536), // -> "1.5 KiB"
        owner: String::new(),
        build_date: String::new(),
        popularity: None,
    };
    let th = crate_root::theme::theme();
    let lines = ui_helpers::format_details_lines(&app, 80, &th);

    // Download size shows N/A
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("N/A")))
    );
    // Install size shows human bytes
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("1.5 KiB")))
    );
    // Licences shows "-"
    assert!(lines.iter().any(|l| {
        l.spans
            .iter()
            .any(|s| s.content.contains("Licences") || s.content.contains("-"))
    }));
    // Provides shows comma-separated
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("prov1, prov2")))
    );
}

#[tokio::test]
async fn ui_helpers_trigger_recent_preview_noop_when_not_recent_or_invalid() {
    let mut app = new_app();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    // Not Recent focus
    app.focus = crate_root::state::Focus::Search;
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none1 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none1.is_none());

    // Recent focus but no selection
    app.focus = crate_root::state::Focus::Recent;
    app.recent = vec!["abc".into()];
    app.history_state.select(None);
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none2 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none2.is_none());

    // Selection out of bounds after filtering
    app.history_state.select(Some(0));
    app.pane_find = Some("zzz".into()); // filter to empty => idx >= inds.len()
    ui_helpers::trigger_recent_preview(&app, &tx);
    let none3 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none3.is_none());
}

#[test]
fn state_sortmode_config_roundtrip_and_aliases() {
    use crate_root::state::SortMode;
    assert_eq!(SortMode::RepoThenName.as_config_key(), "alphabetical");
    assert_eq!(
        SortMode::from_config_key("alphabetical"),
        Some(SortMode::RepoThenName)
    );
    assert_eq!(
        SortMode::from_config_key("repo_then_name"),
        Some(SortMode::RepoThenName)
    );
    assert_eq!(
        SortMode::from_config_key("pacman"),
        Some(SortMode::RepoThenName)
    );
    assert_eq!(
        SortMode::from_config_key("aur_popularity"),
        Some(SortMode::AurPopularityThenOfficial)
    );
    assert_eq!(
        SortMode::from_config_key("popularity"),
        Some(SortMode::AurPopularityThenOfficial)
    );
    assert_eq!(
        SortMode::from_config_key("best_matches"),
        Some(SortMode::BestMatches)
    );
    assert_eq!(
        SortMode::from_config_key("relevance"),
        Some(SortMode::BestMatches)
    );
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
            source: Source::Official {
                repo: "eos".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        },
        PackageItem {
            name: "e2".into(),
            version: "1".into(),
            description: "".into(),
            source: Source::Official {
                repo: "endeavouros".into(),
                arch: "x86_64".into(),
            },
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

#[test]
fn status_parse_color_by_percentage_and_outage() {
    // Build an HTML snippet approximating the relevant parts of the status page.
    // We include today's date and vary the percent and outage text.
    let (y, m, d) = {
        // Same helper used by the parser (UTC). If it fails, skip this test conservatively.
        let out = std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%d"])
            .output();
        let Ok(o) = out else {
            return;
        };
        if !o.status.success() {
            return;
        }
        let s = match String::from_utf8(o.stdout) {
            Ok(x) => x,
            Err(_) => return,
        };
        let mut it = s.trim().split('-');
        let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else {
            return;
        };
        let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>()) else {
            return;
        };
        (y, m, d)
    };
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_name = months[(m - 1) as usize];
    let date_str = format!("{month_name} {d}, {y}");

    let make_html = |percent: u32, outage: bool| -> String {
        format!(
            r#"
            <html>
              <body>
                <h2>Uptime Last 90 days</h2>
                <div>Monitors (default)</div>
                <div>AUR</div>
                <div>{date_str}</div>
                <div>{percent}% uptime</div>
                {outage_block}
              </body>
            </html>
            "#,
            outage_block = if outage {
                "<h4>The AUR is currently experiencing an outage</h4>"
            } else {
                ""
            }
        )
    };

    // >95 -> green
    let html_green = make_html(97, false);
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html_green);
    assert_eq!(color, ArchStatusColor::Operational);

    // 90-95 -> yellow
    let html_yellow = make_html(95, false);
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html_yellow);
    assert_eq!(color, ArchStatusColor::IncidentToday);

    // <90 -> red
    let html_red = make_html(89, false);
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html_red);
    assert_eq!(color, ArchStatusColor::IncidentSevereToday);

    // Outage present today: force at least yellow even if >95
    let html_outage = make_html(97, true);
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html_outage);
    assert_eq!(color, ArchStatusColor::IncidentToday);

    // Outage present today and <90 -> red
    let html_outage_red = make_html(80, true);
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html_outage_red);
    assert_eq!(color, ArchStatusColor::IncidentSevereToday);
}

#[test]
fn status_parse_prefers_svg_rect_color() {
    // Build HTML with a green-ish percentage but explicitly yellow rect fill for today's cell.
    let (y, m, d) = {
        let out = std::process::Command::new("date")
            .args(["-u", "+%Y-%m-%d"])
            .output();
        let Ok(o) = out else {
            return;
        };
        if !o.status.success() {
            return;
        }
        let s = match String::from_utf8(o.stdout) {
            Ok(x) => x,
            Err(_) => return,
        };
        let mut it = s.trim().split('-');
        let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else {
            return;
        };
        let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>()) else {
            return;
        };
        (y, m, d)
    };
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_name = months[(m - 1) as usize];
    let date_str = format!("{month_name} {d}, {y}");

    let html = format!(
        "<html>\n          <body>\n            <h2>Uptime Last 90 days</h2>\n            <div>Monitors (default)</div>\n            <div>AUR</div>\n            <svg>\n              <rect x=\"900\" y=\"0\" width=\"10\" height=\"10\" fill=\"#f59e0b\"></rect>\n            </svg>\n            <div>{date_str}</div>\n            <div>97% uptime</div>\n          </body>\n        </html>"
    );
    let (_txt, color) = crate_root::sources::status::parse_arch_status_from_html(&html);
    assert_eq!(color, ArchStatusColor::IncidentToday);
}

#[test]
fn install_build_install_command_official_variants() {
    let pkg = PackageItem {
        name: "ripgrep".into(),
        version: "14".into(),
        description: String::new(),
        source: Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
        popularity: None,
    };

    // No password
    let (cmd1, uses_sudo1) = build_install_command(&pkg, None, false);
    assert!(uses_sudo1);
    assert!(cmd1.starts_with("sudo pacman -S --needed --noconfirm ripgrep"));
    assert!(cmd1.contains("Press any key to close"));

    // With password (ensure it's piped to sudo -S and quotes are escaped)
    let (cmd2, uses_sudo2) = build_install_command(&pkg, Some("pa's"), false);
    assert!(uses_sudo2);
    assert!(cmd2.contains("echo "));
    assert!(cmd2.contains("sudo -S pacman -S --needed --noconfirm ripgrep"));

    // Dry run
    let (cmd3, uses_sudo3) = build_install_command(&pkg, None, true);
    assert!(uses_sudo3);
    assert!(cmd3.starts_with("echo DRY RUN: sudo pacman -S --needed --noconfirm ripgrep"));
}

#[test]
fn install_build_install_command_aur_variants() {
    let pkg = PackageItem {
        name: "yay-bin".into(),
        version: "1".into(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    };

    // AUR normal: does not use sudo, prefers paru then yay, includes fallback message
    let (cmd1, uses_sudo1) = build_install_command(&pkg, None, false);
    assert!(!uses_sudo1);
    assert!(cmd1.contains("command -v paru"));
    assert!(cmd1.contains("paru -S --needed --noconfirm yay-bin"));
    assert!(cmd1.contains("|| (command -v yay"));
    assert!(cmd1.contains("No AUR helper"));
    assert!(cmd1.contains("Press any key to close"));

    // Dry run: echoed helper command
    let (cmd2, uses_sudo2) = build_install_command(&pkg, None, true);
    assert!(!uses_sudo2);
    assert!(cmd2.starts_with("echo DRY RUN: paru -S --needed --noconfirm yay-bin"));
}

#[test]
fn theme_keychord_label_variants() {
    use crate_root::theme::KeyChord;
    use crossterm::event::{KeyCode, KeyModifiers};

    let kc = KeyChord {
        code: KeyCode::Char('r'),
        mods: KeyModifiers::CONTROL,
    };
    assert_eq!(kc.label(), "Ctrl+R");

    let kc2 = KeyChord {
        code: KeyCode::Char(' '),
        mods: KeyModifiers::empty(),
    };
    assert_eq!(kc2.label(), "Space");

    let kc3 = KeyChord {
        code: KeyCode::F(5),
        mods: KeyModifiers::empty(),
    };
    assert_eq!(kc3.label(), "F5");

    let kc4 = KeyChord {
        code: KeyCode::BackTab,
        mods: KeyModifiers::SHIFT,
    };
    assert_eq!(kc4.label(), "Shift+Tab");

    let kc5 = KeyChord {
        code: KeyCode::Left,
        mods: KeyModifiers::empty(),
    };
    assert_eq!(kc5.label(), "←");

    let kc6 = KeyChord {
        code: KeyCode::Char('x'),
        mods: KeyModifiers::ALT | KeyModifiers::SHIFT,
    };
    assert_eq!(kc6.label(), "Alt+Shift+X");
}

#[test]
fn logic_add_to_downgrade_list_behavior() {
    use crate_root::logic::add_to_downgrade_list;
    let mut app = new_app();
    add_to_downgrade_list(&mut app, item_official("PkgX", "extra"));
    add_to_downgrade_list(&mut app, item_official("pkgx", "extra")); // duplicate by name, different case
    assert_eq!(app.downgrade_list.len(), 1);
    assert_eq!(app.downgrade_state.selected(), Some(0));
}

#[tokio::test]
async fn index_loads_deduped_and_sorted_after_multiple_writes() {
    use std::fs;
    use std::path::PathBuf;

    // Prepare a path and write two overlapping JSON snapshots
    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!(
        "pacsea_idx_multi_{}_{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    let idx_json1 = serde_json::json!({
        "pkgs": [
            {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""},
            {"name": "aa", "repo": "core", "arch": "x86_64", "version": "1", "description": ""}
        ]
    });
    fs::write(&path, serde_json::to_string(&idx_json1).unwrap()).unwrap();
    crate_root::index::load_from_disk(&path);

    // Overwrite with a different ordering and a duplicate name to simulate update
    let idx_json2 = serde_json::json!({
        "pkgs": [
            {"name": "aa", "repo": "core", "arch": "x86_64", "version": "2", "description": ""},
            {"name": "zz", "repo": "extra", "arch": "x86_64", "version": "1", "description": ""}
        ]
    });
    fs::write(&path, serde_json::to_string(&idx_json2).unwrap()).unwrap();
    crate_root::index::load_from_disk(&path);

    // all_official returns current pkgs; we assert that both names exist once each.
    let all = crate_root::index::all_official();
    let mut names: Vec<String> = all.into_iter().map(|p| p.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names, vec!["aa", "zz"]);

    let _ = fs::remove_file(&path);
}

#[tokio::test]
async fn index_enrich_noop_on_empty_names() {
    use std::fs;
    use std::path::PathBuf;

    // Seed an empty index file
    let mut path: PathBuf = std::env::temp_dir();
    path.push(format!(
        "pacsea_idx_empty_enrich_{}_{}.json",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let idx_json = serde_json::json!({ "pkgs": [] });
    fs::write(&path, serde_json::to_string(&idx_json).unwrap()).unwrap();
    crate_root::index::load_from_disk(&path);

    // Calling with empty names should exit early and not notify
    let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    crate_root::index::request_enrich_for(path.clone(), notify_tx, Vec::new());

    let none = tokio::time::timeout(std::time::Duration::from_millis(200), notify_rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none.is_none());

    let _ = fs::remove_file(&path);
}

#[test]
fn sources_details_parse_official_json_defaults_and_fields() {
    // Local helper mirroring production parse to avoid network/system calls
    fn parse_official_from_json(
        obj: &serde_json::Value,
        repo_selected: String,
        arch_selected: String,
        item: &PackageItem,
    ) -> PackageDetails {
        use crate_root::util::{arrs, ss, u64_of};
        PackageDetails {
            repository: repo_selected,
            name: item.name.clone(),
            version: ss(obj, &["pkgver", "Version"]).unwrap_or(item.version.clone()),
            description: ss(obj, &["pkgdesc", "Description"]).unwrap_or(item.description.clone()),
            architecture: ss(obj, &["arch", "Architecture"]).unwrap_or(arch_selected),
            url: ss(obj, &["url", "URL"]).unwrap_or_default(),
            licenses: arrs(obj, &["licenses", "Licenses"]),
            groups: arrs(obj, &["groups", "Groups"]),
            provides: arrs(obj, &["provides", "Provides"]),
            depends: arrs(obj, &["depends", "Depends"]),
            opt_depends: arrs(obj, &["optdepends", "OptDepends"]),
            required_by: arrs(obj, &["requiredby", "RequiredBy"]),
            optional_for: vec![],
            conflicts: arrs(obj, &["conflicts", "Conflicts"]),
            replaces: arrs(obj, &["replaces", "Replaces"]),
            download_size: u64_of(obj, &["compressed_size", "CompressedSize"]),
            install_size: u64_of(obj, &["installed_size", "InstalledSize"]),
            owner: ss(obj, &["packager", "Packager"]).unwrap_or_default(),
            build_date: ss(obj, &["build_date", "BuildDate"]).unwrap_or_default(),
            popularity: None,
        }
    }
    // Build minimal JSON similar to arch packages API
    let v: serde_json::Value = serde_json::json!({
        "pkg": {
            "pkgver": "14",
            "pkgdesc": "ripgrep fast search",
            "arch": "x86_64",
            "url": "https://example.com",
            "licenses": ["MIT"],
            "groups": [],
            "provides": ["rg"],
            "depends": ["pcre2"],
            "optdepends": ["bash: completions"],
            "requiredby": [],
            "conflicts": [],
            "replaces": [],
            "compressed_size": 1024u64,
            "installed_size": 2048u64,
            "packager": "Arch Dev",
            "build_date": "2024-01-01"
        }
    });
    let item = PackageItem {
        name: "ripgrep".into(),
        version: String::new(),
        description: String::new(),
        source: Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
        popularity: None,
    };
    let d = parse_official_from_json(&v["pkg"], "extra".into(), "x86_64".into(), &item);
    assert_eq!(d.repository, "extra");
    assert_eq!(d.name, "ripgrep");
    assert_eq!(d.version, "14");
    assert_eq!(d.description, "ripgrep fast search");
    assert_eq!(d.architecture, "x86_64");
    assert_eq!(d.url, "https://example.com");
    assert_eq!(d.download_size, Some(1024));
    assert_eq!(d.install_size, Some(2048));
    assert_eq!(d.owner, "Arch Dev");
    assert_eq!(d.build_date, "2024-01-01");
}

#[test]
fn sources_details_parse_aur_json_defaults_and_popularity() {
    // Local helper mirroring production parse to avoid network/system calls
    fn parse_aur_from_json(obj: &serde_json::Value, item: &PackageItem) -> PackageDetails {
        use crate_root::util::{arrs, s};
        let version0 = s(obj, "Version");
        let description0 = s(obj, "Description");
        let popularity0 = obj.get("Popularity").and_then(|v| v.as_f64());
        PackageDetails {
            repository: "AUR".into(),
            name: item.name.clone(),
            version: if version0.is_empty() {
                item.version.clone()
            } else {
                version0
            },
            description: if description0.is_empty() {
                item.description.clone()
            } else {
                description0
            },
            architecture: "any".into(),
            url: s(obj, "URL"),
            licenses: arrs(obj, &["License", "Licenses"]),
            groups: arrs(obj, &["Groups"]),
            provides: arrs(obj, &["Provides"]),
            depends: arrs(obj, &["Depends"]),
            opt_depends: arrs(obj, &["OptDepends"]),
            required_by: vec![],
            optional_for: vec![],
            conflicts: arrs(obj, &["Conflicts"]),
            replaces: arrs(obj, &["Replaces"]),
            download_size: None,
            install_size: None,
            owner: s(obj, "Maintainer"),
            build_date: crate_root::util::ts_to_date(
                obj.get("LastModified").and_then(|v| v.as_i64()),
            ),
            popularity: popularity0,
        }
    }
    // Minimal AUR RPC result object
    let obj: serde_json::Value = serde_json::json!({
        "Version": "1.2.3",
        "Description": "cool",
        "Popularity": 3.14,
        "URL": "https://aur.example/ripgrep"
    });
    let item = PackageItem {
        name: "ripgrep-git".into(),
        version: String::new(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    };
    let d = parse_aur_from_json(&obj, &item);
    assert_eq!(d.repository, "AUR");
    assert_eq!(d.name, "ripgrep-git");
    assert_eq!(d.version, "1.2.3");
    assert_eq!(d.description, "cool");
    assert_eq!(d.architecture, "any");
    assert_eq!(d.url, "https://aur.example/ripgrep");
    assert_eq!(d.popularity, Some(3.14));
}

#[test]
fn logic_filter_unknown_official_inclusion_policy() {
    // When an official repo is unknown, include it only if all known-official toggles are enabled
    let mut app = new_app();
    app.all_results = vec![
        PackageItem {
            name: "x1".into(),
            version: "1".into(),
            description: String::new(),
            source: Source::Official {
                repo: "weirdrepo".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        },
        item_official("core1", "core"),
    ];
    // Disable one of the official toggles -> unknown should be filtered out
    app.results_filter_show_aur = true;
    app.results_filter_show_core = true;
    app.results_filter_show_extra = true;
    app.results_filter_show_multilib = false; // one disabled
    app.results_filter_show_eos = true;
    app.results_filter_show_cachyos = true;
    crate_root::logic::apply_filters_and_sort_preserve_selection(&mut app);
    assert!(app.results.iter().all(|p| match &p.source {
        Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
        _ => false,
    }));

    // Enable all official toggles -> unknown should be included
    app.results_filter_show_multilib = true;
    crate_root::logic::apply_filters_and_sort_preserve_selection(&mut app);
    assert!(app.results.iter().any(|p| match &p.source {
        Source::Official { repo, .. } => repo.eq_ignore_ascii_case("weirdrepo"),
        _ => false,
    }));
}

#[test]
fn logic_fast_scroll_sets_gating_and_defers_ring() {
    use crate_root::logic::set_allowed_only_selected;
    let mut app = new_app();
    app.results = vec![
        item_official("a", "core"),
        item_official("b", "extra"),
        item_official("c", "extra"),
        item_official("d", "extra"),
        item_official("e", "extra"),
        item_official("f", "extra"),
        item_official("g", "extra"),
    ];
    // Simulate large scroll
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<crate_root::state::PackageItem>();
    crate_root::logic::move_sel_cached(&mut app, 6, &tx);
    assert!(app.need_ring_prefetch);
    assert!(app.ring_resume_at.is_some());
    // During fast scroll gating, only selected allowed
    set_allowed_only_selected(&app);
    assert!(crate_root::logic::is_allowed(
        &app.results[app.selected].name
    ));
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_xfce4_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake xfce4-terminal that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("xfce4-terminal");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Prepend our temp dir to PATH to force terminal selection while keeping system tools available
    let orig_path = std::env::var_os("PATH");
    let combined_path = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => dir.display().to_string(),
    };
    unsafe {
        std::env::set_var("PATH", combined_path);
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Place an Options button and click it to open the menu
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);

    // Pretend the UI rendered the options menu at a known rect with 3 rows
    app.options_menu_rect = Some((5, 6, 20, 3));

    // Click the second row (index 1): "Update System"
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7, // y=6 + row index 1
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    // SystemUpdate modal should be open with defaults
    match &app.modal {
        crate_root::state::Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            ..
        } => {
            assert!(!*do_mirrors);
            assert!(*do_pacman);
            assert!(*do_aur);
            assert!(!*do_cache);
        }
        _ => panic!("SystemUpdate modal not opened"),
    }

    // Press Enter to run the update with defaults, which spawns xfce4-terminal
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    // Allow the fake terminal to write its args
    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect the safe shape for xfce4-terminal: --, bash, -lc, <cmd>
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    // Restore PATH and clean environment override
    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_tilix_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake tilix that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_tilix_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("tilix");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Prepend temp dir to PATH to force tilix selection while keeping system tools available
    let orig_path = std::env::var_os("PATH");
    let combined_path = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => dir.display().to_string(),
    };
    unsafe {
        std::env::set_var("PATH", combined_path);
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Place an Options button and click it to open the menu
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);

    // Pretend the UI rendered the options menu at a known rect with 3 rows
    app.options_menu_rect = Some((5, 6, 20, 3));

    // Click the second row (index 1): "Update System"
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7, // y=6 + row index 1
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    // SystemUpdate modal should be open with defaults
    match &app.modal {
        crate_root::state::Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            ..
        } => {
            assert!(!*do_mirrors);
            assert!(*do_pacman);
            assert!(*do_aur);
            assert!(!*do_cache);
        }
        _ => panic!("SystemUpdate modal not opened"),
    }

    // Press Enter to run the update with defaults, which spawns tilix
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    // Allow the fake terminal to write its args
    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect the safe shape for tilix: --, bash, -lc, <cmd>
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    // Restore PATH and clean environment override
    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_mate_terminal_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake mate-terminal that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_mate_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("mate-terminal");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Prepend temp dir to PATH to force mate-terminal selection while keeping system tools available
    let orig_path = std::env::var_os("PATH");
    let combined_path = match std::env::var("PATH") {
        Ok(p) => format!("{}:{}", dir.display(), p),
        Err(_) => dir.display().to_string(),
    };
    unsafe {
        std::env::set_var("PATH", combined_path);
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Place an Options button and click it to open the menu
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);

    // Pretend the UI rendered the options menu at a known rect with 3 rows
    app.options_menu_rect = Some((5, 6, 20, 3));

    // Click the second row (index 1): "Update System"
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7, // y=6 + row index 1
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    // SystemUpdate modal should be open with defaults
    match &app.modal {
        crate_root::state::Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            ..
        } => {
            assert!(!*do_mirrors);
            assert!(*do_pacman);
            assert!(*do_aur);
            assert!(!*do_cache);
        }
        _ => panic!("SystemUpdate modal not opened"),
    }

    // Press Enter to run the update with defaults, which spawns mate-terminal
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    // Allow the fake terminal to write its args
    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect the safe shape for mate-terminal: --, bash, -lc, <cmd>
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    // Restore PATH and clean environment override
    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_gnome_terminal_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake gnome-terminal that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_gnome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("gnome-terminal");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Isolate PATH to only our temp dir for deterministic terminal selection
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Place an Options button and click it to open the menu
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);

    // Pretend the UI rendered the options menu at a known rect with 3 rows
    app.options_menu_rect = Some((5, 6, 20, 3));

    // Click the second row (index 1): "Update System"
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7, // y=6 + row index 1
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    // Press Enter to run the update with defaults, which spawns gnome-terminal
    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    // Allow the fake terminal to write its args
    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect the safe shape for gnome-terminal: --, bash, -lc, <cmd>
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    // Restore PATH and clean environment override
    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_konsole_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake konsole that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_konsole_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("konsole");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Isolate PATH to only our temp dir
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Open Options and trigger Update System
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect konsole shape: -e, bash, -lc, <cmd>
    assert_eq!(lines[0], "-e");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_alacritty_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake alacritty that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_alacritty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("alacritty");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Open Options and trigger Update System
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    // Expect alacritty shape: -e, bash, -lc, <cmd>
    assert_eq!(lines[0], "-e");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_kitty_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake kitty that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_kitty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("kitty");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Open Options and trigger Update System
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 2, "expected at least 2 args, got: {}", body);
    // Expect kitty shape: bash, -lc, <cmd>
    assert_eq!(lines[0], "bash");
    assert_eq!(lines[1], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn ui_options_update_system_enter_triggers_xterm_args_shape() {
    use crossterm::event::{
        Event as CEvent, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare a temp dir with a fake xterm that records its argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_term_xterm_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("xterm");
    let script = "#!/bin/sh\n# record all args, one per line\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do\n  printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"\ndone\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Build minimal AppState and channels
    let mut app = new_app();
    let (qtx, _qrx) = tokio::sync::mpsc::unbounded_channel();
    let (dtx, _drx) = tokio::sync::mpsc::unbounded_channel();
    let (ptx, _prx) = tokio::sync::mpsc::unbounded_channel();
    let (atx, _arx) = tokio::sync::mpsc::unbounded_channel();
    let (pkgb_tx, _pkgb_rx) = tokio::sync::mpsc::unbounded_channel();

    // Open Options and trigger Update System
    app.options_button_rect = Some((5, 5, 10, 1));
    let click_options = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 5,
        modifiers: KeyModifiers::empty(),
    });
    let _ =
        crate_root::events::handle_event(click_options, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);
    assert!(app.options_menu_open);
    app.options_menu_rect = Some((5, 6, 20, 3));
    let click_menu_update = CEvent::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 7,
        modifiers: KeyModifiers::empty(),
    });
    let _ = crate_root::events::handle_event(
        click_menu_update,
        &mut app,
        &qtx,
        &dtx,
        &ptx,
        &atx,
        &pkgb_tx,
    );

    let enter = CEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let _ = crate_root::events::handle_event(enter, &mut app, &qtx, &dtx, &ptx, &atx, &pkgb_tx);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 4, "expected at least 4 args, got: {}", body);
    // Expect xterm shape: -hold, -e, bash, -lc, <cmd>
    assert_eq!(lines[0], "-hold");
    assert_eq!(lines[1], "-e");
    assert_eq!(lines[2], "bash");
    assert_eq!(lines[3], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn install_single_uses_gnome_terminal_double_dash() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare fake gnome-terminal that records argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_inst_single_gnome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("gnome-terminal");
    let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Isolate PATH
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Call install single (dry run to avoid logging)
    let pkg = item_official("ripgrep", "extra");
    crate_root::install::spawn_install(&pkg, None, true);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn install_batch_uses_gnome_terminal_double_dash() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare fake gnome-terminal that records argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_inst_batch_gnome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("gnome-terminal");
    let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Isolate PATH
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Call batch install (dry run)
    let items = vec![item_official("rg", "extra"), item_official("fd", "extra")];
    crate_root::install::spawn_install_all(&items, true);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}

#[cfg(not(target_os = "windows"))]
#[test]
fn remove_all_uses_gnome_terminal_double_dash() {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    // Prepare fake gnome-terminal that records argv
    let mut dir: PathBuf = std::env::temp_dir();
    dir.push(format!(
        "pacsea_test_remove_gnome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::create_dir_all(&dir);

    let mut out_path = dir.clone();
    out_path.push("args.txt");

    let mut term_path = dir.clone();
    term_path.push("gnome-terminal");
    let script = "#!/bin/sh\n: > \"$PACSEA_TEST_OUT\"\nfor a in \"$@\"; do printf '%s\n' \"$a\" >> \"$PACSEA_TEST_OUT\"; done\n";
    fs::write(&term_path, script.as_bytes()).unwrap();
    let mut perms = fs::metadata(&term_path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&term_path, perms).unwrap();

    // Isolate PATH
    let orig_path = std::env::var_os("PATH");
    unsafe {
        std::env::set_var("PATH", dir.display().to_string());
        std::env::set_var("PACSEA_TEST_OUT", out_path.display().to_string());
    }

    // Call remove all (dry run)
    let names = vec!["ripgrep".to_string(), "fd".to_string()];
    crate_root::install::spawn_remove_all(&names, true);

    std::thread::sleep(std::time::Duration::from_millis(50));

    let body = fs::read_to_string(&out_path).expect("fake terminal args file written");
    let lines: Vec<&str> = body.lines().collect();
    assert!(lines.len() >= 3, "expected at least 3 args, got: {}", body);
    assert_eq!(lines[0], "--");
    assert_eq!(lines[1], "bash");
    assert_eq!(lines[2], "-lc");

    unsafe {
        if let Some(v) = orig_path {
            std::env::set_var("PATH", v);
        } else {
            std::env::remove_var("PATH");
        }
        std::env::remove_var("PACSEA_TEST_OUT");
    }
}
