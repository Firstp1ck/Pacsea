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

/// What: Parse YAML array lines from i18n strings into help lines.
///
/// Inputs:
/// - `yaml_text`: YAML array string from i18n
///
/// Output:
/// - Vector of Lines extracted from YAML format
///
/// Details:
/// - Handles quoted and unquoted YAML list items, stripping prefixes and quotes.
fn parse_yaml_lines(yaml_text: &str) -> Vec<Line<'static>> {
    let mut result = Vec::new();
    for line in yaml_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("- \"") || trimmed.starts_with("- '") {
            let content = trimmed
                .strip_prefix("- \"")
                .or_else(|| trimmed.strip_prefix("- '"))
                .and_then(|s| s.strip_suffix('"').or_else(|| s.strip_suffix('\'')))
                .unwrap_or(trimmed);
            result.push(Line::from(Span::raw(content.to_string())));
        } else if trimmed.starts_with("- ") {
            result.push(Line::from(Span::raw(
                trimmed.strip_prefix("- ").unwrap_or(trimmed).to_string(),
            )));
        }
    }
    result
}

/// What: Add a section header to the lines vector.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state for i18n
/// - `th`: Theme reference
/// - `key`: i18n key for section title
///
/// Output:
/// - Adds empty line and styled section header to lines
///
/// Details:
/// - Formats section headers with consistent styling.
fn add_section_header(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    key: &str,
) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, key),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
}

/// What: Conditionally add a binding line if the key exists.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state for i18n
/// - `th`: Theme reference
/// - `key_opt`: Optional keybinding
/// - `label_key`: i18n key for label
///
/// Output:
/// - Adds formatted binding line if key exists
///
/// Details:
/// - Uses fmt closure to format binding consistently.
fn add_binding_if_some(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    key_opt: Option<KeyChord>,
    label_key: &str,
) {
    if let Some(k) = key_opt {
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
        lines.push(fmt(&i18n::t(app, label_key), k));
    }
}

/// What: Build global keybindings section.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state
/// - `th`: Theme reference
/// - `km`: Keymap reference
///
/// Output:
/// - Adds global bindings to lines
///
/// Details:
/// - Formats all global application keybindings.
fn build_global_bindings(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    km: &crate::theme::KeyMap,
) {
    add_binding_if_some(
        lines,
        app,
        th,
        km.help_overlay.first().copied(),
        "app.modals.help.key_labels.help_overlay",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.exit.first().copied(),
        "app.modals.help.key_labels.exit",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.reload_config.first().copied(),
        "app.modals.help.key_labels.reload_config",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.pane_next.first().copied(),
        "app.modals.help.key_labels.next_pane",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.pane_left.first().copied(),
        "app.modals.help.key_labels.focus_left",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.pane_right.first().copied(),
        "app.modals.help.key_labels.focus_right",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.show_pkgbuild.first().copied(),
        "app.modals.help.key_labels.show_pkgbuild",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.change_sort.first().copied(),
        "app.modals.help.key_labels.change_sorting",
    );
}

/// What: Build search pane keybindings section.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state
/// - `th`: Theme reference
/// - `km`: Keymap reference
///
/// Output:
/// - Adds search bindings to lines
///
/// Details:
/// - Formats search pane navigation and action keybindings.
fn build_search_bindings(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    km: &crate::theme::KeyMap,
) {
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
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_add.first().copied(),
        "app.modals.help.key_labels.add",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_install.first().copied(),
        "app.modals.help.key_labels.install",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_backspace.first().copied(),
        "app.modals.help.key_labels.delete",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_insert_clear.first().copied(),
        "app.modals.help.key_labels.clear_input",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.toggle_fuzzy.first().copied(),
        "app.modals.help.key_labels.toggle_fuzzy",
    );
}

/// What: Build search normal mode keybindings section.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state
/// - `th`: Theme reference
/// - `km`: Keymap reference
///
/// Output:
/// - Adds search normal mode bindings to lines if any exist
///
/// Details:
/// - Only renders section if at least one normal mode binding is configured.
fn build_search_normal_bindings(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    km: &crate::theme::KeyMap,
) {
    let has_normal_bindings = !km.search_normal_toggle.is_empty()
        || !km.search_normal_insert.is_empty()
        || !km.search_normal_select_left.is_empty()
        || !km.search_normal_select_right.is_empty()
        || !km.search_normal_delete.is_empty()
        || !km.search_normal_clear.is_empty()
        || !km.search_normal_open_status.is_empty()
        || !km.config_menu_toggle.is_empty()
        || !km.options_menu_toggle.is_empty()
        || !km.panels_menu_toggle.is_empty();

    if !has_normal_bindings {
        return;
    }

    add_section_header(lines, app, th, "app.modals.help.sections.search_normal");

    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_toggle.first().copied(),
        "app.modals.help.key_labels.toggle_normal",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_insert.first().copied(),
        "app.modals.help.key_labels.insert_mode",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_select_left.first().copied(),
        "app.modals.help.key_labels.select_left",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_select_right.first().copied(),
        "app.modals.help.key_labels.select_right",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_delete.first().copied(),
        "app.modals.help.key_labels.delete",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_clear.first().copied(),
        "app.modals.help.key_labels.clear_input",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.search_normal_open_status.first().copied(),
        "app.modals.help.key_labels.open_arch_status",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.config_menu_toggle.first().copied(),
        "app.modals.help.key_labels.config_lists_menu",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.options_menu_toggle.first().copied(),
        "app.modals.help.key_labels.options_menu",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.panels_menu_toggle.first().copied(),
        "app.modals.help.key_labels.panels_menu",
    );
}

/// What: Build install pane keybindings section.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state
/// - `th`: Theme reference
/// - `km`: Keymap reference
///
/// Output:
/// - Adds install bindings to lines
///
/// Details:
/// - Formats install pane navigation and action keybindings.
fn build_install_bindings(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    km: &crate::theme::KeyMap,
) {
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
    add_binding_if_some(
        lines,
        app,
        th,
        km.install_confirm.first().copied(),
        "app.modals.help.key_labels.confirm",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.install_remove.first().copied(),
        "app.modals.help.key_labels.remove",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.install_clear.first().copied(),
        "app.modals.help.key_labels.clear",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.install_find.first().copied(),
        "app.modals.help.key_labels.find",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.install_to_search.first().copied(),
        "app.modals.help.key_labels.to_search",
    );
}

/// What: Build recent pane keybindings section.
///
/// Inputs:
/// - `lines`: Mutable reference to lines vector
/// - `app`: Application state
/// - `th`: Theme reference
/// - `km`: Keymap reference
///
/// Output:
/// - Adds recent bindings to lines
///
/// Details:
/// - Formats recent pane navigation and action keybindings, including explicit Shift+Del.
fn build_recent_bindings(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &crate::theme::Theme,
    km: &crate::theme::KeyMap,
) {
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
    add_binding_if_some(
        lines,
        app,
        th,
        km.recent_use.first().copied(),
        "app.modals.help.key_labels.use",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.recent_add.first().copied(),
        "app.modals.help.key_labels.add",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.recent_find.first().copied(),
        "app.modals.help.key_labels.find",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.recent_to_search.first().copied(),
        "app.modals.help.key_labels.to_search",
    );
    add_binding_if_some(
        lines,
        app,
        th,
        km.recent_remove.first().copied(),
        "app.modals.help.key_labels.remove",
    );
    // Explicit: Shift+Del clears Recent (display only)
    lines.push(fmt(
        &i18n::t(app, "app.modals.help.key_labels.clear"),
        KeyChord {
            code: crossterm::event::KeyCode::Delete,
            mods: crossterm::event::KeyModifiers::SHIFT,
        },
    ));
}

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
#[allow(clippy::many_single_char_names)]
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

    // Build all sections using helper functions
    build_global_bindings(&mut lines, app, &th, km);
    lines.push(Line::from(""));

    add_section_header(&mut lines, app, &th, "app.modals.help.sections.search");
    build_search_bindings(&mut lines, app, &th, km);
    build_search_normal_bindings(&mut lines, app, &th, km);

    add_section_header(&mut lines, app, &th, "app.modals.help.sections.install");
    build_install_bindings(&mut lines, app, &th, km);

    add_section_header(&mut lines, app, &th, "app.modals.help.sections.recent");
    build_recent_bindings(&mut lines, app, &th, km);

    // Mouse and UI controls
    add_section_header(&mut lines, app, &th, "app.modals.help.sections.mouse");
    lines.extend(parse_yaml_lines(&i18n::t(
        app,
        "app.modals.help.mouse_lines",
    )));

    // Dialogs
    add_section_header(
        &mut lines,
        app,
        &th,
        "app.modals.help.sections.system_update_dialog",
    );
    lines.extend(parse_yaml_lines(&i18n::t(
        app,
        "app.modals.help.system_update_lines",
    )));

    add_section_header(&mut lines, app, &th, "app.modals.help.sections.news_dialog");
    lines.extend(parse_yaml_lines(&i18n::t(
        app,
        "app.modals.help.news_lines",
    )));

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
