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
/// - Clickable "Copy PKGBUILD" button in the PKGBUILD title (copies to clipboard).
/// - Clickable Sort button and filter toggles in the Results title.
/// - Click-to-select in Results; mouse wheel scroll moves selection in Results/Recent/Install.
/// - Mouse wheel scroll within the PKGBUILD viewer scrolls the content.
///
/// What: Handle a single mouse event and update application state and UI accordingly.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `app`: Mutable application state (rects, focus, lists, details)
/// - `details_tx`: Channel to request package details when selection changes
/// - `preview_tx`: Channel to request preview details for Recent pane interactions
/// - `_add_tx`: Channel for adding items (used by Import button handler)
/// - `pkgb_tx`: Channel to request PKGBUILD content for the current selection
///
/// Output:
/// - `true` to request application exit (never used here); otherwise `false`.
///
/// Details:
/// - Modal-first: When Help or News is open, clicks/scroll are handled within modal bounds
///   (close on outside click), consuming the event.
/// - Details area: Ctrl+Shift+LeftClick opens URL; PKGBUILD toggle and copy button respond to clicks;
///   while text selection is enabled, clicks inside details are ignored by the app.
/// - Title bar: Sort/options/panels/config buttons toggle menus; filter toggles apply filters.
/// - Results: Click selects; scroll wheel moves selection and triggers details fetch.
/// - Recent/Install/Remove/Downgrade panes: Scroll moves selection; click focuses/sets selection.
/// - Import/Export buttons: Import opens a system file picker to enqueue names; Export writes the
///   current Install list to a timestamped file and shows a toast.
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

    // If VirusTotalSetup modal is open, only open the URL when clicking on the link area; consume all mouse events
    if let crate::state::Modal::VirusTotalSetup { .. } = &app.modal {
        if is_left_down
            && let Some((x, y, w, h)) = app.vt_url_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            let url = "https://www.virustotal.com/gui/my-apikey";
            std::thread::spawn(move || {
                let _ = std::process::Command::new("xdg-open")
                    .arg(url)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            });
        }
        // Consume all mouse events while VirusTotal setup modal is open
        return false;
    }

    // If Preflight modal is open, handle mouse events
    if let crate::state::Modal::Preflight {
        tab,
        items,
        action,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        file_info,
        file_selected,
        file_tree_expanded,
    } = &mut app.modal
    {
        // Handle tab clicks
        if is_left_down {
            for (i, tab_rect_opt) in app.preflight_tab_rects.iter().enumerate() {
                if let Some((x, y, w, h)) = tab_rect_opt
                    && mx >= *x
                    && mx < x + w
                    && my >= *y
                    && my < y + h
                {
                    // Clicked on tab i - switch to that tab
                    let new_tab = match i {
                        0 => crate::state::PreflightTab::Summary,
                        1 => crate::state::PreflightTab::Deps,
                        2 => crate::state::PreflightTab::Files,
                        3 => crate::state::PreflightTab::Services,
                        4 => crate::state::PreflightTab::Sandbox,
                        _ => continue,
                    };
                    *tab = new_tab;

                    // Resolve dependencies when switching to Deps tab
                    if *tab == crate::state::PreflightTab::Deps
                        && dependency_info.is_empty()
                        && matches!(*action, crate::state::PreflightAction::Install)
                    {
                        *dependency_info = crate::logic::deps::resolve_dependencies(items);
                        *dep_selected = 0;
                    }
                    // Resolve files when switching to Files tab
                    if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                        *file_info = crate::logic::files::resolve_file_changes(items, *action);
                        *file_selected = 0;
                    }
                    return false;
                }
            }

            // Handle package group header clicks in Deps tab
            if *tab == crate::state::PreflightTab::Deps
                && !dependency_info.is_empty()
                && let Some((content_x, content_y, content_w, content_h)) =
                    app.preflight_content_rect
                && mx >= content_x
                && mx < content_x + content_w
                && my >= content_y
                && my < content_y + content_h
            {
                // Calculate which row was clicked relative to content area
                let clicked_row = (my - content_y) as usize;

                // Build display items list to find which package header was clicked
                use std::collections::{HashMap, HashSet};
                let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                    HashMap::new();
                for dep in dependency_info.iter() {
                    for req_by in &dep.required_by {
                        grouped.entry(req_by.clone()).or_default().push(dep);
                    }
                }

                let mut display_items: Vec<(bool, String)> = Vec::new();
                for pkg_name in items.iter().map(|p| &p.name) {
                    if grouped.contains_key(pkg_name) {
                        display_items.push((true, pkg_name.clone()));
                        if dep_tree_expanded.contains(pkg_name) {
                            let mut seen_deps = HashSet::new();
                            if let Some(pkg_deps) = grouped.get(pkg_name) {
                                for dep in pkg_deps.iter() {
                                    if seen_deps.insert(dep.name.as_str()) {
                                        display_items.push((false, String::new()));
                                    }
                                }
                            }
                        }
                    }
                }

                // Calculate offset for summary line before the list
                // Deps tab has: summary line (1) + empty line (1) = 2 lines
                let list_start_offset = 2;

                // Only process clicks that are on or after the list starts
                if clicked_row >= list_start_offset {
                    let list_clicked_row = clicked_row - list_start_offset;

                    // Calculate scroll position to find actual index
                    let available_height =
                        content_h.saturating_sub(list_start_offset as u16) as usize;
                    let start_idx = (*dep_selected)
                        .saturating_sub(available_height / 2)
                        .min(display_items.len().saturating_sub(available_height));

                    let actual_idx = start_idx + list_clicked_row;
                    if actual_idx < display_items.len()
                        && let Some((is_header, pkg_name)) = display_items.get(actual_idx)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                        return false;
                    }
                }
            }

            // Handle package group header clicks in Files tab
            if *tab == crate::state::PreflightTab::Files
                && !file_info.is_empty()
                && let Some((content_x, content_y, content_w, content_h)) =
                    app.preflight_content_rect
                && mx >= content_x
                && mx < content_x + content_w
                && my >= content_y
                && my < content_y + content_h
            {
                // Calculate which row was clicked relative to content area
                let clicked_row = (my - content_y) as usize;

                // Build display items list to find which package header was clicked
                let mut display_items: Vec<(bool, String)> = Vec::new();
                for pkg_info in file_info.iter() {
                    if !pkg_info.files.is_empty() {
                        display_items.push((true, pkg_info.name.clone()));
                        if file_tree_expanded.contains(&pkg_info.name) {
                            for _file in pkg_info.files.iter() {
                                display_items.push((false, String::new()));
                            }
                        }
                    }
                }

                // Calculate offset for summary lines before the list
                // Files tab has: summary line (1) + empty line (1) + optional sync timestamp (0-2) + empty line (0-1)
                // Minimum offset is 2 lines (summary + empty)
                let sync_timestamp_lines = if crate::logic::files::get_file_db_sync_info().is_some()
                {
                    2 // timestamp line + empty line
                } else {
                    0
                };
                let list_start_offset = 2 + sync_timestamp_lines; // summary + empty + sync timestamp lines

                // Only process clicks that are on or after the list starts
                if clicked_row >= list_start_offset {
                    let list_clicked_row = clicked_row - list_start_offset;

                    // Calculate scroll position to find actual index
                    let total_items = display_items.len();
                    let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
                    let available_height =
                        content_h.saturating_sub(list_start_offset as u16) as usize;
                    let (start_idx, _end_idx) = if total_items <= available_height {
                        (0, total_items)
                    } else {
                        let start = file_selected_clamped
                            .saturating_sub(available_height / 2)
                            .min(total_items.saturating_sub(available_height));
                        let end = (start + available_height).min(total_items);
                        (start, end)
                    };

                    let actual_idx = start_idx + list_clicked_row;
                    if actual_idx < display_items.len()
                        && let Some((is_header, pkg_name)) = display_items.get(actual_idx)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                        return false;
                    }
                }
            }
        }

        // Handle mouse scroll for Deps tab
        if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
            // Compute display_items length (headers + dependencies, accounting for folded groups)
            use std::collections::{HashMap, HashSet};
            let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                HashMap::new();
            for dep in dependency_info.iter() {
                for req_by in &dep.required_by {
                    grouped.entry(req_by.clone()).or_default().push(dep);
                }
            }
            let mut display_len: usize = 0;
            for pkg_name in items.iter().map(|p| &p.name) {
                if let Some(pkg_deps) = grouped.get(pkg_name) {
                    display_len += 1; // Header
                    // Count dependencies only if expanded
                    if dep_tree_expanded.contains(pkg_name) {
                        let mut seen_deps = HashSet::new();
                        for dep in pkg_deps.iter() {
                            if seen_deps.insert(dep.name.as_str()) {
                                display_len += 1;
                            }
                        }
                    }
                }
            }

            // Handle mouse scroll to navigate dependency list
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if *dep_selected > 0 {
                        *dep_selected -= 1;
                    }
                    return false;
                }
                MouseEventKind::ScrollDown => {
                    if *dep_selected < display_len.saturating_sub(1) {
                        *dep_selected += 1;
                    }
                    return false;
                }
                _ => {}
            }
        }

        // Handle mouse scroll for Files tab
        if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    if *file_selected > 0 {
                        *file_selected -= 1;
                    }
                    return false;
                }
                MouseEventKind::ScrollDown => {
                    let mut display_len = 0;
                    for pkg_info in file_info.iter() {
                        if !pkg_info.files.is_empty() {
                            display_len += 1; // Package header
                            if file_tree_expanded.contains(&pkg_info.name) {
                                display_len += pkg_info.files.len(); // Files only if expanded
                            }
                        }
                    }
                    if *file_selected < display_len.saturating_sub(1) {
                        *file_selected += 1;
                    }
                    return false;
                }
                _ => {}
            }
        }

        // Consume all mouse events while Preflight modal is open
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
                // Only open if clicking on an actual news item line (not empty space)
                if row < items.len() {
                    *selected = row;
                    if let Some(it) = items.get(*selected) {
                        crate::util::open_url(&it.url);
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
    // While any modal is open, prevent main window interaction by consuming mouse events
    if !matches!(app.modal, crate::state::Modal::None) {
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
            crate::util::open_url(&app.details.url);
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
            app.pkgb_package_name = None;
            app.pkgb_scroll = 0;
            app.pkgb_rect = None;
        } else {
            // Open and (re)load
            app.pkgb_visible = true;
            app.pkgb_text = None;
            app.pkgb_package_name = None;
            if let Some(item) = app.results.get(app.selected).cloned() {
                let _ = pkgb_tx.send(item);
            }
        }
        return false;
    }

    // 2b) Click on "Copy PKGBUILD" title button
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

    // 2c) Click on "Reload PKGBUILD" title button
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_reload_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false;
        if let Some(item) = app.results.get(app.selected).cloned() {
            // Schedule debounced reload (same as auto-reload)
            app.pkgb_reload_requested_at = Some(std::time::Instant::now());
            app.pkgb_reload_requested_for = Some(item.name.clone());
            app.pkgb_text = None; // Clear old PKGBUILD while loading
        }
        return false;
    }

    // 3) Scroll support inside Package Info details pane using mouse wheel (before click blocking)
    // Allow scrolling even when mouse clicks are disabled for text selection
    if let Some((x, y, w, h)) = app.details_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                app.details_scroll = app.details_scroll.saturating_sub(1);
                return false;
            }
            MouseEventKind::ScrollDown => {
                app.details_scroll = app.details_scroll.saturating_add(1);
                return false;
            }
            _ => {}
        }
    }

    // 4) If details should be markable, ignore other clicks within it
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

    // 5) Sort button, filters, options button, and dropdowns in Results title
    if is_left_down {
        // Click on Install pane bottom Import button
        if let Some((x, y, w, h)) = app.install_import_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            // Show ImportHelp modal first
            app.modal = crate::state::Modal::ImportHelp;
            return false;
        }
        // Click on Install pane bottom Export button
        if let Some((x, y, w, h)) = app.install_export_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            // Export current Install List package names to config export dir
            let mut names: Vec<String> = app.install_list.iter().map(|p| p.name.clone()).collect();
            names.sort();
            if names.is_empty() {
                app.toast_message = Some("Install List is empty".to_string());
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                return false;
            }
            // Build export directory and file name install_list_YYYYMMDD_serial
            let export_dir = crate::theme::config_dir().join("export");
            let _ = std::fs::create_dir_all(&export_dir);
            let date_str = crate::util::today_yyyymmdd_utc();
            let mut serial: u32 = 1;
            let file_path = loop {
                let fname = format!("install_list_{date_str}_{serial}.txt");
                let path = export_dir.join(&fname);
                if !path.exists() {
                    break path;
                }
                serial += 1;
                if serial > 9999 {
                    break export_dir.join(format!("install_list_{date_str}_fallback.txt"));
                }
            };
            let body = names.join("\n");
            match std::fs::write(&file_path, body) {
                Ok(_) => {
                    app.toast_message = Some(format!("Exported to {}", file_path.display()));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
                    tracing::info!(path = %file_path.display().to_string(), count = names.len(), "export: wrote install list");
                }
                Err(e) => {
                    app.toast_message = Some(format!("Export failed: {e}"));
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    tracing::error!(error = %e, path = %file_path.display().to_string(), "export: failed to write install list");
                }
            }
            return false;
        }
        // Click on Arch status label (opens status URL)
        if let Some((x, y, w, h)) = app.arch_status_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            crate::util::open_url("https://status.archlinux.org");
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
        if let Some((x, y, w, h)) = app.results_filter_manjaro_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
        {
            app.results_filter_show_manjaro = !app.results_filter_show_manjaro;
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
                                let is_eos = crate::index::is_eos_name(&name);
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
                    let prefs = crate::theme::settings();
                    let initial_country_idx = {
                        let sel = prefs
                            .selected_countries
                            .split(',')
                            .next()
                            .map(|s| s.trim().to_string())
                            .unwrap_or_else(|| "Worldwide".to_string());
                        countries.iter().position(|c| c == &sel).unwrap_or(0)
                    };
                    app.modal = crate::state::Modal::SystemUpdate {
                        do_mirrors: false,
                        do_pacman: true,
                        do_aur: true,
                        do_cache: false,
                        country_idx: initial_country_idx,
                        countries,
                        mirror_count: prefs.mirror_count,
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
                3 => {
                    // Build Optional Deps rows and open modal
                    let mut rows: Vec<crate::state::types::OptionalDepRow> = Vec::new();
                    let is_pkg_installed = |pkg: &str| crate::index::is_installed(pkg);
                    let on_path = |cmd: &str| crate::install::command_on_path(cmd);

                    // Security scanners (moved below AUR helper)

                    // Editor: show the one installed, otherwise all possibilities
                    // Map: (binary, package)
                    let editor_candidates: &[(&str, &str)] = &[
                        ("nvim", "neovim"),
                        ("vim", "vim"),
                        ("hx", "helix"),
                        ("helix", "helix"),
                        ("emacsclient", "emacs"),
                        ("emacs", "emacs"),
                        ("nano", "nano"),
                    ];
                    let mut editor_installed: Option<(&str, &str)> = None;
                    for (bin, pkg) in editor_candidates.iter() {
                        if on_path(bin) || is_pkg_installed(pkg) {
                            editor_installed = Some((*bin, *pkg));
                            break;
                        }
                    }
                    if let Some((bin, pkg)) = editor_installed {
                        rows.push(crate::state::types::OptionalDepRow {
                            label: format!("Editor: {}", bin),
                            package: pkg.to_string(),
                            installed: (is_pkg_installed(pkg)
                                || on_path(bin)
                                || ((pkg == "helix") && (on_path("hx") || on_path("helix")))
                                || ((pkg == "emacs")
                                    && (on_path("emacs") || on_path("emacsclient")))),
                            selectable: false,
                            note: None,
                        });
                    } else {
                        // Show unique packages (avoid hx+helix duplication)
                        let mut seen = std::collections::HashSet::new();
                        for (bin, pkg) in editor_candidates.iter() {
                            if seen.insert(*pkg) {
                                rows.push(crate::state::types::OptionalDepRow {
                                    label: format!("Editor: {}", bin),
                                    package: pkg.to_string(),
                                    installed: (is_pkg_installed(pkg)
                                        || on_path(bin)
                                        || ((*pkg == "helix")
                                            && (on_path("hx") || on_path("helix")))
                                        || ((*pkg == "emacs")
                                            && (on_path("emacs") || on_path("emacsclient")))),
                                    selectable: !(is_pkg_installed(pkg)
                                        || on_path(bin)
                                        || ((*pkg == "helix")
                                            && (on_path("hx") || on_path("helix")))
                                        || ((*pkg == "emacs")
                                            && (on_path("emacs") || on_path("emacsclient")))),
                                    note: None,
                                });
                            }
                        }
                    }

                    // Terminal: show only the one installed, otherwise all possibilities
                    let term_candidates: &[(&str, &str)] = &[
                        ("alacritty", "alacritty"),
                        ("ghostty", "ghostty"),
                        ("kitty", "kitty"),
                        ("xterm", "xterm"),
                        ("gnome-terminal", "gnome-terminal"),
                        ("konsole", "konsole"),
                        ("xfce4-terminal", "xfce4-terminal"),
                        ("tilix", "tilix"),
                        ("mate-terminal", "mate-terminal"),
                    ];
                    let mut term_installed: Option<(&str, &str)> = None;
                    for (bin, pkg) in term_candidates.iter() {
                        if on_path(bin) || is_pkg_installed(pkg) {
                            term_installed = Some((*bin, *pkg));
                            break;
                        }
                    }
                    if let Some((bin, pkg)) = term_installed {
                        rows.push(crate::state::types::OptionalDepRow {
                            label: format!("Terminal: {}", bin),
                            package: pkg.to_string(),
                            installed: (is_pkg_installed(pkg) || on_path(bin)),
                            selectable: false,
                            note: None,
                        });
                    } else {
                        for (bin, pkg) in term_candidates.iter() {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: format!("Terminal: {}", bin),
                                package: pkg.to_string(),
                                installed: (is_pkg_installed(pkg) || on_path(bin)),
                                selectable: !(is_pkg_installed(pkg) || on_path(bin)),
                                note: None,
                            });
                        }
                    }

                    // Clipboard: Prefer Klipper when KDE session detected; else Wayland/X11 specific
                    let is_kde = std::env::var("KDE_FULL_SESSION").is_ok()
                        || std::env::var("XDG_CURRENT_DESKTOP")
                            .ok()
                            .map(|v| {
                                let u = v.to_uppercase();
                                u.contains("KDE") || u.contains("PLASMA")
                            })
                            .unwrap_or(false)
                        || on_path("klipper");
                    if is_kde {
                        let pkg = "plasma-workspace";
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Clipboard: Klipper (KDE)".to_string(),
                            package: pkg.to_string(),
                            installed: is_pkg_installed(pkg) || on_path("klipper"),
                            selectable: !(is_pkg_installed(pkg) || on_path("klipper")),
                            note: Some("KDE Plasma".to_string()),
                        });
                    } else {
                        let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
                        if is_wayland {
                            let pkg = "wl-clipboard";
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Clipboard: wl-clipboard".to_string(),
                                package: pkg.to_string(),
                                installed: is_pkg_installed(pkg) || on_path("wl-copy"),
                                selectable: !(is_pkg_installed(pkg) || on_path("wl-copy")),
                                note: Some("Wayland".to_string()),
                            });
                        } else {
                            let pkg = "xclip";
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "Clipboard: xclip".to_string(),
                                package: pkg.to_string(),
                                installed: is_pkg_installed(pkg) || on_path("xclip"),
                                selectable: !(is_pkg_installed(pkg) || on_path("xclip")),
                                note: Some("X11".to_string()),
                            });
                        }
                    }

                    // Reflector/pacman-mirrors: Manjaro -> pacman-mirrors, else reflector
                    let manjaro = std::fs::read_to_string("/etc/os-release")
                        .map(|s| s.contains("Manjaro"))
                        .unwrap_or(false);
                    if manjaro {
                        let pkg = "pacman-mirrors";
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Mirrors: pacman-mirrors".to_string(),
                            package: pkg.to_string(),
                            installed: is_pkg_installed(pkg),
                            selectable: !is_pkg_installed(pkg),
                            note: Some("Manjaro".to_string()),
                        });
                    } else {
                        let pkg = "reflector";
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Mirrors: reflector".to_string(),
                            package: pkg.to_string(),
                            installed: is_pkg_installed(pkg),
                            selectable: !is_pkg_installed(pkg),
                            note: None,
                        });
                    }

                    // AUR helper: if one is installed show only that; else show both
                    let paru_inst = on_path("paru") || is_pkg_installed("paru");
                    let yay_inst = on_path("yay") || is_pkg_installed("yay");
                    if paru_inst || yay_inst {
                        if paru_inst {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "AUR helper: paru".to_string(),
                                package: "paru".to_string(),
                                installed: true,
                                selectable: false,
                                note: None,
                            });
                        } else if yay_inst {
                            rows.push(crate::state::types::OptionalDepRow {
                                label: "AUR helper: yay".to_string(),
                                package: "yay".to_string(),
                                installed: true,
                                selectable: false,
                                note: None,
                            });
                        }
                    } else {
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "AUR helper: paru".to_string(),
                            package: "paru".to_string(),
                            installed: false,
                            selectable: true,
                            note: Some("Install via git clone + makepkg -si".to_string()),
                        });
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "AUR helper: yay".to_string(),
                            package: "yay".to_string(),
                            installed: false,
                            selectable: true,
                            note: Some("Install via git clone + makepkg -si".to_string()),
                        });
                    }

                    // Security scanners (after AUR helper)
                    {
                        // ClamAV (official)
                        let pkg = "clamav";
                        let installed = is_pkg_installed(pkg) || on_path("clamscan");
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: clamav".to_string(),
                            package: pkg.to_string(),
                            installed,
                            selectable: !installed,
                            note: None,
                        });
                        // Trivy (official)
                        let pkg = "trivy";
                        let installed = is_pkg_installed(pkg) || on_path("trivy");
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: trivy".to_string(),
                            package: pkg.to_string(),
                            installed,
                            selectable: !installed,
                            note: None,
                        });
                        // Semgrep (AUR: semgrep-bin)
                        let pkg = "semgrep-bin";
                        let installed = is_pkg_installed(pkg) || on_path("semgrep");
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: semgrep-bin".to_string(),
                            package: pkg.to_string(),
                            installed,
                            selectable: !installed,
                            note: Some("AUR".to_string()),
                        });
                        // ShellCheck (official)
                        let pkg = "shellcheck";
                        let installed = is_pkg_installed(pkg) || on_path("shellcheck");
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: shellcheck".to_string(),
                            package: pkg.to_string(),
                            installed,
                            selectable: !installed,
                            note: None,
                        });
                    }
                    // VirusTotal API setup
                    {
                        let vt_key_present =
                            !crate::theme::settings().virustotal_api_key.is_empty();
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: VirusTotal API".to_string(),
                            package: "virustotal-setup".to_string(),
                            installed: vt_key_present,
                            selectable: true,
                            note: Some("Setup".to_string()),
                        });
                    }

                    // aur-sleuth setup
                    {
                        let sleuth_installed = {
                            let onpath = on_path("aur-sleuth");
                            let home = std::env::var("HOME").ok();
                            let user_local = home
                                .as_deref()
                                .map(|h| {
                                    std::path::Path::new(h)
                                        .join(".local/bin/aur-sleuth")
                                        .exists()
                                })
                                .unwrap_or(false);
                            let usr_local =
                                std::path::Path::new("/usr/local/bin/aur-sleuth").exists();
                            onpath || user_local || usr_local
                        };
                        rows.push(crate::state::types::OptionalDepRow {
                            label: "Security: aur-sleuth".to_string(),
                            package: "aur-sleuth-setup".to_string(),
                            installed: sleuth_installed,
                            selectable: true,
                            note: Some("Setup".to_string()),
                        });
                    }
                    app.modal = crate::state::Modal::OptionalDeps { rows, selected: 0 };
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
            let row = my.saturating_sub(y) as usize; // rows: 0 settings.conf, 1 theme.conf, 2 keybinds.conf, 3 install_list, 4 installed_list, 5 recent_searches
            // Resolve file paths
            let settings_path = crate::theme::config_dir().join("settings.conf");
            let theme_path = crate::theme::config_dir().join("theme.conf");
            let keybinds_path = crate::theme::config_dir().join("keybinds.conf");
            let install_path = app.install_path.clone();
            let recent_path = app.recent_path.clone();
            // Export installed package names to config directory as plaintext list
            let installed_list_path = crate::theme::config_dir().join("installed_packages.txt");
            if row == 4 {
                // Build installed names as newline-separated list
                let mut names: Vec<String> = crate::index::explicit_names().into_iter().collect();
                names.sort();
                let body = names.join("\n");
                let _ = std::fs::write(&installed_list_path, body);
            }

            let target = match row {
                0 => settings_path,
                1 => theme_path,
                2 => keybinds_path,
                3 => install_path,
                4 => installed_list_path,
                5 => recent_path,
                _ => {
                    app.config_menu_open = false;
                    return false;
                }
            };

            // Build a single OR-chained command so only the first available editor runs
            let path_str = target.display().to_string();
            let editor_cmd = format!(
                "((command -v nvim >/dev/null 2>&1 || sudo pacman -Qi neovim >/dev/null 2>&1) && nvim '{path_str}') || \
                 ((command -v vim >/dev/null 2>&1 || sudo pacman -Qi vim >/dev/null 2>&1) && vim '{path_str}') || \
                 ((command -v hx >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && hx '{path_str}') || \
                 ((command -v helix >/dev/null 2>&1 || sudo pacman -Qi helix >/dev/null 2>&1) && helix '{path_str}') || \
                 ((command -v emacsclient >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t '{path_str}') || \
                 ((command -v emacs >/dev/null 2>&1 || sudo pacman -Qi emacs >/dev/null 2>&1) && emacs -nw '{path_str}') || \
                 ((command -v nano >/dev/null 2>&1 || sudo pacman -Qi nano >/dev/null 2>&1) && nano '{path_str}') || \
                 (echo 'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'; echo 'File: {path_str}'; read -rn1 -s _ || true)",
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
            // Keep menu open after toggling panels
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
        let inds = crate::ui::helpers::filtered_recent_indices(app);
        if !inds.is_empty() {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let new = sel.saturating_sub(1);
                    app.history_state.select(Some(new));
                    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                }
                MouseEventKind::ScrollDown => {
                    let sel = app.history_state.selected().unwrap_or(0);
                    let max = inds.len().saturating_sub(1);
                    let new = std::cmp::min(sel.saturating_add(1), max);
                    app.history_state.select(Some(new));
                    crate::ui::helpers::trigger_recent_preview(app, preview_tx);
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
            let inds = crate::ui::helpers::filtered_install_indices(app);
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
            let inds = crate::ui::helpers::filtered_install_indices(app);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
    }

    #[test]
    /// What: Clicking PKGBUILD toggle opens viewer and enqueues fetch
    ///
    /// - Input: Click inside pkgb_button_rect with a selected result
    /// - Output: pkgb_visible=true and item sent to pkgb_tx
    fn click_pkgb_toggle_opens() {
        let mut app = new_app();
        app.results = vec![crate::state::PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.selected = 0;
        app.pkgb_button_rect = Some((10, 10, 5, 1));
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, mut pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 11,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };
        let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);
        assert!(app.pkgb_visible);
        assert!(pkgb_rx.try_recv().ok().is_some());
    }

    #[test]
    /// What: Clicking Hide PKGBUILD closes viewer and resets scroll/rect state
    ///
    /// - Input: PKGBUILD viewer visible with non-zero scroll and rect set; click inside pkgb_button_rect
    /// - Output: pkgb_visible=false; pkgb_text=None; pkgb_scroll==0; pkgb_rect=None
    fn click_pkgb_toggle_closes_and_resets() {
        let mut app = new_app();
        app.results = vec![crate::state::PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.selected = 0;
        app.pkgb_button_rect = Some((10, 10, 5, 1));
        // Pre-set PKGBUILD viewer as open with state to be reset
        app.pkgb_visible = true;
        app.pkgb_text = Some("dummy".into());
        app.pkgb_scroll = 7;
        app.pkgb_rect = Some((50, 50, 20, 5));

        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let (pkgb_tx, _pkgb_rx) = mpsc::unbounded_channel::<PackageItem>();
        // Click inside the toggle area to hide
        let ev = MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 11,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };
        let _ = handle_mouse_event(ev, &mut app, &dtx, &ptx, &atx, &pkgb_tx);

        assert!(!app.pkgb_visible);
        assert!(app.pkgb_text.is_none());
        assert_eq!(app.pkgb_scroll, 0);
        assert!(app.pkgb_rect.is_none());
    }
}
