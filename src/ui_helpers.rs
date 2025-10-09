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

/// Format the current [`AppState::details`] into themed `ratatui` lines.
///
/// Returns key/value rows for commonly displayed metadata followed by a
/// trailing "Show PKGBUILD" action line. The `_area_width` parameter is
/// reserved for future wrapping/layout needs and is currently unused.
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

/// Resolve a free-form query string to a best-effort matching package.
///
/// Selection priority:
///
/// 1. Exact-name match from the official index
/// 2. Exact-name match from AUR
/// 3. First result from the official index
/// 4. Otherwise, first AUR result (if any)
///
/// This function performs network I/O to fetch AUR results and therefore is
/// asynchronous. Errors from network calls are tolerated; the function simply
/// returns `None` if nothing usable is found.
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
    let (aur, _errors) = crate::net::fetch_all_with_errors(q.clone()).await;
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

/// Produce the list of visible indices into `app.recent`, respecting pane-find
/// filtering only when the Recent pane has focus and a non-empty pattern is
/// set.
///
/// Returns indices in ascending order without modifying application state.
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

/// Produce the list of visible indices into `app.install_list`, respecting
/// pane-find filtering only when the Install pane has focus and a non-empty
/// pattern is set.
///
/// Returns indices in ascending order without modifying application state.
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

/// Trigger an asynchronous preview fetch for the currently selected Recent
/// query, if applicable.
///
/// This function exits early unless:
///
/// - The Recent pane has focus
/// - There is a valid selection within bounds of the filtered view
/// - The corresponding query string exists
///
/// When conditions are met, a Tokio task is spawned to resolve the query to a
/// candidate package (via [`fetch_first_match_for_query`]) and send it over the
/// provided `preview_tx`. Send errors are ignored to keep the UI responsive even
/// if downstream receivers were dropped.
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
