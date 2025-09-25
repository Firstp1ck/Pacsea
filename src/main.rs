use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use crossterm::{
    event::{self, Event as CEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::{select, sync::mpsc, time::sleep};

mod state;
use state::*;
mod theme;
mod ui;
use ui::ui;
mod install;
mod logic;
mod ui_helpers;
use logic::{add_to_install_list, send_query};
mod events;
use events::handle_event;
mod index;
mod net;
mod util;
use index as pkgindex;
use net::fetch_details;
use util::{match_rank, repo_order};

// Official index logic is in the pkgindex module

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
    if now.duration_since(app.last_input_change) < Duration::from_secs(5) {
        return;
    }
    if app.last_saved_value.as_deref() == Some(app.input.trim()) {
        return;
    }

    let value = app.input.trim().to_string();
    // de-dup and move-to-front
    if let Some(pos) = app
        .recent
        .iter()
        .position(|s| s.eq_ignore_ascii_case(&value))
    {
        app.recent.remove(pos);
    }
    app.recent.insert(0, value.clone());
    // keep only last 20
    if app.recent.len() > 20 {
        app.recent.truncate(20);
    }
    app.last_saved_value = Some(value);
    app.recent_dirty = true;
}

fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    Ok(())
}

fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // parse a simple --dry-run flag (esp. for Windows testing)
    let dry_run = std::env::args().any(|a| a == "--dry-run");

    setup_terminal()?;

    let res = run_app_with_flags(dry_run).await;

    restore_terminal()?;
    if let Err(err) = res {
        eprintln!("Error: {err:?}");
    }
    Ok(())
}

async fn run_app_with_flags(dry_run: bool) -> Result<()> {
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = AppState {
        dry_run,
        last_input_change: Instant::now(),
        ..Default::default()
    };

    // Load cache from disk if present
    if let Ok(s) = fs::read_to_string(&app.cache_path)
        && let Ok(map) = serde_json::from_str::<HashMap<String, PackageDetails>>(&s)
    {
        app.details_cache = map;
    }

    // Load official index from disk and refresh in background
    pkgindex::load_from_disk(&app.official_index_path);

    // Load recent searches from disk if present
    if let Ok(s) = fs::read_to_string(&app.recent_path)
        && let Ok(list) = serde_json::from_str::<Vec<String>>(&s)
    {
        app.recent = list;
        // de-duplicate (case-insensitive) and keep latest first, cap at 20
        let mut seen = std::collections::HashSet::new();
        app.recent.retain(|q| seen.insert(q.to_lowercase()));
        if app.recent.len() > 20 {
            app.recent.truncate(20);
        }
        if !app.recent.is_empty() {
            app.history_state.select(Some(0));
        }
    }

    // Load install list from disk if present
    if let Ok(s) = fs::read_to_string(&app.install_path)
        && let Ok(list) = serde_json::from_str::<Vec<PackageItem>>(&s)
    {
        app.install_list = list;
        if !app.install_list.is_empty() {
            app.install_state.select(Some(0));
        }
    }

    // Channels
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<CEvent>();
    let (search_result_tx, mut results_rx) = mpsc::unbounded_channel::<SearchResults>();
    let (details_req_tx, mut details_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (details_res_tx, mut details_res_rx) = mpsc::unbounded_channel::<PackageDetails>();
    let (tick_tx, mut tick_rx) = mpsc::unbounded_channel::<()>();
    let (net_err_tx, mut net_err_rx) = mpsc::unbounded_channel::<String>();
    // Preview items (from Recent list hover)
    let (preview_tx, mut preview_rx) = mpsc::unbounded_channel::<PackageItem>();
    // Add-to-install-list channel
    let (add_tx, mut add_rx) = mpsc::unbounded_channel::<PackageItem>();
    // Notify when official index updates
    let (index_notify_tx, mut index_notify_rx) = mpsc::unbounded_channel::<()>();

    // Spawn blocking reader of crossterm events
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50))
                && let Ok(ev) = event::read()
            {
                let _ = event_tx.send(ev);
            }
        }
    });

    // periodic ticks for history saving and cache flush
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            let _ = tick_tx.send(());
        }
    });

    // Background refresh of official index (once on startup)
    pkgindex::update_in_background(
        app.official_index_path.clone(),
        net_err_tx.clone(),
        index_notify_tx.clone(),
    )
    .await;

    // Refresh installed packages cache once at startup
    pkgindex::refresh_installed_cache().await;

    // Search worker with debounce + throttle + tagging
    let (query_tx, mut query_rx) = mpsc::unbounded_channel::<QueryInput>();
    let net_err_tx_search = net_err_tx.clone();
    let index_path = app.official_index_path.clone();
    tokio::spawn(async move {
        const DEBOUNCE_MS: u64 = 250;
        const MIN_INTERVAL_MS: u64 = 300; // throttle
        let mut last_sent = Instant::now() - Duration::from_millis(MIN_INTERVAL_MS);
        loop {
            // wait for first input
            let mut latest = match query_rx.recv().await {
                Some(q) => q,
                None => break,
            };
            // debounce further updates
            loop {
                select! { Some(new_q) = query_rx.recv() => { latest = new_q; } _ = sleep(Duration::from_millis(DEBOUNCE_MS)) => { break; } }
            }
            if latest.text.trim().is_empty() {
                // show full official list when query is empty; fetch if index empty
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
            // enforce min interval between outgoing network searches
            let elapsed = last_sent.elapsed();
            if elapsed < Duration::from_millis(MIN_INTERVAL_MS) {
                sleep(Duration::from_millis(MIN_INTERVAL_MS) - elapsed).await;
            }
            last_sent = Instant::now();

            let qtext = latest.text.clone();
            let sid = latest.id;
            let tx = search_result_tx.clone();
            let err_tx = net_err_tx_search.clone();
            tokio::spawn(async move {
                let mut items = pkgindex::search_official(&qtext);
                let q_for_net = qtext.clone();
                let (aur_items, errors) = net::fetch_all_with_errors(q_for_net).await;
                items.extend(aur_items);
                // sort like before
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

    // Trigger initial search now that the query channel exists
    send_query(&mut app, &query_tx);

    // Details worker (debounced)
    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_DEBOUNCE_MS: u64 = 120; // was 200, speed up
        loop {
            let mut latest = match details_req_rx.recv().await {
                Some(i) => i,
                None => break,
            };
            // collect rapid changes briefly
            loop {
                select! { Some(next) = details_req_rx.recv() => { latest = next; } _ = sleep(Duration::from_millis(DETAILS_DEBOUNCE_MS)) => { break; } }
            }
            match fetch_details(latest.clone()).await {
                Ok(details) => {
                    let _ = details_res_tx.send(details);
                }
                Err(e) => {
                    let msg = match latest.source {
                        Source::Official { .. } => format!(
                            "Official package details unavailable for {}: {}",
                            latest.name, e
                        ),
                        Source::Aur => {
                            format!("AUR package details unavailable for {}: {}", latest.name, e)
                        }
                    };
                    let _ = net_err_tx_details.send(msg);
                }
            }
        }
    });

    loop {
        let _ = terminal.draw(|f| ui(f, &mut app));

        select! {
            // UI events
            Some(ev) = event_rx.recv() => { if handle_event(ev, &mut app, &query_tx, &details_req_tx, &preview_tx, &add_tx) { break; } }
            // Official index updated -> rerun current search (refresh UI)
            Some(_) = index_notify_rx.recv() => { send_query(&mut app, &query_tx); }
            // Search results
            Some(new_results) = results_rx.recv() => {
                // ignore stale results
                if new_results.id != app.latest_query_id { continue; }
                app.results = new_results.items; app.selected = 0; app.list_state.select(if app.results.is_empty(){None}else{Some(0)});
                if let Some(item) = app.results.first().cloned() { if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item.clone()); } }
                // Prefetch details for first few visible items to improve perceived speed
                let prefetch: Vec<PackageItem> = app.results.iter().take(10).cloned().collect();
                for it in prefetch.into_iter().skip(1) {
                    if !app.details_cache.contains_key(&it.name) { let _ = details_req_tx.send(it); }
                }
                // Opportunistically enrich official descriptions for this page
                if !app.results.is_empty() {
                    let names: Vec<String> = app
                        .results
                        .iter()
                        .filter(|p| matches!(p.source, Source::Official { .. }))
                        .map(|p| p.name.clone())
                        .collect();
                    crate::index::request_enrich_for(
                        app.official_index_path.clone(),
                        index_notify_tx.clone(),
                        names,
                    );
                }
            }
            // Details ready
            Some(details) = details_res_rx.recv() => {
                // store and persist later
                app.details = details.clone();
                app.details_cache.insert(details.name.clone(), details);
                app.cache_dirty = true;
            }
            // Preview item from Recent focus
            Some(item) = preview_rx.recv() => {
                if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item); }
                // Opportunistically enrich official index descriptions for current page
                if !app.results.is_empty() {
                    let names: Vec<String> = app
                        .results
                        .iter()
                        .filter(|p| matches!(p.source, Source::Official { .. }))
                        .map(|p| p.name.clone())
                        .collect();
                    crate::index::request_enrich_for(
                        app.official_index_path.clone(),
                        index_notify_tx.clone(),
                        names,
                    );
                }
            }
            // Add to install list
            Some(item) = add_rx.recv() => {
                add_to_install_list(&mut app, item);
            }
            Some(msg) = net_err_rx.recv() => { app.modal = Modal::Alert { message: msg }; }
            Some(_) = tick_rx.recv() => { maybe_save_recent(&mut app); maybe_flush_cache(&mut app); maybe_flush_recent(&mut app); maybe_flush_install(&mut app); }
            else => {}
        }
    }

    // Final flush before exit
    maybe_flush_cache(&mut app);
    maybe_flush_recent(&mut app);
    maybe_flush_install(&mut app);

    Ok(())
}
