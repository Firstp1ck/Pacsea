use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    state::{AppState, Focus},
    theme::Theme,
};

pub fn format_details_lines(app: &AppState, _area_width: u16, th: &Theme) -> Vec<Line<'static>> {
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
    vec![
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
    ]
}

fn join(list: &[String]) -> String {
    if list.is_empty() {
        "-".into()
    } else {
        list.join(", ")
    }
}

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
