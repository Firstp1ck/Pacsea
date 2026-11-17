use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, ConfigEditorState, EditPopupKind, Focus, RightPaneFocus};
use crate::theme::{EditableSetting, KeyChord, Theme, theme};

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

/// What: Build the GLOBALS footer lines.
///
/// Inputs:
/// - `app`: Application state.
/// - `th`: Theme.
/// - `key_style`: Style for keys.
/// - `sep_style`: Style for separators.
///
/// Output:
/// - Returns the general and Shift-shortcut lines for the GLOBALS section.
///
/// Details:
/// - Keeps application-wide Shift actions out of the Search Normal Mode section.
fn build_globals_section(
    app: &AppState,
    th: &Theme,
    key_style: Style,
    sep_style: Style,
) -> Vec<Line<'static>> {
    let km = &app.keymap;
    let heading = || {
        build_section_header(
            format!("{}  ", i18n::t(app, "app.headings.globals")),
            th.overlay1,
        )
    };
    let mut spans = heading();

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
        km.comments_toggle.first(),
        key_style,
        &i18n::t(app, "app.actions.show_hide_comments"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.run_pkgbuild_checks.first(),
        key_style,
        &i18n::t(app, "app.actions.run_pkgbuild_checks"),
        sep_style,
    );
    add_keybind_entry(
        &mut spans,
        km.cycle_pkgbuild_sections.first(),
        key_style,
        &i18n::t(app, "app.actions.cycle_pkgbuild_sections"),
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

    let mut shift_spans = heading();
    for (key, label_key) in [
        (
            km.config_menu_toggle.first(),
            "app.modals.help.key_labels.config_lists_menu",
        ),
        (
            km.options_menu_toggle.first(),
            "app.modals.help.key_labels.options_menu",
        ),
        (
            km.panels_menu_toggle.first(),
            "app.modals.help.key_labels.panels_menu",
        ),
        (
            km.search_normal_open_status.first(),
            "app.modals.help.key_labels.open_arch_status",
        ),
        (
            km.search_normal_import.first(),
            "app.modals.help.normal_mode.import",
        ),
        (
            km.search_normal_export.first(),
            "app.modals.help.normal_mode.export",
        ),
        (
            km.search_normal_updates.first(),
            "app.modals.help.normal_mode.updates",
        ),
        (
            km.search_normal_pi_scan.first(),
            "app.modals.help.key_labels.open_pi_scan",
        ),
    ] {
        add_keybind_entry(
            &mut shift_spans,
            key,
            key_style,
            i18n::t(app, label_key).trim(),
            sep_style,
        );
    }

    vec![Line::from(spans), Line::from(shift_spans)]
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
    spans.extend([
        Span::styled("[Ctrl+E]", key_style),
        Span::raw(" Toggle AUR vote for selected package"),
        Span::styled("  |  ", sep_style),
    ]);

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
            format!("{}  ", i18n::t(app, "app.headings.install")),
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
/// - Returns one line containing only actions that depend on Search Normal Mode.
fn build_normal_mode_section(app: &AppState, th: &Theme, key_style: Style) -> Vec<Line<'static>> {
    let km = &app.keymap;
    let label =
        |v: &Vec<KeyChord>, def: &str| v.first().map_or_else(|| def.to_string(), KeyChord::label);
    let toggle_label = label(&km.search_normal_toggle, "Esc");
    let insert_label = label(&km.search_normal_insert, "i");
    let clear_label = label(&km.search_normal_clear, "Shift+Del");

    let n_spans: Vec<Span> = vec![
        Span::styled(
            i18n::t(app, "app.modals.help.normal_mode.label"),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(format!("[{toggle_label} / {insert_label}]"), key_style),
        Span::raw(i18n::t(app, "app.modals.help.normal_mode.insert_mode")),
        Span::styled("[j / k]", key_style),
        Span::raw(i18n::t(app, "app.modals.help.normal_mode.move")),
        Span::styled("[Ctrl+d / Ctrl+u]", key_style),
        Span::raw(i18n::t(app, "app.modals.help.normal_mode.page")),
        Span::styled(format!("[{clear_label}]"), key_style),
        Span::raw(i18n::t(app, "app.modals.help.normal_mode.clear_input")),
    ];

    vec![Line::from(n_spans)]
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

        let key_style = Style::default()
            .fg(th.text)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD);
        let sep_style = Style::default().fg(th.overlay2);

        // Build all sections
        let global_lines = build_globals_section(app, &th, key_style, sep_style);
        let s_spans = build_search_section(app, &th, key_style, sep_style);
        let (right_lines_install, right_lines_split) =
            build_install_section(app, &th, key_style, sep_style);
        let r_spans = build_recent_section(app, &th, key_style, sep_style);

        // Assemble lines based on focus
        let mut lines: Vec<Line<'static>> = global_lines;
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

/// What: Build the lines for the news keybind footer (global, navigation, menus, news actions, optional normal mode).
///
/// Inputs:
/// - `app`: Application state providing keymap, focus, and mode flags
/// - `th`: Theme for styling
///
/// Output:
/// - Vector of lines ready to render in the news footer
///
/// Details:
/// - Always includes Global/Nav/Menus/News actions on the first line
/// - Adds Normal Mode helper lines when search is focused and normal mode is active
fn build_news_footer_lines(app: &AppState, th: &Theme) -> Vec<Line<'static>> {
    let key_style = Style::default()
        .fg(th.text)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);
    let sep_style = Style::default().fg(th.overlay2);

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Line 1: Global/help
    let mut global_spans: Vec<Span<'static>> = Vec::new();
    global_spans.extend(build_section_header("Global".to_string(), th.overlay1));
    add_keybind_entry(
        &mut global_spans,
        app.keymap.help_overlay.first(),
        key_style,
        "Help",
        sep_style,
    );
    add_keybind_entry(
        &mut global_spans,
        app.keymap.exit.first(),
        key_style,
        "Exit",
        sep_style,
    );
    lines.push(Line::from(global_spans));

    // Line 2: Navigation and Menus
    let mut nav_menus_spans: Vec<Span<'static>> = Vec::new();
    nav_menus_spans.extend(build_section_header("Nav".to_string(), th.overlay1));
    add_dual_keybind_entry(
        &mut nav_menus_spans,
        app.keymap.search_move_up.first(),
        app.keymap.search_move_down.first(),
        key_style,
        "Move",
        sep_style,
    );
    add_dual_keybind_entry(
        &mut nav_menus_spans,
        app.keymap.search_page_up.first(),
        app.keymap.search_page_down.first(),
        key_style,
        "Page",
        sep_style,
    );
    nav_menus_spans.extend(build_section_header("Menus".to_string(), th.overlay1));
    add_keybind_entry(
        &mut nav_menus_spans,
        app.keymap.options_menu_toggle.first(),
        key_style,
        "Options",
        sep_style,
    );
    add_keybind_entry(
        &mut nav_menus_spans,
        app.keymap.panels_menu_toggle.first(),
        key_style,
        "Panels",
        sep_style,
    );
    add_keybind_entry(
        &mut nav_menus_spans,
        app.keymap.config_menu_toggle.first(),
        key_style,
        "Config/Lists",
        sep_style,
    );
    lines.push(Line::from(nav_menus_spans));

    // Line 3: News actions
    let mut news_spans: Vec<Span<'static>> = Vec::new();
    news_spans.extend(build_section_header("News".to_string(), th.overlay1));
    add_multi_keybind_entry(
        &mut news_spans,
        &app.keymap.news_mark_read_feed,
        key_style,
        "Mark read",
        sep_style,
    );
    add_multi_keybind_entry(
        &mut news_spans,
        &app.keymap.news_mark_unread_feed,
        key_style,
        "Mark unread",
        sep_style,
    );
    add_multi_keybind_entry(
        &mut news_spans,
        &app.keymap.news_toggle_read_feed,
        key_style,
        "Toggle read",
        sep_style,
    );
    lines.push(Line::from(news_spans));

    // Normal mode specific help when search pane is focused
    if matches!(app.focus, Focus::Search) {
        if app.search_normal_mode {
            lines.extend(build_normal_mode_section(app, th, key_style));
        } else {
            let label = |v: &Vec<KeyChord>, def: &str| {
                v.first().map_or_else(|| def.to_string(), KeyChord::label)
            };
            let toggle_label = label(&app.keymap.search_normal_toggle, "Esc");
            let clear_label = label(&app.keymap.search_insert_clear, "Shift+Del");
            let fuzzy_label = label(&app.keymap.toggle_fuzzy, "Ctrl+f");
            let change_sort_label = label(&app.keymap.change_sort, "Shift+Tab");

            let mut insert_spans = build_section_header(
                i18n::t(app, "app.modals.help.normal_mode.insert_mode"),
                th.mauve,
            );
            insert_spans.push(Span::styled(format!("[{toggle_label}]"), key_style));
            insert_spans.push(Span::raw(format!(
                " {}",
                i18n::t(app, "app.modals.help.key_labels.toggle_normal")
            )));
            insert_spans.push(Span::styled("  |  ", sep_style));

            insert_spans.push(Span::styled(format!("[{clear_label}]"), key_style));
            insert_spans.push(Span::raw(format!(
                " {}",
                i18n::t(app, "app.modals.help.key_labels.clear_input")
            )));
            insert_spans.push(Span::styled("  |  ", sep_style));

            insert_spans.push(Span::styled(format!("[{fuzzy_label}]"), key_style));
            insert_spans.push(Span::raw(format!(
                " {}",
                i18n::t(app, "app.modals.help.key_labels.toggle_fuzzy")
            )));
            insert_spans.push(Span::styled("  |  ", sep_style));

            insert_spans.push(Span::styled(format!("[{change_sort_label}]"), key_style));
            insert_spans.push(Span::raw(format!(
                " {}",
                i18n::t(app, "app.actions.change_sort_mode")
            )));

            lines.push(Line::from(insert_spans));
        }
    }

    lines
}

/// What: Render a simplified keybind footer for news management mode.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (keymap, focus)
/// - `area`: Full area of the news details pane
/// - `footer_height`: Reserved footer height
///
/// Output:
/// - Draws news-relevant keybinds (navigation, menus, mark read) along the bottom.
pub fn render_news_footer(f: &mut Frame, app: &AppState, area: Rect, footer_height: u16) {
    if footer_height == 0 || area.height < footer_height {
        return;
    }
    let th = theme();
    let lines = build_news_footer_lines(app, &th);
    if lines.is_empty() {
        return;
    }

    let footer_rect = Rect {
        x: area.x + 1, // inside border
        y: area
            .y
            .saturating_add(area.height.saturating_sub(footer_height)),
        width: area.width.saturating_sub(2),
        height: footer_height,
    };
    // Fill the whole reserved footer area with a uniform background
    f.render_widget(
        Block::default().style(Style::default().bg(th.base)),
        Rect {
            x: area.x,
            y: footer_rect.y,
            width: area.width,
            height: footer_height,
        },
    );
    let paragraph = Paragraph::new(lines)
        .style(Style::default().fg(th.subtext1))
        .wrap(Wrap { trim: true });
    f.render_widget(paragraph, footer_rect);
}

/// What: Compute the preferred height (in rows) for the news keybind footer.
///
/// Inputs:
/// - `app`: Application state (determines whether normal mode lines are included)
///
/// Output:
/// - Height in rows needed to render the footer without truncation (minimum 1 when enabled)
pub fn news_footer_height(app: &AppState) -> u16 {
    let th = theme();
    let line_count = build_news_footer_lines(app, &th).len();
    u16::try_from(line_count.max(1)).unwrap_or(1)
}

/// One packable footer item for the config editor.
enum EditorFooterItem {
    /// Bold section heading that starts a group (e.g. `Globals`).
    Heading(String),
    /// Bracketed key label followed by its description.
    Entry {
        /// Text inside the `[` `]` brackets.
        key: String,
        /// Description shown after the key.
        text: String,
    },
}

impl EditorFooterItem {
    /// Build an entry with a literal key label handled directly by the editor.
    fn literal(key: &str, app: &AppState, text_key: &str) -> Self {
        Self::Entry {
            key: key.to_string(),
            text: i18n::t(app, text_key),
        }
    }

    /// Build an entry from a configured chord.
    fn chord(chord: &KeyChord, app: &AppState, text_key: &str) -> Self {
        Self::Entry {
            key: chord.label(),
            text: i18n::t(app, text_key),
        }
    }

    /// Spans and display width of this item.
    fn spans(&self, th: &Theme, key_style: Style) -> (Vec<Span<'static>>, usize) {
        let spans = match self {
            Self::Heading(label) => vec![Span::styled(
                label.clone(),
                Style::default()
                    .fg(th.overlay1)
                    .add_modifier(Modifier::BOLD),
            )],
            Self::Entry { key, text } => vec![
                Span::styled(format!("[{key}]"), key_style),
                Span::raw(format!(" {text}")),
            ],
        };
        let width = spans.iter().map(Span::width).sum();
        (spans, width)
    }
}

/// Separator drawn between two entries on the same footer row.
const EDITOR_FOOTER_SEPARATOR: &str = "  |  ";

/// What: Greedily pack footer items into rows no wider than `width`.
///
/// Inputs:
/// - `items`: Headings and entries in display order.
/// - `width`: Available columns; `0` disables packing (one row).
/// - `th`: Theme for headings and separators.
/// - `key_style`: Style for bracketed keys.
///
/// Output:
/// - Rows that never exceed `width` unless one item alone is wider.
///
/// Details:
/// - The renderer draws these rows without wrapping, so the row count is exactly the
///   footer height reported by [`config_editor_footer_reserved_rows`].
fn pack_editor_footer(
    items: &[EditorFooterItem],
    width: u16,
    th: &Theme,
    key_style: Style,
) -> Vec<Line<'static>> {
    let limit = if width == 0 {
        usize::MAX
    } else {
        usize::from(width)
    };
    let sep_style = Style::default().fg(th.overlay2);
    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut used = 0usize;
    let mut previous_was_entry = false;
    for item in items {
        let (spans, item_width) = item.spans(th, key_style);
        let gap = match (used, item, previous_was_entry) {
            (0, _, _) => "",
            (_, EditorFooterItem::Entry { .. }, true) => EDITOR_FOOTER_SEPARATOR,
            _ => "  ",
        };
        let gap_width = gap.chars().count();
        if used > 0 && used + gap_width + item_width > limit {
            rows.push(Vec::new());
            used = 0;
        } else if !gap.is_empty() {
            let style = if gap == EDITOR_FOOTER_SEPARATOR {
                sep_style
            } else {
                Style::default()
            };
            if let Some(row) = rows.last_mut() {
                row.push(Span::styled(gap, style));
            }
            used += gap_width;
        }
        if let Some(row) = rows.last_mut() {
            row.extend(spans);
        }
        used += item_width;
        previous_was_entry = matches!(item, EditorFooterItem::Entry { .. });
    }
    rows.into_iter()
        .filter(|row| !row.is_empty())
        .map(Line::from)
        .collect()
}

/// What: Footer items for the global shortcuts and top-row menu toggles.
///
/// Inputs:
/// - `app`: Application state (keymap + locale).
///
/// Output:
/// - Globals heading with exit/help, then the Menus heading with the three menu toggles.
///
/// Details:
/// - Skips `reload_config` because that binding is intentionally disabled while the integrated
///   editor is active.
fn config_editor_global_items(app: &AppState) -> Vec<EditorFooterItem> {
    let km = &app.keymap;
    let mut items = vec![EditorFooterItem::Heading(i18n::t(
        app,
        "app.headings.globals",
    ))];
    for (chord, text_key) in [
        (km.exit.first(), "app.actions.exit"),
        (km.help_overlay.first(), "app.actions.help"),
    ] {
        items.extend(chord.map(|chord| EditorFooterItem::chord(chord, app, text_key)));
    }
    items.push(EditorFooterItem::Heading(i18n::t(
        app,
        "app.modals.config_editor.footer.heading_menus",
    )));
    for (chord, text_key) in [
        (
            km.config_menu_toggle.first(),
            "app.modals.config_editor.footer.menu_config",
        ),
        (
            km.options_menu_toggle.first(),
            "app.modals.config_editor.footer.menu_options",
        ),
        (
            km.panels_menu_toggle.first(),
            "app.modals.config_editor.footer.menu_panels",
        ),
    ] {
        items.extend(chord.map(|chord| EditorFooterItem::chord(chord, app, text_key)));
    }
    items
}

/// What: Footer items for the key list (no popup, no recent-search dropdown).
///
/// Inputs:
/// - `app`: Application state (keymap + locale).
/// - `state`: Editor state deciding the Esc label.
/// - `selected`: Selected key, deciding the Pi Scan setup entry.
///
/// Output:
/// - Editor heading followed by the list shortcuts.
///
/// Details:
/// - Literal labels mirror `events::modals::config_editor`; the Pi Scan setup entry uses the
///   configured `config_editor_pi_scan_setup` chord and appears only on `pi_scan_*` rows.
fn config_editor_list_items(
    app: &AppState,
    state: &ConfigEditorState,
    selected: Option<&EditableSetting>,
) -> Vec<EditorFooterItem> {
    let esc_key = if state.query.is_empty() {
        "app.modals.config_editor.footer.esc_leave"
    } else {
        "app.modals.config_editor.footer.esc_clear"
    };
    let mut items = vec![
        EditorFooterItem::Heading(i18n::t(
            app,
            "app.modals.config_editor.footer.heading_editor",
        )),
        EditorFooterItem::literal(
            "↑↓ PgUp/PgDn",
            app,
            "app.modals.config_editor.footer.move_key",
        ),
        EditorFooterItem::literal("Enter", app, "app.modals.config_editor.footer.edit_key"),
        EditorFooterItem::literal("Tab ← →", app, "app.modals.config_editor.footer.switch_tab"),
        EditorFooterItem::literal("Ctrl+B", app, "app.modals.config_editor.footer.star"),
        EditorFooterItem::literal("Ctrl+R", app, "app.modals.config_editor.footer.recent"),
        EditorFooterItem::literal("Ctrl+E", app, "app.modals.config_editor.footer.export"),
        EditorFooterItem::literal("Esc", app, esc_key),
    ];
    if selected.is_some_and(|setting| setting.key.starts_with("pi_scan_")) {
        items.extend(app.keymap.config_editor_pi_scan_setup.first().map(|chord| {
            EditorFooterItem::chord(chord, app, "app.modals.config_editor.footer.pi_scan_setup")
        }));
    }
    items
}

/// What: Footer items while the recent-search dropdown is open.
///
/// Inputs:
/// - `app`: Application state (locale).
///
/// Output:
/// - Editor heading plus choose/apply/close entries.
///
/// Details:
/// - The dropdown consumes every key, so list shortcuts are hidden meanwhile.
fn config_editor_recent_items(app: &AppState) -> Vec<EditorFooterItem> {
    vec![
        EditorFooterItem::Heading(i18n::t(
            app,
            "app.modals.config_editor.footer.heading_recent",
        )),
        EditorFooterItem::literal("↑↓", app, "app.modals.config_editor.footer.recent_choose"),
        EditorFooterItem::literal("Enter", app, "app.modals.config_editor.footer.recent_apply"),
        EditorFooterItem::literal("Esc", app, "app.modals.config_editor.footer.recent_close"),
    ]
}

/// What: Footer items for an open edit popup.
///
/// Inputs:
/// - `app`: Application state (locale).
/// - `popup`: Active popup whose kind selects the extra entries.
///
/// Output:
/// - Popup heading with save/cancel/complete/pick/reset plus kind-specific entries.
///
/// Details:
/// - While a key-chord popup records, only Esc is listed because every other key is captured.
fn config_editor_popup_items(
    app: &AppState,
    popup: &crate::state::EditPopupState,
) -> Vec<EditorFooterItem> {
    let heading = |key: &str| EditorFooterItem::Heading(i18n::t(app, key));
    if matches!(popup.kind, EditPopupKind::KeyChord { capturing: true }) {
        return vec![
            heading("app.modals.config_editor.footer.heading_recording"),
            EditorFooterItem::literal(
                "Esc",
                app,
                "app.modals.config_editor.footer.popup_capture_cancel",
            ),
        ];
    }
    let complete_key = if popup.is_strict() {
        "app.modals.config_editor.footer.popup_next_choice"
    } else {
        "app.modals.config_editor.footer.popup_complete"
    };
    let mut items = vec![
        heading("app.modals.config_editor.footer.heading_popup"),
        EditorFooterItem::literal(
            "Enter/Ctrl+S",
            app,
            "app.modals.config_editor.footer.popup_save",
        ),
        EditorFooterItem::literal("Esc", app, "app.modals.config_editor.footer.popup_cancel"),
        EditorFooterItem::literal("Tab", app, complete_key),
        EditorFooterItem::literal("↑↓", app, "app.modals.config_editor.footer.popup_pick"),
        EditorFooterItem::literal("Ctrl+Z", app, "app.modals.config_editor.footer.popup_reset"),
    ];
    match popup.kind {
        EditPopupKind::Secret { .. } => items.push(EditorFooterItem::literal(
            "Ctrl+R",
            app,
            "app.modals.config_editor.footer.popup_reveal",
        )),
        EditPopupKind::KeyChord { .. } => items.push(EditorFooterItem::literal(
            "Ctrl+K",
            app,
            "app.modals.config_editor.footer.popup_record",
        )),
        EditPopupKind::Int { .. } => items.push(EditorFooterItem::literal(
            "+/-",
            app,
            "app.modals.config_editor.footer.popup_adjust",
        )),
        EditPopupKind::Bool(_) => items.push(EditorFooterItem::literal(
            "Space",
            app,
            "app.modals.config_editor.footer.popup_toggle",
        )),
        EditPopupKind::Enum { .. } | EditPopupKind::Text => {}
    }
    items
}

/// What: Build integrated config editor footer hint lines packed to the footer width.
///
/// Inputs:
/// - `app`: Application state (keymap + locale).
/// - `state`: Config editor state (popup, recent dropdown, query).
/// - `selected`: Selected key, passed in so callers reuse their `filtered_keys()` result.
/// - `th`: Theme for colors.
/// - `width`: Footer width in columns; `0` keeps each group on one row.
///
/// Output:
/// - Globals/menus rows followed by the rows for the active editor context.
///
/// Details:
/// - Rows are pre-packed so the renderer can draw them without wrapping; the row count is the
///   footer height used for layout and toast placement.
pub fn build_config_editor_footer_hint_lines(
    app: &AppState,
    state: &ConfigEditorState,
    selected: Option<&EditableSetting>,
    th: &Theme,
    width: u16,
) -> Vec<Line<'static>> {
    let key_style = Style::default()
        .fg(th.text)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);
    let context_items = state.popup.as_ref().map_or_else(
        || {
            if state.recent_dropdown.is_some() {
                config_editor_recent_items(app)
            } else {
                config_editor_list_items(app, state, selected)
            }
        },
        |popup| config_editor_popup_items(app, popup),
    );
    let mut lines = pack_editor_footer(&config_editor_global_items(app), width, th, key_style);
    lines.extend(pack_editor_footer(&context_items, width, th, key_style));
    lines
}

/// What: Compute how many rows the integrated config editor reserves for its footer band.
///
/// Inputs:
/// - `show_keybinds_footer`: Whether contextual hint lines are visible.
/// - `app`: Application state (for hint line assembly).
/// - `state`: Editor state (status text, popup, dropdown).
/// - `selected`: Selected key (adds the Pi Scan setup entry on `pi_scan_*` rows).
/// - `width`: Footer width in columns used for packing (`0` = unlimited).
///
/// Output:
/// - Total footer height in rows (`0` when nothing to show).
///
/// Details:
/// - Sums the packed hint rows with an optional status row so transient save/bookmark messages
///   stay visible; must match what the renderer draws.
pub fn config_editor_footer_reserved_rows(
    show_keybinds_footer: bool,
    app: &AppState,
    state: &ConfigEditorState,
    selected: Option<&EditableSetting>,
    width: u16,
) -> u16 {
    let mut rows: u16 = 0;
    if show_keybinds_footer {
        let th = theme();
        let hint_lines = build_config_editor_footer_hint_lines(app, state, selected, &th, width);
        rows = rows.saturating_add(u16::try_from(hint_lines.len()).unwrap_or(u16::MAX));
    }
    if state
        .status
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        rows = rows.saturating_add(1);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Verify global Shift shortcuts appear only in the Globals footer section.
    ///
    /// Inputs:
    /// - Default application state and theme.
    ///
    /// Output:
    /// - Globals contains the Pi Scan and menu chords while Normal Mode does not.
    ///
    /// Details:
    /// - Keeps the footer categories aligned with the shortcuts' application-wide routing.
    #[test]
    fn global_shift_shortcuts_appear_in_globals_footer() {
        let app = AppState::default();
        let globals = build_globals_section(&app, &theme(), Style::default(), Style::default())
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let normal = build_normal_mode_section(&app, &theme(), Style::default())
            .iter()
            .flat_map(|line| line.spans.iter())
            .map(|span| span.content.as_ref())
            .collect::<String>();

        for chord in [
            "Shift+A", "Shift+C", "Shift+E", "Shift+I", "Shift+O", "Shift+P", "Shift+S", "Shift+U",
        ] {
            assert!(globals.contains(chord), "Globals omitted {chord}");
            assert!(
                !normal.contains(chord),
                "Normal Mode still contains {chord}"
            );
        }
    }

    /// Config-editor Pi Scan setup footer must display the configured chord label.
    #[test]
    fn config_editor_footer_uses_configured_pi_scan_setup_label() {
        let mut app = AppState::default();
        app.keymap.config_editor_pi_scan_setup = vec![KeyChord {
            code: crossterm::event::KeyCode::Char('p'),
            mods: crossterm::event::KeyModifiers::ALT,
        }];
        let mut state = ConfigEditorState {
            selected_file: crate::theme::ConfigFile::Settings,
            view: crate::state::ConfigEditorView::KeyList,
            query: "pi_scan_binary".to_string(),
            ..ConfigEditorState::default()
        };
        state.clamp_key_cursor();

        let text =
            build_config_editor_footer_hint_lines(&app, &state, state.selected_key(), &theme(), 0)
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref())
                .collect::<String>();

        assert!(text.contains("[Alt+P]"));
        assert!(!text.contains("[Ctrl+G]"));
    }

    /// Packed config-editor footer rows never exceed the footer width.
    #[test]
    fn config_editor_footer_rows_fit_width_and_match_reserved_rows() {
        let mut app = AppState::default();
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
        let translations = i18n::load_locale_file("en-US", &locales).expect("English locale");
        app.translations.clone_from(&translations);
        app.translations_fallback = translations;
        let state = ConfigEditorState::default();
        for width in [40u16, 80, 120, 200] {
            let lines = build_config_editor_footer_hint_lines(&app, &state, None, &theme(), width);
            assert!(
                lines.iter().all(|line| line.width() <= usize::from(width)),
                "a footer row overflowed {width} columns"
            );
            assert_eq!(
                config_editor_footer_reserved_rows(true, &app, &state, None, width),
                u16::try_from(lines.len()).expect("row count")
            );
        }
        assert_eq!(
            build_config_editor_footer_hint_lines(&app, &state, None, &theme(), 0).len(),
            2
        );
    }

    /// Recording shows only the stop-recording entry in the editor section.
    #[test]
    fn config_editor_footer_recording_lists_only_esc() {
        let app = AppState::default();
        let setting = crate::theme::find_setting("keybind_help").expect("keybind schema");
        let mut popup = crate::state::EditPopupState::from_current(setting, "F1");
        popup.kind = EditPopupKind::KeyChord { capturing: true };
        let state = ConfigEditorState {
            popup: Some(popup),
            ..ConfigEditorState::default()
        };
        let lines = build_config_editor_footer_hint_lines(&app, &state, None, &theme(), 0);
        let editor_row = lines
            .last()
            .expect("editor row")
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        assert!(editor_row.contains("[Esc]"));
        assert!(!editor_row.contains("Ctrl+S"));
        assert!(!editor_row.contains("Ctrl+K"));
    }
}
