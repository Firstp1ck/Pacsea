use std::{process::Command, time::Duration};

use anyhow::Result;
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

#[derive(Clone, Debug, Default)]
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
    fn format_lines(&self, area_width: u16) -> Vec<Line<'static>> {
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
            // Removed Groups
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
        if area_width > 0 { /* keep clippy calm */ }
        lines
    }
}

#[derive(Debug, Default)]
struct AppState {
    input: String,
    results: Vec<PackageItem>,
    selected: usize,
    details: PackageDetails,
    list_state: ListState,
    status: String,
    modal: Modal,
    dry_run: bool,
}

#[derive(Debug, Clone)]
enum Modal {
    None,
    Password { buffer: String, pkg: PackageItem },
}
impl Default for Modal {
    fn default() -> Self {
        Modal::None
    }
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

    let mut app = AppState::default();
    app.dry_run = dry_run;
    app.status = if dry_run {
        "DRY RUN: installs will open a terminal and only echo commands".into()
    } else {
        "Type to search (Esc/Ctrl-C to quit)".into()
    };

    // Channels
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<CEvent>();
    let (search_result_tx, mut results_rx) = mpsc::unbounded_channel::<Vec<PackageItem>>();
    let (details_req_tx, mut details_req_rx) = mpsc::unbounded_channel::<PackageItem>();
    let (details_res_tx, mut details_res_rx) = mpsc::unbounded_channel::<PackageDetails>();

    // Spawn blocking reader of crossterm events
    std::thread::spawn(move || {
        loop {
            if let Ok(true) = event::poll(Duration::from_millis(50)) {
                if let Ok(ev) = event::read() {
                    let _ = event_tx.send(ev);
                }
            }
        }
    });

    // Search worker with debounce
    let (query_tx, mut query_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        loop {
            let mut latest = match query_rx.recv().await {
                Some(q) => q,
                None => break,
            };
            loop {
                select! {
                    Some(new_q) = query_rx.recv() => { latest = new_q; }
                    _ = sleep(Duration::from_millis(250)) => { break; }
                }
            }
            if latest.trim().is_empty() {
                let _ = search_result_tx.send(Vec::new());
                continue;
            }
            let tx = search_result_tx.clone();
            tokio::spawn(async move {
                let items = fetch_all(latest).await.unwrap_or_default();
                let _ = tx.send(items);
            });
        }
    });

    // Details worker
    tokio::spawn(async move {
        while let Some(item) = details_req_rx.recv().await {
            let details = fetch_details(item.clone()).await.unwrap_or_default();
            let _ = details_res_tx.send(details);
        }
    });

    loop {
        terminal.draw(|f| ui(f, &mut app)).ok();

        select! {
            // UI events
            Some(ev) = event_rx.recv() => { if handle_event(ev, &mut app, &query_tx, &details_req_tx) { break; } }
            // Search results
            Some(new_results) = results_rx.recv() => {
                app.results = new_results; app.selected = 0; app.list_state.select(if app.results.is_empty(){None}else{Some(0)});
                if let Some(item) = app.results.get(0).cloned() { let _ = details_req_tx.send(item); }
            }
            // Details ready
            Some(details) = details_res_rx.recv() => { app.details = details; }
            else => {}
        }
    }

    Ok(())
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
    let area = f.area();
    let total_h = area.height;
    let search_h: u16 = 3;
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
                Source::Official { repo, .. } => (format!("{repo}"), Color::LightGreen),
                Source::Aur => ("AUR".to_string(), Color::Yellow),
            };
            let line = Line::from(vec![
                Span::styled(format!("{src} "), Style::default().fg(color)),
                Span::styled(
                    p.name.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!("  {}", p.version)),
                Span::raw("  - "),
                Span::styled(p.description.clone(), Style::default().fg(Color::Gray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(format!("Results ({})", app.results.len()))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Green)),
        )
        .highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("> ");

    f.render_stateful_widget(list, chunks[0], &mut app.list_state);

    // Search bar (middle)
    let input_line = Line::from(vec![
        Span::styled("> ", Style::default().fg(Color::Green)),
        Span::raw(app.input.as_str()),
    ]);
    let input = Paragraph::new(input_line).block(
        Block::default()
            .title("Search")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green)),
    );
    f.render_widget(input, chunks[1]);

    // place cursor inside the search bar after the prompt
    let right = chunks[1].x + chunks[1].width.saturating_sub(1);
    let x = std::cmp::min(chunks[1].x + 1 + 2 + app.input.len() as u16, right); // 1 padding + 2 for "> "
    let y = chunks[1].y + 1;
    f.set_cursor_position(Position::new(x, y));

    // Details (bottom)
    let details_lines = app.details.format_lines(chunks[2].width);
    let details = Paragraph::new(details_lines)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title("Package Info")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded),
        );
    f.render_widget(details, chunks[2]);

    // Modal overlay for install password / confirmation
    if let Modal::Password { buffer, pkg } = &app.modal {
        // center rect
        let w: u16 = 64;
        let h: u16 = 9;
        let x = area.x + (area.width.saturating_sub(w)) / 2;
        let y = area.y + (area.height.saturating_sub(h)) / 2;
        let rect = ratatui::prelude::Rect {
            x,
            y,
            width: w,
            height: h,
        };

        // clear popup area to make it stand out
        f.render_widget(Clear, rect);

        // Header and content lines
        let mask_char = '•';
        let masked: String = std::iter::repeat(mask_char).take(buffer.len()).collect();
        let lines = vec![
            Line::from(vec![Span::styled(
                "Authentication required",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(Span::styled(
                format!("Package: {}", pkg.name),
                Style::default().fg(Color::Cyan),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("Password (optional): ", Style::default().fg(Color::White)),
                Span::styled(masked, Style::default().fg(Color::Gray)),
            ]),
            Line::from(Span::styled(
                "Leave blank to be prompted in the new terminal.",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Enter = Proceed   Esc = Cancel   Backspace = Delete",
                Style::default().fg(Color::Gray),
            )),
        ];

        let popup = Paragraph::new(lines).block(
            Block::default()
                .title(Span::styled(
                    " Password ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Magenta)),
        );
        f.render_widget(popup, rect);

        // place cursor after the label inside the popup
        let cursor_x = rect.x + "Password (optional): ".len() as u16 + buffer.len() as u16;
        let cursor_y = rect.y + 4; // line index where password is
        f.set_cursor_position(Position::new(cursor_x, cursor_y));
    }
}

fn handle_event(
    ev: CEvent,
    app: &mut AppState,
    query_tx: &mpsc::UnboundedSender<String>,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    match ev {
        CEvent::Key(ke) => {
            if ke.kind != KeyEventKind::Press {
                return false;
            }
            // If modal open, capture password input
            if let Modal::Password { buffer, pkg } = &mut app.modal {
                match ke.code {
                    KeyCode::Esc => {
                        app.modal = Modal::None;
                    }
                    KeyCode::Enter => {
                        let pwd = buffer.clone();
                        let pkg = pkg.clone();
                        app.modal = Modal::None;
                        spawn_install(
                            &pkg,
                            if pwd.is_empty() { None } else { Some(&pwd) },
                            app.dry_run,
                        );
                    }
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(ch) => {
                        buffer.push(ch);
                    }
                    _ => {}
                }
                return false;
            }

            let KeyEvent {
                code, modifiers, ..
            } = ke;
            match (code, modifiers) {
                (KeyCode::Esc, _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => return true,
                (KeyCode::Backspace, _) => {
                    app.input.pop();
                    let _ = query_tx.send(app.input.clone());
                }
                (KeyCode::Char('\n') | KeyCode::Enter, _) => {
                    if let Some(item) = app.results.get(app.selected).cloned() {
                        match &item.source {
                            Source::Official { .. } => {
                                // Always open password modal (also in dry-run) so it can be tested
                                app.modal = Modal::Password {
                                    buffer: String::new(),
                                    pkg: item,
                                };
                            }
                            Source::Aur => {
                                // AUR install does not need sudo by default; still respect dry-run
                                spawn_install(&item, None, app.dry_run);
                            }
                        }
                    }
                }
                (KeyCode::Char(ch), _) => {
                    app.input.push(ch);
                    let _ = query_tx.send(app.input.clone());
                }
                (KeyCode::Up, _) => move_sel(app, -1, details_tx),
                (KeyCode::Down, _) => move_sel(app, 1, details_tx),
                (KeyCode::PageUp, _) => move_sel(app, -10, details_tx),
                (KeyCode::PageDown, _) => move_sel(app, 10, details_tx),
                _ => {}
            }
        }
        _ => {}
    }
    false
}

fn move_sel(app: &mut AppState, delta: isize, details_tx: &mpsc::UnboundedSender<PackageItem>) {
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
        let _ = details_tx.send(item);
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

async fn fetch_all(query: String) -> Result<Vec<PackageItem>> {
    let client = reqwest::Client::builder()
        .user_agent("Pacsea/0.1 (+https://archlinux.org)")
        .build()?;

    let q = urlencoding::encode(query.trim());

    let official_url = format!("https://archlinux.org/packages/search/json/?q={}", q);
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search?by=name&arg={}", q);

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
        Ok::<_, anyhow::Error>(items)
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
        Ok::<_, anyhow::Error>(items)
    };

    let (official, aur) = tokio::join!(official_fut, aur_fut);

    let mut combined = Vec::new();
    combined.extend(official.unwrap_or_default());
    combined.extend(aur.unwrap_or_default());

    // sort by repo group (Core, Extra, other official, AUR) and match quality, then name
    let ql = query.trim().to_lowercase();
    combined.sort_by(|a, b| {
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

    Ok(combined)
}

fn repo_order(src: &Source) -> u8 {
    match src {
        Source::Official { repo, .. } => match repo.to_lowercase().as_str() {
            "core" => 0,
            "extra" => 1,
            _ => 2, // other official repos
        },
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

    let mut d = PackageDetails::default();
    d.repository = repo;
    d.name = item.name.clone();
    d.version = ss(obj, &["pkgver", "Version"]).unwrap_or(item.version);
    d.description = ss(obj, &["pkgdesc", "Description"]).unwrap_or(item.description);
    d.architecture = ss(obj, &["arch", "Architecture"]).unwrap_or(arch);
    d.url = ss(obj, &["url", "URL"]).unwrap_or_default();
    d.licenses = arrs(obj, &["licenses", "Licenses"]);
    d.groups = arrs(obj, &["groups", "Groups"]);
    d.provides = arrs(obj, &["provides", "Provides"]);
    d.depends = arrs(obj, &["depends", "Depends"]);
    d.opt_depends = arrs(obj, &["optdepends", "OptDepends"]);
    d.required_by = arrs(obj, &["requiredby", "RequiredBy"]);
    d.optional_for = vec![]; // Not available from API
    d.conflicts = arrs(obj, &["conflicts", "Conflicts"]);
    d.replaces = arrs(obj, &["replaces", "Replaces"]);
    d.download_size = u64_of(obj, &["compressed_size", "CompressedSize"]);
    d.install_size = u64_of(obj, &["installed_size", "InstalledSize"]);
    d.owner = ss(obj, &["packager", "Packager"]).unwrap_or_default();
    d.build_date = ss(obj, &["build_date", "BuildDate"]).unwrap_or_default();
    Ok(d)
}

async fn fetch_aur_details(item: PackageItem) -> Result<PackageDetails> {
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/info?arg[]={}",
        urlencoding::encode(&item.name)
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
    let obj = arr.get(0).cloned().unwrap_or(Value::Null);

    let mut d = PackageDetails::default();
    d.repository = "AUR".into();
    d.name = item.name.clone();
    d.version = s(&obj, "Version");
    if d.version.is_empty() {
        d.version = item.version.clone();
    }
    d.description = s(&obj, "Description");
    if d.description.is_empty() {
        d.description = item.description.clone();
    }
    d.architecture = "any".into();
    d.url = s(&obj, "URL");
    d.licenses = arrs(&obj, &["License", "Licenses"]);
    d.groups = arrs(&obj, &["Groups"]);
    d.provides = arrs(&obj, &["Provides"]);
    d.depends = arrs(&obj, &["Depends"]);
    d.opt_depends = arrs(&obj, &["OptDepends"]);
    d.required_by = vec![]; // Not available via RPC
    d.optional_for = vec![];
    d.conflicts = arrs(&obj, &["Conflicts"]);
    d.replaces = arrs(&obj, &["Replaces"]);
    d.download_size = None; // Not available
    d.install_size = None; // Not available
    d.owner = s(&obj, "Maintainer");
    d.build_date = ts_to_date(obj.get("LastModified").and_then(|v| v.as_i64()));
    Ok(d)
}

fn ts_to_date(ts: Option<i64>) -> String {
    if let Some(t) = ts {
        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(t, 0) {
            return dt.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        }
    }
    "".into()
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}
fn ss<'a>(v: &'a Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}
fn arrs(v: &Value, keys: &[&str]) -> Vec<String> {
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr
                .iter()
                .filter_map(|e| e.as_str().map(|s| s.to_string()))
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
            if let Some(i) = n.as_i64() {
                return Some(i as u64);
            }
            if let Some(s) = n.as_str() {
                if let Ok(p) = s.parse::<u64>() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn spawn_install(item: &PackageItem, password: Option<&str>, dry_run: bool) {
    let (cmd_str, _uses_sudo) = build_install_command(item, password, dry_run);
    #[cfg(target_os = "windows")]
    {
        // Open new window and keep it open
        let _ = Command::new("cmd")
            .args(["/C", "start", "Pacsea Install", "cmd", "/K", &cmd_str])
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
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
            if which::which(term).is_ok() {
                let _ = Command::new(term)
                    .args(args.iter().copied())
                    .arg(cmd_str.clone())
                    .spawn();
                launched = true;
                break;
            }
        }
        if !launched {
            let _ = Command::new("bash").args(["-lc", &cmd_str]).spawn();
        }
    }
}

fn build_install_command(
    item: &PackageItem,
    password: Option<&str>,
    dry_run: bool,
) -> (String, bool) {
    match &item.source {
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed {}", item.name);
            if dry_run {
                return (
                    format!("echo DRY RUN: sudo {} && echo Done & pause", base_cmd),
                    true,
                );
            }
            // Escape password for bash
            let pass = password.unwrap_or("");
            let escaped = pass.replace('\'', "'\"'\"'\'");
            let pipe = if pass.is_empty() {
                String::new()
            } else {
                format!("echo '{}' | ", escaped)
            };
            let bash = format!(
                "{}sudo -S {}; echo; echo 'Finished.'; read -n1 -s -r -p 'Press any key to close...'",
                pipe, base_cmd
            );
            (bash, true)
        }
        Source::Aur => {
            let aur_cmd = if dry_run {
                format!(
                    "echo DRY RUN: paru -S --needed {} || yay -S --needed {}; echo Done; read -n1 -s -r -p 'Press any key...'",
                    item.name, item.name
                )
            } else {
                format!(
                    "(command -v paru >/dev/null 2>&1 && paru -S --needed {n}) || (command -v yay >/dev/null 2>&1 && yay -S --needed {n}) || echo 'No AUR helper (paru/yay) found.'; echo; read -n1 -s -r -p 'Press any key...'",
                    n = item.name
                )
            };
            (aur_cmd, false)
        }
    }
}

fn is_cmd_available(bin: &str) -> bool {
    Command::new(bin).arg("--version").output().is_ok()
}

fn shell_escape(s: &str) -> String {
    // Minimal escape for single quotes in sh; works for both bash and sh
    if s.contains('\'') {
        s.replace('\'', "'\\''")
    } else {
        s.to_string()
    }
}
