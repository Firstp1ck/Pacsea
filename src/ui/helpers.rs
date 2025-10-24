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
pub fn format_details_lines(app: &AppState, _area_width: u16, th: &Theme) -> Vec<Line<'static>> {
    /// Build a key-value display line with themed styling.
    ///
    /// The key is shown in bold with an accent color, followed by a value in
    /// the primary text color.
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
    // Each line is a label/value pair derived from the current details view.
    let mut lines = vec![
        kv("Repository", d.repository.clone(), th),
        kv("Package Name", d.name.clone(), th),
        kv("Version", d.version.clone(), th),
        kv("Description", d.description.clone(), th),
        kv("Architecture", d.architecture.clone(), th),
        kv("URL", d.url.clone(), th),
        kv("Licences", join(&d.licenses), th),
        kv("Provides", join(&d.provides), th),
        kv("Depends on", join(&d.depends), th),
        kv("Optional dependencies", join(&d.opt_depends), th),
        kv("Required by", join(&d.required_by), th),
        kv("Optional for", join(&d.optional_for), th),
        kv("Conflicts with", join(&d.conflicts), th),
        kv("Replaces", join(&d.replaces), th),
        kv(
            "Download size",
            d.download_size
                .map(human_bytes)
                .unwrap_or_else(|| "N/A".to_string()),
            th,
        ),
        kv(
            "Install size",
            d.install_size
                .map(human_bytes)
                .unwrap_or_else(|| "N/A".to_string()),
            th,
        ),
        kv("Package Owner", d.owner.clone(), th),
        kv("Build date", d.build_date.clone(), th),
    ];
    // Add a clickable helper line to Show/Hide PKGBUILD below Build date
    let pkgb_label = if app.pkgb_visible {
        "Hide PKGBUILD"
    } else {
        "Show PKGBUILD"
    };
    lines.push(Line::from(vec![Span::styled(
        pkgb_label,
        Style::default()
            .fg(th.mauve)
            .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
    )]));
    lines
}

/// Join a list of strings into a comma-separated value.
///
/// Returns "-" when the list is empty to keep the UI compact and readable.
/// What: Join a slice of strings with ", ", or return "-" when empty.
///
/// Inputs:
/// - `list`: Slice of strings to join
///
/// Output:
/// - Comma-separated string or "-" when `list` is empty.
fn join(list: &[String]) -> String {
    if list.is_empty() {
        "-".into()
    } else {
        list.join(", ")
    }
}

/// Convert a byte count to a concise human-readable string using binary units.
///
/// Uses 1024-based units (KiB, MiB, GiB, ...) with one decimal place.
/// What: Format bytes using binary units with one decimal place (e.g., 1.5 KiB).
///
/// Inputs:
/// - `n`: Byte count
///
/// Output:
/// - Human-friendly size string using 1024-based units.
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

#[cfg(test)]
mod tests {
    use super::*;

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
    /// What: Validate filtered indices for Recent/Install and details formatting labels
    ///
    /// - Input: Recent entries with pane_find; Install list with search; details with fields
    /// - Output: Correct filtered indices; Show/Hide PKGBUILD label toggles
    fn filtered_indices_and_details_lines() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
    /// What: Validate details field rendering for sizes and list formatting
    ///
    /// - Input: Details with empty licenses, provides list, Some install_size
    /// - Output: Shows N/A for missing; 1.5 KiB formatting; lists joined with commas
    fn details_lines_sizes_and_lists() {
        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
    /// What: Ensure recent preview trigger is a no-op for non-Recent or invalid selection
    ///
    /// - Input: Focus not Recent; Recent without selection; filtered-out index
    /// - Output: No messages sent on preview channel
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
