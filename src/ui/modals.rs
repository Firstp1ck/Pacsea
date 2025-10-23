use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
use tracing;
use crate::theme::{KeyChord, theme};

/// Render modal overlays: Alert, ConfirmInstall, and Help.
///
/// Clears the area behind the modal and draws a styled box centered on the
/// screen. The Help modal dynamically reflects the current keymap.
pub fn render_modals(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    match &app.modal {
        crate::state::Modal::Alert { message } => {
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
            // Choose labels depending on error type (config vs network/other)
            let is_config = message.contains("Unknown key")
                || message.contains("Missing required keys")
                || message.contains("Missing '='")
                || message.contains("Missing key before '='")
                || message.contains("Duplicate key")
                || message.contains("Invalid color")
                || message.to_lowercase().contains("theme configuration");
            let clippy_block = {
                let ml = message.to_lowercase();
                ml.contains("clipboard")
                    || ml.contains("wl-copy")
                    || ml.contains("xclip")
                    || ml.contains("wl-clipboard")
            };
            let header_text = if is_config {
                "Configuration error"
            } else if clippy_block {
                "Clipboard Copy"
            } else {
                "Connection issue"
            };
            let is_clipboard = {
                let ml = message.to_lowercase();
                ml.contains("clipboard")
                    || ml.contains("wl-copy")
                    || ml.contains("xclip")
                    || ml.contains("wl-clipboard")
            };
            let box_title = if is_config {
                " Configuration Error "
            } else if is_clipboard {
                " Clipboard Copy "
            } else {
                " Connection issue "
            };
            let header_color = if is_config { th.mauve } else { th.red };
            let lines = vec![
                Line::from(Span::styled(
                    header_text,
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
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
                            box_title,
                            Style::default()
                                .fg(header_color)
                                .add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(header_color))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ConfirmInstall { items } => {
            let w = area.width.saturating_sub(6).min(90);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Confirm installation",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Nothing to install",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for p in items.iter().take((h as usize).saturating_sub(6)) {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", p.name),
                        Style::default().fg(th.text),
                    )));
                }
                if items.len() + 6 > h as usize {
                    lines.push(Line::from(Span::styled(
                        "…",
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to confirm or Esc to cancel",
                Style::default().fg(th.subtext1),
            )));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Install ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::Preflight { items, action, tab } => {
            tracing::info!(count = items.len(), ?action, "rendering Preflight modal");
            let w = area.width.saturating_sub(6).min(96);
            let h = area.height.saturating_sub(8).min(22);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect { x, y, width: w, height: h };
            f.render_widget(Clear, rect);

            let title = match action {
                crate::state::PreflightAction::Install => " Preflight: Install ",
                crate::state::PreflightAction::Remove => " Preflight: Remove ",
            };
            let border_color = th.lavender;
            let bg_color = th.crust;

            // Build header tab labels
            let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
            let mut header = String::new();
            for (i, lbl) in tab_labels.iter().enumerate() {
                let is = match (i, tab) {
                    (0, crate::state::PreflightTab::Summary) => true,
                    (1, crate::state::PreflightTab::Deps) => true,
                    (2, crate::state::PreflightTab::Files) => true,
                    (3, crate::state::PreflightTab::Services) => true,
                    (4, crate::state::PreflightTab::Sandbox) => true,
                    _ => false,
                };
                if i > 0 { header.push_str("  "); }
                if is { header.push_str("["); header.push_str(lbl); header.push_str("]"); }
                else { header.push_str(lbl); }
            }

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                header,
                Style::default().fg(th.overlay1).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            match tab {
                crate::state::PreflightTab::Summary => {
                    lines.push(Line::from(Span::styled(
                        "Summary (placeholder)",
                        Style::default().fg(border_color).add_modifier(Modifier::BOLD),
                    )));
                    lines.push(Line::from(""));
                    if items.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "No items selected.",
                            Style::default().fg(th.subtext1),
                        )));
                    } else {
                        for p in items.iter().take((h as usize).saturating_sub(8)) {
                            lines.push(Line::from(Span::styled(
                                format!("- {}", p.name),
                                Style::default().fg(th.text),
                            )));
                        }
                    }
                }
                crate::state::PreflightTab::Deps => {
                    lines.push(Line::from(Span::styled(
                        "Deps (placeholder) — dependency graph preview will appear here",
                        Style::default().fg(th.text),
                    )));
                }
                crate::state::PreflightTab::Files => {
                    lines.push(Line::from(Span::styled(
                        "Files (placeholder) — file list diff/pacnew prediction will appear here",
                        Style::default().fg(th.text),
                    )));
                }
                crate::state::PreflightTab::Services => {
                    lines.push(Line::from(Span::styled(
                        "Services (placeholder) — impacted services/restarts will appear here",
                        Style::default().fg(th.text),
                    )));
                }
                crate::state::PreflightTab::Sandbox => {
                    lines.push(Line::from(Span::styled(
                        "Sandbox (placeholder) — AUR preflight build checks will appear here",
                        Style::default().fg(th.text),
                    )));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Left/Right: tabs  •  s: sandbox  •  d: dry-run  •  p: proceed (disabled)  •  q: close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(bg_color))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            title,
                            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(bg_color)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::PreflightExec { items, action, tab, verbose, log_lines, abortable } => {
            let th = theme();
            let w = area.width.saturating_sub(4).min(110);
            let h = area.height.saturating_sub(4).min(area.height);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect { x, y, width: w, height: h };
            f.render_widget(Clear, rect);

            let border_color = th.lavender;
            let bg_color = th.crust;
            let title = match action {
                crate::state::PreflightAction::Install => " Execute: Install ",
                crate::state::PreflightAction::Remove => " Execute: Remove ",
            };

            // Split inner content: left (sidebar) 30%, right (log) 70%
            let inner = ratatui::prelude::Rect { x: rect.x + 1, y: rect.y + 1, width: rect.width.saturating_sub(2), height: rect.height.saturating_sub(2) };
            let cols = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Horizontal)
                .constraints([ratatui::layout::Constraint::Percentage(30), ratatui::layout::Constraint::Percentage(70)])
                .split(inner);

            // Sidebar: show selected tab header and items
            let mut s_lines: Vec<Line<'static>> = Vec::new();
            let tab_labels = ["Summary", "Deps", "Files", "Services", "Sandbox"];
            let mut header = String::new();
            for (i, lbl) in tab_labels.iter().enumerate() {
                let is = match (i, tab) {
                    (0, crate::state::PreflightTab::Summary) => true,
                    (1, crate::state::PreflightTab::Deps) => true,
                    (2, crate::state::PreflightTab::Files) => true,
                    (3, crate::state::PreflightTab::Services) => true,
                    (4, crate::state::PreflightTab::Sandbox) => true,
                    _ => false,
                };
                if i > 0 { header.push_str("  "); }
                if is { header.push_str("["); header.push_str(lbl); header.push_str("]"); } else { header.push_str(lbl); }
            }
            s_lines.push(Line::from(Span::styled(header, Style::default().fg(th.overlay1).add_modifier(Modifier::BOLD))));
            s_lines.push(Line::from(""));
            for p in items.iter().take(12) {
                s_lines.push(Line::from(Span::styled(format!("- {}", p.name), Style::default().fg(th.text))));
            }
            let sidebar = Paragraph::new(s_lines)
                .style(Style::default().fg(th.text).bg(bg_color))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(" Plan ", Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(bg_color)),
                );
            f.render_widget(sidebar, cols[0]);

            // Log panel
            let mut log_text: Vec<Line<'static>> = Vec::new();
            if log_lines.is_empty() {
                log_text.push(Line::from(Span::styled(
                    "Starting… (placeholder; real logs will stream here)",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for l in log_lines.iter().rev().take(cols[1].height as usize - 2).rev() {
                    log_text.push(Line::from(Span::styled(l.clone(), Style::default().fg(th.text))));
                }
            }
            log_text.push(Line::from(""));
            let footer = format!("l: verbose={}  •  x: abort{}  •  q/Esc/Enter: close", if *verbose {"ON"} else {"OFF"}, if *abortable {" (available)"} else {""});
            log_text.push(Line::from(Span::styled(footer, Style::default().fg(th.subtext1))));

            let logw = Paragraph::new(log_text)
                .style(Style::default().fg(th.text).bg(th.base))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .title(Span::styled(title, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(th.base)),
                );
            f.render_widget(logw, cols[1]);
        }
        crate::state::Modal::PostSummary { success, changed_files, pacnew_count, pacsave_count, services_pending, snapshot_label } => {
            let th = theme();
            let w = area.width.saturating_sub(8).min(96);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect { x, y, width: w, height: h };
            f.render_widget(Clear, rect);

            let border_color = if *success { th.green } else { th.red };
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                if *success { "Success" } else { "Failed" },
                Style::default().fg(border_color).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!("Changed files: {} (pacnew: {}, pacsave: {})", changed_files, pacnew_count, pacsave_count),
                Style::default().fg(th.text),
            )));
            if let Some(label) = snapshot_label {
                lines.push(Line::from(Span::styled(
                    format!("Snapshot: {}", label),
                    Style::default().fg(th.text),
                )));
            }
            if !services_pending.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled("Services pending restart:", Style::default().fg(th.overlay1).add_modifier(Modifier::BOLD))));
                for s in services_pending.iter().take((h as usize).saturating_sub(10)) {
                    lines.push(Line::from(Span::styled(format!("- {}", s), Style::default().fg(th.text))));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "r: rollback  •  s: restart services  •  Enter/Esc: close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Post-Transaction Summary ",
                            Style::default().fg(border_color).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(border_color))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::ConfirmRemove { items } => {
            let w = area.width.saturating_sub(6).min(90);
            let h = area.height.saturating_sub(6).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Confirm removal",
                Style::default().fg(th.red).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            // Warn explicitly if any core packages are present
            let has_core = items.iter().any(|p| match &p.source {
                crate::state::Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
                _ => false,
            });
            if has_core {
                lines.push(Line::from(Span::styled(
                    "WARNING: core packages selected. Removing core packages may break your system.",
                    Style::default()
                        .fg(th.red)
                        .add_modifier(Modifier::BOLD),
                )));
                lines.push(Line::from(""));
            }
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "Nothing to remove",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for p in items.iter().take((h as usize).saturating_sub(6)) {
                    lines.push(Line::from(Span::styled(
                        format!("- {}", p.name),
                        Style::default().fg(th.text),
                    )));
                }
                if items.len() + 6 > h as usize {
                    lines.push(Line::from(Span::styled(
                        "…",
                        Style::default().fg(th.subtext1),
                    )));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter to confirm or Esc to cancel",
                Style::default().fg(th.subtext1),
            )));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Confirm Remove ",
                            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.red))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            country_idx,
            countries,
            cursor,
        } => {
            let w = area.width.saturating_sub(8).min(80);
            let h = 14;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "System Update",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            let mark = |b: bool| if b { "[x]" } else { "[ ]" };

            let entries: [(&str, bool); 4] = [
                ("Update Arch Mirrors", *do_mirrors),
                ("Update Pacman (sudo pacman -Syu)", *do_pacman),
                ("Update AUR (paru/yay)", *do_aur),
                ("Remove Cache (pacman/yay)", *do_cache),
            ];
            for (i, (label, on)) in entries.iter().enumerate() {
                let style = if *cursor == i {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.lavender)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(th.text)
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("{} ", mark(*on)), Style::default().fg(th.overlay1)),
                    Span::styled((*label).to_string(), style),
                ]));
            }

            // Country selector (mirrors)
            lines.push(Line::from(""));
            let country_label = if *country_idx < countries.len() {
                &countries[*country_idx]
            } else {
                "Worldwide"
            };
            let style = if *cursor == entries.len() {
                Style::default()
                    .fg(th.crust)
                    .bg(th.lavender)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            lines.push(Line::from(vec![
                Span::styled("Country (Mirrors): ", Style::default().fg(th.overlay1)),
                Span::styled(country_label.to_string(), style),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Space: toggle  •  Left/Right: change country  •  Enter: run  •  Esc: cancel",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Update System ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::Help => {
            // Full-screen translucent help overlay
            let w = area.width.saturating_sub(6).min(96);
            let h = area.height.saturating_sub(4).min(28);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);
            // Record inner content rect (exclude borders) for mouse hit-testing
            app.help_rect = Some((
                rect.x + 1,
                rect.y + 1,
                rect.width.saturating_sub(2),
                rect.height.saturating_sub(2),
            ));
            let km = &app.keymap;

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Pacsea Help",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            // Utility to format a binding line
            let fmt = |label: &str, chord: KeyChord| -> Line<'static> {
                Line::from(vec![
                    Span::styled(
                        format!("{label:18}"),
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        format!("[{}]", chord.label()),
                        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
                    ),
                ])
            };

            if let Some(k) = km.help_overlay.first().copied() {
                lines.push(fmt("Help overlay", k));
            }
            if let Some(k) = km.exit.first().copied() {
                lines.push(fmt("Exit", k));
            }
            if let Some(k) = km.reload_theme.first().copied() {
                lines.push(fmt("Reload theme", k));
            }
            if let Some(k) = km.pane_next.first().copied() {
                lines.push(fmt("Next pane", k));
            }
            if let Some(k) = km.pane_left.first().copied() {
                lines.push(fmt("Focus left", k));
            }
            if let Some(k) = km.pane_right.first().copied() {
                lines.push(fmt("Focus right", k));
            }
            if let Some(k) = km.show_pkgbuild.first().copied() {
                lines.push(fmt("Show PKGBUILD", k));
            }
            // Show configured key for change sorting
            if let Some(k) = km.change_sort.first().copied() {
                lines.push(fmt("Change sorting", k));
            }
            lines.push(Line::from(""));

            // Dynamic section for per-pane actions based on keymap
            lines.push(Line::from(Span::styled(
                "Search:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pu.code,
                        mods: pu.mods,
                    },
                ));
                lines.push(fmt(
                    "  Page",
                    KeyChord {
                        code: pd.code,
                        mods: pd.mods,
                    },
                ));
            }
            if let Some(k) = km.search_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.search_install.first().copied() {
                lines.push(fmt("  Install", k));
            }
            if let Some(k) = km.search_backspace.first().copied() {
                lines.push(fmt("  Delete", k));
            }

            // Search normal mode
            if km
                .search_normal_toggle
                .first()
                .or(km.search_normal_insert.first())
                .or(km.search_normal_select_left.first())
                .or(km.search_normal_select_right.first())
                .or(km.search_normal_delete.first())
                .is_some()
            {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "Search (Normal mode):",
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD),
                )));
                if let Some(k) = km.search_normal_toggle.first().copied() {
                    lines.push(fmt("  Toggle normal", k));
                }
                if let Some(k) = km.search_normal_insert.first().copied() {
                    lines.push(fmt("  Insert mode", k));
                }
                if let Some(k) = km.search_normal_select_left.first().copied() {
                    lines.push(fmt("  Select left", k));
                }
                if let Some(k) = km.search_normal_select_right.first().copied() {
                    lines.push(fmt("  Select right", k));
                }
                if let Some(k) = km.search_normal_delete.first().copied() {
                    lines.push(fmt("  Delete", k));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Install:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.install_confirm.first().copied() {
                lines.push(fmt("  Confirm", k));
            }
            if let Some(k) = km.install_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            if let Some(k) = km.install_clear.first().copied() {
                lines.push(fmt("  Clear", k));
            }
            if let Some(k) = km.install_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.install_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Recent:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: up.code,
                        mods: up.mods,
                    },
                ));
                lines.push(fmt(
                    "  Move",
                    KeyChord {
                        code: dn.code,
                        mods: dn.mods,
                    },
                ));
            }
            if let Some(k) = km.recent_use.first().copied() {
                lines.push(fmt("  Use", k));
            }
            if let Some(k) = km.recent_add.first().copied() {
                lines.push(fmt("  Add", k));
            }
            if let Some(k) = km.recent_find.first().copied() {
                lines.push(fmt("  Find", k));
            }
            if let Some(k) = km.recent_to_search.first().copied() {
                lines.push(fmt("  To Search", k));
            }
            if let Some(k) = km.recent_remove.first().copied() {
                lines.push(fmt("  Remove", k));
            }
            // Explicit: Shift+Del clears Recent (display only)
            lines.push(fmt(
                "  Clear",
                crate::theme::KeyChord {
                    code: crossterm::event::KeyCode::Delete,
                    mods: crossterm::event::KeyModifiers::SHIFT,
                },
            ));

            // Mouse and UI controls
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Mouse:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Scroll lists (Results/Recent/Install) and PKGBUILD with mouse wheel",
            )));
            lines.push(Line::from(Span::raw(
                "  • Toggle PKGBUILD: click 'Show PKGBUILD' in details",
            )));
            lines.push(Line::from(Span::raw(
                "  • Copy 'Copy Package Build': click the title button in PKGBUILD",
            )));
            lines.push(Line::from(Span::raw(
                "  • Open details URL: Ctrl+Shift+Left click on the URL",
            )));
            lines.push(Line::from(Span::raw(
                "  • Results title bar: click Sort/Options/Panels/Config to open menus",
            )));
            lines.push(Line::from(Span::raw(
                "  • Toggle filters (AUR/core/extra/multilib/EOS/cachyos): click their labels",
            )));
            lines.push(Line::from(Span::raw(
                "  • Arch Status (top-right): click to open status.archlinux.org",
            )));

            // Dialogs
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "System Update dialog:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Open via Options → Update System",
            )));
            lines.push(Line::from(Span::raw(
                "  • Up/Down: move • Space: toggle • Left/Right: change country • Enter: run • Esc: close",
            )));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "News dialog:",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(Span::raw(
                "  • Open via Options → News • Up/Down: select • Enter: open • Esc: close",
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .scroll((app.help_scroll, 0))
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Help ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::News { items, selected } => {
            let w = (area.width * 2) / 3;
            let h = area.height.saturating_sub(8).min(20);
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            // Record outer and inner rects for mouse hit-testing
            app.news_rect = Some((rect.x, rect.y, rect.width, rect.height));

            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "Arch Linux News",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
            if items.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No news items available.",
                    Style::default().fg(th.subtext1),
                )));
            } else {
                for (i, it) in items.iter().enumerate() {
                    let tl = it.title.to_lowercase();
                    let is_critical = tl.contains("critical")
                        || tl.contains("require manual intervention")
                        || tl.contains("requires manual intervention");
                    let style = if *selected == i {
                        let fg = if is_critical { th.red } else { th.crust };
                        Style::default()
                            .fg(fg)
                            .bg(th.lavender)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        let fg = if is_critical { th.red } else { th.text };
                        Style::default().fg(fg)
                    };
                    let line = format!("{}  {}", it.date, it.title);
                    lines.push(Line::from(Span::styled(line, style)));
                }
            }
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Up/Down: select  •  Enter: open  •  Esc: close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " News ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);

            // The list content starts two lines after title and blank line, and ends before footer hint lines.
            // Approximate inner list area (exclude 1-char borders):
            let list_inner_x = rect.x + 1;
            let list_inner_y = rect.y + 1 + 2; // header + blank line
            let list_inner_w = rect.width.saturating_sub(2);
            // Compute visible rows budget: total height minus borders, header (2 lines), footer (2 lines)
            let inner_h = rect.height.saturating_sub(2);
            let list_rows = inner_h.saturating_sub(4);
            app.news_list_rect = Some((list_inner_x, list_inner_y, list_inner_w, list_rows));
        }
        crate::state::Modal::None => {}
    }
}
