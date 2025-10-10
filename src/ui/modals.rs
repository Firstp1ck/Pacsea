use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
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
            if let Some(k) = km.pane_prev.first().copied() {
                lines.push(fmt("Previous pane", k));
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
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Press Enter or Esc to close",
                Style::default().fg(th.subtext1),
            )));

            let boxw = Paragraph::new(lines)
                .style(Style::default().fg(th.text).bg(th.mantle))
                .wrap(Wrap { trim: true })
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
        crate::state::Modal::None => {}
    }
}
