use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, Focus, RightPaneFocus};
use crate::theme::{KeyChord, Theme, theme};

/// What: Determine label color based on focus state.
///
/// Inputs:
/// - `is_focused`: Whether the section is currently focused
///
/// Output:
/// - Returns mauve if focused, overlay1 otherwise.
///
/// Details:
/// - Used to highlight active sections in the footer.
const fn get_label_color(is_focused: bool, th: &Theme) -> ratatui::style::Color {
    if is_focused { th.mauve } else { th.overlay1 }
}

/// What: Add a single keybind entry to spans if key exists.
///
/// Inputs:
/// - `spans`: Mutable reference to spans vector to append to
/// - `key_opt`: Optional key chord
/// - `key_style`: Style for the key label
/// - `text`: Text to display after the key
/// - `sep_style`: Style for the separator
///
/// Output:
/// - Appends keybind entry to spans if key exists.
///
/// Details:
/// - Only adds entry if key is present, reducing conditional complexity.
fn add_keybind_entry(
    spans: &mut Vec<Span<'static>>,
    key_opt: Option<&KeyChord>,
    key_style: Style,
    text: &str,
    sep_style: Style,
) {
    if let Some(k) = key_opt {
        spans.extend([
            Span::styled(format!("[{}]", k.label()), key_style),
            Span::raw(format!(" {text}")),
            Span::styled("  |  ", sep_style),
        ]);
    }
}

/// What: Add a dual keybind entry (up/down) to spans if both keys exist.
///
/// Inputs:
/// - `spans`: Mutable reference to spans vector to append to
/// - `up_opt`: Optional up key chord
/// - `down_opt`: Optional down key chord
/// - `key_style`: Style for the key label
/// - `text`: Text to display after the keys
/// - `sep_style`: Style for the separator
///
/// Output:
/// - Appends dual keybind entry to spans if both keys exist.
///
/// Details:
/// - Only adds entry if both keys are present.
fn add_dual_keybind_entry(
    spans: &mut Vec<Span<'static>>,
    up_opt: Option<&KeyChord>,
    down_opt: Option<&KeyChord>,
    key_style: Style,
    text: &str,
    sep_style: Style,
) {
    if let (Some(up), Some(dn)) = (up_opt, down_opt) {
        spans.extend([
            Span::styled(format!("[{} / {}]", up.label(), dn.label()), key_style),
            Span::raw(format!(" {text}")),
            Span::styled("  |  ", sep_style),
        ]);
    }
}

/// What: Add multiple keybind labels joined by " / " to spans.
///
/// Inputs:
/// - `spans`: Mutable reference to spans vector to append to
/// - `keys`: Vector of key chords
/// - `key_style`: Style for the key label
/// - `text`: Text to display after the keys
/// - `sep_style`: Style for the separator
///
/// Output:
/// - Appends multi-keybind entry to spans if keys exist.
///
/// Details:
/// - Only adds entry if keys vector is not empty.
fn add_multi_keybind_entry(
    spans: &mut Vec<Span<'static>>,
    keys: &[KeyChord],
    key_style: Style,
    text: &str,
    sep_style: Style,
) {
    if !keys.is_empty() {
        let keys_str = keys
            .iter()
            .map(KeyChord::label)
            .collect::<Vec<_>>()
            .join(" / ");
        spans.extend([
            Span::styled(format!("[{keys_str}]"), key_style),
            Span::raw(format!(" {text}")),
            Span::styled("  |  ", sep_style),
        ]);
    }
}

/// What: Build section header span with label and color.
///
/// Inputs:
/// - `label`: Section label text
/// - `label_color`: Color for the label
///
/// Output:
/// - Returns vector with styled label span and spacing.
///
/// Details:
/// - Creates bold, colored label followed by spacing.
fn build_section_header(label: String, label_color: ratatui::style::Color) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            label,
            Style::default()
                .fg(label_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
    ]
}

/// What: Build GLOBALS section spans.
///
/// Inputs:
/// - `app`: Application state
/// - `th`: Theme
/// - `key_style`: Style for keys
/// - `sep_style`: Style for separator
///
/// Output:
/// - Returns vector of spans for GLOBALS section.
///
/// Details:
/// - Includes exit, help, reload theme, show pkgbuild, change sort, and normal mode toggle.
fn build_globals_section(
    app: &AppState,
    th: &Theme,
    key_style: Style,
    sep_style: Style,
) -> Vec<Span<'static>> {
    let km = &app.keymap;
    let mut spans = build_section_header(
        format!("{}  ", i18n::t(app, "app.headings.globals")),
        th.overlay1,
    );

    add_keybind_entry(
        &mut spans,
        km.exit.first(),
        key_style,
        &i18n::t(app, "app.actions.exit"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.help_overlay.first(),
        key_style,
        &i18n::t(app, "app.actions.help"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.reload_config.first(),
        key_style,
        &i18n::t(app, "app.actions.reload_config"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.show_pkgbuild.first(),
        key_style,
        &i18n::t(app, "app.actions.show_hide_pkgbuild"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.change_sort.first(),
        key_style,
        &i18n::t(app, "app.actions.change_sort_mode"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.search_normal_toggle.first(),
        key_style,
        &i18n::t(app, "app.actions.insert_mode"),
        sep_style,
    );

    spans
}

/// What: Build SEARCH section spans.
///
/// Inputs:
/// - `app`: Application state
/// - `th`: Theme
/// - `key_style`: Style for keys
/// - `sep_style`: Style for separator
///
/// Output:
/// - Returns vector of spans for SEARCH section.
///
/// Details:
/// - Includes move, page, add, and install actions.
fn build_search_section(
    app: &AppState,
    th: &Theme,
    key_style: Style,
    sep_style: Style,
) -> Vec<Span<'static>> {
    let km = &app.keymap;
    let search_label_color = get_label_color(matches!(app.focus, Focus::Search), th);
    let mut spans = build_section_header(
        format!("{}   ", i18n::t(app, "app.headings.search")),
        search_label_color,
    );

    add_dual_keybind_entry(
        &mut spans,
        km.search_move_up.first(),
        km.search_move_down.first(),
        key_style,
        &i18n::t(app, "app.actions.move"),
        sep_style,
    );
    add_dual_keybind_entry(
        &mut spans,
        km.search_page_up.first(),
        km.search_page_down.first(),
        key_style,
        &i18n::t(app, "app.actions.move_page"),
        sep_style,
    );
    let add_text = if app.installed_only_mode {
        i18n::t(app, "app.actions.add_to_remove")
    } else {
        i18n::t(app, "app.actions.add_to_install")
    };
    add_keybind_entry(
        &mut spans,
        km.search_add.first(),
        key_style,
        &add_text,
        sep_style,
    );
    if app.installed_only_mode {
        spans.extend([
            Span::styled("[Ctrl+Space]", key_style),
            Span::raw(format!(" {}", i18n::t(app, "app.actions.add_to_downgrade"))),
            Span::styled("  |  ", sep_style),
        ]);
    }
    add_keybind_entry(
        &mut spans,
        km.search_install.first(),
        key_style,
        &i18n::t(app, "app.actions.install"),
        sep_style,
    );
    // Show toggle based on search mode: toggle_normal label when fuzzy is active, toggle_fuzzy label when normal is active
    // Both use the same keybind (toggle_fuzzy) but with different labels
    if app.fuzzy_search_enabled {
        add_keybind_entry(
            &mut spans,
            km.toggle_fuzzy.first(),
            key_style,
            &i18n::t(app, "app.modals.help.key_labels.toggle_normal"),
            sep_style,
        );
    } else {
        add_keybind_entry(
            &mut spans,
            km.toggle_fuzzy.first(),
            key_style,
            &i18n::t(app, "app.modals.help.key_labels.toggle_fuzzy"),
            sep_style,
        );
    }
    // Show clear keybind based on mode: insert_clear for insert mode, normal_clear for normal mode
    if app.search_normal_mode {
        add_keybind_entry(
            &mut spans,
            km.search_normal_clear.first(),
            key_style,
            &i18n::t(app, "app.modals.help.key_labels.clear_input"),
            sep_style,
        );
    } else {
        add_keybind_entry(
            &mut spans,
            km.search_insert_clear.first(),
            key_style,
            &i18n::t(app, "app.modals.help.key_labels.clear_input"),
            sep_style,
        );
    }

    spans
}

/// What: Build common right-pane action spans (install/downgrade/remove).
///
/// Inputs:
/// - `app`: Application state
/// - `label`: Section label
/// - `label_color`: Color for label
/// - `confirm_text`: Text for confirm action
/// - `key_style`: Style for keys
/// - `sep_style`: Style for separator
///
/// Output:
/// - Returns vector of spans for right-pane section.
///
/// Details:
/// - Includes move, confirm, remove, clear, find, and go to search actions.
fn build_right_pane_spans(
    app: &AppState,
    label: String,
    label_color: ratatui::style::Color,
    confirm_text: &str,
    key_style: Style,
    sep_style: Style,
) -> Vec<Span<'static>> {
    let km = &app.keymap;
    let mut spans = build_section_header(label, label_color);

    add_dual_keybind_entry(
        &mut spans,
        km.install_move_up.first(),
        km.install_move_down.first(),
        key_style,
        &i18n::t(app, "app.actions.move"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.install_confirm.first(),
        key_style,
        confirm_text,
        sep_style,
    );
    add_multi_keybind_entry(
        &mut spans,
        &km.install_remove,
        key_style,
        &i18n::t(app, "app.actions.remove_from_list"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.install_clear.first(),
        key_style,
        &i18n::t(app, "app.actions.clear"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.install_find.first(),
        key_style,
        &i18n::t(app, "app.details.footer.search_hint"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.install_to_search.first(),
        key_style,
        &i18n::t(app, "app.actions.go_to_search"),
        sep_style,
    );

    spans
}

/// What: Build INSTALL section spans (or DOWNGRADE/REMOVE in installed-only mode).
///
/// Inputs:
/// - `app`: Application state
/// - `th`: Theme
/// - `key_style`: Style for keys
/// - `sep_style`: Style for separator
///
/// Output:
/// - Returns tuple: (install line option, split lines option for downgrade/remove).
///
/// Details:
/// - Returns split lines in installed-only mode, single install line otherwise.
fn build_install_section(
    app: &AppState,
    th: &Theme,
    key_style: Style,
    sep_style: Style,
) -> (
    Option<Line<'static>>,
    Option<(Line<'static>, Line<'static>)>,
) {
    if app.installed_only_mode {
        let downgrade_color = get_label_color(
            matches!(app.focus, Focus::Install)
                && matches!(app.right_pane_focus, RightPaneFocus::Downgrade),
            th,
        );
        let remove_color = get_label_color(
            matches!(app.focus, Focus::Install)
                && matches!(app.right_pane_focus, RightPaneFocus::Remove),
            th,
        );

        let d_spans = build_right_pane_spans(
            app,
            format!("{}:", i18n::t(app, "app.headings.downgrade")),
            downgrade_color,
            &i18n::t(app, "app.details.footer.confirm_downgrade"),
            key_style,
            sep_style,
        );
        let r_spans = build_right_pane_spans(
            app,
            format!("{}:   ", i18n::t(app, "app.headings.remove")),
            remove_color,
            &i18n::t(app, "app.details.footer.confirm_removal"),
            key_style,
            sep_style,
        );
        (None, Some((Line::from(d_spans), Line::from(r_spans))))
    } else {
        let install_color = get_label_color(matches!(app.focus, Focus::Install), th);
        let i_spans = build_right_pane_spans(
            app,
            format!("{}:  ", i18n::t(app, "app.headings.install")),
            install_color,
            &i18n::t(app, "app.details.footer.confirm_installation"),
            key_style,
            sep_style,
        );
        (Some(Line::from(i_spans)), None)
    }
}

/// What: Build RECENT section spans.
///
/// Inputs:
/// - `app`: Application state
/// - `th`: Theme
/// - `key_style`: Style for keys
/// - `sep_style`: Style for separator
///
/// Output:
/// - Returns vector of spans for RECENT section.
///
/// Details:
/// - Includes move, use, remove, clear, add, find, and go to search actions.
fn build_recent_section(
    app: &AppState,
    th: &Theme,
    key_style: Style,
    sep_style: Style,
) -> Vec<Span<'static>> {
    let km = &app.keymap;
    let recent_label_color = get_label_color(matches!(app.focus, Focus::Recent), th);
    let mut spans = build_section_header(
        format!("{}   ", i18n::t(app, "app.headings.recent")),
        recent_label_color,
    );

    add_dual_keybind_entry(
        &mut spans,
        km.recent_move_up.first(),
        km.recent_move_down.first(),
        key_style,
        &i18n::t(app, "app.actions.move"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.recent_use.first(),
        key_style,
        &i18n::t(app, "app.actions.add_to_search"),
        sep_style,
    );
    add_multi_keybind_entry(
        &mut spans,
        &km.recent_remove,
        key_style,
        &i18n::t(app, "app.actions.remove_from_list"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.recent_clear.first(),
        key_style,
        &i18n::t(app, "app.actions.clear"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.recent_add.first(),
        key_style,
        &i18n::t(app, "app.actions.add_first_match_to_install"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.recent_find.first(),
        key_style,
        &i18n::t(app, "app.actions.search_hint_enter_next_esc_cancel"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.recent_to_search.first(),
        key_style,
        &i18n::t(app, "app.actions.go_to_search"),
        sep_style,
    );

    spans
}

/// What: Build Normal Mode section spans.
///
/// Inputs:
/// - `app`: Application state
/// - `th`: Theme
/// - `key_style`: Style for keys
///
/// Output:
/// - Returns vector of lines for Normal Mode section (may be empty).
///
/// Details:
/// - Returns two lines: base normal mode help and optional menus/import-export line.
fn build_normal_mode_section(app: &AppState, th: &Theme, key_style: Style) -> Vec<Line<'static>> {
    let km = &app.keymap;
    let mut lines = Vec::new();

    let label =
        |v: &Vec<KeyChord>, def: &str| v.first().map_or_else(|| def.to_string(), KeyChord::label);
    let toggle_label = label(&km.search_normal_toggle, "Esc");
    let insert_label = label(&km.search_normal_insert, "i");
    let clear_label = label(&km.search_normal_clear, "Shift+Del");

    let normal_mode_label = i18n::t(app, "app.modals.help.normal_mode.label");
    let insert_mode_text = i18n::t(app, "app.modals.help.normal_mode.insert_mode");
    let move_text = i18n::t(app, "app.modals.help.normal_mode.move");
    let page_text = i18n::t(app, "app.modals.help.normal_mode.page");
    let clear_input_text = i18n::t(app, "app.modals.help.normal_mode.clear_input");

    let n_spans: Vec<Span> = vec![
        Span::styled(
            normal_mode_label,
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("[{toggle_label} / {insert_label}]"), key_style),
        Span::raw(insert_mode_text),
        Span::styled("[j / k]", key_style),
        Span::raw(move_text),
        Span::styled("[Ctrl+d / Ctrl+u]", key_style),
        Span::raw(page_text),
        Span::styled(format!("[{clear_label}]"), key_style),
        Span::raw(clear_input_text),
    ];
    lines.push(Line::from(n_spans));

    // Second line: menus and import/export (if any)
    let mut second_line_spans: Vec<Span> = Vec::new();

    if !km.config_menu_toggle.is_empty()
        || !km.options_menu_toggle.is_empty()
        || !km.panels_menu_toggle.is_empty()
    {
        let open_config_list_text = i18n::t(app, "app.modals.help.normal_mode.open_config_list");
        let open_options_text = i18n::t(app, "app.modals.help.normal_mode.open_options");
        let open_panels_text = i18n::t(app, "app.modals.help.normal_mode.open_panels");

        if let Some(k) = km.config_menu_toggle.first() {
            second_line_spans.push(Span::raw("  •  "));
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(open_config_list_text));
        }
        if let Some(k) = km.options_menu_toggle.first() {
            second_line_spans.push(Span::raw("  •  "));
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(open_options_text));
        }
        if let Some(k) = km.panels_menu_toggle.first() {
            second_line_spans.push(Span::raw("  •  "));
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(open_panels_text));
        }
    }

    if !app.installed_only_mode
        && (!km.search_normal_import.is_empty()
            || !km.search_normal_export.is_empty()
            || !km.search_normal_updates.is_empty())
    {
        let install_list_text = i18n::t(app, "app.modals.help.normal_mode.install_list");
        let import_text = i18n::t(app, "app.modals.help.normal_mode.import");
        let export_text = i18n::t(app, "app.modals.help.normal_mode.export");
        let updates_text = i18n::t(app, "app.modals.help.normal_mode.updates");

        second_line_spans.push(Span::raw(install_list_text));
        if let Some(k) = km.search_normal_import.first() {
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(import_text));
            if let Some(k2) = km.search_normal_export.first() {
                second_line_spans.push(Span::raw(", "));
                second_line_spans.push(Span::styled(format!("[{}]", k2.label()), key_style));
                second_line_spans.push(Span::raw(export_text));
            }
            if let Some(k3) = km.search_normal_updates.first() {
                second_line_spans.push(Span::raw(", "));
                second_line_spans.push(Span::styled(format!("[{}]", k3.label()), key_style));
                second_line_spans.push(Span::raw(updates_text));
            }
        } else if let Some(k) = km.search_normal_export.first() {
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(export_text));
            if let Some(k2) = km.search_normal_updates.first() {
                second_line_spans.push(Span::raw(", "));
                second_line_spans.push(Span::styled(format!("[{}]", k2.label()), key_style));
                second_line_spans.push(Span::raw(updates_text));
            }
        } else if let Some(k) = km.search_normal_updates.first() {
            second_line_spans.push(Span::styled(format!("[{}]", k.label()), key_style));
            second_line_spans.push(Span::raw(updates_text));
        }
    }

    if !second_line_spans.is_empty() {
        lines.push(Line::from(second_line_spans));
    }

    lines
}

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

        let key_style = Style::default().fg(th.text).add_modifier(Modifier::BOLD);
        let sep_style = Style::default().fg(th.overlay2);

        // Build all sections
        let g_spans = build_globals_section(app, &th, key_style, sep_style);
        let s_spans = build_search_section(app, &th, key_style, sep_style);
        let (right_lines_install, right_lines_split) =
            build_install_section(app, &th, key_style, sep_style);
        let r_spans = build_recent_section(app, &th, key_style, sep_style);

        // Assemble lines based on focus
        let mut lines: Vec<Line<'static>> = vec![Line::from(g_spans)];
        if matches!(app.focus, Focus::Search) {
            lines.push(Line::from(s_spans));
        }
        if matches!(app.focus, Focus::Install) {
            if let Some(i_line) = right_lines_install {
                lines.push(i_line);
            }
            if let Some((d_line, rm_line)) = right_lines_split {
                match app.right_pane_focus {
                    RightPaneFocus::Downgrade => lines.push(d_line),
                    RightPaneFocus::Remove => lines.push(rm_line),
                    RightPaneFocus::Install => {}
                }
            }
        }
        if matches!(app.focus, Focus::Recent) {
            lines.push(Line::from(r_spans));
        }
        if matches!(app.focus, Focus::Search) && app.search_normal_mode {
            lines.extend(build_normal_mode_section(app, &th, key_style));
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
