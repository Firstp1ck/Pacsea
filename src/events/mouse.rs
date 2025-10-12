use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::execute;
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

use super::utils::{refresh_install_details, refresh_selected_details};
use crate::logic::move_sel_cached;

/// Handle a single mouse event and update the [`AppState`].
///
/// Returns `true` to request application exit (never used here), `false` otherwise.
///
/// Behavior summary:
/// - Clickable URL in the details pane with Ctrl+Shift+LeftClick (opens via `xdg-open`).
/// - Clickable "Show/Hide PKGBUILD" action in the details content.
/// - Clickable "Check Package Build" button in the PKGBUILD title (copies to clipboard).
/// - Clickable Sort button and filter toggles in the Results title.
/// - Click-to-select in Results; mouse wheel scroll moves selection in Results/Recent/Install.
/// - Mouse wheel scroll within the PKGBUILD viewer scrolls the content.
pub fn handle_mouse_event(
    m: MouseEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    _add_tx: &mpsc::UnboundedSender<PackageItem>,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    let mx = m.column;
    let my = m.row;
    let is_left_down = matches!(m.kind, MouseEventKind::Down(MouseButton::Left));
    let ctrl = m.modifiers.contains(KeyModifiers::CONTROL);
    let shift = m.modifiers.contains(KeyModifiers::SHIFT);

    // Track last mouse position for dynamic capture toggling
    app.last_mouse_pos = Some((mx, my));

    // A) Modal-first handling: Help and News intercept mouse events
    if let crate::state::Modal::Help = &app.modal {
        // Scroll within Help content area
        if let Some((x, y, w, h)) = app.help_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    app.help_scroll = app.help_scroll.saturating_sub(1);
                    return false;
                }
                MouseEventKind::ScrollDown => {
                    app.help_scroll = app.help_scroll.saturating_add(1);
                    return false;
                }
                _ => {}
            }
        }
        // Clicking outside closes the Help modal
        if is_left_down {
            if let Some((x, y, w, h)) = app.help_rect {
                // Outer rect includes borders around inner help rect
                let outer_x = x.saturating_sub(1);
                let outer_y = y.saturating_sub(1);
                let outer_w = w.saturating_add(2);
                let outer_h = h.saturating_add(2);
                if mx < outer_x
                    || mx >= outer_x + outer_w
                    || my < outer_y
                    || my >= outer_y + outer_h
                {
                    app.modal = crate::state::Modal::None;
                }
            } else {
                // Fallback: close on any click if no rect is known
                app.modal = crate::state::Modal::None;
            }
            return false;
        }
        // Consume remaining mouse events while Help is open
        return false;
    }

    // If News modal is open, intercept mouse events before anything else
    if let crate::state::Modal::News { items, selected } = &mut app.modal {
        // Left click: select/open or close on outside
        if is_left_down {
            if let Some((x, y, w, h)) = app.news_list_rect
                && mx >= x
                && mx < x + w
                && my >= y
                && my < y + h
            {
                let row = my.saturating_sub(y) as usize;
                if !items.is_empty() {
                    let idx = std::cmp::min(row, items.len().saturating_sub(1));
                    *selected = idx;
                    if let Some(it) = items.get(*selected) {
                        let url = it.url.clone();
                        std::thread::spawn(move || {
                            let _ = std::process::Command::new("xdg-open")
                                .arg(url)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        });
                    }
                }
            } else if let Some((x, y, w, h)) = app.news_rect
                && (mx < x || mx >= x + w || my < y || my >= y + h)
            {
                // Click outside closes the modal
                app.modal = crate::state::Modal::None;
            }
            return false;
        }
        // Scroll within modal: move selection
        match m.kind {
            MouseEventKind::ScrollUp => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return false;
            }
            MouseEventKind::ScrollDown => {
                if *selected + 1 < items.len() {
                    *selected += 1;
                }
                return false;
            }
            _ => {}
        }
        // If modal is open and event wasn't handled above, consume it
        return false;
    }

    // 1) Handle modifier-clicks in details first, even when selection is enabled
    if is_left_down && ctrl && shift {
        // URL click
        if let Some((x, y, w, h)) = app.url_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
            && !app.details.url.is_empty()
        {
            app.mouse_disabled_in_details = false; // temporarily allow action
            let url = app.details.url.clone();
            std::thread::spawn(move || {
                let _ = std::process::Command::new("xdg-open")
                    .arg(url)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            });
            return false;
        }
        // Show PKGBUILD click (legacy Ctrl+Shift) — no longer active
    }

    // 2) New behavior: plain left click on Show/Hide PKGBUILD
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false; // allow this action
        if app.pkgb_visible {
            // Close if already open
            app.pkgb_visible = false;
            app.pkgb_text = None;
        } else {
            // Open and (re)load
            app.pkgb_visible = true;
            app.pkgb_text = None;
            if let Some(item) = app.results.get(app.selected).cloned() {
                let _ = pkgb_tx.send(item);
            }
        }
        return false;
    }

    // 2b) Click on "Check Package Build" title button
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_check_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false;
        if let Some(text) = app.pkgb_text.clone() {
            // Best-effort: Wayland -> wl-copy; X11 -> xclip; otherwise show guidance modal
            let (tx_msg, rx_msg) = std::sync::mpsc::channel::<Option<String>>();
            std::thread::spawn(move || {
                let suffix = {
                    let s = crate::theme::settings().clipboard_suffix;
                    if s.trim().is_empty() {
                        String::new()
                    } else {
                        format!("\n\n{s}\n")
                    }
                };
                let payload = if suffix.is_empty() {
                    text.clone()
                } else {
                    format!("{text}{suffix}")
                };
                // Try wl-copy on Wayland
                let tried_wl = if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    if let Ok(mut child) = std::process::Command::new("wl-copy")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        if let Some(mut sin) = child.stdin.take() {
                            let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
                        }
                        let _ = child.wait();
                        let _ = tx_msg.send(Some("PKGBUILD is added to the Clipboard".to_string()));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                if tried_wl {
                    return;
                }

                // Try xclip as a generic fallback on X11
                if let Ok(mut child) = std::process::Command::new("xclip")
                    .args(["-selection", "clipboard"]) // send to clipboard selection
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(mut sin) = child.stdin.take() {
                        let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
                    }
                    let _ = child.wait();
                    let _ = tx_msg.send(Some("PKGBUILD is added to the Clipboard".to_string()));
                    return;
                }

                // Neither wl-copy nor xclip worked — report guidance to UI thread
                let hint = if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    "Clipboard tool not found. Please install 'wl-clipboard' (provides wl-copy) or 'xclip'."
                } else {
                    "Clipboard tool not found. Please install 'xclip' or 'wl-clipboard' (wl-copy)."
                };
                let _ = tx_msg.send(Some(hint.to_string()));
            });
            // Default optimistic toast; overwritten by worker if needed
            app.toast_message = Some("Copying PKGBUILD to clipboard…".to_string());
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            // Try to receive the result quickly without blocking UI long
            if let Ok(Some(msg)) = rx_msg.recv_timeout(std::time::Duration::from_millis(50)) {
                app.toast_message = Some(msg);
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            }
        } else {
            app.toast_message = Some("PKGBUILD not loaded yet".to_string());
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        }
        return false;
    }

    // 3) If details should be markable, ignore other clicks within it
    if app.mouse_disabled_in_details
        && let Some((x, y, w, h)) = app.details_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        // Ensure terminal mouse capture stays enabled globally, while app ignores clicks here
        if !app.mouse_capture_enabled {
            let _ = execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
            app.mouse_capture_enabled = true;
        }
        return false;
    }

    // 4) Sort button, filters, options button, and dropdowns in Results title
    if is_left_down {
        // Click on Arch status label (opens status URL)
        if let Some((x, y, w, h)) = app.arch_status_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            std::thread::spawn(move || {
                let _ = std::process::Command::new("xdg-open")
                    .arg("https://status.archlinux.org")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            });
            return false;
        }
        // Toggle sort menu when clicking the button on the title
        if let Some((x, y, w, h)) = app.sort_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.sort_menu_open = !app.sort_menu_open;
            if app.sort_menu_open {
                app.sort_menu_auto_close_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
            } else {
                app.sort_menu_auto_close_at = None;
            }
            return false;
        }
        // Toggle options menu when clicking the Options button
        if let Some((x, y, w, h)) = app.options_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.options_menu_open = !app.options_menu_open;
            if app.options_menu_open {
                app.panels_menu_open = false;
                app.config_menu_open = false;
            }
            return false;
        }
        // Toggle Config/Lists menu when clicking the button
        if let Some((x, y, w, h)) = app.config_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.config_menu_open = !app.config_menu_open;
            if app.config_menu_open {
                app.options_menu_open = false;
                app.panels_menu_open = false;
            }
            return false;
        }
        // Toggle panels menu when clicking the Panels button
        if let Some((x, y, w, h)) = app.panels_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.panels_menu_open = !app.panels_menu_open;
            if app.panels_menu_open {
                app.options_menu_open = false;
                app.config_menu_open = false;
            }
            return false;
        }
        // Toggle filters when clicking their labels
        if let Some((x, y, w, h)) = app.results_filter_aur_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_aur = !app.results_filter_show_aur;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        if let Some((x, y, w, h)) = app.results_filter_core_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_core = !app.results_filter_show_core;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        if let Some((x, y, w, h)) = app.results_filter_extra_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_extra = !app.results_filter_show_extra;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        if let Some((x, y, w, h)) = app.results_filter_multilib_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_multilib = !app.results_filter_show_multilib;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        if let Some((x, y, w, h)) = app.results_filter_eos_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_eos = !app.results_filter_show_eos;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        if let Some((x, y, w, h)) = app.results_filter_cachyos_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_cachyos = !app.results_filter_show_cachyos;
            crate::logic::apply_filters_and_sort_preserve_selection(app);
            return false;
        }
        // If sort menu open, handle option click inside menu
        if app.sort_menu_open
            && let Some((x, y, w, h)) = app.sort_menu_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            let row = my.saturating_sub(y) as usize; // 0-based within options
            match row {
                0 => {
                    app.sort_mode = crate::state::SortMode::RepoThenName;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                1 => {
                    app.sort_mode = crate::state::SortMode::AurPopularityThenOfficial;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                2 => {
                    app.sort_mode = crate::state::SortMode::BestMatches;
                    crate::theme::save_sort_mode(app.sort_mode);
                }
                _ => {}
            }
            app.sort_menu_open = false;
            app.sort_menu_auto_close_at = None;
            // Apply sort immediately
            crate::logic::sort_results_preserve_selection(app);
            // Jump selection to top and refresh details
            if !app.results.is_empty() {
                app.selected = 0;
                app.list_state.select(Some(0));
                refresh_selected_details(app, details_tx);
            } else {
                app.list_state.select(None);
            }
            return false;
        }
        // If options menu open, handle clicks inside menu
        if app.options_menu_open
            && let Some((x, y, w, h)) = app.options_menu_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            let row = my.saturating_sub(y) as usize; // rows: 0 installed-only toggle, 1 update system, 2 news
            match row {
                0 => {
                    if app.installed_only_mode {
                        // Toggle OFF: restore full results (from backup) and label back
                        if let Some(prev) = app.results_backup_for_toggle.take() {
                            app.all_results = prev;
                        }
                        app.installed_only_mode = false;
                        app.right_pane_focus = crate::state::RightPaneFocus::Install;
                        crate::logic::apply_filters_and_sort_preserve_selection(app);
                        super::utils::refresh_selected_details(app, details_tx);
                    } else {
                        // Toggle ON: show only explicitly installed leaf packages
                        app.results_backup_for_toggle = Some(app.all_results.clone());
                        let explicit = crate::index::explicit_names();
                        // Official items filtered by explicit set
                        let mut items: Vec<crate::state::PackageItem> =
                            crate::index::all_official()
                                .into_iter()
                                .filter(|p| explicit.contains(&p.name))
                                .collect();
                        // For explicit names that are not in official index, represent as AUR entries
                        use std::collections::HashSet;
                        let official_names: HashSet<String> =
                            items.iter().map(|p| p.name.clone()).collect();
                        for name in explicit.into_iter() {
                            if !official_names.contains(&name) {
                                // If name indicates EOS, classify as official EOS
                                let is_eos = name.to_lowercase().contains("eos-");
                                let src = if is_eos {
                                    crate::state::Source::Official {
                                        repo: "EOS".to_string(),
                                        arch: String::new(),
                                    }
                                } else {
                                    crate::state::Source::Aur
                                };
                                items.push(crate::state::PackageItem {
                                    name: name.clone(),
                                    version: String::new(),
                                    description: String::new(),
                                    source: src,
                                    popularity: None,
                                });
                            }
                        }
                        app.all_results = items;
                        app.installed_only_mode = true;
                        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                        crate::logic::apply_filters_and_sort_preserve_selection(app);
                        super::utils::refresh_selected_details(app, details_tx);

                        // Save exported list to config directory as requested
                        let path = crate::theme::config_dir().join("installed_packages.txt");
                        let mut names: Vec<String> =
                            crate::index::explicit_names().into_iter().collect();
                        names.sort();
                        let body = names.join("\n");
                        let _ = std::fs::write(path, body);
                    }
                }
                1 => {
                    // Open SystemUpdate modal with defaults
                    let countries = vec![
                        "Worldwide".to_string(),
                        "Germany".to_string(),
                        "United States".to_string(),
                        "United Kingdom".to_string(),
                        "France".to_string(),
                        "Netherlands".to_string(),
                        "Sweden".to_string(),
                        "Canada".to_string(),
                        "Australia".to_string(),
                        "Japan".to_string(),
                    ];
                    app.modal = crate::state::Modal::SystemUpdate {
                        do_mirrors: false,
                        do_pacman: true,
                        do_aur: true,
                        do_cache: false,
                        country_idx: 0,
                        countries,
                        cursor: 0,
                    };
                }
                2 => {
                    // Fetch latest news (top 10) and open modal
                    let (tx, rx) = std::sync::mpsc::channel();
                    std::thread::spawn(move || {
                        // Use a small Tokio runtime to await async fetch
                        let rt = tokio::runtime::Builder::new_current_thread()
                            .enable_all()
                            .build();
                        let res = match rt {
                            Ok(rt) => rt.block_on(crate::sources::fetch_arch_news(10)),
                            Err(e) => {
                                Err::<Vec<crate::state::NewsItem>, _>(format!("rt: {e}").into())
                            }
                        };
                        let _ = tx.send(res);
                    });
                    match rx.recv_timeout(std::time::Duration::from_secs(3)) {
                        Ok(Ok(list)) => {
                            app.modal = crate::state::Modal::News {
                                items: list,
                                selected: 0,
                            };
                        }
                        Ok(Err(e)) => {
                            app.modal = crate::state::Modal::Alert {
                                message: format!("Failed to fetch news: {e}"),
                            };
                        }
                        Err(_) => {
                            app.modal = crate::state::Modal::Alert {
                                message: "Timed out fetching news".to_string(),
                            };
                        }
                    }
                }
                _ => {}
            }
            app.options_menu_open = false;
            return false;
        }
        // If config menu open, handle clicks inside menu
        if app.config_menu_open
            && let Some((x, y, w, h)) = app.config_menu_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            let row = my.saturating_sub(y) as usize; // rows: 0 pacsea.conf, 1 install_list, 2 installed_list, 3 recent_searches
            // Resolve file paths
            let conf_path = crate::theme::config_dir().join("pacsea.conf");
            let install_path = app.install_path.clone();
            let recent_path = app.recent_path.clone();
            // For installed list, write a transient file under config dir (keep as requested name)
            let installed_list_path = crate::theme::config_dir().join("installed_list.json");
            if row == 2 {
                // Build installed names JSON array (explicit set is closer to user expectation? use explicit_names for stability)
                let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
                names.sort();
                let body = serde_json::to_string_pretty(&names).unwrap_or("[]".to_string());
                let _ = std::fs::write(&installed_list_path, body);
            }

            let target = match row {
                0 => conf_path,
                1 => install_path,
                2 => installed_list_path,
                3 => recent_path,
                _ => {
                    app.config_menu_open = false;
                    return false;
                }
            };

            // Build a single OR-chained command so only the first available editor runs
            let path_str = target.display().to_string();
            let editor_cmd = format!(
                "(command -v nvim >/dev/null 2>&1 && nvim '{p}') || \
                 (command -v vim >/dev/null 2>&1 && vim '{p}') || \
                 (command -v hx >/dev/null 2>&1 && hx '{p}') || \
                 (command -v helix >/dev/null 2>&1 && helix '{p}') || \
                 (command -v nano >/dev/null 2>&1 && nano '{p}') || \
                 (echo 'No terminal editor found (nvim/vim/hx/helix/nano).'; echo 'File: {p}'; read -rn1 -s _ || true)",
                p = path_str,
            );
            let cmds = vec![editor_cmd];

            // Run in external terminal window
            std::thread::spawn(move || {
                crate::install::spawn_shell_commands_in_terminal(&cmds);
            });

            app.config_menu_open = false;
            return false;
        }
        // If panels menu open, handle clicks inside menu
        if app.panels_menu_open
            && let Some((x, y, w, h)) = app.panels_menu_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            let row = my.saturating_sub(y) as usize; // rows: 0 toggle recent, 1 toggle install, 2 toggle keybinds
            match row {
                0 => {
                    app.show_recent_pane = !app.show_recent_pane;
                    if !app.show_recent_pane && matches!(app.focus, crate::state::Focus::Recent) {
                        app.focus = crate::state::Focus::Search;
                    }
                    crate::theme::save_show_recent_pane(app.show_recent_pane);
                }
                1 => {
                    app.show_install_pane = !app.show_install_pane;
                    if !app.show_install_pane && matches!(app.focus, crate::state::Focus::Install) {
                        app.focus = crate::state::Focus::Search;
                    }
                    crate::theme::save_show_install_pane(app.show_install_pane);
                }
                2 => {
                    app.show_keybinds_footer = !app.show_keybinds_footer;
                    crate::theme::save_show_keybinds_footer(app.show_keybinds_footer);
                }
                _ => {}
            }
            app.panels_menu_open = false;
            return false;
        }
        // Click outside menu closes it
        if app.sort_menu_open {
            app.sort_menu_open = false;
            app.sort_menu_auto_close_at = None;
        }
        if app.options_menu_open {
            app.options_menu_open = false;
        }
        if app.panels_menu_open {
            app.panels_menu_open = false;
        }
        if app.config_menu_open {
            app.config_menu_open = false;
        }
    }

    // 5) Results: click to select
    if is_left_down
        && let Some((x, y, w, h)) = app.results_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let row = my.saturating_sub(y) as usize; // row in viewport
        let offset = app.list_state.offset();
        let idx = offset + row;
        if idx < app.results.len() {
            app.selected = idx;
            app.list_state.select(Some(idx));
        }
    }

    // 6) Results: scroll with mouse wheel to move selection
    if let Some((x, y, w, h)) = app.results_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                move_sel_cached(app, -1, details_tx);
            }
            MouseEventKind::ScrollDown => {
                move_sel_cached(app, 1, details_tx);
            }
            _ => {}
        }
    }

    // 7) Recent pane: scroll with mouse wheel to change selection
    if let Some((x, y, w, h)) = app.recent_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let inds = crate::ui_helpers::filtered_recent_indices(app);
        if !inds.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let new = sel.saturating_sub(1);
                    app.history_state.select(Some(new));
                    crate::ui_helpers::trigger_recent_preview(app, preview_tx);
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.history_state.select(Some(new));
                    crate::ui_helpers::trigger_recent_preview(app, preview_tx);
                }
                _ => {}
            }
        }
    }

    // 8) Right panes: click to focus/select rows and scroll to change selection
    // Click inside Remove/Install area (right subpane or full right pane)
    if is_left_down
        && let Some((x, y, w, h)) = app.install_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.focus = crate::state::Focus::Install;
        if app.installed_only_mode {
            app.right_pane_focus = crate::state::RightPaneFocus::Remove;
            let row = my.saturating_sub(y) as usize;
            let max = app.remove_list.len().saturating_sub(1);
            if !app.remove_list.is_empty() {
                let idx = std::cmp::min(row, max);
                app.remove_state.select(Some(idx));
                super::utils::refresh_remove_details(app, details_tx);
            }
        } else {
            app.right_pane_focus = crate::state::RightPaneFocus::Install;
            let row = my.saturating_sub(y) as usize;
            let inds = crate::ui_helpers::filtered_install_indices(app);
            if !inds.is_empty() {
                let max = inds.len().saturating_sub(1);
                let vis_idx = std::cmp::min(row, max);
                app.install_state.select(Some(vis_idx));
                super::utils::refresh_install_details(app, details_tx);
            }
        }
        return false;
    }

    // Click inside Downgrade subpane (left half in installed-only mode)
    if app.installed_only_mode
        && is_left_down
        && let Some((x, y, w, h)) = app.downgrade_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.focus = crate::state::Focus::Install;
        app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
        let row = my.saturating_sub(y) as usize;
        let max = app.downgrade_list.len().saturating_sub(1);
        if !app.downgrade_list.is_empty() {
            let idx = std::cmp::min(row, max);
            app.downgrade_state.select(Some(idx));
            super::utils::refresh_downgrade_details(app, details_tx);
        }
        return false;
    }

    // Scroll inside Remove/Install area
    // 8a) Right panes: scroll with mouse wheel to change selection
    // Remove (or Install in normal mode)
    if let Some((x, y, w, h)) = app.install_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        if app.installed_only_mode {
            let len = app.remove_list.len();
            if len > 0 {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        if let Some(sel) = app.remove_state.selected() {
                            let new = sel.saturating_sub(1);
                            app.remove_state.select(Some(new));
                            super::utils::refresh_remove_details(app, details_tx);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let sel = app.remove_state.selected().unwrap_or(0);
                        let max = len.saturating_sub(1);
                        let new = std::cmp::min(sel.saturating_add(1), max);
                        app.remove_state.select(Some(new));
                        super::utils::refresh_remove_details(app, details_tx);
                    }
                    _ => {}
                }
            }
        } else {
            let inds = crate::ui_helpers::filtered_install_indices(app);
            if !inds.is_empty() {
                match m.kind {
                    MouseEventKind::ScrollUp => {
                        if let Some(sel) = app.install_state.selected() {
                            let new = sel.saturating_sub(1);
                            app.install_state.select(Some(new));
                            refresh_install_details(app, details_tx);
                        }
                    }
                    MouseEventKind::ScrollDown => {
                        let sel = app.install_state.selected().unwrap_or(0);
                        let max = inds.len().saturating_sub(1);
                        let new = std::cmp::min(sel.saturating_add(1), max);
                        app.install_state.select(Some(new));
                        refresh_install_details(app, details_tx);
                    }
                    _ => {}
                }
            }
        }
    }

    // 8b) Downgrade subpane scroll
    if app.installed_only_mode
        && let Some((x, y, w, h)) = app.downgrade_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        let len = app.downgrade_list.len();
        if len > 0 {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if let Some(sel) = app.downgrade_state.selected() {
                        let new = sel.saturating_sub(1);
                        app.downgrade_state.select(Some(new));
                        super::utils::refresh_downgrade_details(app, details_tx);
                    }
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.downgrade_state.selected().unwrap_or(0);
                    let max = len.saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.downgrade_state.select(Some(new));
                    super::utils::refresh_downgrade_details(app, details_tx);
                }
                _ => {}
            }
        }
    }

    // 9) Scroll support inside PKGBUILD viewer using mouse wheel
    if let Some((x, y, w, h)) = app.pkgb_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                app.pkgb_scroll = app.pkgb_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                app.pkgb_scroll = app.pkgb_scroll.saturating_add(1);
            }
            _ => {}
        }
    }
    false
}
