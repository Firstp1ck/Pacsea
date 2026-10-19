//! Centered edit popup: input, suggestion dropdown, validation, and Record action.
//!
//! ```text
//! ╭ Edit · Mirror count ─────────────────────────────────╮
//! │ mirror_count · integer 1–200 · applies on save       │
//! │╭────────────────────────────────────────────────────╮│
//! ││20▏                                                 ││
//! │╰────────────────────────────────────────────────────╯│
//! │▶ 20        [Current] [Default]                       │
//! │  1         [Min]                                     │
//! │  200       [Max]                                     │
//! │✓ Valid                                               │
//! │Enter saves: 20                                       │
//! │Type a number · +/- adjust · ↑↓ pick · Tab complete   │
//! ╰──────────────────────────────────────────────────────╯
//! ```
//!
//! Keybind popups add a `● Record (Ctrl+K)` button below the input; bool popups
//! skip the input and show only the on/off picker.

use ratatui::{
    Frame,
    prelude::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use super::format::{
    humanize_duration, setting_label, suggestion_text, tag_color, tag_label, type_label,
    value_spans,
};
use super::text::{input_window, pad_right, truncate, width, wrap};
use super::{FrameCtx, rect_tuple};
use crate::state::config_editor::suggestions::parse_swatch;
use crate::state::config_editor::{DROPDOWN_PAGE_ROWS, Suggestion};
use crate::state::{ConfigEditorState, EditPopupKind, EditPopupState};
use crate::theme::{EditableSetting, ValueKind};

/// Widest popup.
const POPUP_MAX_WIDTH: u16 = 76;
/// Widest on/off picker popup.
const BOOL_POPUP_MAX_WIDTH: u16 = 48;
/// Narrowest popup when the editor is wide enough.
const POPUP_MIN_WIDTH: u16 = 24;
/// Bordered input box height.
const INPUT_ROWS: usize = 3;
/// Row limits for wrapped text sections.
const META_MAX_ROWS: usize = 2;
/// Validation messages may name a conflicting binding, so allow more rows.
const VALIDATION_MAX_ROWS: usize = 3;
/// Hint rows.
const HINT_MAX_ROWS: usize = 2;
/// Rows left free above and below the popup inside the editor area.
const VERTICAL_MARGIN: u16 = 1;
/// Columns used by a `██ ` swatch prefix.
const SWATCH_COLUMNS: usize = 3;

/// Wrapped text sections whose heights are known before layout.
struct PopupText {
    /// `key · type/range · applies` rows.
    meta: Vec<String>,
    /// Validation rows.
    validation: Vec<String>,
    /// Whether the cached validation passed.
    validation_ok: bool,
    /// Kind-specific hint rows.
    hint: Vec<String>,
}

impl PopupText {
    /// Wrap every text section to `columns`.
    fn build(ctx: &FrameCtx<'_>, popup: &EditPopupState, columns: usize) -> Self {
        let setting = popup.setting;
        let meta = format!(
            "{} · {} · {}",
            setting.key,
            if matches!(setting.kind, ValueKind::KeyChord) {
                ctx.t("popup.type_chord")
            } else {
                type_label(ctx, setting)
            },
            ctx.t(match setting.reload {
                crate::theme::ReloadBehavior::AutoReload => "popup.applies_auto_reload",
                crate::theme::ReloadBehavior::AppliesOnSave => "popup.applies_on_save",
                crate::theme::ReloadBehavior::RequiresRestart => "popup.applies_restart",
            })
        );
        let (validation, validation_ok) = match &popup.validation {
            Ok(()) => (ctx.t("popup.valid"), true),
            Err(message) => (ctx.t_args("popup.invalid", &[message]), false),
        };
        Self {
            meta: wrap(&meta, columns, META_MAX_ROWS),
            validation: wrap(&validation, columns, VALIDATION_MAX_ROWS),
            validation_ok,
            hint: wrap(&ctx.t(hint_key(popup)), columns, HINT_MAX_ROWS),
        }
    }
}

/// How many dropdown rows fit and whether clip indicators are needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DropdownFit {
    /// Suggestion rows drawn.
    rows: usize,
    /// Whether `↑ N more` / `↓ N more` rows surround the suggestions.
    indicators: bool,
    /// Whether a single "no matching suggestions" row is drawn instead.
    empty_line: bool,
}

impl DropdownFit {
    /// What: Fit `total` visible suggestions into `budget` rows.
    ///
    /// Inputs:
    /// - `total`: Number of filtered suggestions.
    /// - `budget`: Rows available for the dropdown section.
    /// - `enabled`: Whether this popup has any suggestions at all.
    ///
    /// Output:
    /// - Row allocation; indicators are used only when suggestions are clipped.
    ///
    /// Details:
    /// - Capped at [`DROPDOWN_PAGE_ROWS`] so the popup stays compact on tall terminals.
    fn new(total: usize, budget: usize, enabled: bool) -> Self {
        let none = Self {
            rows: 0,
            indicators: false,
            empty_line: false,
        };
        if !enabled || budget == 0 {
            return none;
        }
        if total == 0 {
            return Self {
                empty_line: true,
                ..none
            };
        }
        let cap = DROPDOWN_PAGE_ROWS.min(budget);
        let indicators = total > cap && budget >= 3;
        Self {
            rows: total
                .min(DROPDOWN_PAGE_ROWS)
                .min(budget.saturating_sub(if indicators { 2 } else { 0 })),
            indicators,
            empty_line: false,
        }
    }

    /// Total rows of the dropdown section.
    const fn height(self) -> usize {
        self.rows + if self.indicators { 2 } else { 0 } + if self.empty_line { 1 } else { 0 }
    }
}

/// Hands out consecutive full-width rows of the popup body.
struct RowCursor {
    /// Popup body.
    area: Rect,
    /// Next free row.
    next: u16,
}

impl RowCursor {
    /// Start at the top of `area`.
    const fn new(area: Rect) -> Self {
        Self { area, next: area.y }
    }

    /// Take up to `rows` rows; `None` when the body is exhausted.
    fn take(&mut self, rows: usize) -> Option<Rect> {
        let remaining = self.area.bottom().saturating_sub(self.next);
        let height = u16::try_from(rows).unwrap_or(u16::MAX).min(remaining);
        if height == 0 {
            return None;
        }
        let rect = Rect {
            x: self.area.x,
            y: self.next,
            width: self.area.width,
            height,
        };
        self.next = self.next.saturating_add(height);
        Some(rect)
    }
}

/// What: Draw the edit popup centered in `area` and record its hit rectangles.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `area`: Editor area to center in.
/// - `state`: Editor state; `popup.scroll` is clamped and `popup_rect`,
///   `popup_choice_rects`, and `record_button_rect` are set.
///
/// Output:
/// - Popup with meta line, input, Record button (keybinds), dropdown, validation,
///   "Enter saves" line, and hint.
///
/// Details:
/// - The height adapts to the content; the dropdown shrinks first when space is short.
/// - `popup_choice_rects[i]` addresses `popup.visible[popup.scroll + i]`.
/// - Validation is read from the cached `popup.validation`; nothing is validated here.
pub(super) fn render_popup(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Rect,
    state: &mut ConfigEditorState,
) {
    let ConfigEditorState {
        popup: Some(popup),
        popup_choice_rects,
        record_button_rect,
        popup_rect,
        ..
    } = state
    else {
        return;
    };
    let th = ctx.th;
    let popup_width = popup_width(area, popup);
    let columns = usize::from(popup_width.saturating_sub(2));
    let text = PopupText::build(ctx, popup, columns);
    let has_input = !matches!(popup.kind, EditPopupKind::Bool(_));
    let is_chord = matches!(popup.kind, EditPopupKind::KeyChord { .. });
    let fixed = text.meta.len()
        + if has_input { INPUT_ROWS } else { 0 }
        + usize::from(is_chord)
        + text.validation.len()
        + 1
        + text.hint.len();
    let max_body = usize::from(area.height.saturating_sub(2 + 2 * VERTICAL_MARGIN));
    let fit = DropdownFit::new(
        popup.visible.len(),
        max_body.saturating_sub(fixed),
        !popup.suggestions.is_empty(),
    );
    clamp_scroll(popup, fit.rows);
    let height = u16::try_from(fixed + fit.height() + 2)
        .unwrap_or(u16::MAX)
        .min(area.height);
    let rect = centered(area, popup_width, height);
    *popup_rect = Some(rect_tuple(rect));

    f.render_widget(Clear, rect);
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.yellow))
        .title(Span::styled(
            format!(
                " {} ",
                ctx.t_args("popup.title", &[&setting_label(ctx, popup.setting)])
            ),
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        ));
    let body = block.inner(rect);
    f.render_widget(block, rect);

    let mut rows = RowCursor::new(body);
    render_rows(
        f,
        rows.take(text.meta.len()),
        &text.meta,
        Style::default().fg(th.overlay2),
    );
    if has_input && let Some(input) = rows.take(INPUT_ROWS) {
        render_input(f, ctx, input, popup);
    }
    if is_chord && let Some(button) = rows.take(1) {
        let capturing = matches!(popup.kind, EditPopupKind::KeyChord { capturing: true });
        *record_button_rect = Some(render_record_button(f, ctx, button, capturing));
    }
    if let Some(dropdown) = rows.take(fit.height()) {
        render_dropdown(f, ctx, dropdown, popup, fit, popup_choice_rects);
    }
    let validation_color = if text.validation_ok { th.green } else { th.red };
    render_rows(
        f,
        rows.take(text.validation.len()),
        &text.validation,
        Style::default().fg(validation_color),
    );
    if let Some(saves) = rows.take(1) {
        f.render_widget(Paragraph::new(saves_line(ctx, popup)), saves);
    }
    render_rows(
        f,
        rows.take(text.hint.len()),
        &text.hint,
        Style::default().fg(th.subtext0),
    );
}

/// Popup width for the editor area and popup kind.
fn popup_width(area: Rect, popup: &EditPopupState) -> u16 {
    let max = if matches!(popup.kind, EditPopupKind::Bool(_)) {
        BOOL_POPUP_MAX_WIDTH
    } else {
        POPUP_MAX_WIDTH
    };
    area.width
        .saturating_sub(4)
        .min(max)
        .max(POPUP_MIN_WIDTH.min(area.width))
}

/// Rectangle of `width`×`height` centered in `area`.
const fn centered(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

/// What: Keep the highlighted suggestion inside the drawn dropdown window.
///
/// Inputs:
/// - `popup`: Popup whose `scroll` is adjusted.
/// - `rows`: Suggestion rows actually drawn this frame.
///
/// Output:
/// - Mutates `popup.scroll`.
///
/// Details:
/// - Event handlers use the shared page size; this re-clamps to the real height.
fn clamp_scroll(popup: &mut EditPopupState, rows: usize) {
    if rows > 0
        && let Some(highlight) = popup.highlight
    {
        if highlight < popup.scroll {
            popup.scroll = highlight;
        } else if highlight >= popup.scroll + rows {
            popup.scroll = highlight + 1 - rows;
        }
    }
    popup.scroll = popup.scroll.min(popup.visible.len().saturating_sub(rows));
}

/// Draw pre-wrapped text rows into `rect` with one style.
fn render_rows(f: &mut Frame, rect: Option<Rect>, rows: &[String], style: Style) {
    if let Some(rect) = rect {
        let lines: Vec<Line<'static>> = rows
            .iter()
            .map(|row| Line::from(Span::styled(row.clone(), style)))
            .collect();
        f.render_widget(Paragraph::new(lines), rect);
    }
}

/// What: Text shown in the input box and the caret column inside it.
///
/// Inputs:
/// - `ctx`: Frame context (placeholders).
/// - `popup`: Popup state.
///
/// Output:
/// - `(text, caret_chars, is_placeholder)`.
///
/// Details:
/// - Hidden secrets show one `•` per character so the caret still lines up.
/// - Strict pickers show their filter text, or a "type to filter" placeholder.
fn input_content(ctx: &FrameCtx<'_>, popup: &EditPopupState) -> (String, usize, bool) {
    match popup.kind {
        EditPopupKind::Secret { revealed: false } if !popup.buffer.is_empty() => (
            "•".repeat(popup.buffer.chars().count()),
            popup.caret_column(),
            false,
        ),
        _ if popup.is_strict() && (!popup.filter_active || popup.buffer.is_empty()) => {
            (ctx.t("popup.placeholder_filter"), 0, true)
        }
        _ if popup.buffer.is_empty() => (ctx.t("popup.placeholder_text"), 0, true),
        _ => (popup.buffer.clone(), popup.caret_column(), false),
    }
}

/// What: Draw the bordered input box and place the terminal cursor.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `rect`: Three-row input area.
/// - `popup`: Popup state (buffer, caret, kind).
///
/// Output:
/// - Input text (or placeholder) with the real cursor at `caret_column()`; a color swatch on
///   the right when the buffer parses as a color; a recording banner while capturing.
///
/// Details:
/// - Long input scrolls horizontally so the caret stays visible.
fn render_input(f: &mut Frame, ctx: &FrameCtx<'_>, rect: Rect, popup: &EditPopupState) {
    let th = ctx.th;
    let capturing = matches!(popup.kind, EditPopupKind::KeyChord { capturing: true });
    let mut block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if capturing { th.red } else { th.lavender }));
    if capturing {
        block = block.title(Span::styled(
            format!(" {} ", ctx.t("popup.recording")),
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        ));
    }
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if capturing {
        f.render_widget(
            Paragraph::new(Span::styled(
                ctx.t("popup.recording_prompt"),
                Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
            )),
            inner,
        );
        return;
    }
    let swatch = if matches!(popup.setting.kind, ValueKind::Color) {
        parse_swatch(&popup.buffer)
    } else {
        None
    };
    let text_columns = usize::from(inner.width)
        .saturating_sub(if swatch.is_some() { SWATCH_COLUMNS } else { 0 })
        .max(1);
    let (content, caret, placeholder) = input_content(ctx, popup);
    let (visible, caret_offset) = if placeholder {
        (truncate(&content, text_columns), 0)
    } else {
        input_window(&content, caret, text_columns)
    };
    let style = if placeholder {
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::ITALIC)
    } else {
        Style::default().fg(th.text)
    };
    let mut spans = vec![Span::styled(pad_right(&visible, text_columns), style)];
    if let Some((r, g, b)) = swatch {
        spans.push(Span::raw(" "));
        spans.push(Span::styled("██", Style::default().fg(Color::Rgb(r, g, b))));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), inner);
    let x = inner
        .x
        .saturating_add(u16::try_from(caret_offset).unwrap_or(u16::MAX))
        .min(inner.right().saturating_sub(1));
    f.set_cursor_position(Position::new(x, inner.y));
}

/// What: Draw the `● Record (Ctrl+K)` button and return its hit rectangle.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `rect`: One-row area below the input.
/// - `capturing`: Whether recording is active.
///
/// Output:
/// - Rectangle covering only the button label (not the trailing hint).
///
/// Details:
/// - The button inverts to a red "Recording…" state while capturing.
fn render_record_button(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    rect: Rect,
    capturing: bool,
) -> (u16, u16, u16, u16) {
    let th = ctx.th;
    let label = format!(
        " {} ",
        ctx.t(if capturing {
            "popup.recording"
        } else {
            "popup.record"
        })
    );
    let button_width = u16::try_from(width(&label))
        .unwrap_or(u16::MAX)
        .min(rect.width);
    let style = if capturing {
        Style::default()
            .fg(th.base)
            .bg(th.red)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(th.red)
            .bg(th.surface1)
            .add_modifier(Modifier::BOLD)
    };
    let mut spans = vec![Span::styled(label, style)];
    if !capturing {
        spans.push(Span::styled(
            format!("  {}", ctx.t("popup.record_alt")),
            Style::default().fg(th.overlay1),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), rect);
    (rect.x, rect.y, button_width, 1)
}

/// What: Draw the suggestion dropdown and record one hit rectangle per suggestion row.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context.
/// - `rect`: Dropdown section (may be shorter than `fit.height()` when clipped).
/// - `popup`: Popup state (`visible`, `scroll`, `highlight`).
/// - `fit`: Row allocation.
/// - `rects`: Receives `popup_choice_rects`.
///
/// Output:
/// - Suggestion rows with optional `↑/↓ N more` indicator rows.
///
/// Details:
/// - Only rows actually inside `rect` get hit rectangles.
fn render_dropdown(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    rect: Rect,
    popup: &EditPopupState,
    fit: DropdownFit,
    rects: &mut Vec<(u16, u16, u16, u16)>,
) {
    let th = ctx.th;
    let dim = Style::default().fg(th.overlay1);
    if fit.empty_line {
        f.render_widget(
            Paragraph::new(Span::styled(ctx.t("popup.no_matches"), dim)),
            rect,
        );
        return;
    }
    let window: Vec<&Suggestion> = popup
        .visible
        .iter()
        .skip(popup.scroll)
        .take(fit.rows)
        .filter_map(|&index| popup.suggestions.get(index))
        .collect();
    let value_columns = window
        .iter()
        .map(|candidate| suggestion_columns(ctx, popup.setting, candidate))
        .max()
        .unwrap_or(0)
        .min(usize::from(rect.width) * 3 / 5);
    let mut lines = Vec::with_capacity(fit.height());
    if fit.indicators {
        lines.push(indicator_line(ctx, "popup.more_above", popup.scroll));
    }
    let first_row = rect.y + u16::from(fit.indicators);
    for (offset, candidate) in window.iter().enumerate() {
        let highlighted = popup.highlight == Some(popup.scroll + offset);
        lines.push(dropdown_line(
            ctx,
            popup.setting,
            candidate,
            highlighted,
            value_columns,
        ));
        let y = first_row.saturating_add(u16::try_from(offset).unwrap_or(u16::MAX));
        if y < rect.bottom() {
            rects.push((rect.x, y, rect.width, 1));
        }
    }
    if fit.indicators {
        let below = popup
            .visible
            .len()
            .saturating_sub(popup.scroll + window.len());
        lines.push(indicator_line(ctx, "popup.more_below", below));
    }
    f.render_widget(Paragraph::new(lines), rect);
}

/// Display columns of a suggestion's value cell (swatch included).
fn suggestion_columns(
    ctx: &FrameCtx<'_>,
    entry: &EditableSetting,
    candidate: &Suggestion,
) -> usize {
    width(&suggestion_text(ctx, entry, &candidate.value))
        + if candidate.swatch.is_some() {
            SWATCH_COLUMNS
        } else {
            0
        }
}

/// `↑ N more` / `↓ N more` row; blank when nothing is clipped on that side.
fn indicator_line(ctx: &FrameCtx<'_>, key: &str, count: usize) -> Line<'static> {
    if count == 0 {
        return Line::from("");
    }
    Line::from(Span::styled(
        format!("  {}", ctx.t_args(key, &[&count])),
        Style::default().fg(ctx.th.overlay1),
    ))
}

/// What: Build one dropdown row.
///
/// Inputs:
/// - `ctx`: Frame context.
/// - `entry`: Schema row of the popup.
/// - `candidate`: Suggestion to draw.
/// - `highlighted`: Whether the row is highlighted.
/// - `value_columns`: Width of the value cell (swatch included).
///
/// Output:
/// - `▶ ██ value  [Current] [Default] · 5 min — description`.
///
/// Details:
/// - Descriptions come from `label_key` and are shown only when translated.
fn dropdown_line(
    ctx: &FrameCtx<'_>,
    entry: &EditableSetting,
    candidate: &Suggestion,
    highlighted: bool,
    value_columns: usize,
) -> Line<'static> {
    let th = ctx.th;
    let mut spans = vec![Span::styled(
        if highlighted { "▶ " } else { "  " },
        Style::default().fg(th.mauve),
    )];
    let mut value_width = value_columns;
    if let Some((r, g, b)) = candidate.swatch {
        spans.push(Span::styled(
            "██ ",
            Style::default().fg(Color::Rgb(r, g, b)),
        ));
        value_width = value_width.saturating_sub(SWATCH_COLUMNS);
    }
    let value_style = if highlighted {
        Style::default().fg(th.text).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };
    spans.push(Span::styled(
        pad_right(&suggestion_text(ctx, entry, &candidate.value), value_width),
        value_style,
    ));
    for tag in &candidate.tags {
        spans.push(Span::styled(
            format!(" [{}]", tag_label(ctx, *tag)),
            Style::default().fg(tag_color(ctx, *tag)),
        ));
    }
    if let Some(secs) = candidate.duration_secs {
        spans.push(Span::styled(
            format!(" · {}", humanize_duration(ctx, secs)),
            Style::default().fg(th.subtext0),
        ));
    }
    if let Some(description) = candidate
        .label_key
        .as_deref()
        .and_then(|key| ctx.translated(key))
    {
        spans.push(Span::styled(
            format!(" — {description}"),
            Style::default().fg(th.overlay1),
        ));
    }
    let line = Line::from(spans);
    if highlighted {
        line.style(Style::default().bg(th.surface1))
    } else {
        line
    }
}

/// `Enter saves: <effective value>` row; secrets stay masked unless revealed.
fn saves_line(ctx: &FrameCtx<'_>, popup: &EditPopupState) -> Line<'static> {
    let th = ctx.th;
    let revealed = matches!(popup.kind, EditPopupKind::Secret { revealed: true });
    let mut spans = vec![Span::styled(
        format!("{} ", ctx.t("popup.saves")),
        Style::default().fg(th.subtext1),
    )];
    spans.extend(
        value_spans(ctx, popup.setting, &popup.effective_value(), revealed)
            .into_iter()
            .map(|span| {
                let style = span.style.add_modifier(Modifier::BOLD);
                span.style(style)
            }),
    );
    Line::from(spans)
}

/// Translation suffix of the hint row for the popup kind.
fn hint_key(popup: &EditPopupState) -> &'static str {
    match popup.kind {
        EditPopupKind::Bool(_) => "popup.hint_bool",
        EditPopupKind::Enum { .. } => "popup.hint_enum",
        EditPopupKind::Int { .. } => "popup.hint_int",
        EditPopupKind::Secret { .. } => "popup.hint_secret",
        EditPopupKind::KeyChord { capturing: true } => "popup.hint_recording",
        EditPopupKind::KeyChord { capturing: false } => "popup.hint_chord",
        EditPopupKind::Text if popup.comma_list() => "popup.hint_list",
        EditPopupKind::Text => "popup.hint_text",
    }
}
