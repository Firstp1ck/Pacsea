use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::{KeyChord, theme};

/// What: Render the interactive help overlay summarizing keybindings and mouse tips.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (keymap, help scroll, rect tracking)
/// - `area`: Full screen area used to center the help modal
///
/// Output:
/// - Draws the help dialog, updates `app.help_rect`, and respects stored scroll offset.
///
/// Details:
/// - Formats bindings per pane, includes normal-mode guidance, and records clickable bounds to
///   enable mouse scrolling while using the current theme colors.
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
        i18n::t(app, "app.modals.help.heading"),
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
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.help_overlay"),
            k,
        ));
    }
    if let Some(k) = km.exit.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.exit"), k));
    }
    if let Some(k) = km.reload_theme.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.reload_theme"),
            k,
        ));
    }
    // Move menu toggles into Normal Mode section; omit here
    if let Some(k) = km.pane_next.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.next_pane"),
            k,
        ));
    }
    if let Some(k) = km.pane_left.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.focus_left"),
            k,
        ));
    }
    if let Some(k) = km.pane_right.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.focus_right"),
            k,
        ));
    }
    if let Some(k) = km.show_pkgbuild.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.show_pkgbuild"),
            k,
        ));
    }
    // Show configured key for change sorting
    if let Some(k) = km.change_sort.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.change_sorting"),
            k,
        ));
    }
    lines.push(Line::from(""));

    // Dynamic section for per-pane actions based on keymap
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.search"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: up.code,
                mods: up.mods,
            },
        ));
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: dn.code,
                mods: dn.mods,
            },
        ));
    }
    if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.page"),
            KeyChord {
                code: pu.code,
                mods: pu.mods,
            },
        ));
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.page"),
            KeyChord {
                code: pd.code,
                mods: pd.mods,
            },
        ));
    }
    if let Some(k) = km.search_add.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.add"), k));
    }
    if let Some(k) = km.search_install.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.install"), k));
    }
    if let Some(k) = km.search_backspace.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.delete"), k));
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
            i18n::t(app, "app.modals.help.sections.search_normal"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )));
        if let Some(k) = km.search_normal_toggle.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.toggle_normal"),
                k,
            ));
        }
        if let Some(k) = km.search_normal_insert.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.insert_mode"),
                k,
            ));
        }
        if let Some(k) = km.search_normal_select_left.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.select_left"),
                k,
            ));
        }
        if let Some(k) = km.search_normal_select_right.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.select_right"),
                k,
            ));
        }
        if let Some(k) = km.search_normal_delete.first().copied() {
            lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.delete"), k));
        }
        if let Some(k) = km.search_normal_open_status.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.open_arch_status"),
                k,
            ));
        }
        if let Some(k) = km.config_menu_toggle.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.config_lists_menu"),
                k,
            ));
        }
        if let Some(k) = km.options_menu_toggle.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.options_menu"),
                k,
            ));
        }
        if let Some(k) = km.panels_menu_toggle.first().copied() {
            lines.push(fmt(
                &i18n::t(app, "app.modals.help.key_labels.panels_menu"),
                k,
            ));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.install"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first()) {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: up.code,
                mods: up.mods,
            },
        ));
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: dn.code,
                mods: dn.mods,
            },
        ));
    }
    if let Some(k) = km.install_confirm.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.confirm"), k));
    }
    if let Some(k) = km.install_remove.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.remove"), k));
    }
    if let Some(k) = km.install_clear.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.clear"), k));
    }
    if let Some(k) = km.install_find.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.find"), k));
    }
    if let Some(k) = km.install_to_search.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.to_search"),
            k,
        ));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.recent"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: up.code,
                mods: up.mods,
            },
        ));
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.move"),
            KeyChord {
                code: dn.code,
                mods: dn.mods,
            },
        ));
    }
    if let Some(k) = km.recent_use.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.use"), k));
    }
    if let Some(k) = km.recent_add.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.add"), k));
    }
    if let Some(k) = km.recent_find.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.find"), k));
    }
    if let Some(k) = km.recent_to_search.first().copied() {
        lines.push(fmt(
            &i18n::t(app, "app.modals.help.key_labels.to_search"),
            k,
        ));
    }
    if let Some(k) = km.recent_remove.first().copied() {
        lines.push(fmt(&i18n::t(app, "app.modals.help.key_labels.remove"), k));
    }
    // Explicit: Shift+Del clears Recent (display only)
    lines.push(fmt(
        &i18n::t(app, "app.modals.help.key_labels.clear"),
        KeyChord {
            code: crossterm::event::KeyCode::Delete,
            mods: crossterm::event::KeyModifiers::SHIFT,
        },
    ));

    // Mouse and UI controls
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.mouse"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    // Arrays are stored as YAML strings, so we need to parse them
    let mouse_lines_yaml = i18n::t(app, "app.modals.help.mouse_lines");
    for line in mouse_lines_yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- \"") || trimmed.starts_with("- '") {
            let content = trimmed
                .strip_prefix("- \"")
                .or_else(|| trimmed.strip_prefix("- '"))
                .and_then(|s| s.strip_suffix('"').or_else(|| s.strip_suffix('\'')))
                .unwrap_or(trimmed);
            lines.push(Line::from(Span::raw(content.to_string())));
        } else if trimmed.starts_with("- ") {
            lines.push(Line::from(Span::raw(
                trimmed.strip_prefix("- ").unwrap_or(trimmed).to_string(),
            )));
        }
    }

    // Dialogs
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.system_update_dialog"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    let system_update_yaml = i18n::t(app, "app.modals.help.system_update_lines");
    for line in system_update_yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- \"") || trimmed.starts_with("- '") {
            let content = trimmed
                .strip_prefix("- \"")
                .or_else(|| trimmed.strip_prefix("- '"))
                .and_then(|s| s.strip_suffix('"').or_else(|| s.strip_suffix('\'')))
                .unwrap_or(trimmed);
            lines.push(Line::from(Span::raw(content.to_string())));
        } else if trimmed.starts_with("- ") {
            lines.push(Line::from(Span::raw(
                trimmed.strip_prefix("- ").unwrap_or(trimmed).to_string(),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.sections.news_dialog"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    let news_yaml = i18n::t(app, "app.modals.help.news_lines");
    for line in news_yaml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- \"") || trimmed.starts_with("- '") {
            let content = trimmed
                .strip_prefix("- \"")
                .or_else(|| trimmed.strip_prefix("- '"))
                .and_then(|s| s.strip_suffix('"').or_else(|| s.strip_suffix('\'')))
                .unwrap_or(trimmed);
            lines.push(Line::from(Span::raw(content.to_string())));
        } else if trimmed.starts_with("- ") {
            lines.push(Line::from(Span::raw(
                trimmed.strip_prefix("- ").unwrap_or(trimmed).to_string(),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.help.close_hint"),
        Style::default().fg(th.subtext1),
    )));

    let help_title = format!(" {} ", i18n::t(app, "app.titles.help"));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .scroll((app.help_scroll, 0))
        .block(
            Block::default()
                .title(Span::styled(
                    &help_title,
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
