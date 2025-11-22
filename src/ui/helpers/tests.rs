//! Test module for UI helpers.

use super::*;

/// What: Initialize minimal English translations for tests.
///
/// Inputs:
/// - `app`: `AppState` to populate with translations
///
/// Output:
/// - Populates `app.translations` and `app.translations_fallback` with minimal English translations
///
/// Details:
/// - Sets up only the translations needed for tests to pass
fn init_test_translations(app: &mut crate::state::AppState) {
    use std::collections::HashMap;
    let mut translations = HashMap::new();
    // Details fields
    translations.insert(
        "app.details.fields.repository".to_string(),
        "Repository".to_string(),
    );
    translations.insert(
        "app.details.fields.package_name".to_string(),
        "Package Name".to_string(),
    );
    translations.insert(
        "app.details.fields.version".to_string(),
        "Version".to_string(),
    );
    translations.insert(
        "app.details.fields.description".to_string(),
        "Description".to_string(),
    );
    translations.insert(
        "app.details.fields.architecture".to_string(),
        "Architecture".to_string(),
    );
    translations.insert("app.details.fields.url".to_string(), "URL".to_string());
    translations.insert(
        "app.details.fields.licences".to_string(),
        "Licences".to_string(),
    );
    translations.insert(
        "app.details.fields.provides".to_string(),
        "Provides".to_string(),
    );
    translations.insert(
        "app.details.fields.depends_on".to_string(),
        "Depends on".to_string(),
    );
    translations.insert(
        "app.details.fields.optional_dependencies".to_string(),
        "Optional dependencies".to_string(),
    );
    translations.insert(
        "app.details.fields.required_by".to_string(),
        "Required by".to_string(),
    );
    translations.insert(
        "app.details.fields.optional_for".to_string(),
        "Optional for".to_string(),
    );
    translations.insert(
        "app.details.fields.conflicts_with".to_string(),
        "Conflicts with".to_string(),
    );
    translations.insert(
        "app.details.fields.replaces".to_string(),
        "Replaces".to_string(),
    );
    translations.insert(
        "app.details.fields.download_size".to_string(),
        "Download size".to_string(),
    );
    translations.insert(
        "app.details.fields.install_size".to_string(),
        "Install size".to_string(),
    );
    translations.insert(
        "app.details.fields.package_owner".to_string(),
        "Package Owner".to_string(),
    );
    translations.insert(
        "app.details.fields.build_date".to_string(),
        "Build date".to_string(),
    );
    translations.insert(
        "app.details.fields.not_available".to_string(),
        "N/A".to_string(),
    );
    translations.insert(
        "app.details.show_pkgbuild".to_string(),
        "Show PKGBUILD".to_string(),
    );
    translations.insert(
        "app.details.hide_pkgbuild".to_string(),
        "Hide PKGBUILD".to_string(),
    );
    translations.insert("app.details.url_label".to_string(), "URL:".to_string());
    // Results
    translations.insert("app.results.title".to_string(), "Results".to_string());
    translations.insert("app.results.buttons.sort".to_string(), "Sort".to_string());
    translations.insert(
        "app.results.buttons.options".to_string(),
        "Options".to_string(),
    );
    translations.insert(
        "app.results.buttons.panels".to_string(),
        "Panels".to_string(),
    );
    translations.insert(
        "app.results.buttons.config_lists".to_string(),
        "Config/Lists".to_string(),
    );
    translations.insert("app.results.filters.aur".to_string(), "AUR".to_string());
    translations.insert("app.results.filters.core".to_string(), "core".to_string());
    translations.insert("app.results.filters.extra".to_string(), "extra".to_string());
    translations.insert(
        "app.results.filters.multilib".to_string(),
        "multilib".to_string(),
    );
    translations.insert("app.results.filters.eos".to_string(), "EOS".to_string());
    translations.insert(
        "app.results.filters.cachyos".to_string(),
        "CachyOS".to_string(),
    );
    translations.insert("app.results.filters.artix".to_string(), "Artix".to_string());
    translations.insert(
        "app.results.filters.artix_omniverse".to_string(),
        "OMNI".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_universe".to_string(),
        "UNI".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_lib32".to_string(),
        "LIB32".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_galaxy".to_string(),
        "GALAXY".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_world".to_string(),
        "WORLD".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_system".to_string(),
        "SYSTEM".to_string(),
    );
    translations.insert(
        "app.results.filters.manjaro".to_string(),
        "Manjaro".to_string(),
    );
    app.translations = translations.clone();
    app.translations_fallback = translations;
}

fn item_official(name: &str, repo: &str) -> crate::state::PackageItem {
    crate::state::PackageItem {
        name: name.to_string(),
        version: "1.0".to_string(),
        description: format!("{name} desc"),
        source: crate::state::Source::Official {
            repo: repo.to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    }
}

#[test]
/// What: Validate helper functions that filter recent/install indices and toggle details labels.
///
/// Inputs:
/// - Recent list with pane find queries, install list search term, and details populated with metadata.
///
/// Output:
/// - Filtered indices match expectations and the details footer alternates between `Show`/`Hide PKGBUILD` labels.
///
/// Details:
/// - Covers the case-insensitive dedupe path plus button label toggling when PKGBUILD visibility flips.
fn filtered_indices_and_details_lines() {
    let mut app = crate::state::AppState::default();
    init_test_translations(&mut app);
    app.recent = vec!["alpha".into(), "bravo".into(), "charlie".into()];
    assert_eq!(filtered_recent_indices(&app), vec![0, 1, 2]);
    app.focus = crate::state::Focus::Recent;
    app.pane_find = Some("a".into());
    let inds = filtered_recent_indices(&app);
    assert_eq!(inds, vec![0, 1, 2]);

    app.install_list = vec![
        item_official("ripgrep", "extra"),
        crate::state::PackageItem {
            name: "fd".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        },
    ];
    app.focus = crate::state::Focus::Install;
    app.pane_find = Some("rip".into());
    let inds2 = filtered_install_indices(&app);
    assert_eq!(inds2, vec![0]);

    app.details = crate::state::PackageDetails {
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
    let th = crate::theme::theme();
    let lines = format_details_lines(&app, 80, &th);
    assert!(
        lines
            .last()
            .expect("lines should not be empty in test")
            .spans[0]
            .content
            .contains("Show PKGBUILD")
    );
    let mut app2 = crate::state::AppState::default();
    init_test_translations(&mut app2);
    app2.details = app.details.clone();
    app2.pkgb_visible = true;
    let lines2 = format_details_lines(&app2, 80, &th);
    assert!(
        lines2
            .last()
            .expect("lines2 should not be empty in test")
            .spans[0]
            .content
            .contains("Hide PKGBUILD")
    );
}

#[test]
/// What: Ensure details rendering formats lists and byte sizes into human-friendly strings.
///
/// Inputs:
/// - `PackageDetails` with empty license list, multiple provides, and a non-zero install size.
///
/// Output:
/// - Renders `N/A` for missing values, formats bytes into `1.5 KiB`, and joins lists with commas.
///
/// Details:
/// - Confirms string composition matches UI expectations for optional fields.
fn details_lines_sizes_and_lists() {
    let mut app = crate::state::AppState::default();
    init_test_translations(&mut app);
    app.details = crate::state::PackageDetails {
        repository: "extra".into(),
        name: "ripgrep".into(),
        version: "14".into(),
        description: "desc".into(),
        architecture: "x86_64".into(),
        url: String::new(),
        licenses: vec![],
        groups: vec![],
        provides: vec!["prov1".into(), "prov2".into()],
        depends: vec![],
        opt_depends: vec![],
        required_by: vec![],
        optional_for: vec![],
        conflicts: vec![],
        replaces: vec![],
        download_size: None,
        install_size: Some(1536),
        owner: String::new(),
        build_date: String::new(),
        popularity: None,
    };
    let th = crate::theme::theme();
    let lines = format_details_lines(&app, 80, &th);
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("N/A")))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("1.5 KiB")))
    );
    assert!(lines.iter().any(|l| {
        l.spans
            .iter()
            .any(|s| s.content.contains("Licences") || s.content.contains('-'))
    }));
    assert!(
        lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content.contains("prov1, prov2")))
    );
}

#[tokio::test]
/// What: Ensure the recent preview trigger becomes a no-op when focus or selection is invalid.
///
/// Inputs:
/// - `app`: Focus initially outside Recent, then Recent with no selection, then Recent with a filtered-out entry.
/// - `tx`: Preview channel observed for emitted messages across each scenario.
///
/// Output:
/// - Each invocation leaves the channel empty, showing no preview requests were issued.
///
/// Details:
/// - Applies a short timeout for each check to guard against unexpected asynchronous sends.
async fn trigger_recent_preview_noop_when_not_recent_or_invalid() {
    let mut app = crate::state::AppState::default();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    app.focus = crate::state::Focus::Search;
    trigger_recent_preview(&app, &tx);
    let none1 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none1.is_none());

    app.focus = crate::state::Focus::Recent;
    app.recent = vec!["abc".into()];
    app.history_state.select(None);
    trigger_recent_preview(&app, &tx);
    let none2 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none2.is_none());

    app.history_state.select(Some(0));
    app.pane_find = Some("zzz".into());
    trigger_recent_preview(&app, &tx);
    let none3 = tokio::time::timeout(std::time::Duration::from_millis(30), rx.recv())
        .await
        .ok()
        .flatten();
    assert!(none3.is_none());
}
