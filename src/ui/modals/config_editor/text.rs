//! Display-width helpers shared by the config editor renderer.
//!
//! All helpers measure terminal columns with `unicode-width`, so wide glyphs
//! and combining characters line up with what ratatui draws.

use ratatui::text::Span;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Ellipsis appended when text is cut to fit a column.
const ELLIPSIS: char = '…';

/// Terminal columns occupied by `text`.
pub(super) fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Total terminal columns occupied by a span sequence.
pub(super) fn spans_width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// What: Cut `text` so it fits into `max` columns.
///
/// Inputs:
/// - `text`: Text to fit.
/// - `max`: Maximum columns.
///
/// Output:
/// - `text` unchanged when it fits, otherwise a prefix ending in `…`.
///
/// Details:
/// - `max == 0` yields an empty string; the ellipsis counts toward `max`.
pub(super) fn truncate(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width + 1 > max {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push(ELLIPSIS);
    out
}

/// Truncate and right-pad `text` to exactly `columns` terminal columns.
pub(super) fn pad_right(text: &str, columns: usize) -> String {
    let cut = truncate(text, columns);
    let fill = columns.saturating_sub(width(&cut));
    format!("{cut}{}", " ".repeat(fill))
}

/// What: Fit a span sequence into `max` columns.
///
/// Inputs:
/// - `spans`: Styled spans in display order.
/// - `max`: Maximum total columns.
///
/// Output:
/// - Spans whose combined width is at most `max`; the last kept span is cut with `…`.
///
/// Details:
/// - Styles are preserved; spans after the cut are dropped.
pub(super) fn fit_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    let mut out = Vec::with_capacity(spans.len());
    let mut used = 0usize;
    for span in spans {
        let span_width = span.width();
        if used + span_width <= max {
            used += span_width;
            out.push(span);
            continue;
        }
        let cut = truncate(&span.content, max.saturating_sub(used));
        if !cut.is_empty() {
            out.push(Span::styled(cut, span.style));
        }
        break;
    }
    out
}

/// What: Word-wrap `text` into rows of at most `columns` terminal columns.
///
/// Inputs:
/// - `text`: Text to wrap (single paragraph).
/// - `columns`: Row width.
/// - `max_rows`: Maximum rows to return; the final row is truncated with `…` when cut.
///
/// Output:
/// - At least one row (possibly empty) when `max_rows > 0`.
///
/// Details:
/// - Used where the renderer needs an exact row count (popup sections), unlike
///   ratatui's `Wrap`, which does not report how many rows it produced.
/// - Words longer than a row are hard-cut so a row never exceeds `columns`.
pub(super) fn wrap(text: &str, columns: usize, max_rows: usize) -> Vec<String> {
    if max_rows == 0 {
        return Vec::new();
    }
    let columns = columns.max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate_width = if current.is_empty() {
            width(word)
        } else {
            width(&current) + 1 + width(word)
        };
        if candidate_width <= columns {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }
        if !current.is_empty() {
            rows.push(std::mem::take(&mut current));
        }
        current = hard_cut(word, columns, &mut rows);
    }
    if !current.is_empty() || rows.is_empty() {
        rows.push(current);
    }
    if rows.len() > max_rows {
        let rest = rows[max_rows - 1..].join(" ");
        rows.truncate(max_rows - 1);
        rows.push(truncate(&rest, columns));
    }
    rows
}

/// Split an over-long word into full rows, returning the remainder that still fits.
fn hard_cut(word: &str, columns: usize, rows: &mut Vec<String>) -> String {
    let mut current = String::new();
    let mut used = 0usize;
    for ch in word.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width > columns && !current.is_empty() {
            rows.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(ch);
        used += ch_width;
    }
    current
}

/// What: Select the visible part of a single-line input so the caret stays in view.
///
/// Inputs:
/// - `text`: Full input text.
/// - `caret`: Caret position in characters.
/// - `columns`: Visible columns.
///
/// Output:
/// - `(visible_text, caret_offset)` where `caret_offset` is the caret column inside the view.
///
/// Details:
/// - Scrolls by whole characters so the caret cell is always inside `columns`.
pub(super) fn input_window(text: &str, caret: usize, columns: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let caret = caret.min(chars.len());
    let columns = columns.max(1);
    let mut start = 0usize;
    while start < caret && column_span(&chars[start..caret]) >= columns {
        start += 1;
    }
    let mut visible = String::new();
    let mut used = 0usize;
    for ch in &chars[start..] {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width > columns {
            break;
        }
        visible.push(*ch);
        used += ch_width;
    }
    (visible, column_span(&chars[start..caret]))
}

/// Terminal columns of a character slice.
fn column_span(chars: &[char]) -> usize {
    chars.iter().map(|ch| ch.width().unwrap_or(0)).sum()
}
