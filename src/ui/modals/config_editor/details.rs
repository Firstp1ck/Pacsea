//! Details pane for the selected configuration key.

use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use super::FrameCtx;
use super::format::{
    reload_color, reload_label, setting_label, setting_summary, type_label, value_spans,
};
use super::text::{pad_right, width, wrap};
use crate::state::ConfigEditorState;
use crate::theme::{EditableSetting, KeyChord, resolved_config_path};

/// Rows a wrapped field value may occupy before it is cut with `…`.
const FIELD_MAX_ROWS: usize = 4;

/// Field label translation suffixes, in display order.
const FIELD_KEYS: [&str; 8] = [
    "details.key",
    "details.current",
    "details.default",
    "details.type",
    "details.applies",
    "details.file",
    "details.extra_chords",
    "details.pi_scan_setup",
];

/// What: Draw the details pane.
///
/// Inputs:
/// - `f`: Frame.
/// - `ctx`: Frame context (values snapshot, keymap).
/// - `area`: Bordered pane area, or `None` when the layout has no room for it.
/// - `state`: Editor state (bookmarks, popup focus).
/// - `selected`: Selected key, if any.
///
/// Output:
/// - Label, key, current/default values, type/range, apply behavior, file, summary,
///   extra chords, and the Pi Scan setup chord for `pi_scan_*` keys.
///
/// Details:
/// - Text wraps inside the pane; nothing here is clickable.
pub(super) fn render_details(
    f: &mut Frame,
    ctx: &FrameCtx<'_>,
    area: Option<Rect>,
    state: &ConfigEditorState,
    selected: Option<&'static EditableSetting>,
) {
    let Some(area) = area else {
        return;
    };
    let th = ctx.th;
    let block = Block::default()
        .style(Style::default().bg(th.base))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(th.surface2))
        .title(Span::styled(
            format!(" {} ", ctx.t("details.title")),
            Style::default().fg(th.overlay1),
        ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = selected.map_or_else(
        || {
            vec![Line::from(Span::styled(
                ctx.t("details.no_key"),
                Style::default().fg(th.overlay1),
            ))]
        },
        |entry| detail_lines(ctx, state, entry, usize::from(inner.width)),
    );
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().bg(th.base))
            .wrap(Wrap { trim: false }),
        inner,
    );
}

/// What: Build the details lines for one key.
///
/// Inputs:
/// - `ctx`: Frame context.
/// - `state`: Editor state (bookmark marker).
/// - `entry`: Selected schema row.
/// - `columns`: Inner width of the pane.
///
/// Output:
/// - Title line, aligned field rows, and the wrapped summary.
///
/// Details:
/// - Field labels are padded to the widest localized label so values line up; long text values
///   wrap with a hanging indent under the value column.
fn detail_lines(
    ctx: &FrameCtx<'_>,
    state: &ConfigEditorState,
    entry: &'static EditableSetting,
    columns: usize,
) -> Vec<Line<'static>> {
    let th = ctx.th;
    let labels: Vec<String> = FIELD_KEYS.iter().map(|key| ctx.t(key)).collect();
    let label_width = labels.iter().map(|label| width(label)).max().unwrap_or(0) + 2;
    let label_style = Style::default().fg(th.subtext1);
    let field = |index: usize, value: Vec<Span<'static>>| {
        let mut spans = vec![Span::styled(
            pad_right(&labels[index], label_width),
            label_style,
        )];
        spans.extend(value);
        Line::from(spans)
    };
    let value_columns = columns.saturating_sub(label_width).max(8);
    let wrapped = |index: usize, text: &str, style: Style| -> Vec<Line<'static>> {
        wrap(text, value_columns, FIELD_MAX_ROWS)
            .into_iter()
            .enumerate()
            .map(|(row, part)| {
                let label = if row == 0 { labels[index].as_str() } else { "" };
                Line::from(vec![
                    Span::styled(pad_right(label, label_width), label_style),
                    Span::styled(part, style),
                ])
            })
            .collect()
    };
    let text_style = Style::default().fg(th.text);

    let mut title = vec![Span::styled(
        setting_label(ctx, entry),
        Style::default().fg(th.text).add_modifier(Modifier::BOLD),
    )];
    if state.is_bookmarked(entry.key) {
        title.push(Span::styled(
            format!("  {}", ctx.t("details.starred")),
            Style::default().fg(th.yellow),
        ));
    }
    let current = ctx.values.value_for(entry);
    let default = ctx.values.default_for(entry);
    let mut default_spans = value_spans(ctx, entry, &default, false);
    if ctx.values.is_modified(entry) {
        default_spans.push(Span::styled(
            format!("  ● {}", ctx.t("details.modified")),
            Style::default().fg(th.yellow),
        ));
    }
    let mut lines = vec![
        Line::from(title),
        field(
            0,
            vec![Span::styled(
                entry.key.to_string(),
                Style::default().fg(th.overlay2),
            )],
        ),
    ];
    lines.extend(wrapped_field(
        &Span::styled(labels[1].clone(), label_style),
        label_width,
        value_spans(ctx, entry, &current, false),
        columns,
    ));
    lines.extend(wrapped_field(
        &Span::styled(labels[2].clone(), label_style),
        label_width,
        default_spans,
        columns,
    ));
    lines.extend(wrapped(3, &type_label(ctx, entry), text_style));
    lines.push(field(
        4,
        vec![Span::styled(
            reload_label(ctx, entry.reload),
            Style::default().fg(reload_color(ctx, entry.reload)),
        )],
    ));
    lines.extend(wrapped(
        5,
        &resolved_config_path(entry.file).display().to_string(),
        Style::default().fg(th.subtext0),
    ));
    let extra = ctx.values.extra_chords(entry);
    if !extra.is_empty() {
        lines.extend(wrapped(
            6,
            &format!(
                "{}  {}",
                extra.join(", "),
                ctx.t("details.extra_chords_note")
            ),
            text_style,
        ));
    }
    if entry.key.starts_with("pi_scan_")
        && let Some(chord) = ctx.app.keymap.config_editor_pi_scan_setup.first()
    {
        lines.push(field(7, pi_scan_setup_spans(ctx, chord)));
    }
    if let Some(summary) = setting_summary(ctx, entry) {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            summary,
            Style::default().fg(th.subtext0),
        )));
    }
    lines
}

/// Configured guided-setup chord as a key cap plus its explanation.
fn pi_scan_setup_spans(ctx: &FrameCtx<'_>, chord: &KeyChord) -> Vec<Span<'static>> {
    let th = ctx.th;
    vec![
        Span::styled(
            format!("[{}]", chord.label()),
            Style::default()
                .fg(th.text)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}", ctx.t("details.pi_scan_setup_hint")),
            Style::default().fg(th.text),
        ),
    ]
}

/// What: Wrap styled field values with continuation rows aligned to the value column.
///
/// Inputs: Localized label, label width, styled value spans, and available columns.
///
/// Output: At most four field rows, preserving value styles and color swatches.
///
/// Details: Long unbroken values wrap by display width; clipped values end in an ellipsis.
fn wrapped_field(
    label: &Span<'_>,
    label_width: usize,
    values: Vec<Span<'static>>,
    columns: usize,
) -> Vec<Line<'static>> {
    use unicode_width::UnicodeWidthChar as _;
    let indent = label_width.min(columns.saturating_sub(1));
    let value_columns = columns.saturating_sub(indent).max(1);
    let mut lines = Vec::new();
    let mut spans = vec![Span::styled(pad_right(&label.content, indent), label.style)];
    let mut used = 0;
    for span in values {
        for ch in span.content.chars() {
            let ch_width = ch.width().unwrap_or(0);
            if used + ch_width > value_columns {
                lines.push(Line::from(std::mem::take(&mut spans)));
                if lines.len() == FIELD_MAX_ROWS {
                    if let Some(last) = lines.last_mut().and_then(|line| line.spans.last_mut()) {
                        last.content = super::text::truncate(
                            &format!("{}…", last.content),
                            width(&last.content),
                        )
                        .into();
                    }
                    return lines;
                }
                spans.push(Span::raw(" ".repeat(indent)));
                used = 0;
            }
            if let Some(last) = spans.last_mut().filter(|last| last.style == span.style) {
                last.content.to_mut().push(ch);
            } else {
                spans.push(Span::styled(ch.to_string(), span.style));
            }
            used += ch_width;
        }
    }
    lines.push(Line::from(spans));
    lines
}
