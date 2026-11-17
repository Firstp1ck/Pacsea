//! Renderer for the integrated TUI config editor (`AppMode::ConfigEditor`).
//!
//! Wide layout (terminal at least [`STACK_BELOW_WIDTH`] columns):
//!
//! ```text
//! ╭ Config editor ───────────────────────────────────────────────────────────────╮
//! │ [settings.conf]   keybinds.conf    theme.conf    ★ Starred (2)               │
//! │ Search: mirror▏                                     2 of 85 · Ctrl+R recent  │
//! │╭ Keys ───────────────────────────────────────╮╭ Details ─────────────────────╮│
//! ││ Mirrors                                     ││ Mirror count ★               ││
//! ││▶ ★ Mirror count                     20 ●    ││ Key      mirror_count        ││
//! ││    Mirror countries          Worldwide      ││ Current  20                  ││
//! │╰─────────────────────────────────────────────╯╰──────────────────────────────╯│
//! │ Globals  [Ctrl+C] Exit  |  [?] Help   Menus  [Shift+C] Config / lists  | …    │
//! │ Editor  [↑↓ PgUp/PgDn] Move selection  |  [Enter] Edit value  |  …            │
//! ╰──────────────────────────────────────────────────────────────────────────────╯
//! ```
//!
//! Narrower terminals stack the details pane below the key list. An open edit
//! popup is drawn centered on top (see [`popup`]); the recent-search dropdown
//! opens under the search line.
//!
//! Every frame clears and repopulates the hit rectangles read by
//! `events::modals::config_editor::mouse`, captures exactly one
//! [`ValueSnapshot`], and computes `filtered_keys()` once inside this renderer.
//! The outer `ui::keybind_footer_top` calculation also calls `selected_key()`,
//! which computes the filtered list separately before rendering.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::state::config_editor::ValueSnapshot;
use crate::state::{AppState, ConfigEditorState};
use crate::theme::{EditableSetting, Theme, theme};

/// Details pane next to the key list.
mod details;
/// Localized labels and value formatting.
mod format;
/// Tab strip, search line, grouped key list, and recent searches.
mod list;
/// Centered edit popup overlay.
mod popup;
/// Render tests.
#[cfg(test)]
mod tests;
/// Width-aware text helpers.
mod text;

/// Terminal width below which the details pane is stacked under the key list.
const STACK_BELOW_WIDTH: u16 = 100;
/// Minimum body height needed before the stacked layout shows a details pane.
const STACKED_DETAILS_MIN_BODY: u16 = 14;

/// Read-only inputs shared by every section of one frame.
struct FrameCtx<'a> {
    /// Application state (translations, keymap, footer visibility).
    app: &'a AppState,
    /// Active palette.
    th: &'a Theme,
    /// Effective and default values captured once for this frame.
    values: &'a ValueSnapshot,
}

/// Rectangles of the editor sections inside the outer block.
#[derive(Debug, Clone, Copy)]
struct EditorLayout {
    /// File tab strip (one row).
    tabs: Rect,
    /// Search line (one row).
    search: Rect,
    /// Bordered grouped key list.
    list: Rect,
    /// Bordered details pane; `None` when there is no room.
    details: Option<Rect>,
    /// Footer band (hint rows plus optional status row).
    footer: Rect,
}

impl EditorLayout {
    /// What: Split the editor's inner rectangle into its sections.
    ///
    /// Inputs:
    /// - `inner`: Area inside the outer editor border.
    /// - `ctx`: Frame context (footer visibility).
    /// - `state`: Editor state (status row, popup, dropdown for footer hints).
    /// - `selected`: Selected key from this frame's `filtered_keys()`.
    ///
    /// Output:
    /// - Section rectangles; the footer height equals the packed footer row count.
    ///
    /// Details:
    /// - Uses the same footer calculation as [`keybind_footer_top`] so toasts never cover it.
    fn compute(
        inner: Rect,
        ctx: &FrameCtx<'_>,
        state: &ConfigEditorState,
        selected: Option<&EditableSetting>,
    ) -> Self {
        let footer_h = super::super::details::config_editor_footer_reserved_rows(
            ctx.app.show_keybinds_footer,
            ctx.app,
            state,
            selected,
            inner.width,
        )
        .min(inner.height);
        let [tabs, search, body, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(footer_h),
        ])
        .areas(inner);
        let (list, details) = split_body(body, inner.width.saturating_add(2));
        Self {
            tabs,
            search,
            list,
            details,
            footer,
        }
    }
}

/// What: Divide the body into list and details panes.
///
/// Inputs:
/// - `body`: Area below the search line and above the footer.
/// - `total_width`: Width of the whole editor including its border.
///
/// Output:
/// - `(list, details)`; details is `None` on very short stacked layouts.
///
/// Details:
/// - Side by side from [`STACK_BELOW_WIDTH`] columns, otherwise stacked.
fn split_body(body: Rect, total_width: u16) -> (Rect, Option<Rect>) {
    if total_width >= STACK_BELOW_WIDTH {
        let [list, details] =
            Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)])
                .areas(body);
        return (list, Some(details));
    }
    if body.height < STACKED_DETAILS_MIN_BODY {
        return (body, None);
    }
    let details_h = (body.height * 2 / 5).clamp(7, 12);
    let [list, details] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(details_h)]).areas(body);
    (list, Some(details))
}

/// What: Render the integrated config editor as a full-window view.
///
/// Inputs:
/// - `f`: Frame to draw into.
/// - `app`: Application state (theme, i18n, stale hit rects).
/// - `area`: Full main-content area below the updates row.
/// - `state`: Editor state; hit rects, `list_page_rows`, and popup scroll are updated.
///
/// Output:
/// - Draws tabs, search line, key list, details, footer, and any overlay into `area`.
///
/// Details:
/// - Behaves like a mode switch (news/package mode), not a padded modal.
/// - Captures exactly one [`ValueSnapshot`] per frame.
pub(in crate::ui) fn render_config_editor_window(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    state: &mut ConfigEditorState,
) {
    // Package/news widgets are not drawn here; stale coordinates from a previous
    // mode must not trigger their click handlers inside the editor.
    clear_stale_main_rects(app);
    let th = theme();
    let values = ValueSnapshot::capture();
    let ctx = FrameCtx {
        app,
        th: &th,
        values: &values,
    };
    render_frame(f, &ctx, area, state);
}

/// What: Draw one editor frame with an explicit value snapshot.
///
/// Inputs:
/// - `f`: Frame to draw into.
/// - `ctx`: Frame context (app, theme, values).
/// - `area`: Editor area.
/// - `state`: Editor state receiving hit rects.
///
/// Output:
/// - Renders all sections and overlays.
///
/// Details:
/// - Split from [`render_config_editor_window`] so tests can inject deterministic values.
fn render_frame(f: &mut Frame, ctx: &FrameCtx<'_>, area: Rect, state: &mut ConfigEditorState) {
    reset_hit_rects(state);
    let keys = state.filtered_keys();
    let selected = keys.get(state.key_cursor).copied();
    let th = ctx.th;
    f.render_widget(Clear, area);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.mauve))
        .title(Line::from(Span::styled(
            format!(" {} ", ctx.t("title")),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let layout = EditorLayout::compute(inner, ctx, state, selected);
    list::render_tabs(f, ctx, layout.tabs, state);
    list::render_search(f, ctx, layout.search, state, keys.len());
    list::render_list(f, ctx, layout.list, state, &keys);
    details::render_details(f, ctx, layout.details, state, selected);
    render_footer(f, ctx, layout.footer, state, selected);
    if state.popup.is_some() {
        popup::render_popup(f, ctx, area, state);
    } else if let Some(cursor) = state.recent_dropdown {
        list::render_recents(f, ctx, &layout, state, cursor);
    }
}

/// Clear every renderer-owned hit rectangle before drawing a frame.
fn reset_hit_rects(state: &mut ConfigEditorState) {
    state.tab_rects.clear();
    state.row_rects.clear();
    state.popup_choice_rects.clear();
    state.recent_rects.clear();
    state.record_button_rect = None;
    state.list_rect = None;
    state.popup_rect = None;
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

/// What: Locate the top edge of the visible config-editor keybind footer.
///
/// Inputs:
/// - `app`: Application state controlling keybind-footer visibility
/// - `area`: Full config-editor rectangle, including its border
/// - `state`: Config-editor state supplying an optional status row
///
/// Output:
/// - The footer's first row when keybind hints are visible, otherwise `None`.
///
/// Details:
/// - Uses the same inner rectangle, width-aware packing, and reserved-row calculation as
///   [`EditorLayout::compute`].
/// - Runs before the renderer (toast anchor), so it resolves the selection on its own.
pub(in crate::ui) fn keybind_footer_top(
    app: &AppState,
    area: Rect,
    state: &ConfigEditorState,
) -> Option<u16> {
    if !app.show_keybinds_footer {
        return None;
    }
    let body = Block::default().borders(Borders::ALL).inner(area);
    let footer_height = super::super::details::config_editor_footer_reserved_rows(
        true,
        app,
        state,
        state.selected_key(),
        body.width,
    )
    .min(body.height);
    Some(
        body.y
            .saturating_add(body.height.saturating_sub(footer_height)),
    )
}

/// What: Draw the footer band (packed key hints plus an optional status row).
///
/// Inputs:
/// - `f`: Active frame.
/// - `ctx`: Frame context (footer visibility, theme).
/// - `area`: Footer rectangle inside the editor block.
/// - `state`: Editor state (popup, dropdown, status line).
/// - `selected`: Selected key from this frame's `filtered_keys()`.
///
/// Output:
/// - Renders rows without wrapping, so the drawn row count equals the reserved height.
///
/// Details:
/// - Hint rows come from `crate::ui::details::build_config_editor_footer_hint_lines`, keeping
///   key-cap styling identical to the package-mode footer.
fn render_footer(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Rect,
    state: &ConfigEditorState,
    selected: Option<&EditableSetting>,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let th = ctx.th;
    let mut lines: Vec<Line<'static>> = Vec::new();
    if ctx.app.show_keybinds_footer {
        lines.extend(
            super::super::details::build_config_editor_footer_hint_lines(
                ctx.app, state, selected, th, area.width,
            ),
        );
    }
    if let Some(status) = state.status.as_deref()
        && !status.trim().is_empty()
    {
        lines.push(Line::from(Span::styled(
            status.to_string(),
            Style::default().fg(th.green),
        )));
    }
    if lines.is_empty() {
        return;
    }
    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(th.subtext1).bg(th.base)),
        area,
    );
}

/// Convert a ratatui rectangle into the `(x, y, width, height)` tuple used by hit testing.
const fn rect_tuple(rect: Rect) -> (u16, u16, u16, u16) {
    (rect.x, rect.y, rect.width, rect.height)
}
