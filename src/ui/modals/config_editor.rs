//! Renderer for the integrated TUI config editor (Phase 1 of the plan in
//! `dev/IMPROVEMENTS/IMPLEMENTATION_PLAN_tui_integrated_config_editing.md`).
//!
//! Layout (full-screen modal):
//!
//! ```text
//! ┌─ Config editor ────────────────────────────────────────────┐
//! │ Top pane: file list or key list                            │
//! │ ...                                                        │
//! ├────────────────────────────────────────────────────────────┤
//! │ Middle pane: fuzzy search input                            │
//! ├────────────────────────────────────────────────────────────┤
//! │ Bottom pane: details for selected key                      │
//! │   - current value, summary, reload behavior, file path     │
//! ├────────────────────────────────────────────────────────────┤
//! │ Footer: keybind strip (package-style) + optional status    │
//! └────────────────────────────────────────────────────────────┘
//! ```
//!
//! When `state.popup` is `Some`, a centered overlay is drawn on top with
//! type-specific controls and a `Ctrl+S` save / `Esc` cancel hint.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::state::{
    AppState, ConfigEditorFocus, ConfigEditorSearchFocus, ConfigEditorState, ConfigEditorView,
    EditPopupKind, EditPopupState,
};
use crate::theme::{ConfigFile, ReloadBehavior, Theme, ValueKind, resolved_config_path, theme};

/// What: Render the integrated config editor as a full-window view.
///
/// Drawn into the supplied content area in place of the normal
/// three-band layout, similar to news mode.
///
/// Inputs:
/// - `f`: Frame to draw into.
/// - `app`: Application state (theme + i18n).
/// - `area`: Full main-content area below the updates row.
/// - `state`: Current editor state.
///
/// Output:
/// - Renders the editor frame, lists, search input, details, footer, and
///   any active popup directly into `area` without extra padding.
///
/// Details:
/// - Skips the modal-style outer padding so it visually behaves like a
///   mode switch (news mode / package mode) rather than a centred popup.
pub(in crate::ui) fn render_config_editor_window(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    state: &ConfigEditorState,
) {
    // Clear hit-test rects for widgets that are NOT drawn by this view
    // (sort button, filter chips, results-rect status label, install
    // pane import/export buttons, etc.) so stale screen coordinates
    // from a previous package/news mode render cannot accidentally
    // trigger those handlers when the user clicks inside the editor.
    clear_stale_main_rects(app);

    let th = theme();
    f.render_widget(Clear, area);
    render_editor_into(f, app, area, state, &th);
    if let Some(popup) = state.popup.as_ref() {
        render_popup(f, app, area, popup, &th);
    }
}

/// Clear hit-test rects for widgets that are not visible while the
/// integrated config editor window is rendered. The top-row buttons
/// (`updates_button_rect`, `config_button_rect`, `panels_button_rect`,
/// `options_button_rect`, `collapsed_menu_button_rect`) are kept since
/// they continue to be drawn on the updates row.
const fn clear_stale_main_rects(app: &mut AppState) {
    app.sort_button_rect = None;
    app.results_rect = None;
    app.results_filter_aur_rect = None;
    app.results_filter_core_rect = None;
    app.results_filter_extra_rect = None;
    app.results_filter_multilib_rect = None;
    app.results_filter_eos_rect = None;
    app.results_filter_cachyos_rect = None;
    app.results_filter_artix_rect = None;
    app.results_filter_custom_repos_rect = None;
    app.artix_filter_menu_rect = None;
    app.custom_repos_filter_menu_rect = None;
    app.install_import_rect = None;
    app.install_export_rect = None;
    app.arch_status_rect = None;
    app.news_age_button_rect = None;
    app.fuzzy_indicator_rect = None;
    app.url_button_rect = None;
    app.details_rect = None;
}

/// Shared body renderer used by both the modal and full-window entry points.
fn render_editor_into(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.mauve))
        .title(Line::from(Span::styled(
            title_for(app, state),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )));
    let body = block.inner(area);
    f.render_widget(block, area);

    let panes = split_body(body, app, state);
    render_top_pane(f, app, panes.top, state, th);
    render_search_pane(f, app, panes.search, state, th);
    render_details_pane(f, app, panes.details, state, th);
    render_footer(f, panes.footer, app, state, th);
}

/// Pane rectangles for the editor body layout.
struct EditorPanes {
    /// Top: file list or key list.
    top: Rect,
    /// Middle: search input.
    search: Rect,
    /// Bottom: details for selected key.
    details: Rect,
    /// Footer: status / key hints.
    footer: Rect,
}

/// Split the modal body into the four editor sub-panes.
fn split_body(body: Rect, app: &AppState, state: &ConfigEditorState) -> EditorPanes {
    let footer_h = super::super::details::config_editor_footer_reserved_rows(
        app.show_keybinds_footer,
        app,
        state,
    );
    let content_h = body.height.saturating_sub(footer_h);
    let heights = compute_editor_content_heights(content_h, &app.vertical_layout_limits);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(heights.top),
            Constraint::Length(heights.search),
            Constraint::Length(heights.details),
            Constraint::Length(footer_h),
        ])
        .split(body);
    EditorPanes {
        top: chunks[0],
        search: chunks[1],
        details: chunks[2],
        footer: chunks[3],
    }
}

/// Height allocation for config-editor content panes (top/search/details).
struct EditorContentHeights {
    /// Top pane height.
    top: u16,
    /// Search pane height.
    search: u16,
    /// Details pane height.
    details: u16,
}

/// Compute config-editor pane heights using the same semantic vertical limits
/// as package mode (`results`/`middle`/`package_info` respectively).
fn compute_editor_content_heights(
    content_h: u16,
    limits: &crate::state::VerticalLayoutLimits,
) -> EditorContentHeights {
    let min_top_middle_total = limits.min_results + limits.min_middle;
    let space_after_min = content_h.saturating_sub(min_top_middle_total);

    let (top, search, details) = if space_after_min >= limits.min_package_info {
        // Same strategy as package mode with package-info enabled.
        let top_middle_share = (content_h * 3) / 4;
        let search_initial = if top_middle_share >= limits.max_middle + limits.min_results {
            limits.max_middle
        } else if top_middle_share >= limits.min_middle + limits.min_results {
            top_middle_share.saturating_sub(limits.min_results)
        } else {
            limits.min_middle
        };
        let remaining_for_top = top_middle_share.saturating_sub(search_initial);
        let top = remaining_for_top.clamp(limits.min_results, limits.max_results);
        let unused_top_space = remaining_for_top.saturating_sub(top);
        let search = (search_initial + unused_top_space).min(limits.max_middle);
        let details = content_h.saturating_sub(top).saturating_sub(search);
        if details >= limits.min_package_info {
            (top, search, details)
        } else {
            // Fallback: no details, redistribute between top/search.
            let search = if content_h >= limits.max_middle + limits.min_results {
                limits.max_middle
            } else if content_h >= limits.min_middle + limits.min_results {
                content_h.saturating_sub(limits.min_results)
            } else {
                limits.min_middle
            };
            let top = content_h
                .saturating_sub(search)
                .clamp(limits.min_results, limits.max_results);
            let search = content_h
                .saturating_sub(top)
                .clamp(limits.min_middle, limits.max_middle);
            (top, search, 0)
        }
    } else {
        // Same strategy as package mode without package-info.
        let search = if content_h >= limits.max_middle + limits.min_results {
            limits.max_middle
        } else if content_h >= limits.min_middle + limits.min_results {
            content_h.saturating_sub(limits.min_results)
        } else {
            limits.min_middle
        };
        let mut top = content_h
            .saturating_sub(search)
            .clamp(limits.min_results, limits.max_results);
        if top + search > content_h {
            top = content_h
                .saturating_sub(limits.min_middle)
                .clamp(limits.min_results, limits.max_results);
        }
        let search = content_h
            .saturating_sub(top)
            .clamp(limits.min_middle, limits.max_middle);
        (top, search, 0)
    };

    EditorContentHeights {
        top,
        search,
        details,
    }
}

/// Build the modal title from the editor state.
fn title_for(app: &AppState, state: &ConfigEditorState) -> String {
    // Avoid duplicate "Config editor — Config editor — files" title in
    // file-list mode and keep parity with package-mode style.
    let title = match state.view {
        ConfigEditorView::FileList => crate::i18n::t(app, "app.modals.config_editor.title_files"),
        ConfigEditorView::KeyList => {
            let base = crate::i18n::t(app, "app.modals.config_editor.title");
            format!("{base} — {}", file_label(state.selected_file))
        }
    };
    format!(" {title} ")
}

/// Display label for a `ConfigFile`. Used in the file-list view and title.
const fn file_label(file: ConfigFile) -> &'static str {
    match file {
        ConfigFile::Settings => "settings.conf",
        ConfigFile::Theme => "theme.conf",
        ConfigFile::Keybinds => "keybinds.conf",
        ConfigFile::Repos => "repos.conf",
    }
}

/// Phase availability hint for non-Settings files.
const fn file_phase_hint(file: ConfigFile) -> &'static str {
    match file {
        ConfigFile::Settings | ConfigFile::Keybinds => "",
        ConfigFile::Theme => " (Phase 3)",
        ConfigFile::Repos => " (later phase)",
    }
}

/// Whether a file is interactive in the current phase.
const fn is_file_enabled(file: ConfigFile) -> bool {
    matches!(file, ConfigFile::Settings | ConfigFile::Keybinds)
}

/// Top-pane dispatcher: file list or key list.
fn render_top_pane(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    match state.view {
        ConfigEditorView::FileList => render_file_list(f, app, area, state, th),
        ConfigEditorView::KeyList => render_key_list(f, app, area, state, th),
    }
}

/// Render the four-row file list (Settings is enabled in Phase 1).
fn render_file_list(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let list_focused = matches!(state.focus, ConfigEditorFocus::List);
    let files = [
        ConfigFile::Settings,
        ConfigFile::Keybinds,
        ConfigFile::Theme,
        ConfigFile::Repos,
    ];
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(files.len() + 1);
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.config_editor.files_heading"),
        Style::default()
            .fg(th.subtext1)
            .add_modifier(Modifier::BOLD),
    )));
    for (i, file) in files.iter().enumerate() {
        let is_sel = state.file_cursor == i;
        let enabled = is_file_enabled(*file);
        let marker = if is_sel { "▶ " } else { "  " };
        let label = format!("{}{}{}", marker, file_label(*file), file_phase_hint(*file));
        let style = if !enabled {
            Style::default().fg(th.overlay1)
        } else if is_sel {
            Style::default().fg(th.text).bg(th.surface1)
        } else {
            Style::default().fg(th.text)
        };
        lines.push(Line::from(Span::styled(label, style)));
    }
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if list_focused { th.mauve } else { th.surface1 }))
        .title(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.files_pane_title"),
            Style::default().fg(if list_focused { th.mauve } else { th.overlay1 }),
        ));
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

/// Render the filtered key list for the currently selected file.
fn render_key_list(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let list_focused = matches!(state.focus, ConfigEditorFocus::List);
    let keys = state.filtered_keys();
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(keys.len() + 1);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if list_focused { th.mauve } else { th.surface1 }))
        .title(Span::styled(
            format!(
                " {} — {} ",
                crate::i18n::t(app, "app.modals.config_editor.keys_pane_title"),
                file_label(state.selected_file)
            ),
            Style::default().fg(if list_focused { th.mauve } else { th.overlay1 }),
        ));
    let inner = block.inner(area);

    let heading = if state.query.trim().is_empty() {
        crate::i18n::t(app, "app.modals.config_editor.keys_heading")
    } else {
        format!(
            "{} ({})",
            crate::i18n::t(app, "app.modals.config_editor.keys_heading_filtered"),
            keys.len()
        )
    };
    lines.push(Line::from(Span::styled(
        heading,
        Style::default()
            .fg(th.subtext1)
            .add_modifier(Modifier::BOLD),
    )));
    if keys.is_empty() {
        lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.no_matches"),
            Style::default().fg(th.overlay1),
        )));
    } else {
        // Keep selected row visible/centered similarly to package results mode.
        let viewport_rows = inner.height.saturating_sub(1) as usize; // 1 row reserved for heading
        let len = keys.len();
        let max_offset = len.saturating_sub(viewport_rows.max(1));
        let desired_offset = state
            .key_cursor
            .saturating_sub(viewport_rows.saturating_div(2))
            .min(max_offset);
        let end = desired_offset.saturating_add(viewport_rows).min(len);
        for (row_idx, entry) in keys[desired_offset..end].iter().enumerate() {
            let absolute_idx = desired_offset + row_idx;
            let is_sel = state.key_cursor == absolute_idx;
            let marker = if is_sel { "▶ " } else { "  " };
            let line = format!(
                "{}{}  ·  {}",
                marker,
                localized_setting_label(app, entry),
                entry.key
            );
            let style = if is_sel {
                Style::default().fg(th.text).bg(th.surface1)
            } else {
                Style::default().fg(th.text)
            };
            lines.push(Line::from(Span::styled(line, style)));
        }
    }
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

/// Render the middle pane: a single-line fuzzy-search input.
fn render_search_pane(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let left_pct = if app.show_recent_pane {
        app.layout_left_pct.min(100)
    } else {
        0
    };
    let right_pct = if app.show_install_pane {
        app.layout_right_pct.min(100)
    } else {
        0
    };
    let center_pct = 100u16
        .saturating_sub(left_pct)
        .saturating_sub(right_pct)
        .min(100);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(left_pct),
            Constraint::Percentage(center_pct),
            Constraint::Percentage(right_pct),
        ])
        .split(area);

    if app.show_recent_pane && cols[0].width > 0 {
        render_search_recent_column(f, app, cols[0], state, th);
    }
    render_search_input_column(f, app, cols[1], state, th);
    if app.show_install_pane && cols[2].width > 0 {
        render_search_bookmarks_column(f, app, cols[2], state, th);
    }
}

/// Render the left search-subpane with recent queries.
fn render_search_recent_column(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let focused = matches!(state.focus, ConfigEditorFocus::Search)
        && matches!(state.search_focus, ConfigEditorSearchFocus::Recent);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { th.mauve } else { th.surface1 }))
        .title(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.search_recent_title"),
            Style::default().fg(if focused { th.mauve } else { th.overlay1 }),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if state.recent_queries.is_empty() {
        lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.search_recent_empty"),
            Style::default().fg(th.overlay1),
        )));
    } else {
        for (idx, q) in state.recent_queries.iter().enumerate().take(4) {
            let sel = focused && state.recent_cursor == idx;
            let prefix = if sel { "▶ " } else { "  " };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{q}"),
                if sel {
                    Style::default().fg(th.text).bg(th.surface1)
                } else {
                    Style::default().fg(th.text)
                },
            )));
        }
    }
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

/// Render the center search-subpane with editable query input.
fn render_search_input_column(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let focused = matches!(state.focus, ConfigEditorFocus::Search)
        && matches!(state.search_focus, ConfigEditorSearchFocus::Input);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { th.mauve } else { th.surface1 }))
        .title(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.search_input_title"),
            Style::default().fg(if focused { th.mauve } else { th.overlay1 }),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let content = state.query.clone();
    let hint = crate::i18n::t(app, "app.modals.config_editor.search_hint");
    let para = Paragraph::new(vec![
        Line::from(Span::styled(
            content,
            if focused || !state.query.is_empty() {
                Style::default().fg(th.text)
            } else {
                Style::default().fg(th.overlay1)
            },
        )),
        Line::from(Span::styled(hint, Style::default().fg(th.overlay1))),
    ])
    .style(Style::default().bg(th.base));
    f.render_widget(para, inner);

    // Use a real terminal cursor in the input field, matching package/news search behavior.
    if focused {
        let right = inner.x + inner.width.saturating_sub(1);
        let caret_cols = u16::try_from(state.query.chars().count()).unwrap_or(u16::MAX);
        let x = std::cmp::min(inner.x + caret_cols, right);
        let y = inner.y;
        f.set_cursor_position(Position::new(x, y));
    }
}

/// Render the right search-subpane with bookmarked setting keys.
fn render_search_bookmarks_column(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let focused = matches!(state.focus, ConfigEditorFocus::Search)
        && matches!(state.search_focus, ConfigEditorSearchFocus::Bookmarks);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { th.mauve } else { th.surface1 }))
        .title(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.search_bookmarks_title"),
            Style::default().fg(if focused { th.mauve } else { th.overlay1 }),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines: Vec<Line<'static>> = Vec::new();
    if state.bookmarked_keys.is_empty() {
        lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.search_bookmarks_empty"),
            Style::default().fg(th.overlay1),
        )));
    } else {
        for (idx, key) in state.bookmarked_keys.iter().enumerate().take(4) {
            let sel = focused && state.bookmark_cursor == idx;
            let prefix = if sel { "▶ " } else { "  " };
            lines.push(Line::from(Span::styled(
                format!("{prefix}{key}"),
                if sel {
                    Style::default().fg(th.text).bg(th.surface1)
                } else {
                    Style::default().fg(th.text)
                },
            )));
        }
    }
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

/// Render the bottom pane: details for the currently selected key.
fn render_details_pane(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
    th: &Theme,
) {
    let details_focused = !matches!(state.focus, ConfigEditorFocus::Search);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if details_focused {
            th.mauve
        } else {
            th.surface1
        }))
        .title(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.details_pane_title"),
            Style::default().fg(if details_focused {
                th.mauve
            } else {
                th.overlay1
            }),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = build_details_lines(app, state, th);
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

/// Build the detail-pane content lines for the selected key (or a hint if
/// the file list is active).
fn build_details_lines(
    app: &AppState,
    state: &ConfigEditorState,
    th: &Theme,
) -> Vec<Line<'static>> {
    if matches!(state.view, ConfigEditorView::FileList) {
        let selected = selected_file_from_cursor(state.file_cursor);
        let active_path = resolved_config_path(selected);
        return vec![
            Line::from(Span::styled(
                crate::i18n::t(app, "app.modals.config_editor.hint_pick_file"),
                Style::default().fg(th.overlay1),
            )),
            Line::from(Span::styled(
                format!("  active file: {}", active_path.display()),
                Style::default().fg(th.subtext0),
            )),
        ];
    }
    let Some(entry) = state.selected_key() else {
        return vec![Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.config_editor.hint_no_key"),
            Style::default().fg(th.overlay1),
        ))];
    };
    let value = crate::state::config_editor::current_value_string(entry);
    let displayed = if matches!(entry.kind, ValueKind::Secret) {
        mask_secret(&value)
    } else {
        value
    };
    vec![
        Line::from(vec![
            Span::styled(
                format!("{}: ", localized_setting_label(app, entry)),
                Style::default().fg(th.subtext1),
            ),
            Span::styled(entry.key.to_string(), Style::default().fg(th.overlay1)),
        ]),
        Line::from(vec![
            Span::styled("  value: ", Style::default().fg(th.subtext0)),
            Span::styled(displayed, Style::default().fg(th.text)),
        ]),
        Line::from(Span::styled(
            format!("  {}", localized_setting_summary(app, entry)),
            Style::default().fg(th.subtext0),
        )),
        Line::from(Span::styled(
            format!("  apply: {}", reload_label(entry.reload)),
            Style::default().fg(th.overlay1),
        )),
        Line::from(Span::styled(
            format!(
                "  active file: {}",
                resolved_config_path(entry.file).display()
            ),
            Style::default().fg(th.subtext0),
        )),
    ]
}

/// Resolve the file represented by the current file-list cursor index.
const fn selected_file_from_cursor(file_cursor: usize) -> ConfigFile {
    match file_cursor {
        1 => ConfigFile::Keybinds,
        2 => ConfigFile::Theme,
        3 => ConfigFile::Repos,
        _ => ConfigFile::Settings,
    }
}

/// What: Draw the integrated editor footer using the same key-cap styling as package mode.
///
/// Inputs:
/// - `f`: Active frame
/// - `area`: Footer rectangle inside the editor block
/// - `app`: Application state (`show_keybinds_footer`, keymap, locale)
/// - `state`: Editor state (view, popup, status line)
/// - `th`: Theme palette
///
/// Output:
/// - Renders a filled background plus wrapped hint lines and optional status text.
///
/// Details:
/// - Delegates hint assembly to `crate::ui::details::build_config_editor_footer_hint_lines` so
///   separators and `[key]` styling stay consistent with the package details footer.
fn render_footer(f: &mut Frame, area: Rect, app: &AppState, state: &ConfigEditorState, th: &Theme) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut lines: Vec<Line<'static>> = Vec::new();
    if app.show_keybinds_footer {
        lines.extend(super::super::details::build_config_editor_footer_hint_lines(app, state, th));
    }
    if let Some(s) = state.status.as_deref()
        && !s.trim().is_empty()
    {
        lines.push(Line::from(Span::styled(
            s.to_string(),
            Style::default().fg(th.green),
        )));
    }
    if lines.is_empty() {
        return;
    }
    f.render_widget(Block::default().style(Style::default().bg(th.base)), area);
    let para = Paragraph::new(lines)
        .style(Style::default().fg(th.subtext1))
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });
    f.render_widget(para, area);
}

/// Render the centered popup overlay.
fn render_popup(f: &mut Frame, app: &AppState, area: Rect, popup: &EditPopupState, th: &Theme) {
    let width = area.width.saturating_sub(8).min(60);
    let height = 9u16;
    let pos_x = area.x + (area.width.saturating_sub(width)) / 2;
    let pos_y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect {
        x: pos_x,
        y: pos_y,
        width,
        height,
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.yellow))
        .title(Span::styled(
            format!(" Edit · {} ", localized_setting_label(app, popup.setting)),
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    f.render_widget(block, rect);

    let lines = build_popup_lines(popup, th);
    let para = Paragraph::new(lines)
        .style(Style::default().bg(th.base))
        .wrap(Wrap { trim: false });
    f.render_widget(para, inner);
}

/// Build the popup body content based on the popup variant.
fn build_popup_lines(popup: &EditPopupState, th: &Theme) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        format!("key: {}", popup.setting.key),
        Style::default().fg(th.overlay1),
    )));
    match &popup.kind {
        EditPopupKind::Bool(b) => {
            let on_style = if *b {
                Style::default().fg(th.text).bg(th.surface1)
            } else {
                Style::default().fg(th.overlay1)
            };
            let off_style = if *b {
                Style::default().fg(th.overlay1)
            } else {
                Style::default().fg(th.text).bg(th.surface1)
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled("[ true ]", on_style),
                Span::raw("   "),
                Span::styled("[ false ]", off_style),
            ]));
            lines.push(Line::from(Span::styled(
                "Space/←/→ toggle  ·  Ctrl+S save  ·  Esc cancel",
                Style::default().fg(th.subtext0),
            )));
        }
        EditPopupKind::Enum { choices, index } => {
            for (i, c) in choices.iter().enumerate() {
                let style = if i == *index {
                    Style::default().fg(th.text).bg(th.surface1)
                } else {
                    Style::default().fg(th.text)
                };
                let marker = if i == *index { "▶ " } else { "  " };
                lines.push(Line::from(Span::styled(format!("{marker}{c}"), style)));
            }
            lines.push(Line::from(Span::styled(
                "↑/↓ select  ·  Ctrl+S save  ·  Esc cancel",
                Style::default().fg(th.subtext0),
            )));
        }
        EditPopupKind::Int { min, max } => {
            lines.push(Line::from(vec![
                Span::styled("  value: ", Style::default().fg(th.subtext1)),
                Span::styled(format!("{}▏", popup.buffer), Style::default().fg(th.text)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("range: {min}..={max}"),
                Style::default().fg(th.overlay1),
            )));
            lines.push(Line::from(Span::styled(
                "↑/↓ ±1  ·  digits/Backspace edit  ·  Ctrl+S save  ·  Esc cancel",
                Style::default().fg(th.subtext0),
            )));
        }
        EditPopupKind::Text => {
            lines.push(Line::from(vec![
                Span::styled("  value: ", Style::default().fg(th.subtext1)),
                Span::styled(format!("{}▏", popup.buffer), Style::default().fg(th.text)),
            ]));
            lines.push(Line::from(Span::styled(
                "Type to edit  ·  Ctrl+S save  ·  Esc cancel",
                Style::default().fg(th.subtext0),
            )));
        }
        EditPopupKind::Secret { revealed } => {
            let display = if *revealed {
                popup.buffer.clone()
            } else {
                mask_secret(&popup.buffer)
            };
            lines.push(Line::from(vec![
                Span::styled("  value: ", Style::default().fg(th.subtext1)),
                Span::styled(format!("{display}▏"), Style::default().fg(th.text)),
            ]));
            lines.push(Line::from(Span::styled(
                "Type to edit  ·  Ctrl+R reveal  ·  Ctrl+S save  ·  Esc cancel",
                Style::default().fg(th.subtext0),
            )));
        }
    }
    lines
}

/// Mask a secret for display.
fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "(unset)".into()
    } else {
        "•".repeat(s.chars().count().min(8))
    }
}

/// Human-readable label for a `ReloadBehavior`.
const fn reload_label(r: ReloadBehavior) -> &'static str {
    match r {
        ReloadBehavior::AutoReload => "auto-reloads",
        ReloadBehavior::AppliesOnSave => "applies on save",
        ReloadBehavior::RequiresRestart => "requires restart",
    }
}

/// Resolve a translated label for a setting key with schema fallback.
fn localized_setting_label(app: &AppState, entry: &crate::theme::EditableSetting) -> String {
    let key = entry.label_i18n_key();
    let translated = crate::i18n::t(app, &key);
    if translated == key {
        entry.key.replace('_', " ")
    } else {
        translated
    }
}

/// Resolve a translated summary for a setting key with schema fallback.
fn localized_setting_summary(app: &AppState, entry: &crate::theme::EditableSetting) -> String {
    let key = entry.summary_i18n_key();
    let translated = crate::i18n::t(app, &key);
    if translated == key {
        String::new()
    } else {
        translated
    }
}

#[cfg(test)]
mod footer_rows_tests {
    use crate::state::{AppState, ConfigEditorState};

    use crate::ui::details::config_editor_footer_reserved_rows;

    #[test]
    fn footer_hint_rows_match_line_count_when_hints_on() {
        let app = AppState::default();
        let state = ConfigEditorState::default();
        assert_eq!(config_editor_footer_reserved_rows(true, &app, &state), 2);
    }

    #[test]
    fn footer_zero_when_hints_off_and_no_status() {
        let app = AppState::default();
        let state = ConfigEditorState::default();
        assert_eq!(config_editor_footer_reserved_rows(false, &app, &state), 0);
    }

    #[test]
    fn footer_one_row_when_hints_off_but_status_set() {
        let app = AppState::default();
        let state = ConfigEditorState {
            status: Some("saved".into()),
            ..Default::default()
        };
        assert_eq!(config_editor_footer_reserved_rows(false, &app, &state), 1);
    }

    #[test]
    fn footer_zero_when_hints_off_and_blank_status() {
        let app = AppState::default();
        let state = ConfigEditorState {
            status: Some("   ".into()),
            ..Default::default()
        };
        assert_eq!(config_editor_footer_reserved_rows(false, &app, &state), 0);
    }

    #[test]
    fn footer_adds_status_row_when_hints_on() {
        let app = AppState::default();
        let state = ConfigEditorState {
            status: Some("ok".into()),
            ..Default::default()
        };
        assert_eq!(config_editor_footer_reserved_rows(true, &app, &state), 3);
    }
}
