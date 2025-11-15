use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, Focus, RightPaneFocus};
use crate::theme::{KeyChord, theme};

/// What: Render the keybind help footer inside the Package Info pane.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state providing focus, keymap, and pane flags
/// - `bottom_container`: Overall rect covering the package info + footer region
/// - `help_h`: Calculated footer height in rows
///
/// Output:
/// - Draws footer content and updates no state beyond rendering; uses rect geometry only locally.
///
/// Details:
/// - Highlights sections based on focused pane, reflects installed-only mode splits, and emits
///   optional Normal Mode help when active; fills background to avoid bleed-through.
pub fn render_footer(f: &mut Frame, app: &AppState, bottom_container: Rect, help_h: u16) {
    let th = theme();

    // Footer occupies the bottom rows of the bottom container using reserved height above
    let footer_container = bottom_container;
    if footer_container.height > help_h + 2 {
        let x = footer_container.x + 1; // inside border
        let y_top = footer_container.y + footer_container.height.saturating_sub(help_h);
        let w = footer_container.width.saturating_sub(2);
        let h = help_h;
        let footer_rect = Rect {
            x,
            y: y_top,
            width: w,
            height: h,
        };

        let search_label_color = if matches!(app.focus, Focus::Search) {
            th.mauve
        } else {
            th.overlay1
        };
        let install_label_color = if matches!(app.focus, Focus::Install) {
            th.mauve
        } else {
            th.overlay1
        };
        // Subpane label colors when installed-only mode splits the right pane
        let downgrade_label_color = if matches!(app.focus, Focus::Install)
            && matches!(app.right_pane_focus, RightPaneFocus::Downgrade)
        {
            th.mauve
        } else {
            th.overlay1
        };
        let remove_label_color = if matches!(app.focus, Focus::Install)
            && matches!(app.right_pane_focus, RightPaneFocus::Remove)
        {
            th.mauve
        } else {
            th.overlay1
        };
        let recent_label_color = if matches!(app.focus, Focus::Recent) {
            th.mauve
        } else {
            th.overlay1
        };

        let key_style = Style::default().fg(th.text).add_modifier(Modifier::BOLD);
        let sep = Span::styled("  |  ", Style::default().fg(th.overlay2));

        // GLOBALS (dynamic from keymap)
        let km = &app.keymap;
        let globals_label = format!("{}  ", i18n::t(app, "app.headings.globals"));
        let mut g_spans: Vec<Span> = vec![
            Span::styled(
                globals_label,
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
        ];
        if let Some(k) = km.exit.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.exit"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.help_overlay.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.help"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.reload_theme.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.reload_theme"))),
                sep.clone(),
            ]);
        }
        // Menu toggles are shown under Search (Normal mode) now
        if let Some(k) = km.show_pkgbuild.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(
                    " {}",
                    i18n::t(app, "app.actions.show_hide_pkgbuild")
                )),
                sep.clone(),
            ]);
        }
        // Change sorting (global) using configured keybind
        if let Some(k) = km.change_sort.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.change_sort_mode"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.search_normal_toggle.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.insert_mode"))),
                sep.clone(),
            ]);
        }
        // (Pane focus left/right intentionally omitted from footer)

        // SEARCH
        let search_label = format!("{}   ", i18n::t(app, "app.headings.search"));
        let mut s_spans: Vec<Span> = vec![
            Span::styled(
                search_label,
                Style::default()
                    .fg(search_label_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
        ];
        // Move
        if let (Some(up), Some(dn)) = (km.search_move_up.first(), km.search_move_down.first()) {
            s_spans.extend([
                Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.move"))),
                sep.clone(),
            ]);
        }
        // Page
        if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
            s_spans.extend([
                Span::styled(format!("[{} / {}]", pu.label(), pd.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.move_page"))),
                sep.clone(),
            ]);
        }
        // Add / Install
        if let Some(k) = km.search_add.first() {
            s_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(
                    " {}",
                    if app.installed_only_mode {
                        i18n::t(app, "app.actions.add_to_remove")
                    } else {
                        i18n::t(app, "app.actions.add_to_install")
                    }
                )),
                sep.clone(),
            ]);
        }
        if app.installed_only_mode {
            s_spans.extend([
                Span::styled("[Ctrl+Space]", key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.add_to_downgrade"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.search_install.first() {
            s_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.install"))),
                sep.clone(),
            ]);
        }
        // (Pane next, delete char, and focus left/right intentionally omitted from footer)

        // INSTALL or split into DOWNGRADE and REMOVE when installed-only mode is active
        // Helper to build common spans for right-pane actions
        let build_right_spans = |label: &str, label_color, confirm_text: &str| {
            let mut spans: Vec<Span> = vec![
                Span::styled(
                    label.to_string(),
                    Style::default()
                        .fg(label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.move"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_confirm.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(" {confirm_text}")),
                    sep.clone(),
                ]);
            }
            if !km.install_remove.is_empty() {
                let keys = km
                    .install_remove
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(" / ");
                spans.extend([
                    Span::styled(format!("[{keys}]"), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.remove_from_list"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_clear.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.clear"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_find.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(
                        " {}",
                        i18n::t(app, "app.details.footer.search_hint")
                    )),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_to_search.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.go_to_search"))),
                    sep.clone(),
                ]);
            }
            spans
        };

        let (right_lines_install, right_lines_split) = if app.installed_only_mode {
            let d_spans = build_right_spans(
                &format!("{}:", i18n::t(app, "app.headings.downgrade")),
                downgrade_label_color,
                &i18n::t(app, "app.details.footer.confirm_downgrade"),
            );
            let r_spans = build_right_spans(
                &format!("{}:   ", i18n::t(app, "app.headings.remove")),
                remove_label_color,
                &i18n::t(app, "app.details.footer.confirm_removal"),
            );
            (None, Some((Line::from(d_spans), Line::from(r_spans))))
        } else {
            let mut i_spans: Vec<Span> = vec![
                Span::styled(
                    format!("{}:  ", i18n::t(app, "app.headings.install")),
                    Style::default()
                        .fg(install_label_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
            ];
            if let (Some(up), Some(dn)) = (km.install_move_up.first(), km.install_move_down.first())
            {
                i_spans.extend([
                    Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.move"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_confirm.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(
                        " {}",
                        i18n::t(app, "app.details.footer.confirm_installation")
                    )),
                    sep.clone(),
                ]);
            }
            if !km.install_remove.is_empty() {
                let keys = km
                    .install_remove
                    .iter()
                    .map(|c| c.label())
                    .collect::<Vec<_>>()
                    .join(" / ");
                i_spans.extend([
                    Span::styled(format!("[{keys}]"), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.remove_from_list"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_clear.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.clear"))),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_find.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(
                        " {}",
                        i18n::t(app, "app.details.footer.search_hint")
                    )),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_to_search.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(format!(" {}", i18n::t(app, "app.actions.go_to_search"))),
                    sep.clone(),
                ]);
            }
            (Some(Line::from(i_spans)), None)
        };

        // RECENT
        let recent_label = format!("{}   ", i18n::t(app, "app.headings.recent"));
        let mut r_spans: Vec<Span> = vec![
            Span::styled(
                recent_label,
                Style::default()
                    .fg(recent_label_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
        ];
        if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
            r_spans.extend([
                Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.move"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_use.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.add_to_search"))),
                sep.clone(),
            ]);
        }
        if !km.recent_remove.is_empty() {
            let keys = km
                .recent_remove
                .iter()
                .map(|c| c.label())
                .collect::<Vec<_>>()
                .join(" / ");
            r_spans.extend([
                Span::styled(format!("[{keys}]"), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.remove_from_list"))),
                sep.clone(),
            ]);
        }
        // Clear all entries in Recent: configurable keybind (fallback to Shift+Del label)
        if let Some(k) = km.recent_clear.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.clear"))),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_add.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(
                    " {}",
                    i18n::t(app, "app.actions.add_first_match_to_install")
                )),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_find.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(
                    " {}",
                    i18n::t(app, "app.actions.search_hint_enter_next_esc_cancel")
                )),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_to_search.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(format!(" {}", i18n::t(app, "app.actions.go_to_search"))),
                sep.clone(),
            ]);
        }
        // (Pane next and focus right intentionally omitted from footer)

        // Optional Normal Mode line when Search is focused and active
        let mut lines: Vec<Line> = vec![Line::from(g_spans)];
        if matches!(app.focus, Focus::Search) {
            lines.push(Line::from(s_spans));
        }
        if matches!(app.focus, Focus::Install)
            && let Some(i_line) = right_lines_install
        {
            lines.push(i_line);
        }
        if matches!(app.focus, Focus::Install)
            && let Some((d_line, rm_line)) = right_lines_split
        {
            match app.right_pane_focus {
                RightPaneFocus::Downgrade => lines.push(d_line),
                RightPaneFocus::Remove => lines.push(rm_line),
                _ => {}
            }
        }
        if matches!(app.focus, Focus::Recent) {
            lines.push(Line::from(r_spans));
        }
        if matches!(app.focus, Focus::Search) && app.search_normal_mode {
            // Use configured labels
            let label = |v: &Vec<KeyChord>, def: &str| {
                v.first()
                    .map(|c| c.label())
                    .unwrap_or_else(|| def.to_string())
            };
            let toggle_label = label(&km.search_normal_toggle, "Esc");
            let insert_label = label(&km.search_normal_insert, "i");
            let left_label = label(&km.search_normal_select_left, "h");
            let right_label = label(&km.search_normal_select_right, "l");
            let delete_label = label(&km.search_normal_delete, "d");
            let clear_label = label(&km.search_normal_clear, "Shift+Del");

            // Store translated strings to avoid borrow checker issues
            let normal_mode_label = i18n::t(app, "app.modals.help.normal_mode.label");
            let insert_mode_text = i18n::t(app, "app.modals.help.normal_mode.insert_mode");
            let move_text = i18n::t(app, "app.modals.help.normal_mode.move");
            let page_text = i18n::t(app, "app.modals.help.normal_mode.page");
            let select_text_text = i18n::t(app, "app.modals.help.normal_mode.select_text");
            let delete_text_text = i18n::t(app, "app.modals.help.normal_mode.delete_text");
            let clear_input_text = i18n::t(app, "app.modals.help.normal_mode.clear_input");

            let n_spans: Vec<Span> = vec![
                Span::styled(
                    normal_mode_label.clone(),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(format!("[{toggle_label} / {insert_label}]"), key_style),
                Span::raw(insert_mode_text.clone()),
                Span::styled("[j / k]", key_style),
                Span::raw(move_text.clone()),
                Span::styled("[Ctrl+d / Ctrl+u]", key_style),
                Span::raw(page_text.clone()),
                Span::styled(format!("[{left_label} / {right_label}]"), key_style),
                Span::raw(select_text_text.clone()),
                Span::styled(format!("[{delete_label}]"), key_style),
                Span::raw(delete_text_text.clone()),
                Span::styled(format!("[{clear_label}]"), key_style),
                Span::raw(clear_input_text.clone()),
                // Close first line (base Normal Mode help)
            ];
            lines.push(Line::from(n_spans));

            // Second line: menus and import/export (if any)
            let mut n2_spans: Vec<Span> = Vec::new();

            // Menus: explicit entries in Normal Mode
            if !km.config_menu_toggle.is_empty()
                || !km.options_menu_toggle.is_empty()
                || !km.panels_menu_toggle.is_empty()
            {
                let open_config_list_text = i18n::t(app, "app.modals.help.normal_mode.open_config_list");
                let open_options_text = i18n::t(app, "app.modals.help.normal_mode.open_options");
                let open_panels_text = i18n::t(app, "app.modals.help.normal_mode.open_panels");
                
                if let Some(k) = km.config_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(open_config_list_text.clone()));
                }
                if let Some(k) = km.options_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(open_options_text.clone()));
                }
                if let Some(k) = km.panels_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(open_panels_text.clone()));
                }
            }

            // Import/Export shortcuts on the same second line
            if !app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
            {
                let install_list_text = i18n::t(app, "app.modals.help.normal_mode.install_list");
                let import_text = i18n::t(app, "app.modals.help.normal_mode.import");
                let export_text = i18n::t(app, "app.modals.help.normal_mode.export");
                
                n2_spans.push(Span::raw(install_list_text.clone()));
                if let Some(k) = km.search_normal_import.first() {
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(import_text.clone()));
                    if let Some(k2) = km.search_normal_export.first() {
                        n2_spans.push(Span::raw(", "));
                        n2_spans.push(Span::styled(format!("[{}]", k2.label()), key_style));
                        n2_spans.push(Span::raw(export_text.clone()));
                    }
                } else if let Some(k) = km.search_normal_export.first() {
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(export_text.clone()));
                }
            }

            if !n2_spans.is_empty() {
                lines.push(Line::from(n2_spans));
            }
        }

        // Bottom-align the content within the reserved footer area
        // Use full footer height (including buffer) for the keybind viewer
        let content_lines: u16 = h;
        let content_y = y_top;
        let content_rect = Rect {
            x,
            y: content_y,
            width: w,
            height: content_lines,
        };
        // Fill the whole reserved footer area with a uniform background
        f.render_widget(
            Block::default().style(Style::default().bg(th.base)),
            footer_rect,
        );
        let footer = Paragraph::new(lines)
            .style(Style::default().fg(th.subtext1))
            .wrap(Wrap { trim: true });
        f.render_widget(footer, content_rect);
    }
}
