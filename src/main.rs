use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

// Replace anyhow::Result with std error based alias
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use crossterm::{
    event::{self, Event as CEvent},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use serde_json::Value;
use tokio::{select, sync::mpsc, time::sleep};

mod state;
use state::*;
mod theme;
mod ui;
use ui::ui;
mod install;
mod logic;
mod ui_helpers;
use logic::add_to_install_list;
mod events;
use events::handle_event;

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

    // Search worker with debounce + throttle + tagging
    let (query_tx, mut query_rx) = mpsc::unbounded_channel::<QueryInput>();
    let net_err_tx_search = net_err_tx.clone();
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
                let _ = search_result_tx.send(SearchResults {
                    id: latest.id,
                    items: Vec::new(),
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
                let (items, errors) = fetch_all_with_errors(qtext).await;
                for e in errors {
                    let _ = err_tx.send(e);
                }
                let _ = tx.send(SearchResults { id: sid, items });
            });
        }
    });

    // Details worker (debounced)
    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_DEBOUNCE_MS: u64 = 200;
        loop {
            let mut latest = match details_req_rx.recv().await {
                Some(i) => i,
                None => break,
            };
            // collect rapid changes
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
            // Search results
            Some(new_results) = results_rx.recv() => {
                // ignore stale results
                if new_results.id != app.latest_query_id { continue; }
                app.results = new_results.items; app.selected = 0; app.list_state.select(if app.results.is_empty(){None}else{Some(0)});
                if let Some(item) = app.results.first().cloned() { if let Some(cached) = app.details_cache.get(&item.name).cloned() { app.details = cached; } else { let _ = details_req_tx.send(item); } }
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

// Helper: simple percent-encoding for query components (RFC 3986 unreserved set)
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

// Helper that returns items and any server error messages
async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    let client = match reqwest::Client::builder()
        .user_agent("Pacsea/0.1 (+https://archlinux.org)")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return (Vec::new(), vec![format!("HTTP client error: {}", e)]);
        }
    };
    let q = percent_encode(query.trim());
    let official_url = format!("https://archlinux.org/packages/search/json/?q={q}");
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search?by=name&arg={q}");

    let official_fut = async {
        let resp = client
            .get(&official_url)
            .send()
            .await?
            .json::<Value>()
            .await?;
        let mut items = Vec::new();
        if let Some(arr) = resp.get("results").and_then(|v| v.as_array()) {
            for pkg in arr.iter().take(200) {
                let name = s(pkg, "pkgname");
                let version = s(pkg, "pkgver");
                let description = s(pkg, "pkgdesc");
                let repo = s(pkg, "repo");
                let arch = s(pkg, "arch");
                if name.is_empty() {
                    continue;
                }
                items.push(PackageItem {
                    name,
                    version,
                    description,
                    source: Source::Official { repo, arch },
                });
            }
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(items)
    };

    let aur_fut = async {
        let resp = client.get(&aur_url).send().await?.json::<Value>().await?;
        let mut items = Vec::new();
        if let Some(arr) = resp.get("results").and_then(|v| v.as_array()) {
            for pkg in arr.iter().take(200) {
                let name = s(pkg, "Name");
                let version = s(pkg, "Version");
                let description = s(pkg, "Description");
                if name.is_empty() {
                    continue;
                }
                items.push(PackageItem {
                    name,
                    version,
                    description,
                    source: Source::Aur,
                });
            }
        }
        Ok::<_, Box<dyn std::error::Error + Send + Sync>>(items)
    };

    let (o, a) = tokio::join!(official_fut, aur_fut);
    let mut items = Vec::new();
    let mut errors = Vec::new();
    match o {
        Ok(v) => items.extend(v),
        Err(e) => errors.push(format!("Official search unavailable: {e}")),
    }
    match a {
        Ok(v) => items.extend(v),
        Err(e) => errors.push(format!("AUR search unavailable: {e}")),
    }

    // sort like fetch_all
    let ql = query.trim().to_lowercase();
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

    (items, errors)
}

async fn fetch_details(item: PackageItem) -> Result<PackageDetails> {
    match item.source.clone() {
        Source::Official { repo, arch } => fetch_official_details(repo, arch, item).await,
        Source::Aur => fetch_aur_details(item).await,
    }
}

async fn fetch_official_details(
    repo: String,
    arch: String,
    item: PackageItem,
) -> Result<PackageDetails> {
    let url = format!(
        "https://archlinux.org/packages/{}/{}/{}/json/",
        repo.to_lowercase(),
        arch,
        item.name
    );
    let client = reqwest::Client::builder()
        .user_agent("Pacsea/0.1")
        .build()?;
    let v = client.get(url).send().await?.json::<Value>().await?;
    let obj = v.get("pkg").unwrap_or(&v);

    let d = PackageDetails {
        repository: repo,
        name: item.name.clone(),
        version: ss(obj, &["pkgver", "Version"]).unwrap_or(item.version),
        description: ss(obj, &["pkgdesc", "Description"]).unwrap_or(item.description),
        architecture: ss(obj, &["arch", "Architecture"]).unwrap_or(arch),
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
    };
    Ok(d)
}

async fn fetch_aur_details(item: PackageItem) -> Result<PackageDetails> {
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/info?arg[]={}",
        percent_encode(&item.name)
    );
    let client = reqwest::Client::builder()
        .user_agent("Pacsea/0.1")
        .build()?;
    let v = client.get(url).send().await?.json::<Value>().await?;
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let obj = arr.first().cloned().unwrap_or(Value::Null);

    let version0 = s(&obj, "Version");
    let description0 = s(&obj, "Description");

    let d = PackageDetails {
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
        url: s(&obj, "URL"),
        licenses: arrs(&obj, &["License", "Licenses"]),
        groups: arrs(&obj, &["Groups"]),
        provides: arrs(&obj, &["Provides"]),
        depends: arrs(&obj, &["Depends"]),
        opt_depends: arrs(&obj, &["OptDepends"]),
        required_by: vec![],
        optional_for: vec![],
        conflicts: arrs(&obj, &["Conflicts"]),
        replaces: arrs(&obj, &["Replaces"]),
        download_size: None,
        install_size: None,
        owner: s(&obj, "Maintainer"),
        build_date: ts_to_date(obj.get("LastModified").and_then(|v| v.as_i64())),
    };
    Ok(d)
}

fn ts_to_date(ts: Option<i64>) -> String {
    if let Some(t) = ts
        && let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(t, 0)
    {
        return dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();
    }
    "".into()
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn ss(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            return Some(s.to_owned());
        }
    }
    None
}
fn arrs(v: &Value, keys: &[&str]) -> Vec<String> {
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr
                .iter()
                .filter_map(|e| e.as_str().map(|s| s.to_owned()))
                .collect();
        }
    }
    Vec::new()
}
fn u64_of(v: &Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(n) = v.get(*k) {
            if let Some(u) = n.as_u64() {
                return Some(u);
            }
            if let Some(i) = n.as_i64()
                && let Ok(u) = u64::try_from(i)
            {
                return Some(u);
            }
            if let Some(s) = n.as_str()
                && let Ok(p) = s.parse::<u64>()
            {
                return Some(p);
            }
        }
    }
    None
}

fn repo_order(src: &Source) -> u8 {
    match src {
        Source::Official { repo, .. } => {
            if repo.eq_ignore_ascii_case("core") {
                0
            } else if repo.eq_ignore_ascii_case("extra") {
                1
            } else {
                2
            }
        }
        Source::Aur => 3,
    }
}
fn match_rank(name: &str, query_lower: &str) -> u8 {
    let n = name.to_lowercase();
    if !query_lower.is_empty() {
        if n == query_lower {
            return 0;
        }
        if n.starts_with(query_lower) {
            return 1;
        }
        if n.contains(query_lower) {
            return 2;
        }
    }
    3
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
