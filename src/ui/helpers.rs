//! UI helper utilities for formatting and pane-specific behaviors.
//!
//! This module contains small, focused helpers used by the TUI layer:
//!
//! - Formatting package details into rich `ratatui` lines
//! - Human-readable byte formatting
//! - In-pane filtering for Recent and Install panes
//! - Triggering background preview fetches for Recent selections
//! - Resolving a query string to a best-effort first matching package
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    i18n,
    state::{AppState, Focus},
    theme::Theme,
};

/// What: Format the current [`AppState::details`] into themed `ratatui` lines.
///
/// Inputs:
/// - `app`: Read-only application state; uses `app.details` to render fields
/// - `_area_width`: Reserved for future wrapping/layout needs (currently unused)
/// - `th`: Active theme for colors/styles
///
/// Output:
/// - Vector of formatted lines for the Details pane, ending with a Show/Hide PKGBUILD action line.
///
/// Details:
/// - Applies repo-specific heuristics, formats numeric sizes via `human_bytes`, and appends a
///   clickable PKGBUILD toggle line using accent styling.
pub fn format_details_lines(app: &AppState, _area_width: u16, th: &Theme) -> Vec<Line<'static>> {
    /// What: Build a themed key-value line for the details pane.
    ///
    /// Inputs:
    /// - `key`: Label to display (styled in accent color)
    /// - `val`: Value text rendered in primary color
    /// - `th`: Active theme for colors/modifiers
    ///
    /// Output:
    /// - `Line` combining the key/value segments with appropriate styling.
    ///
    /// Details:
    /// - Renders the key in bold accent with a trailing colon and the value in standard text color.
    fn kv(key: &str, val: String, th: &Theme) -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("{key}: "),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(val, Style::default().fg(th.text)),
        ])
    }
    let d = &app.details;
    // Compute display repository using unified Manjaro detection (name prefix or owner).
    let repo_display = if crate::index::is_manjaro_name_or_owner(&d.name, &d.owner) {
        "manjaro".to_string()
    } else {
        d.repository.clone()
    };
    // Each line is a label/value pair derived from the current details view.
    let mut lines = vec![
        kv(
            &i18n::t(app, "app.details.fields.repository"),
            repo_display,
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.package_name"),
            d.name.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.version"),
            d.version.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.description"),
            d.description.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.architecture"),
            d.architecture.clone(),
            th,
        ),
        kv(&i18n::t(app, "app.details.fields.url"), d.url.clone(), th),
        kv(
            &i18n::t(app, "app.details.fields.licences"),
            join(&d.licenses),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.provides"),
            join(&d.provides),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.depends_on"),
            join(&d.depends),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.optional_dependencies"),
            join(&d.opt_depends),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.required_by"),
            join(&d.required_by),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.optional_for"),
            join(&d.optional_for),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.conflicts_with"),
            join(&d.conflicts),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.replaces"),
            join(&d.replaces),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.download_size"),
            d.download_size
                .map(human_bytes)
                .unwrap_or_else(|| i18n::t(app, "app.details.fields.not_available")),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.install_size"),
            d.install_size
                .map(human_bytes)
                .unwrap_or_else(|| i18n::t(app, "app.details.fields.not_available")),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.package_owner"),
            d.owner.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.build_date"),
            d.build_date.clone(),
            th,
        ),
    ];
    // Add a clickable helper line to Show/Hide PKGBUILD below Build date
    let pkgb_label = if app.pkgb_visible {
        i18n::t(app, "app.details.hide_pkgbuild")
    } else {
        i18n::t(app, "app.details.show_pkgbuild")
    };
    lines.push(Line::from(vec![Span::styled(
        pkgb_label,
        Style::default()
            .fg(th.mauve)
            .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
    )]));
    lines
}

/// What: Join a slice of strings with `", "`, falling back to "-" when empty.
///
/// Inputs:
/// - `list`: Slice of strings to format
///
/// Output:
/// - Joined string or "-" when no entries are present.
///
/// Details:
/// - Keeps the details pane compact by representing empty lists with a single dash.
fn join(list: &[String]) -> String {
    if list.is_empty() {
        "-".into()
    } else {
        list.join(", ")
    }
}

/// What: Format a byte count using binary units with one decimal place.
///
/// Inputs:
/// - `n`: Raw byte count to format
///
/// Output:
/// - Size string such as "1.5 KiB" using 1024-based units.
///
/// Details:
/// - Iteratively divides by 1024 up to PiB, retaining one decimal place for readability.
fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{:.1} {}", v, UNITS[i])
}

/// What: Resolve a free-form query string to a best-effort matching package.
///
/// Inputs:
/// - `q`: Query string to resolve
///
/// Output:
/// - `Some(PackageItem)` per the priority rules below; `None` if nothing usable is found.
///
/// Details (selection priority):
///   1) Exact-name match from the official index;
///   2) Exact-name match from AUR;
///   3) First official result;
///   4) Otherwise, first AUR result.
///
/// Performs network I/O for AUR; tolerates errors.
pub async fn fetch_first_match_for_query(q: String) -> Option<crate::state::PackageItem> {
    // Prefer exact match from official index, then from AUR, else first official, then first AUR
    let official = crate::index::search_official(&q);
    if let Some(off) = official
        .iter()
        .find(|it| it.name.eq_ignore_ascii_case(&q))
        .cloned()
    {
        return Some(off);
    }
    let (aur, _errors) = crate::sources::fetch_all_with_errors(q.clone()).await;
    if let Some(a) = aur
        .iter()
        .find(|it| it.name.eq_ignore_ascii_case(&q))
        .cloned()
    {
        return Some(a);
    }
    if let Some(off) = official.first().cloned() {
        return Some(off);
    }
    aur.into_iter().next()
}

/// What: Produce visible indices into `app.recent` considering pane-find when applicable.
///
/// Inputs:
/// - `app`: Application state (focus, pane_find, recent list)
///
/// Output:
/// - Vector of indices in ascending order without modifying application state.
///
/// Details:
/// - Applies pane find filtering only when the Recent pane is focused and the finder string is
///   non-empty; otherwise returns the full range.
pub fn filtered_recent_indices(app: &AppState) -> Vec<usize> {
    let apply = matches!(app.focus, Focus::Recent)
        && app
            .pane_find
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
    if !apply {
        return (0..app.recent.len()).collect();
    }
    let pat = app.pane_find.as_ref().unwrap().to_lowercase();
    app.recent
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            if s.to_lowercase().contains(&pat) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// What: Produce visible indices into `app.install_list` with optional pane-find filtering.
///
/// Inputs:
/// - `app`: Application state (focus, pane_find, install list)
///
/// Output:
/// - Vector of indices in ascending order without modifying application state.
///
/// Details:
/// - Restricts matches to name or description substrings when the Install pane is focused and a
///   pane-find expression is active; otherwise surfaces all indices.
pub fn filtered_install_indices(app: &AppState) -> Vec<usize> {
    let apply = matches!(app.focus, Focus::Install)
        && app
            .pane_find
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
    if !apply {
        return (0..app.install_list.len()).collect();
    }
    let pat = app.pane_find.as_ref().unwrap().to_lowercase();
    app.install_list
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let name = p.name.to_lowercase();
            let desc = p.description.to_lowercase();
            if name.contains(&pat) || desc.contains(&pat) {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// What: Trigger an asynchronous preview fetch for the selected Recent query when applicable.
///
/// Inputs:
/// - `app`: Application state (focus, selection, recent list)
/// - `preview_tx`: Channel to send the preview `PackageItem`
///
/// Output:
/// - Spawns a task to resolve and send a preview item; no return payload; exits early when inapplicable.
///
/// Details:
/// - Requires: focus on Recent, a valid selection within the filtered view, and a query string present.
/// - Resolves via [`fetch_first_match_for_query`] and sends over `preview_tx`; ignores send errors.
pub fn trigger_recent_preview(
    app: &crate::state::AppState,
    preview_tx: &tokio::sync::mpsc::UnboundedSender<crate::state::PackageItem>,
) {
    if !matches!(app.focus, crate::state::Focus::Recent) {
        return;
    }
    let idx = match app.history_state.selected() {
        Some(i) => i,
        None => return,
    };
    let inds = filtered_recent_indices(app);
    if idx >= inds.len() {
        return;
    }
    let Some(q) = app.recent.get(inds[idx]).cloned() else {
        return;
    };
    let tx = preview_tx.clone();
    tokio::spawn(async move {
        if let Some(item) = fetch_first_match_for_query(q).await {
            let _ = tx.send(item);
        }
    });
}

/// What: Check if a package is currently being processed by any preflight resolver.
///
/// Inputs:
/// - `app`: Application state containing preflight resolution queues and flags.
/// - `package_name`: Name of the package to check.
///
/// Output:
/// - `true` if the package is in any preflight resolution queue and the corresponding resolver is active.
///
/// Details:
/// - Checks if the package name appears in any of the preflight queues (summary, deps, files, services, sandbox)
///   and if the corresponding resolving flag is set to true.
/// - Also checks install list resolution (when preflight modal is not open) by checking if the package
///   is in `app.install_list` and any resolver is active.
pub fn is_package_loading_preflight(app: &AppState, package_name: &str) -> bool {
    // Check summary resolution (preflight-specific)
    if app.preflight_summary_resolving
        && let Some((ref items, _)) = app.preflight_summary_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check dependency resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_deps_resolving
        && let Some(ref items) = app.preflight_deps_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if deps are actually resolving AND package is in install list
    if app.deps_resolving
        && !app.preflight_deps_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check file resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_files_resolving
        && let Some(ref items) = app.preflight_files_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if files are actually resolving AND preflight is not resolving
    if app.files_resolving
        && !app.preflight_files_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check service resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_services_resolving
        && let Some(ref items) = app.preflight_services_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Only show indicator if services are actually resolving AND preflight is not resolving
    if app.services_resolving
        && !app.preflight_services_resolving
        && app.install_list.iter().any(|p| p.name == package_name)
    {
        return true;
    }

    // Check sandbox resolution
    // First check preflight-specific queue (when modal is open)
    if app.preflight_sandbox_resolving
        && let Some(ref items) = app.preflight_sandbox_items
        && items.iter().any(|p| p.name == package_name)
    {
        return true;
    }
    // Then check install list resolution (when modal is not open)
    // Note: sandbox only applies to AUR packages
    // Only show indicator if sandbox is actually resolving AND preflight is not resolving
    if app.sandbox_resolving
        && !app.preflight_sandbox_resolving
        && app
            .install_list
            .iter()
            .any(|p| p.name == package_name && matches!(p.source, crate::state::Source::Aur))
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Initialize minimal English translations for tests.
    ///
    /// Inputs:
    /// - `app`: AppState to populate with translations
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
            "omniverse".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_universe".to_string(),
            "universe".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_lib32".to_string(),
            "lib32".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_galaxy".to_string(),
            "galaxy".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_world".to_string(),
            "world".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_system".to_string(),
            "system".to_string(),
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
        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
            lines.last().unwrap().spans[0]
                .content
                .contains("Show PKGBUILD")
        );
        let mut app2 = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app2);
        app2.details = app.details.clone();
        app2.pkgb_visible = true;
        let lines2 = format_details_lines(&app2, 80, &th);
        assert!(
            lines2.last().unwrap().spans[0]
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
        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
                .any(|s| s.content.contains("Licences") || s.content.contains("-"))
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
        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
}
