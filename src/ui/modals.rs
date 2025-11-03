use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
use crate::theme::{KeyChord, theme};

/// What: Render modal overlays (Alert, ConfirmInstall, ConfirmRemove, SystemUpdate, Help, News).
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (modal state, rects)
/// - `area`: Full available area; modals are centered within it
///
/// Output:
/// - Draws the active modal overlay and updates any modal-specific rects for hit-testing.
///
/// Details:
/// - Clears the area behind the modal; draws a styled centered box; content varies by modal.
/// - Help dynamically reflects keymap; News draws a selectable list and records list rect.
pub fn render_modals(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    // Draw a full-screen scrim behind any active modal to avoid underlying text bleed/concatenation
    if !matches!(app.modal, crate::state::Modal::None) {
        let scrim = Block::default().style(Style::default().bg(th.mantle));
        f.render_widget(scrim, area);
    }

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
            lines.push(Line::from(Span::styled(
                "Press S to scan AUR package(s) before install",
                Style::default().fg(th.overlay1),
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
            mirror_count,
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
                ("Update Pacman (sudo pacman -Syyu)", *do_pacman),
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
            // Read configured countries and mirror count from settings for display
            let prefs = crate::theme::settings();
            let conf_countries = if prefs.selected_countries.trim().is_empty() {
                "Worldwide".to_string()
            } else {
                prefs.selected_countries.clone()
            };
            // If Worldwide is selected, show the configured countries
            let shown_countries = if country_label == "Worldwide" {
                conf_countries.as_str()
            } else {
                country_label
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
                Span::styled(shown_countries.to_string(), style),
                Span::raw("  •  "),
                Span::styled(
                    format!("Count: {}", mirror_count),
                    Style::default().fg(th.overlay1),
                ),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Space: toggle  •  Left/Right: change country  •  -/+ change count  •  Enter: run  •  Esc: cancel",
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
            // Move menu toggles into Normal Mode section; omit here
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
                .or(km.search_normal_open_status.first())
                .or(km.config_menu_toggle.first())
                .or(km.options_menu_toggle.first())
                .or(km.panels_menu_toggle.first())
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
                if let Some(k) = km.search_normal_open_status.first().copied() {
                    lines.push(fmt("  Open Arch status", k));
                }
                if let Some(k) = km.config_menu_toggle.first().copied() {
                    lines.push(fmt("  Config/Lists menu", k));
                }
                if let Some(k) = km.options_menu_toggle.first().copied() {
                    lines.push(fmt("  Options menu", k));
                }
                if let Some(k) = km.panels_menu_toggle.first().copied() {
                    lines.push(fmt("  Panels menu", k));
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
        crate::state::Modal::OptionalDeps { rows, selected } => {
            // Build content lines with selection and install status markers
            let mut lines: Vec<Line<'static>> = Vec::new();
            lines.push(Line::from(Span::styled(
                "TUI Optional Deps",
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            for (i, row) in rows.iter().enumerate() {
                let is_sel = *selected == i;
                let (mark, color) = if row.installed {
                    ("✔ installed", th.green)
                } else {
                    ("⏺ not installed", th.overlay1)
                };
                let style = if is_sel {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.lavender)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(th.text)
                };
                let mut segs: Vec<Span> = Vec::new();
                segs.push(Span::styled(format!("{}  ", row.label), style));
                segs.push(Span::styled(
                    format!("[{}]", row.package),
                    Style::default().fg(th.overlay1),
                ));
                segs.push(Span::raw("  "));
                segs.push(Span::styled(mark.to_string(), Style::default().fg(color)));
                if let Some(note) = &row.note {
                    segs.push(Span::raw("  "));
                    segs.push(Span::styled(
                        format!("({})", note),
                        Style::default().fg(th.overlay2),
                    ));
                }
                lines.push(Line::from(segs));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Up/Down: select  •  Enter: install  •  Esc: close",
                Style::default().fg(th.subtext1),
            )));

            render_simple_list_modal(f, area, "Optional Deps", lines);
        }
        crate::state::Modal::GnomeTerminalPrompt => {
            // Centered confirmation dialog for installing GNOME Terminal
            let w = area.width.saturating_sub(10).min(90);
            let h = 9;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            let lines: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    "GNOME Terminal or Console recommended",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "GNOME was detected, but no GNOME terminal (gnome-terminal or gnome-console/kgx) is installed.",
                    Style::default().fg(th.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Press Enter to install gnome-terminal  •  Esc to cancel",
                    Style::default().fg(th.subtext1),
                )),
                Line::from(Span::styled(
                    "Cancel may lead to unexpected behavior.",
                    Style::default().fg(th.yellow),
                )),
            ];

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " Install a GNOME Terminal ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::VirusTotalSetup { input, cursor: _ } => {
            // Centered dialog for VirusTotal API key setup with clickable URL and input field
            let w = area.width.saturating_sub(10).min(90);
            let h = 11;
            let x = area.x + (area.width.saturating_sub(w)) / 2;
            let y = area.y + (area.height.saturating_sub(h)) / 2;
            let rect = ratatui::prelude::Rect {
                x,
                y,
                width: w,
                height: h,
            };
            f.render_widget(Clear, rect);

            // Build content
            let vt_url = "https://www.virustotal.com/gui/my-apikey";
            // Show input buffer (not masked)
            let shown = if input.is_empty() {
                "<empty>".to_string()
            } else {
                input.clone()
            };
            let lines: Vec<Line<'static>> = vec![
                Line::from(Span::styled(
                    "VirusTotal API Setup",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Open the link to view your API key:",
                    Style::default().fg(th.text),
                )),
                Line::from(vec![
                    // Surround with spaces to avoid visual concatenation with underlying content
                    Span::styled(" ", Style::default().fg(th.text)),
                    Span::styled(
                        vt_url.to_string(),
                        Style::default()
                            .fg(th.lavender)
                            .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Enter/paste your API key below and press Enter to save (Esc to cancel):",
                    Style::default().fg(th.subtext1),
                )),
                Line::from(Span::styled(
                    format!("API key: {}", shown),
                    Style::default().fg(th.text),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "Tip: After saving, scans will auto-query VirusTotal by file hash.",
                    Style::default().fg(th.overlay1),
                )),
            ];

            let inner_x = rect.x + 1;
            let inner_y = rect.y + 1;
            let url_line_y = inner_y + 3;
            let url_x = inner_x + 1;
            let url_w = vt_url.len() as u16;
            app.vt_url_rect = Some((url_x, url_line_y, url_w, 1));
            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
                .block(
                    Block::default()
                        .title(Span::styled(
                            " VirusTotal ",
                            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                        ))
                        .borders(Borders::ALL)
                        .border_type(BorderType::Double)
                        .border_style(Style::default().fg(th.mauve))
                        .style(Style::default().bg(th.mantle)),
                );
            f.render_widget(boxw, rect);
        }
        crate::state::Modal::None => {}
    }
}

/// Render a centered, simple list modal with a title and provided content lines.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `box_title`: Title shown in the modal border
/// - `lines`: Pre-built content lines
fn render_simple_list_modal(f: &mut Frame, area: Rect, box_title: &str, lines: Vec<Line<'static>>) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(80);
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
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    format!(" {} ", box_title),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

#[cfg(test)]
mod tests {
    /// What: Render all modal variants and record expected rects
    ///
    /// - Input: Cycle Alert, ConfirmInstall, ConfirmRemove(core), Help, News
    /// - Output: No panic; Help/news rects populated where applicable
    #[test]
    fn modals_set_rects_and_render_variants() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };

        // Alert
        app.modal = crate::state::Modal::Alert {
            message: "Test".into(),
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmInstall
        app.modal = crate::state::Modal::ConfirmInstall { items: vec![] };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // ConfirmRemove with core warn
        app.modal = crate::state::Modal::ConfirmRemove {
            items: vec![crate::state::PackageItem {
                name: "glibc".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            }],
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();

        // Help
        app.modal = crate::state::Modal::Help;
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.help_rect.is_some());

        // News
        app.modal = crate::state::Modal::News {
            items: vec![crate::state::NewsItem {
                date: "2025-10-11".into(),
                title: "Test".into(),
                url: "".into(),
            }],
            selected: 0,
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area)
        })
        .unwrap();
        assert!(app.news_rect.is_some());
        assert!(app.news_list_rect.is_some());
    }
}
