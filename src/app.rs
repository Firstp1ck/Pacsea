//! Pacsea application runtime (terminal lifecycle, async workers, and event loop).
//!
//! This module encapsulates the entire TUI runtime previously in `main.rs` so
//! that the binary entrypoint stays minimal.

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::{select, sync::mpsc, time::sleep};

use crate::index as pkgindex;
use crate::logic::{add_to_install_list, send_query};
use crate::net;
use crate::net::fetch_details;
use crate::state::*;
use crate::ui::ui;
use crate::util::{match_rank, repo_order};

/// Start the Pacsea TUI runtime and run the main event loop.
///
/// - Initializes the terminal (raw mode, alternate screen, mouse capture)
/// - Loads persisted caches and settings; spawns background workers for search,
///   details retrieval, PKGBUILD fetch, and periodic ticks
/// - Drives rendering via `ratatui` and delegates input handling to `events`
/// - Persists state periodically (recent searches, details cache, install list)
///
/// Returns `Ok(())` on normal shutdown or an error if initialization fails.
fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}

fn maybe_flush_cache(app: &mut AppState) {
    if !app.cache_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.details_cache) {
        let _ = fs::write(&app.cache_path, s);
        app.cache_dirty = false;
    }
}

fn maybe_flush_recent(app: &mut AppState) {
    if !app.recent_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.recent) {
        let _ = fs::write(&app.recent_path, s);
        app.recent_dirty = false;
    }
}

fn maybe_flush_install(app: &mut AppState) {
    if !app.install_dirty {
        return;
    }
    if let Ok(s) = serde_json::to_string(&app.install_list) {
        let _ = fs::write(&app.install_path, s);
        app.install_dirty = false;
    }
}

fn maybe_save_recent(app: &mut AppState) {
    let now = Instant::now();
    if app.input.trim().is_empty() {
        return;
    }
    if now.duration_since(app.last_input_change) < Duration::from_secs(3) {
        return;
    }
    if app.last_saved_value.as_deref() == Some(app.input.trim()) {
        return;
    }

    let value = app.input.trim().to_string();
    if let Some(pos) = app
        .recent
        .iter()
        .position(|s| s.eq_ignore_ascii_case(&value))
    {
        app.recent.remove(pos);
    }
    app.recent.insert(0, value.clone());
    if app.recent.len() > 20 {
        app.recent.truncate(20);
    }
    app.last_saved_value = Some(value);
    app.recent_dirty = true;
}

/// Return today's UTC date as (year, month, day) using the system `date` command.
fn today_ymd_utc() -> Option<(i32, u32, u32)> {
    let out = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d"]) // e.g., 2025-10-11
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    let mut it = s.split('-');
    let y = it.next()?.parse::<i32>().ok()?;
    let m = it.next()?.parse::<u32>().ok()?;
    let d = it.next()?.parse::<u32>().ok()?;
    Some((y, m, d))
}

/// Try to parse various short date formats used by Arch RSS into (Y,M,D).
fn parse_news_date_to_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let t = s.trim();
    // Case 1: ISO: YYYY-MM-DD
    if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
        let y = t[0..4].parse::<i32>().ok()?;
        let m = t[5..7].parse::<u32>().ok()?;
        let d = t[8..10].parse::<u32>().ok()?;
        return Some((y, m, d));
    }
    // Case 2: "Sat, 05 Oct 2024" or "05 Oct 2024"
    let part = if let Some((_, rhs)) = t.split_once(',') {
        rhs.trim()
    } else {
        t
    };
    let mut it = part.split_whitespace();
    let d_s = it.next()?; // e.g., 05
    let m_s = it.next()?; // e.g., Oct
    let y_s = it.next()?; // e.g., 2024
    let d = d_s.parse::<u32>().ok()?;
    let y = y_s.parse::<i32>().ok()?;
    let m = match m_s {
        "Jan" | "January" => 1,
        "Feb" | "February" => 2,
        "Mar" | "March" => 3,
        "Apr" | "April" => 4,
        "May" => 5,
        "Jun" | "June" => 6,
        "Jul" | "July" => 7,
        "Aug" | "August" => 8,
        "Sep" | "Sept" | "September" => 9,
        "Oct" | "October" => 10,
        "Nov" | "November" => 11,
        "Dec" | "December" => 12,
        _ => return None,
    };
    Some((y, m, d))
}

pub async fn run(dry_run_flag: bool) -> Result<()> {
    setup_terminal()?;

    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = AppState {
        dry_run: if dry_run_flag {
            true
        } else {
            crate::theme::settings().app_dry_run_default
        },
        last_input_change: Instant::now(),
        ..Default::default()
    };

    let prefs = crate::theme::settings();
    app.layout_left_pct = prefs.layout_left_pct;
    app.layout_center_pct = prefs.layout_center_pct;
    app.layout_right_pct = prefs.layout_right_pct;
    app.keymap = prefs.keymap;
    app.sort_mode = prefs.sort_mode;

    if let Ok(s) = fs::read_to_string(&app.cache_path)
        && let Ok(map) = serde_json::from_str::<HashMap<String, PackageDetails>>(&s)
    {
        app.details_cache = map;
    }
    if let Ok(s) = fs::read_to_string(&app.recent_path)
        && let Ok(list) = serde_json::from_str::<Vec<String>>(&s)
    {
        app.recent = list;
        if !app.recent.is_empty() {
            app.history_state.select(Some(0));
        }
    }
    if let Ok(s) = fs::read_to_string(&app.install_path)
        && let Ok(list) = serde_json::from_str::<Vec<PackageItem>>(&s)
    {
        app.install_list = list;
        if !app.install_list.is_empty() {
            app.install_state.select(Some(0));
        }
    }

    pkgindex::load_from_disk(&app.official_index_path);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<CEvent>();
    let (search_result_tx, mut results_rx) = mpsc::unbounded_channel::<SearchResults>();
    let (details_req_tx, mut details_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (details_res_tx, mut details_res_rx) = mpsc::unbounded_channel::<PackageDetails>();
    let (tick_tx, mut tick_rx) = mpsc::unbounded_channel::<()>();
    let (net_err_tx, mut net_err_rx) = mpsc::unbounded_channel::<String>();
    let (preview_tx, mut preview_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (add_tx, mut add_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (index_notify_tx, mut index_notify_rx) = mpsc::unbounded_channel::<()>();
    let (pkgb_req_tx, mut pkgb_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (pkgb_res_tx, mut pkgb_res_rx) = mpsc::unbounded_channel::<(String, String)>();
    let (status_tx, mut status_rx) =
        mpsc::unbounded_channel::<(String, crate::state::ArchStatusColor)>();
    let (news_tx, mut news_rx) = mpsc::unbounded_channel::<Vec<NewsItem>>();

    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_BATCH_WINDOW_MS: u64 = 120;
        loop {
            let first = match details_req_rx.recv().await {
                Some(i) => i,
                None => break,
            };
            let mut batch: Vec<PackageItem> = vec![first];
            loop {
                tokio::select! {
                    Some(next) = details_req_rx.recv() => { batch.push(next); }
                    _ = sleep(Duration::from_millis(DETAILS_BATCH_WINDOW_MS)) => { break; }
                }
            }
            use std::collections::HashSet;
            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<PackageItem> = Vec::with_capacity(batch.len());
            for it in batch.into_iter() {
                if seen.insert(it.name.clone()) {
                    ordered.push(it);
                }
            }
            for it in ordered.into_iter() {
                if !crate::logic::is_allowed(&it.name) {
                    continue;
                }
                match fetch_details(it.clone()).await {
                    Ok(details) => {
                        let _ = details_res_tx.send(details);
                    }
                    Err(e) => {
                        let msg = match it.source {
                            Source::Official { .. } => format!(
                                "Official package details unavailable for {}: {}",
                                it.name, e
                            ),
                            Source::Aur => {
                                format!("AUR package details unavailable for {}: {}", it.name, e)
                            }
                        };
                        let _ = net_err_tx_details.send(msg);
                    }
                }
            }
        }
    });

    tokio::spawn(async move {
        while let Some(item) = pkgb_req_rx.recv().await {
            let name = item.name.clone();
            match net::fetch_pkgbuild_fast(&item).await {
                Ok(txt) => {
                    let _ = pkgb_res_tx.send((name, txt));
                }
                Err(e) => {
                    let _ = pkgb_res_tx.send((name, format!("Failed to fetch PKGBUILD: {e}")));
                }
            }
        }
    });

    // Fetch Arch status text in background once at startup, then occasionally refresh
    let status_tx_once = status_tx.clone();
    tokio::spawn(async move {
        if let Ok((txt, color)) = net::fetch_arch_status_text().await {
            let _ = status_tx_once.send((txt, color));
        }
    });

    // Fetch Arch news once at startup; if any items are dated today (UTC), show modal
    let news_tx_once = news_tx.clone();
    tokio::spawn(async move {
        if let Ok(list) = net::fetch_arch_news(10).await {
            let today = today_ymd_utc();
            let todays: Vec<NewsItem> = match today {
                Some((ty, tm, td)) => list
                    .into_iter()
                    .filter(|it| {
                        parse_news_date_to_ymd(&it.date)
                            == Some((ty, tm, td))
                    })
                    .collect(),
                None => Vec::new(),
            };
            if !todays.is_empty() {
                let _ = news_tx_once.send(todays);
            } else {
                // Signal "no news today" by sending an empty list
                let _ = news_tx_once.send(Vec::new());
            }
        }
    });

    pkgindex::update_in_background(
        app.official_index_path.clone(),
        net_err_tx.clone(),
        index_notify_tx.clone(),
    )
    .await;

    pkgindex::refresh_installed_cache().await;
    pkgindex::refresh_explicit_cache().await;

    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50))
                && let Ok(ev) = event::read()
            {
                let _ = event_tx.send(ev);
            }
        }
    });

    let tick_tx_bg = tick_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(200));
        loop {
            interval.tick().await;
            let _ = tick_tx_bg.send(());
        }
    });

    let (query_tx, mut query_rx) = mpsc::unbounded_channel::<QueryInput>();
    let net_err_tx_search = net_err_tx.clone();
    let index_path = app.official_index_path.clone();
    tokio::spawn(async move {
        const DEBOUNCE_MS: u64 = 250;
        const MIN_INTERVAL_MS: u64 = 300;
        let mut last_sent = Instant::now() - Duration::from_millis(MIN_INTERVAL_MS);
        loop {
            let mut latest = match query_rx.recv().await {
                Some(q) => q,
                None => break,
            };
            loop {
                select! { Some(new_q) = query_rx.recv() => { latest = new_q; } _ = sleep(Duration::from_millis(DEBOUNCE_MS)) => { break; } }
            }
            if latest.text.trim().is_empty() {
                let mut items = pkgindex::all_official_or_fetch(&index_path).await;
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                let _ = search_result_tx.send(SearchResults {
                    id: latest.id,
                    items,
                });
                continue;
            }
            let elapsed = last_sent.elapsed();
            if elapsed < Duration::from_millis(MIN_INTERVAL_MS) {
                sleep(Duration::from_millis(MIN_INTERVAL_MS) - elapsed).await;
            }
            last_sent = Instant::now();

            let qtext = latest.text.clone();
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            let ipath = index_path.clone();
            tokio::spawn(async move {
                if crate::index::all_official().is_empty() {
                    let _ = crate::index::all_official_or_fetch(&ipath).await;
                }
                let mut items = pkgindex::search_official(&qtext);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = net::fetch_all_with_errors(q_for_net).await;
                items.extend(aur_items);
                let ql = qtext.trim().to_lowercase();
                items.sort_by(|a, b| {
                    let oa = repo_order(&a.source);
                    let ob = repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                    let ra = match_rank(&a.name, &ql);
                    let rb = match_rank(&b.name, &ql);
                    if ra != rb {
                        return ra.cmp(&rb);
                    }
                    a.name.to_lowercase().cmp(&b.name.to_lowercase())
                });
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults { id: sid, items });
            });
        }
    });

    send_query(&mut app, &query_tx);

    loop {
        let _ = terminal.draw(|f| ui(f, &mut app));

        select! {
            Some(ev) = event_rx.recv() => { if crate::events::handle_event(ev, &mut app, &query_tx, &details_req_tx, &preview_tx, &add_tx, &pkgb_req_tx) { break; } }
            Some(_) = index_notify_rx.recv() => {
                app.loading_index = false;
                let _ = tick_tx.send(());
            }
            Some(new_results) = results_rx.recv() => {
                if new_results.id != app.latest_query_id { continue; }
                let prev_selected_name = app.results.get(app.selected).map(|p| p.name.clone());
                app.all_results = new_results.items;
                crate::logic::apply_filters_and_sort_preserve_selection(&mut app);
                let new_sel = prev_selected_name
                    .and_then(|name| app.results.iter().position(|p| p.name == name))
                    .unwrap_or(0);
                app.selected = new_sel.min(app.results.len().saturating_sub(1));
                app.list_state.select(if app.results.is_empty(){None}else{Some(app.selected)});
                if let Some(item) = app.results.get(app.selected).cloned() {
                    app.details_focus = Some(item.name.clone());
                    if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item.clone()); }
                }
                crate::logic::set_allowed_ring(&app, 30);
                if app.need_ring_prefetch { /* defer */ } else { crate::logic::ring_prefetch_from_selected(&mut app, &details_req_tx); }
                let len_u = app.results.len();
                let mut enrich_names: Vec<String> = Vec::new();
                if let Some(sel) = app.results.get(app.selected) && matches!(sel.source, Source::Official { .. }) { enrich_names.push(sel.name.clone()); }
                let max_radius: usize = 30; let mut step: usize = 1; while step <= max_radius { if let Some(i) = app.selected.checked_sub(step) && let Some(it) = app.results.get(i) && matches!(it.source, Source::Official { .. }) { enrich_names.push(it.name.clone()); } let below = app.selected + step; if below < len_u && let Some(it) = app.results.get(below) && matches!(it.source, Source::Official { .. }) { enrich_names.push(it.name.clone()); } step += 1; }
                if !enrich_names.is_empty() { crate::index::request_enrich_for(app.official_index_path.clone(), index_notify_tx.clone(), enrich_names); }
            }
            Some(details) = details_res_rx.recv() => {
                if app.details_focus.as_deref() == Some(details.name.as_str()) {
                    app.details = details.clone();
                }
                app.details_cache.insert(details.name.clone(), details.clone());
                app.cache_dirty = true;
                if let Some(pos) = app.results.iter().position(|p| p.name == details.name) {
                    app.results[pos].description = details.description.clone();
                    if !details.version.is_empty() && app.results[pos].version != details.version { app.results[pos].version = details.version.clone(); }
                    if details.popularity.is_some() { app.results[pos].popularity = details.popularity; }
                    if let crate::state::Source::Official { repo, arch } = &mut app.results[pos].source {
                        if repo.is_empty() && !details.repository.is_empty() { *repo = details.repository.clone(); }
                        if arch.is_empty() && !details.architecture.is_empty() { *arch = details.architecture.clone(); }
                    }
                }
                let _ = tick_tx.send(());
            }
            Some(item) = preview_rx.recv() => {
                if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item.clone()); }
                if !app.results.is_empty() && app.selected >= app.results.len() { app.selected = app.results.len() - 1; app.list_state.select(Some(app.selected)); }
            }
            Some(item) = add_rx.recv() => { add_to_install_list(&mut app, item); }
            Some((pkgname, text)) = pkgb_res_rx.recv() => {
                if app.details_focus.as_deref() == Some(pkgname.as_str()) || app.results.get(app.selected).map(|i| i.name.as_str()) == Some(pkgname.as_str()) {
                    app.pkgb_text = Some(text);
                }
                let _ = tick_tx.send(());
            }
            Some(msg) = net_err_rx.recv() => { app.modal = Modal::Alert { message: msg }; }
            Some(_) = tick_rx.recv() => { maybe_save_recent(&mut app); maybe_flush_cache(&mut app); maybe_flush_recent(&mut app); maybe_flush_install(&mut app);
                if app.need_ring_prefetch && app.ring_resume_at.map(|t| std::time::Instant::now() >= t).unwrap_or(false) {
                    crate::logic::set_allowed_ring(&app, 30);
                    crate::logic::ring_prefetch_from_selected(&mut app, &details_req_tx);
                    app.need_ring_prefetch = false;
                    app.scroll_moves = 0; app.ring_resume_at = None;
                }
                if app.sort_menu_open && let Some(deadline) = app.sort_menu_auto_close_at && std::time::Instant::now() >= deadline {
                    app.sort_menu_open = false; app.sort_menu_auto_close_at = None;
                }
                if let Some(deadline) = app.toast_expires_at
                    && std::time::Instant::now() >= deadline {
                        app.toast_message = None;
                        app.toast_expires_at = None;
                    }
            }
            Some(todays) = news_rx.recv() => {
                if todays.is_empty() {
                    app.toast_message = Some("No new News today".to_string());
                    app.toast_expires_at = Some(Instant::now() + Duration::from_secs(10));
                } else {
                    // Show just today's news items; default to first selected
                    app.modal = Modal::News { items: todays, selected: 0 };
                }
            }
            Some((txt, color)) = status_rx.recv() => {
                app.arch_status_text = txt;
                app.arch_status_color = color;
            }
            else => {}
        }
    }

    maybe_flush_cache(&mut app);
    maybe_flush_recent(&mut app);
    maybe_flush_install(&mut app);

    restore_terminal()?;
    Ok(())
}
