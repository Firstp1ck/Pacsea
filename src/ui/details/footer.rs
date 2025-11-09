use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

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
        let mut g_spans: Vec<Span> = vec![
            Span::styled(
                "GLOBALS:  ",
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
        ];
        if let Some(k) = km.exit.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Exit"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.help_overlay.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Help"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.reload_theme.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Reload theme"),
                sep.clone(),
            ]);
        }
        // Menu toggles are shown under Search (Normal mode) now
        if let Some(k) = km.show_pkgbuild.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Show/Hide PKGBUILD"),
                sep.clone(),
            ]);
        }
        // Change sorting (global) using configured keybind
        if let Some(k) = km.change_sort.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Change Sort Mode"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.search_normal_toggle.first() {
            g_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Insert Mode"),
                sep.clone(),
            ]);
        }
        // (Pane focus left/right intentionally omitted from footer)

        // SEARCH
        let mut s_spans: Vec<Span> = vec![
            Span::styled(
                "SEARCH:   ",
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
                Span::raw(" Move"),
                sep.clone(),
            ]);
        }
        // Page
        if let (Some(pu), Some(pd)) = (km.search_page_up.first(), km.search_page_down.first()) {
            s_spans.extend([
                Span::styled(format!("[{} / {}]", pu.label(), pd.label()), key_style),
                Span::raw(" Move Page"),
                sep.clone(),
            ]);
        }
        // Add / Install
        if let Some(k) = km.search_add.first() {
            s_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(if app.installed_only_mode {
                    " Add to Remove"
                } else {
                    " Add to install"
                }),
                sep.clone(),
            ]);
        }
        if app.installed_only_mode {
            s_spans.extend([
                Span::styled("[Ctrl+Space]", key_style),
                Span::raw(" Add to Downgrade"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.search_install.first() {
            s_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Install"),
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
                    Span::raw(" Move"),
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
                    Span::raw(" Remove from List"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_clear.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Clear"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_find.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Search (Enter next, Esc cancel)"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_to_search.first() {
                spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Go to Search"),
                    sep.clone(),
                ]);
            }
            spans
        };

        let (right_lines_install, right_lines_split) = if app.installed_only_mode {
            let d_spans = build_right_spans(
                "DOWNGRADE:",
                downgrade_label_color,
                "Confirm package Downgrade",
            );
            let r_spans =
                build_right_spans("REMOVE:   ", remove_label_color, "Confirm package Removal");
            (None, Some((Line::from(d_spans), Line::from(r_spans))))
        } else {
            let mut i_spans: Vec<Span> = vec![
                Span::styled(
                    "INSTALL:  ",
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
                    Span::raw(" Move"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_confirm.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Confirm"),
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
                    Span::raw(" Remove from List"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_clear.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Clear"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_find.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Search (Enter next, Esc cancel)"),
                    sep.clone(),
                ]);
            }
            if let Some(k) = km.install_to_search.first() {
                i_spans.extend([
                    Span::styled(format!("[{}]", k.label()), key_style),
                    Span::raw(" Go to Search"),
                    sep.clone(),
                ]);
            }
            (Some(Line::from(i_spans)), None)
        };

        // RECENT
        let mut r_spans: Vec<Span> = vec![
            Span::styled(
                "RECENT:   ",
                Style::default()
                    .fg(recent_label_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
        ];
        if let (Some(up), Some(dn)) = (km.recent_move_up.first(), km.recent_move_down.first()) {
            r_spans.extend([
                Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
                Span::raw(" Move"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_use.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Add to Search"),
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
                Span::raw(" Remove from List"),
                sep.clone(),
            ]);
        }
        // Clear all entries in Recent: configurable keybind (fallback to Shift+Del label)
        if let Some(k) = km.recent_clear.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Clear"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_add.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Add first match to Install list"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_find.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Search (Enter next, Esc cancel)"),
                sep.clone(),
            ]);
        }
        if let Some(k) = km.recent_to_search.first() {
            r_spans.extend([
                Span::styled(format!("[{}]", k.label()), key_style),
                Span::raw(" Go to Search"),
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

            let n_spans: Vec<Span> = vec![
                Span::styled(
                    "Normal Mode:",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(format!("[{toggle_label} / {insert_label}]"), key_style),
                Span::raw(" Insert Mode, "),
                Span::styled("[j / k]", key_style),
                Span::raw(" move, "),
                Span::styled("[Ctrl+d / Ctrl+u]", key_style),
                Span::raw(" page, "),
                Span::styled(format!("[{left_label} / {right_label}]"), key_style),
                Span::raw(" Select text, "),
                Span::styled(format!("[{delete_label}]"), key_style),
                Span::raw(" Delete text, "),
                Span::styled(format!("[{clear_label}]"), key_style),
                Span::raw(" Clear input"),
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
                if let Some(k) = km.config_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(" Open Config/List"));
                }
                if let Some(k) = km.options_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(" Open Options"));
                }
                if let Some(k) = km.panels_menu_toggle.first() {
                    n2_spans.push(Span::raw("  •  "));
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(" Open Panels"));
                }
            }

            // Import/Export shortcuts on the same second line
            if !app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
            {
                n2_spans.push(Span::raw("  • Install List:  "));
                if let Some(k) = km.search_normal_import.first() {
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(" Import"));
                    if let Some(k2) = km.search_normal_export.first() {
                        n2_spans.push(Span::raw(", "));
                        n2_spans.push(Span::styled(format!("[{}]", k2.label()), key_style));
                        n2_spans.push(Span::raw(" Export"));
                    }
                } else if let Some(k) = km.search_normal_export.first() {
                    n2_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
                    n2_spans.push(Span::raw(" Export"));
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
