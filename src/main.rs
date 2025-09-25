use std::fs;
use std::{collections::HashMap, path::PathBuf};
use std::{
    process::Command,
    time::{Duration, Instant},
};

// Replace anyhow::Result with std error based alias
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::Position,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};
use serde_json::Value;
use tokio::{select, sync::mpsc, time::sleep};

// Catppuccin Mocha theme
struct Theme {
    base: Color,
    mantle: Color,
    crust: Color,
    surface0: Color,
    surface1: Color,
    surface2: Color,
    overlay0: Color,
    overlay1: Color,
    overlay2: Color,
    text: Color,
    subtext0: Color,
    subtext1: Color,
    blue: Color,
    sapphire: Color,
    mauve: Color,
    green: Color,
    yellow: Color,
    red: Color,
    lavender: Color,
}

fn hex(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}
fn theme() -> Theme {
    Theme {
        base: hex((0x1e, 0x1e, 0x2e)),
        mantle: hex((0x18, 0x18, 0x25)),
        crust: hex((0x11, 0x11, 0x1b)),
        surface0: hex((0x31, 0x32, 0x44)),
        surface1: hex((0x45, 0x47, 0x5a)),
        surface2: hex((0x58, 0x5b, 0x70)),
        overlay0: hex((0x6c, 0x70, 0x86)),
        overlay1: hex((0x7f, 0x84, 0x9c)),
        overlay2: hex((0x93, 0x99, 0xb2)),
        text: hex((0xcd, 0xd6, 0xf4)),
        subtext0: hex((0xa6, 0xad, 0xc8)),
        subtext1: hex((0xba, 0xc2, 0xde)),
        blue: hex((0x89, 0xb4, 0xfa)),
        sapphire: hex((0x74, 0xc7, 0xec)),
        mauve: hex((0xcb, 0xa6, 0xf7)),
        green: hex((0xa6, 0xe3, 0xa1)),
        yellow: hex((0xf9, 0xe2, 0xaf)),
        red: hex((0xf3, 0x8b, 0xa8)),
        lavender: hex((0xb4, 0xbe, 0xfe)),
    }
}

#[derive(Clone, Debug)]
enum Source {
    Official { repo: String, arch: String },
    Aur,
}

#[derive(Clone, Debug)]
struct PackageItem {
    name: String,
    version: String,
    description: String,
    source: Source,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct PackageDetails {
    repository: String,
    name: String,
    version: String,
    description: String,
    architecture: String,
    url: String,
    licenses: Vec<String>,
    groups: Vec<String>,
    provides: Vec<String>,
    depends: Vec<String>,
    opt_depends: Vec<String>,
    required_by: Vec<String>,
    optional_for: Vec<String>,
    conflicts: Vec<String>,
    replaces: Vec<String>,
    download_size: Option<u64>,
    install_size: Option<u64>,
    owner: String, // packager/maintainer
    build_date: String,
}

impl PackageDetails {
    fn format_lines(&self, _area_width: u16) -> Vec<Line<'static>> {
        fn kv(key: &str, val: String) -> Line<'static> {
            Line::from(vec![
                Span::styled(
                    format!("{key}: "),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(val),
            ])
        }
        let lines = vec![
            kv("Repository", self.repository.clone()),
            kv("Package Name", self.name.clone()),
            kv("Version", self.version.clone()),
            kv("Description", self.description.clone()),
            kv("Architecture", self.architecture.clone()),
            kv("URL", self.url.clone()),
            kv("Licences", join(&self.licenses)),
            kv("Provides", join(&self.provides)),
            kv("Depends on", join(&self.depends)),
            kv("Optional dependencies", join(&self.opt_depends)),
            kv("Required by", join(&self.required_by)),
            kv("Optional for", join(&self.optional_for)),
            kv("Conflicts with", join(&self.conflicts)),
            kv("Replaces", join(&self.replaces)),
            kv(
                "Download size",
                self.download_size
                    .map(human_bytes)
                    .unwrap_or_else(|| "N/A".to_string()),
            ),
            kv(
                "Install size",
                self.install_size
                    .map(human_bytes)
                    .unwrap_or_else(|| "N/A".to_string()),
            ),
            kv("Package Owner", self.owner.clone()),
            kv("Build date", self.build_date.clone()),
        ];
        lines
    }

    fn format_lines_with_theme(&self, _area_width: u16, th: &Theme) -> Vec<Line<'static>> {
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
        vec![
            kv("Repository", self.repository.clone(), th),
            kv("Package Name", self.name.clone(), th),
            kv("Version", self.version.clone(), th),
            kv("Description", self.description.clone(), th),
            kv("Architecture", self.architecture.clone(), th),
            kv("URL", self.url.clone(), th),
            kv("Licences", join(&self.licenses), th),
            kv("Provides", join(&self.provides), th),
            kv("Depends on", join(&self.depends), th),
            kv("Optional dependencies", join(&self.opt_depends), th),
            kv("Required by", join(&self.required_by), th),
            kv("Optional for", join(&self.optional_for), th),
            kv("Conflicts with", join(&self.conflicts), th),
            kv("Replaces", join(&self.replaces), th),
            kv(
                "Download size",
                self.download_size
                    .map(human_bytes)
                    .unwrap_or_else(|| "N/A".to_string()),
                th,
            ),
            kv(
                "Install size",
                self.install_size
                    .map(human_bytes)
                    .unwrap_or_else(|| "N/A".to_string()),
                th,
            ),
            kv("Package Owner", self.owner.clone(), th),
            kv("Build date", self.build_date.clone(), th),
        ]
    }
}

// Tagging for searches
#[derive(Clone, Debug)]
struct QueryInput {
    id: u64,
    text: String,
}
#[derive(Clone, Debug)]
struct SearchResults {
    id: u64,
    items: Vec<PackageItem>,
}

#[derive(Debug)]
struct AppState {
    input: String,
    results: Vec<PackageItem>,
    selected: usize,
    details: PackageDetails,
    list_state: ListState,
    modal: Modal,
    dry_run: bool,
    // Recent searches
    recent: Vec<String>,
    history_state: ListState,
    history_focus: bool,
    last_input_change: Instant,
    last_saved_value: Option<String>,
    // Persisted recent searches
    recent_path: PathBuf,
    recent_dirty: bool,

    // Search coordination
    latest_query_id: u64,
    next_query_id: u64,
    // Details cache
    details_cache: HashMap<String, PackageDetails>,
    cache_path: PathBuf,
    cache_dirty: bool,

    // Install list pane
    install_list: Vec<PackageItem>,
    install_state: ListState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            input: String::new(),
            results: Vec::new(),
            selected: 0,
            details: PackageDetails::default(),
            list_state: ListState::default(),
            modal: Modal::None,
            dry_run: false,
            recent: Vec::new(),
            history_state: ListState::default(),
            history_focus: false,
            last_input_change: Instant::now(),
            last_saved_value: None,
            // Persisted recent searches
            recent_path: PathBuf::from("recent_searches.json"),
            recent_dirty: false,

            latest_query_id: 0,
            next_query_id: 1,
            details_cache: HashMap::new(),
            cache_path: PathBuf::from("details_cache.json"),
            cache_dirty: false,

            install_list: Vec::new(),
            install_state: ListState::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
enum Modal {
    #[default]
    None,
    Alert {
        message: String,
    },
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
            Some(_) = tick_rx.recv() => { maybe_save_recent(&mut app); maybe_flush_cache(&mut app); maybe_flush_recent(&mut app); }
            else => {}
        }
    }

    // Final flush before exit
    maybe_flush_cache(&mut app);
    maybe_flush_recent(&mut app);

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

fn ui(f: &mut ratatui::Frame, app: &mut AppState) {
    let th = theme();
    let area = f.area();

    // Background
    let bg = Block::default().style(Style::default().bg(th.base));
    f.render_widget(bg, area);

    let total_h = area.height;
    let search_h: u16 = 5; // give a bit more room for history pane
    let bottom_h: u16 = total_h.saturating_mul(2) / 3; // 2/3 of full height
    let top_h: u16 = total_h.saturating_sub(search_h).saturating_sub(bottom_h);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_h),
            Constraint::Length(search_h),
            Constraint::Length(bottom_h),
        ])
        .split(area);

    // Results list (top)
    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|p| {
            let (src, color) = match &p.source {
                Source::Official { repo, .. } => (repo.to_string(), th.green),
                Source::Aur => ("AUR".to_string(), th.yellow),
            };
            let line = Line::from(vec![
                Span::styled(format!("{src} "), Style::default().fg(color)),
                Span::styled(
                    p.name.clone(),
                    Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  {}", p.version), Style::default().fg(th.overlay1)),
                Span::raw("  - "),
                Span::styled(p.description.clone(), Style::default().fg(th.overlay2)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .style(Style::default().fg(th.text).bg(th.base))
        .block(
            Block::default()
                .title(Span::styled(
                    format!("Results ({})", app.results.len()),
                    Style::default().fg(th.overlay1),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        )
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // Middle row split: left input, right recent searches
    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(60),
            Constraint::Percentage(20),
            Constraint::Percentage(20),
        ])
        .split(chunks[1]);

    // Search input (left)
    let search_focused = !app.history_focus;
    let input_line = Line::from(vec![
        Span::styled(
            "> ",
            Style::default().fg(if search_focused { th.sapphire } else { th.overlay1 }),
        ),
        Span::styled(
            app.input.as_str().to_string(),
            Style::default().fg(if search_focused { th.text } else { th.subtext0 }),
        ),
    ]);
    let search_title = if app.history_focus { "Search" } else { "Search (focused)" };
    let search_title_color = if search_focused { th.mauve } else { th.overlay1 };
    let input = Paragraph::new(input_line)
        .style(Style::default().fg(if search_focused { th.text } else { th.subtext0 }).bg(th.base))
        .block(
            Block::default()
                .title(Span::styled(search_title, Style::default().fg(search_title_color)))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if search_focused { th.mauve } else { th.surface1 })),
        );
    f.render_widget(input, middle[0]);

    // Cursor in input
    let right = middle[0].x + middle[0].width.saturating_sub(1);
    let x = std::cmp::min(middle[0].x + 1 + 2 + app.input.len() as u16, right);
    let y = middle[0].y + 1;
    f.set_cursor_position(Position::new(x, y));

    // Recent searches (middle-right)
    let recent_focused = app.history_focus;
    let rec_items: Vec<ListItem> = app
        .recent
        .iter()
        .map(|s| ListItem::new(Span::styled(
            s.clone(),
            Style::default().fg(if recent_focused { th.text } else { th.subtext0 }),
        )))
        .collect();
    let recent_title = if app.history_focus { "Recent (focused)" } else { "Recent" };
    let recent_title_color = if recent_focused { th.mauve } else { th.overlay1 };
    let rec_block = Block::default()
        .title(Span::styled(recent_title, Style::default().fg(recent_title_color)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if recent_focused { th.mauve } else { th.surface1 }));
    let rec_list = List::new(rec_items)
        .style(Style::default().fg(if recent_focused { th.text } else { th.subtext0 }).bg(th.base))
        .block(rec_block)
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(rec_list, middle[1], &mut app.history_state);

    // Install List (far right)
    let install_items: Vec<ListItem> = app
        .install_list
        .iter()
        .map(|p| {
            let line = Line::from(vec![
                Span::styled(p.name.clone(), Style::default().fg(th.text).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", p.version), Style::default().fg(th.overlay1)),
            ]);
            ListItem::new(line)
        })
        .collect();
    let install_block = Block::default()
        .title(Span::styled("Install List", Style::default().fg(th.overlay1)))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.surface1));
    let install_list = List::new(install_items)
        .style(Style::default().fg(th.subtext0).bg(th.base))
        .block(install_block)
        .highlight_style(Style::default().fg(th.crust).bg(th.lavender))
        .highlight_symbol("▶ ");
    f.render_stateful_widget(install_list, middle[2], &mut app.install_state);

    // Details (bottom)
    let details_lines = app.details.format_lines_with_theme(chunks[2].width, &th);
    let details = Paragraph::new(details_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    "Package Info",
                    Style::default().fg(th.overlay1),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        );
    f.render_widget(details, chunks[2]);

    // Modal overlay for alerts
    if let Modal::Alert { message } = &app.modal {
        let w = area.width.saturating_sub(10).min(80);
        let h = 7;
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = ratatui::prelude::Rect {
            x,
            y,
            width: w,
            height: h,
        };
        f.render_widget(Clear, rect);
        let lines = vec![
            Line::from(Span::styled(
                "Connection issue",
                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(message.clone(), Style::default().fg(th.text))),
            Line::from(""),
            Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )),
        ];
        let boxw = Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.mantle))
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(Span::styled(
                        " Network Error ",
                        Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                    ))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(th.red))
                    .style(Style::default().bg(th.mantle)),
            );
        f.render_widget(boxw, rect);
    }
}

fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<QueryInput>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if let CEvent::Key(ke) = ev {
        if ke.kind != KeyEventKind::Press {
            return false;
        }

        // Alert modal
        if let Modal::Alert { .. } = app.modal {
            match ke.code {
                KeyCode::Enter | KeyCode::Esc => {
                    app.modal = Modal::None;
                }
                _ => {}
            }
            return false;
        }

        // History focus
        if app.history_focus {
            // Allow exiting with Ctrl+C while in Recent pane
            if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
                return true;
            }
            match ke.code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                    app.history_focus = false;
                    // restore details for the currently selected result
                    refresh_selected_details(app, details_tx);
                }
                KeyCode::Left => {
                    // Move focus back to Search input from Recent pane
                    app.history_focus = false;
                    refresh_selected_details(app, details_tx);
                }
                KeyCode::Up => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let new = sel.saturating_sub(1);
                    app.history_state.select(if app.recent.is_empty() {
                        None
                    } else {
                        Some(new)
                    });
                    trigger_recent_preview(app, preview_tx);
                }
                KeyCode::Down => {
                    if !app.recent.is_empty() {
                        let sel = app.history_state.selected().unwrap_or(0);
                        let max = app.recent.len().saturating_sub(1);
                        let new = std::cmp::min(sel + 1, max);
                        app.history_state.select(Some(new));
                        trigger_recent_preview(app, preview_tx);
                    }
                }
                KeyCode::Char(' ') => {
                    // Add from Recent: resolve highlighted query to a package and add
                    if let Some(idx) = app.history_state.selected() {
                        if let Some(q) = app.recent.get(idx).cloned() {
                            let tx = add_tx.clone();
                            tokio::spawn(async move {
                                let (items, _errors) = fetch_all_with_errors(q.clone()).await;
                                if items.is_empty() { return; }
                                if let Some(item) = items.iter().find(|it| it.name.eq_ignore_ascii_case(&q)).cloned() {
                                    let _ = tx.send(item);
                                } else {
                                    let _ = tx.send(items[0].clone());
                                }
                            });
                        }
                    }
                }
                KeyCode::Enter => {
                    if let Some(idx) = app.history_state.selected()
                        && let Some(q) = app.recent.get(idx).cloned() {
                            app.input = q;
                            app.history_focus = false;
                            app.last_input_change = Instant::now();
                            app.last_saved_value = None;
                            send_query(app, query_tx);
                        }
                }
                _ => {}
            }
            return false;
        }

        // Normal mode
        let KeyEvent {
            code, modifiers, ..
        } = ke;
        match (code, modifiers) {
            (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
            (KeyCode::Tab, _) => {
                app.history_focus = true;
                if app.history_state.selected().is_none() {
                    app.history_state.select(Some(0));
                }
                trigger_recent_preview(app, preview_tx);
            }
            (KeyCode::Right, _) => {
                // Move focus to Recent pane with Right arrow
                if !app.recent.is_empty() {
                    app.history_focus = true;
                    if app.history_state.selected().is_none() {
                        app.history_state.select(Some(0));
                    }
                    trigger_recent_preview(app, preview_tx);
                }
            }
            (KeyCode::Char(' '), _) => {
                // Add currently selected result to install list
                if let Some(item) = app.results.get(app.selected).cloned() {
                    let _ = add_tx.send(item);
                }
            }
            (KeyCode::Backspace, _) => {
                app.input.pop();
                app.last_input_change = Instant::now();
                app.last_saved_value = None;
                send_query(app, query_tx);
            }
            (KeyCode::Char('\n') | KeyCode::Enter, _) => {
                if let Some(item) = app.results.get(app.selected).cloned() {
                    // Always install in a new terminal and let sudo prompt there
                    spawn_install(&item, None, app.dry_run);
                }
            }
            (KeyCode::Char(ch), _) => {
                app.input.push(ch);
                app.last_input_change = Instant::now();
                app.last_saved_value = None;
                send_query(app, query_tx);
            }
            (KeyCode::Up, _) => move_sel_cached(app, -1, details_tx),
            (KeyCode::Down, _) => move_sel_cached(app, 1, details_tx),
            (KeyCode::PageUp, _) => move_sel_cached(app, -10, details_tx),
            (KeyCode::PageDown, _) => move_sel_cached(app, 10, details_tx),
            _ => {}
        }
    }
    false
}

fn refresh_selected_details(app: &mut AppState, details_tx: &mpsc::UnboundedSender<PackageItem>) {
    if let Some(item) = app.results.get(app.selected).cloned() {
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

fn trigger_recent_preview(app: &AppState, preview_tx: &mpsc::UnboundedSender<PackageItem>) {
    if !app.history_focus {
        return;
    }
    let idx = match app.history_state.selected() {
        Some(i) => i,
        None => return,
    };
    let Some(q) = app.recent.get(idx).cloned() else {
        return;
    };
    let tx = preview_tx.clone();
    tokio::spawn(async move {
        let (items, _errors) = fetch_all_with_errors(q.clone()).await;
        if items.is_empty() {
            return;
        }
        // prefer exact match by name (case-insensitive), else first item
        if let Some(item) = items
            .iter()
            .find(|it| it.name.eq_ignore_ascii_case(&q))
            .cloned()
        {
            let _ = tx.send(item);
        } else {
            let _ = tx.send(items[0].clone());
        }
    });
}

fn send_query(app: &mut AppState, query_tx: &mpsc::UnboundedSender<QueryInput>) {
    let id = app.next_query_id;
    app.next_query_id += 1;
    app.latest_query_id = id;
    let _ = query_tx.send(QueryInput {
        id,
        text: app.input.clone(),
    });
}

fn move_sel_cached(
    app: &mut AppState,
    delta: isize,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) {
    if app.results.is_empty() {
        return;
    }
    let len = app.results.len() as isize;
    let mut idx = app.selected as isize + delta;
    if idx < 0 {
        idx = 0;
    }
    if idx >= len {
        idx = len - 1;
    }
    app.selected = idx as usize;
    app.list_state.select(Some(app.selected));
    if let Some(item) = app.results.get(app.selected).cloned() {
        if let Some(cached) = app.details_cache.get(&item.name).cloned() {
            app.details = cached;
        } else {
            let _ = details_tx.send(item);
        }
    }
}

fn add_to_install_list(app: &mut AppState, item: PackageItem) {
    // Avoid duplicates by name (case-insensitive)
    if app.install_list.iter().any(|p| p.name.eq_ignore_ascii_case(&item.name)) {
        return;
    }
    app.install_list.push(item);
    // Ensure a selection exists in the Install List
    if app.install_state.selected().is_none() && !app.install_list.is_empty() {
        app.install_state.select(Some(0));
    }
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

fn repo_order(src: &Source) -> u8 {
    match src {
        Source::Official { repo, .. } => {
            if repo.eq_ignore_ascii_case("core") {
                0
            } else if repo.eq_ignore_ascii_case("extra") {
                1
            } else {
                2 // other official repos
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
    let obj = v.get("pkg").unwrap_or(&v); // API sometimes nests under 'pkg'

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
        optional_for: vec![], // Not available from API
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
        required_by: vec![], // Not available via RPC
        optional_for: vec![],
        conflicts: arrs(&obj, &["Conflicts"]),
        replaces: arrs(&obj, &["Replaces"]),
        download_size: None, // Not available
        install_size: None,  // Not available
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

// Helper: check whether a command exists on PATH (Unix-aware exec bit)
fn command_on_path(cmd: &str) -> bool {
    use std::path::Path;

    fn is_exec(p: &std::path::Path) -> bool {
        if !p.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(p) {
                return meta.permissions().mode() & 0o111 != 0;
            }
            false
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    // If path contains a separator, check directly
    if cmd.contains(std::path::MAIN_SEPARATOR) {
        return is_exec(Path::new(cmd));
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(cmd);
            if is_exec(&candidate) {
                return true;
            }
            #[cfg(windows)]
            {
                // On Windows, try PATHEXT variants
                if let Some(pathext) = std::env::var_os("PATHEXT") {
                    for ext in pathext.to_string_lossy().split(';') {
                        let candidate = dir.join(format!("{}{}", cmd, ext));
                        if candidate.is_file() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    // Try common terminals
    let terms: &[(&str, &[&str], bool)] = &[
        ("alacritty", &["-e", "bash", "-lc"], false),
        ("kitty", &["bash", "-lc"], false),
        ("xterm", &["-hold", "-e", "bash", "-lc"], false),
        ("gnome-terminal", &["--", "bash", "-lc"], false),
        ("konsole", &["-e", "bash", "-lc"], false),
        ("xfce4-terminal", &["-e", "bash", "-lc"], false),
        ("tilix", &["-e", "bash", "-lc"], false),
        ("mate-terminal", &["-e", "bash", "-lc"], false),
    ];
    let mut launched = false;
    for (term, args, _hold) in terms {
        if command_on_path(term) {
            let _ = Command::new(term)
                .args(args.iter().copied())
                .arg(&cmd_str)
                .spawn();
            launched = true;
            break;
        }
    }
    if !launched {
        let _ = Command::new("bash").args(["-lc", &cmd_str]).spawn();
    }
}

#[cfg(target_os = "windows")]
fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    // Open new window and keep it open
    let _ = Command::new("cmd")
        .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
        .spawn();
}

fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed {}", item.name);
            // Robust hold tail that works even if read fails (e.g., no TTY)
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let bash = format!("echo DRY RUN: sudo {base_cmd}{hold}", hold = hold_tail);
                return (bash, true);
            }
            // Escape password for bash
            let pass = password.unwrap_or("");
            let escaped = pass.replace('\'', "'\"'\"'\''");
            let pipe = if pass.is_empty() {
                String::new()
            } else {
                format!("echo '{escaped}' | ")
            };
            let bash = format!("{pipe}sudo -S {base_cmd}{hold}", hold = hold_tail);
            (bash, true)
        }
        Source::Aur => {
            // Robust hold tail that works even if read fails (e.g., no TTY)
            let hold_tail = "; echo; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            let aur_cmd = if dry_run {
                format!(
                    "echo DRY RUN: paru -S --needed {n} || yay -S --needed {n}{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            } else {
                format!(
                    "(command -v paru >/dev/null 2>&1 && paru -S --needed {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed {n}) || echo 'No AUR helper (paru/yay) found.'{hold}",
                    n = item.name,
                    hold = hold_tail
                )
            };
            (aur_cmd, false)
        }
    }
}
