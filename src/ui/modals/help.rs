use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::AppState;
use crate::theme::{KeyChord, theme};

pub fn render_help(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
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
            lines.push(fmt("  Insert Mode", k));
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
    if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first()) {
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
        KeyChord {
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
        "  • Copy PKGBUILD: click the title button (copies with suffix from settings.conf)",
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
