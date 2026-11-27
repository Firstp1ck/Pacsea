//! Tab strip, search line, grouped key list, and the recent-search dropdown.

use ratatui::{
    Frame,
    prelude::{Position, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use super::format::{group_label, setting_label, value_spans};
use super::text::{fit_spans, pad_right, spans_width, truncate, width};
use super::{EditorLayout, FrameCtx, rect_tuple};
use crate::state::config_editor::{SettingGroup, setting_group};
use crate::state::{ConfigEditorState, ConfigEditorTab};
use crate::theme::{ConfigFile, EditableSetting, ReloadBehavior, ValueKind, settings_for};

/// List width from which the raw key column is shown next to the value.
const KEY_COLUMN_MIN_WIDTH: u16 = 84;
/// Widest raw key column.
const KEY_COLUMN_MAX: usize = 32;
/// Narrowest and widest value column.
const VALUE_COLUMN_RANGE: (usize, usize) = (6, 24);
/// Columns for the selection marker plus the star column.
const ROW_PREFIX_COLUMNS: usize = 4;
/// Columns for the gap before the value plus the modified/restart markers.
const ROW_MARKER_COLUMNS: usize = 4;
/// Visible rows in the recent-search dropdown.
const RECENT_VISIBLE_ROWS: usize = 8;
/// Widest recent-search dropdown.
const RECENT_MAX_WIDTH: u16 = 56;

/// What: Draw the file tab strip and record each tab's hit rectangle.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `area`: One-row strip.
/// - `state`: Editor state; `tab_rects` is filled.
///
/// Output:
/// - Tabs `settings.conf · keybinds.conf · theme.conf · ★ Starred (n)`.
///
/// Details:
/// - The active tab is bracketed and inverted, so it is identifiable without color.
/// - Labels compact when needed; the strip scrolls to keep the active tab visible.
pub(super) fn render_tabs(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Rect,
    state: &mut ConfigEditorState,
) {
    let th = ctx.th;
    let active = state.active_tab();
    let mut labels: Vec<_> = ConfigEditorTab::ALL
        .into_iter()
        .map(|tab| tab_label(ctx, state, tab))
        .collect();
    let available = usize::from(area.width.saturating_sub(1));
    if labels.iter().map(|label| width(label) + 5).sum::<usize>() > available {
        for label in &mut labels {
            *label = label.replace(".conf", "");
        }
    }
    let active_index = ConfigEditorTab::ALL
        .iter()
        .position(|&tab| tab == active)
        .unwrap_or(0);
    let mut start = 0;
    while start < active_index
        && labels[start..=active_index]
            .iter()
            .map(|label| width(label) + 5)
            .sum::<usize>()
            .saturating_sub(1)
            > available
    {
        start += 1;
    }
    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x.saturating_add(1);
    for (tab, label) in ConfigEditorTab::ALL.into_iter().zip(labels).skip(start) {
        let remaining = usize::from(area.right().saturating_sub(x));
        let label = if tab == active {
            truncate(&label, remaining.saturating_sub(4))
        } else {
            label
        };
        let (text, style) = if tab == active {
            (
                format!("[ {label} ]"),
                Style::default()
                    .fg(th.base)
                    .bg(th.mauve)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (format!("  {label}  "), Style::default().fg(th.subtext0))
        };
        let text_width = u16::try_from(width(&text)).unwrap_or(u16::MAX);
        if x.saturating_add(text_width) > area.right() {
            break;
        }
        state
            .tab_rects
            .push((tab, (x, area.y, text_width, area.height.min(1))));
        spans.push(Span::styled(text, style));
        spans.push(Span::raw(" "));
        x = x.saturating_add(text_width).saturating_add(1);
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(th.base)),
        area,
    );
}

/// Localized tab caption; Starred carries its bookmark count.
fn tab_label(ctx: &FrameCtx<'_>, state: &ConfigEditorState, tab: ConfigEditorTab) -> String {
    match tab {
        ConfigEditorTab::Settings => ctx.t("tabs.settings"),
        ConfigEditorTab::Keybinds => ctx.t("tabs.keybinds"),
        ConfigEditorTab::Theme => ctx.t("tabs.theme"),
        ConfigEditorTab::Starred => {
            let count = starred_total(state);
            let label = ctx.t("tabs.starred");
            if count == 0 {
                label
            } else {
                format!("{label} ({count})")
            }
        }
    }
}

/// Editable files shown as tabs, in tab order.
const TAB_FILES: [ConfigFile; 3] = [
    ConfigFile::Settings,
    ConfigFile::Keybinds,
    ConfigFile::Theme,
];

/// Number of bookmarked keys that still exist in the schema.
fn starred_total(state: &ConfigEditorState) -> usize {
    TAB_FILES
        .into_iter()
        .flat_map(settings_for)
        .filter(|entry| state.is_bookmarked(entry.key))
        .count()
}

/// Number of keys on the active tab before the search filter applies.
fn tab_total(state: &ConfigEditorState) -> usize {
    match state.active_tab() {
        ConfigEditorTab::Starred => starred_total(state),
        _ => settings_for(state.selected_file).len(),
    }
}

/// What: Draw the search line with its match count and recent-search hint.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `area`: One-row line.
/// - `state`: Editor state (query, popup, dropdown).
/// - `shown`: Number of keys matching the query on this tab.
///
/// Output:
/// - `Search: <query>` (or a placeholder) with `N of M · Ctrl+R recent` on the right.
///
/// Details:
/// - Places the real terminal cursor after the query while no overlay owns input.
/// - Long queries scroll so their end (where typing happens) stays visible.
pub(super) fn render_search(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Rect,
    state: &ConfigEditorState,
    shown: usize,
) {
    let th = ctx.th;
    let label = format!(" {} ", ctx.t("search.label"));
    let right = format!(
        "{} · {} ",
        ctx.t_args("search.count", &[&shown, &tab_total(state)]),
        ctx.t("search.recent_hint")
    );
    let total = usize::from(area.width);
    let query_columns = total.saturating_sub(width(&label) + width(&right) + 1);
    let (query_text, query_style, caret) = if state.query.is_empty() {
        (
            truncate(&ctx.t("search.placeholder"), query_columns),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::ITALIC),
            0,
        )
    } else {
        let (visible, caret) =
            super::text::input_window(&state.query, state.query.chars().count(), query_columns);
        (visible, Style::default().fg(th.text), caret)
    };
    let fill = total.saturating_sub(width(&label) + width(&query_text) + width(&right));
    let line = Line::from(vec![
        Span::styled(
            label.clone(),
            Style::default()
                .fg(th.subtext1)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(query_text, query_style),
        Span::raw(" ".repeat(fill)),
        Span::styled(right, Style::default().fg(th.overlay1)),
    ]);
    f.render_widget(
        Paragraph::new(line).style(Style::default().bg(th.base)),
        area,
    );
    if state.popup.is_none() && state.recent_dropdown.is_none() && area.height > 0 {
        let offset = u16::try_from(width(&label) + caret).unwrap_or(u16::MAX);
        let x = area
            .x
            .saturating_add(offset)
            .min(area.right().saturating_sub(1));
        f.set_cursor_position(Position::new(x, area.y));
    }
}

/// One rendered list row: a group heading or a selectable key.
#[derive(Debug, Clone, Copy)]
enum ListRow {
    /// Non-selectable heading; the index addresses the group's first key.
    Heading(usize),
    /// Selectable key; the index addresses `filtered_keys()`.
    Item(usize),
}

/// Interleave group headings with the (already grouped) filtered keys.
fn build_rows(keys: &[&'static EditableSetting]) -> Vec<ListRow> {
    let mut rows = Vec::with_capacity(keys.len() + 8);
    let mut previous: Option<(ConfigFile, SettingGroup)> = None;
    for (index, entry) in keys.iter().enumerate() {
        let group = (entry.file, setting_group(entry));
        if previous != Some(group) {
            rows.push(ListRow::Heading(index));
            previous = Some(group);
        }
        rows.push(ListRow::Item(index));
    }
    rows
}

/// What: First row of the list viewport.
///
/// Inputs:
/// - `selected`: Row position of the selected key.
/// - `total`: Total rows including headings.
/// - `viewport`: Visible rows.
///
/// Output:
/// - Offset that keeps the selection visible, centered when possible.
///
/// Details:
/// - Headings count as rows, so the offset is computed on the interleaved list.
const fn viewport_offset(selected: usize, total: usize, viewport: usize) -> usize {
    if viewport == 0 || total <= viewport {
        return 0;
    }
    let max = total - viewport;
    let centered = selected.saturating_sub(viewport / 2);
    if centered < max { centered } else { max }
}

/// Column widths of one key row.
#[derive(Debug, Clone, Copy)]
struct RowColumns {
    /// Label column.
    label: usize,
    /// Right-aligned value column.
    value: usize,
    /// Raw key column; `0` hides it.
    key: usize,
}

impl RowColumns {
    /// Derive column widths from the list's inner width.
    fn for_width(columns: u16) -> Self {
        let total = usize::from(columns);
        let key = if columns >= KEY_COLUMN_MIN_WIDTH {
            KEY_COLUMN_MAX.min(total / 4)
        } else {
            0
        };
        let value = (total * 3 / 10).clamp(VALUE_COLUMN_RANGE.0, VALUE_COLUMN_RANGE.1);
        let key_part = if key > 0 { key + 2 } else { 0 };
        let label =
            total.saturating_sub(ROW_PREFIX_COLUMNS + ROW_MARKER_COLUMNS + value + key_part);
        Self { label, value, key }
    }
}

/// What: Draw the grouped key list and record row hit rectangles.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context (values snapshot for the value column).
/// - `area`: Bordered list area.
/// - `state`: Editor state; `row_rects`, `list_rect`, and `list_page_rows` are set.
/// - `keys`: `filtered_keys()` computed once for this frame.
///
/// Output:
/// - Group headings and key rows; empty states when nothing matches.
///
/// Details:
/// - `row_rects` indices address `keys`; headings get no rect.
/// - `list_page_rows` is the number of selectable rows visible, used by PgUp/PgDn.
pub(super) fn render_list(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Rect,
    state: &mut ConfigEditorState,
    keys: &[&'static EditableSetting],
) {
    let th = ctx.th;
    let focused = state.popup.is_none() && state.recent_dropdown.is_none();
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { th.mauve } else { th.surface2 }))
        .title(Span::styled(
            format!(" {} ", ctx.t("list.title")),
            Style::default().fg(if focused { th.mauve } else { th.overlay1 }),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    state.list_rect = Some(rect_tuple(inner));
    if keys.is_empty() {
        state.list_page_rows = inner.height.max(1);
        render_empty_list(f, ctx, inner, state);
        return;
    }
    let rows = build_rows(keys);
    let viewport = usize::from(inner.height);
    let selected_row = rows
        .iter()
        .position(|row| matches!(row, ListRow::Item(index) if *index == state.key_cursor))
        .unwrap_or(0);
    let offset = viewport_offset(selected_row, rows.len(), viewport);
    let columns = RowColumns::for_width(inner.width);
    let mut lines = Vec::with_capacity(viewport);
    let mut selectable: u16 = 0;
    for (y, row) in (inner.y..inner.bottom()).zip(rows.iter().skip(offset)) {
        match *row {
            ListRow::Heading(first) => lines.push(heading_line(ctx, keys[first])),
            ListRow::Item(index) => {
                let selected = index == state.key_cursor;
                lines.push(item_line(ctx, state, keys[index], selected, columns));
                state.row_rects.push((index, (inner.x, y, inner.width, 1)));
                selectable = selectable.saturating_add(1);
            }
        }
    }
    state.list_page_rows = selectable.max(1);
    f.render_widget(
        Paragraph::new(lines).style(Style::default().bg(th.base)),
        inner,
    );
}

/// Group heading row.
fn heading_line(ctx: &FrameCtx<'_>, first: &EditableSetting) -> Line<'static> {
    Line::from(Span::styled(
        format!(" {}", group_label(ctx, first)),
        Style::default()
            .fg(ctx.th.sapphire)
            .add_modifier(Modifier::BOLD),
    ))
}

/// What: Build one key row.
///
/// Inputs:
/// - `ctx`: Frame context.
/// - `state`: Editor state (bookmarks).
/// - `entry`: Schema row.
/// - `selected`: Whether the row is the cursor row.
/// - `columns`: Column widths.
///
/// Output:
/// - `▶ ★ Label ……… value ●↻  raw_key` with the value right-aligned.
///
/// Details:
/// - `●` marks values that differ from the codebase default, `↻` restart-bound keys.
/// - Multi-bound keybinds append `+N` for the display-only extra chords.
fn item_line(
    ctx: &FrameCtx<'_>,
    state: &ConfigEditorState,
    entry: &'static EditableSetting,
    selected: bool,
    columns: RowColumns,
) -> Line<'static> {
    let th = ctx.th;
    let raw = ctx.values.value_for(entry);
    let mut value = value_spans(ctx, entry, &raw, false);
    if matches!(entry.kind, ValueKind::KeyChord) {
        let extra = ctx.values.extra_chords(entry).len();
        if extra > 0 {
            value.push(Span::styled(
                format!(" +{extra}"),
                Style::default().fg(th.overlay1),
            ));
        }
    }
    let value = fit_spans(value, columns.value);
    let value_pad = columns.value.saturating_sub(spans_width(&value));
    let label_style = if selected {
        Style::default().fg(th.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    let mut spans = vec![
        Span::styled(
            if selected { "▶ " } else { "  " },
            Style::default().fg(th.mauve),
        ),
        Span::styled(
            if state.is_bookmarked(entry.key) {
                "★ "
            } else {
                "  "
            },
            Style::default().fg(th.yellow),
        ),
        Span::styled(
            pad_right(&setting_label(ctx, entry), columns.label),
            label_style,
        ),
        Span::raw(" ".repeat(1 + value_pad)),
    ];
    spans.extend(value);
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        if ctx.values.is_modified(entry) {
            "●"
        } else {
            " "
        },
        Style::default().fg(th.yellow),
    ));
    spans.push(Span::styled(
        if entry.reload == ReloadBehavior::RequiresRestart {
            "↻"
        } else {
            " "
        },
        Style::default().fg(th.red),
    ));
    if columns.key > 0 {
        spans.push(Span::styled(
            format!("  {}", pad_right(entry.key, columns.key)),
            Style::default().fg(th.overlay1),
        ));
    }
    let line = Line::from(spans);
    if selected {
        line.style(Style::default().bg(th.surface1))
    } else {
        line
    }
}

/// Explain an empty list: no starred keys yet, or no matches for the query.
fn render_empty_list(f: &mut Frame, ctx: &FrameCtx<'_>, inner: Rect, state: &ConfigEditorState) {
    let th = ctx.th;
    let message = if state.active_tab() == ConfigEditorTab::Starred && starred_total(state) == 0 {
        ctx.t("list.starred_empty")
    } else {
        ctx.t_args("list.no_matches", &[&state.query])
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {message}"),
            Style::default().fg(th.overlay1),
        )))
        .style(Style::default().bg(th.base))
        .wrap(Wrap { trim: false }),
        inner,
    );
}

/// What: Draw the recent-search dropdown under the search line.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `layout`: Editor layout (anchor below the search line).
/// - `state`: Editor state; `recent_rects` is filled with indices into `recent_queries`.
/// - `cursor`: Highlighted recent query.
///
/// Output:
/// - Bordered overlay with up to [`RECENT_VISIBLE_ROWS`] queries, or an empty-state hint.
///
/// Details:
/// - Scrolls so the cursor row is always visible.
pub(super) fn render_recents(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    layout: &EditorLayout,
    state: &mut ConfigEditorState,
    cursor: usize,
) {
    let th = ctx.th;
    let anchor = layout.search;
    let below = layout.footer.y.saturating_sub(anchor.bottom());
    let rows = state.recent_queries.len().clamp(1, RECENT_VISIBLE_ROWS);
    let height = u16::try_from(rows + 2).unwrap_or(u16::MAX).min(below);
    let width = anchor
        .width
        .saturating_sub(2)
        .min(RECENT_MAX_WIDTH)
        .max(anchor.width.min(20));
    let rect = Rect {
        x: anchor
            .x
            .saturating_add(1)
            .min(anchor.right().saturating_sub(width)),
        y: anchor.bottom(),
        width,
        height,
    };
    if rect.height < 3 {
        return;
    }
    f.render_widget(Clear, rect);
    let block = Block::default()
        .style(Style::default().bg(th.mantle))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.lavender))
        .title(Span::styled(
            format!(" {} ", ctx.t("recent.title")),
            Style::default()
                .fg(th.lavender)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if state.recent_queries.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                ctx.t("recent.empty"),
                Style::default().fg(th.overlay1),
            ))
            .wrap(Wrap { trim: false }),
            inner,
        );
        return;
    }
    let visible = usize::from(inner.height);
    let offset = cursor.saturating_sub(visible.saturating_sub(1));
    let mut lines = Vec::with_capacity(visible);
    for (y, (index, query)) in
        (inner.y..inner.bottom()).zip(state.recent_queries.iter().enumerate().skip(offset))
    {
        let selected = index == cursor;
        let style = if selected {
            Style::default().fg(th.text).bg(th.surface1)
        } else {
            Style::default().fg(th.subtext0)
        };
        let marker = if selected { "▶ " } else { "  " };
        lines.push(Line::from(Span::styled(
            pad_right(&format!("{marker}{query}"), usize::from(inner.width)),
            style,
        )));
        state
            .recent_rects
            .push((index, (inner.x, y, inner.width, 1)));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
