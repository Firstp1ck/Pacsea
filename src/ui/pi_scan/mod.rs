//! Native keyboard-first Pi Scan workspace rendering.

mod details;
mod overview;
mod progress;
mod results;
mod setup;
mod targets;
mod wizard;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::state::pi_scan_ui::{PiScanNotice, PiScanNoticeSeverity};
use crate::state::{AppState, PiScanAvailability, PiScanView};
use crate::theme::theme;
use unicode_width::UnicodeWidthChar;

/// Number of commit-identity characters shown outside Technical Details.
const SHORT_IDENTITY_LENGTH: usize = 12;

/// Height of the always-visible Pi Scan notice and keybind footer.
const FOOTER_HEIGHT: u16 = 3;

/// Semantic emphasis shared by Pi Scan workspace renderers.
#[derive(Clone, Copy)]
pub(super) enum SemanticTone {
    /// Selected, active, or directly actionable content.
    Active,
    /// Complete, confirmed, or current content.
    Success,
    /// Pending, paused, incomplete, or warning content.
    Warning,
    /// Failed, invalid, disconnected, or critical content.
    Error,
    /// Supporting labels and secondary metadata.
    Muted,
    /// Ordinary primary content without semantic state.
    Normal,
}

/// What: Shorten one immutable identity for human-facing workspace rows.
///
/// Inputs:
/// - `identity`: Exact identity retained by scanner state.
///
/// Output:
/// - At most the first 12 Unicode scalar values.
///
/// Details:
/// - Exact commit identities remain available in Technical Details.
/// - Character iteration avoids slicing a potentially non-ASCII diagnostic value mid-codepoint.
pub(super) fn short_identity(identity: &str) -> String {
    identity.chars().take(SHORT_IDENTITY_LENGTH).collect()
}

/// What: Build a balanced Pi Scan section heading.
///
/// Inputs:
/// - `app`: Application state used for localization.
/// - `key`: Translation key for the heading.
///
/// Output:
/// - Mauve bold heading line shared across workspace pages.
///
/// Details:
/// - Callers own blank-line placement so compact terminals retain useful content.
pub(super) fn section_heading(app: &AppState, key: &str) -> Line<'static> {
    let th = theme();
    Line::from(Span::styled(
        crate::i18n::t(app, key),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    ))
}

/// What: Build one indented label/value row with semantic value emphasis.
///
/// Inputs:
/// - `label`: Human-facing localized label.
/// - `value`: Human-facing value including a textual or symbolic state cue.
/// - `tone`: Semantic color category for the value.
///
/// Output:
/// - A line with a muted label and semantically styled value.
///
/// Details:
/// - Color supplements rather than replaces the supplied textual state cue.
pub(super) fn labeled_line(
    label: impl Into<String>,
    value: impl Into<String>,
    tone: SemanticTone,
) -> Line<'static> {
    let th = theme();
    Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{}: ", label.into()),
            Style::default().fg(th.overlay1),
        ),
        Span::styled(value.into(), semantic_style(tone)),
    ])
}

/// Select the shared style for one semantic emphasis category.
pub(super) fn semantic_style(tone: SemanticTone) -> Style {
    let th = theme();
    let color = match tone {
        SemanticTone::Active => th.sapphire,
        SemanticTone::Success => th.green,
        SemanticTone::Warning => th.yellow,
        SemanticTone::Error => th.red,
        SemanticTone::Muted => th.subtext1,
        SemanticTone::Normal => th.text,
    };
    Style::default().fg(color)
}

/// What: Locate the top edge of the Pi Scan keybind footer.
///
/// Inputs:
/// - `area`: Rectangle assigned to the Pi Scan workspace
///
/// Output:
/// - First row reserved for the footer.
///
/// Details:
/// - Mirrors the fixed footer constraint used by [`render`].
pub(super) const fn keybind_footer_top(area: Rect) -> u16 {
    area.y
        .saturating_add(area.height.saturating_sub(FOOTER_HEIGHT))
}

/// Render the complete Pi Scan workspace at any terminal size.
pub fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .split(area);
    render_tabs(f, app, chunks[0]);
    match app.pi_scan.view {
        PiScanView::Setup => setup::render(f, app, chunks[1]),
        PiScanView::Overview => overview::render(f, app, chunks[1]),
        PiScanView::Targets => targets::render(f, app, chunks[1]),
        PiScanView::Progress => progress::render(f, app, chunks[1]),
        PiScanView::Results => results::render(f, app, chunks[1]),
        PiScanView::Details => details::render(f, app, chunks[1]),
    }
    render_footer(f, app, chunks[2]);
}

/// Draw tabs and record mouse hit rectangles.
fn render_tabs(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let mut spans = vec![Span::styled(
        format!(" {}  ", crate::i18n::t(app, "app.pi_scan.title")),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )];
    let labels = [
        "setup", "overview", "targets", "progress", "results", "details",
    ];
    let mut x = area
        .x
        .saturating_add(u16::try_from(spans[0].content.chars().count()).unwrap_or(0));
    for (index, label) in labels.iter().enumerate() {
        let mut tab_label = crate::i18n::t(app, &format!("app.pi_scan.tabs.{label}"));
        if *label == "results" && app.pi_scan.unseen_result_count > 0 {
            tab_label = format!("{tab_label} ({})", app.pi_scan.unseen_result_count);
        }
        let text = format!(" {tab_label} ");
        let width = u16::try_from(text.chars().count()).unwrap_or(0);
        app.pi_scan.tab_rects[index] =
            (x < area.x.saturating_add(area.width)).then_some((x, area.y, width, 1));
        let style = if app.pi_scan.view.index() == index {
            Style::default()
                .fg(th.base)
                .bg(th.sapphire)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.subtext1)
        };
        spans.push(Span::styled(text, style));
        x = x.saturating_add(width);
    }
    let status = match app.pi_scan.availability {
        PiScanAvailability::Disabled => crate::i18n::t(app, "app.pi_scan.status.disabled"),
        PiScanAvailability::Unsupported => crate::i18n::t(app, "app.pi_scan.status.unsupported"),
        PiScanAvailability::MissingBinary => crate::i18n::t(app, "app.pi_scan.status.missing_pi"),
        PiScanAvailability::RuntimeDisconnected => {
            crate::i18n::t(app, "app.pi_scan.status.disconnected")
        }
        PiScanAvailability::RuntimeConnected => crate::i18n::t(app, "app.pi_scan.status.connected"),
    };
    spans.push(Span::styled(
        format!("  {status}"),
        Style::default().fg(availability_color(&app.pi_scan.availability, &th)),
    ));
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

/// Draw severity-styled foreground feedback, background suffix, and contextual keys.
fn render_footer(f: &mut Frame, app: &AppState, area: Rect) {
    let th = theme();
    let wizard_active = app.pi_scan.wizard.is_some();
    let (notice, notice_color) = if wizard_active {
        (
            crate::i18n::t(app, "app.pi_scan.wizard.footer.notice"),
            th.yellow,
        )
    } else if let Some(notice) = app.pi_scan.notices.foreground.as_ref() {
        (localized_notice(app, notice), notice_color(notice, &th))
    } else {
        (crate::i18n::t(app, "app.pi_scan.footer.notice"), th.yellow)
    };
    let mut notice_spans = vec![Span::styled(notice, Style::default().fg(notice_color))];
    if let Some(background) = app.pi_scan.notices.background.as_ref() {
        notice_spans.push(Span::styled(
            format!(
                " · {}: {}",
                crate::i18n::t(app, "app.pi_scan.footer.background"),
                localized_notice(app, background)
            ),
            Style::default().fg(th.overlay1).add_modifier(Modifier::DIM),
        ));
    }
    let key = if wizard_active {
        "app.pi_scan.wizard.footer.keys"
    } else {
        match app.pi_scan.view {
            PiScanView::Setup => "app.pi_scan.footer.keys.setup",
            PiScanView::Overview => "app.pi_scan.footer.keys.overview",
            PiScanView::Targets => "app.pi_scan.footer.keys.targets",
            PiScanView::Progress => "app.pi_scan.footer.keys.progress",
            PiScanView::Results => "app.pi_scan.footer.keys.results",
            PiScanView::Details => "app.pi_scan.footer.keys.details",
        }
    };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(notice_spans),
            Line::from(crate::i18n::t(app, key)),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .title(crate::i18n::t(app, "app.pi_scan.footer.title")),
        ),
        area,
    );
}

/// Localize a state-owned notice key while preserving bounded runtime detail strings.
pub(super) fn localized_notice(app: &AppState, notice: &PiScanNotice) -> String {
    if notice.text.starts_with("app.pi_scan.") {
        return crate::i18n::t(app, &notice.text);
    }
    let key = match notice.text.as_str() {
        "A previous Pi Scan setup Apply is still resolving; wait for its rollback or completion" => {
            Some("app.pi_scan.notices.setup_still_resolving")
        }
        "Guided setup cancelled — press r to restart it, Esc to leave" => {
            Some("app.pi_scan.notices.guided_setup_cancelled")
        }
        "Apply in progress — press Esc again to abandon and roll back" => {
            Some("app.pi_scan.notices.apply_abandon_warning")
        }
        "Pi Scan setup abandonment requested; waiting for explicit rollback" => {
            Some("app.pi_scan.notices.apply_abandon_requested")
        }
        _ => None,
    };
    key.map_or_else(|| notice.text.clone(), |key| crate::i18n::t(app, key))
}

/// Select the semantic foreground color for one workspace availability state.
const fn availability_color(availability: &PiScanAvailability, th: &crate::theme::Theme) -> Color {
    match availability {
        PiScanAvailability::RuntimeConnected => th.green,
        PiScanAvailability::Disabled => th.yellow,
        PiScanAvailability::Unsupported
        | PiScanAvailability::MissingBinary
        | PiScanAvailability::RuntimeDisconnected => th.red,
    }
}

/// Select a semantic foreground color for one notice severity.
pub(super) const fn notice_color(notice: &PiScanNotice, th: &crate::theme::Theme) -> Color {
    match notice.severity {
        PiScanNoticeSeverity::Info => th.sapphire,
        PiScanNoticeSeverity::Success => th.green,
        PiScanNoticeSeverity::Warning => th.yellow,
        PiScanNoticeSeverity::Error => th.red,
    }
}

/// What: Format a token count with grouped decimal digits.
///
/// Inputs:
/// - `value`: Exact non-negative token count.
///
/// Output:
/// - Locale-neutral grouped digits suitable for the compact terminal UI.
///
/// Details:
/// - Uses commas consistently because the surrounding translated label carries the locale context.
pub(super) fn format_token_count(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

/// What: Format an exact integer micro-USD amount as readable USD.
///
/// Inputs:
/// - `value`: Exact amount in millionths of one US dollar.
///
/// Output:
/// - Dollar text with two to six fractional digits and an explicit USD suffix.
///
/// Details:
/// - Integer arithmetic preserves micro-dollar precision and avoids floating-point rounding.
pub(super) fn format_microusd(value: u64) -> String {
    let dollars = value / 1_000_000;
    let mut fraction = format!("{:06}", value % 1_000_000);
    while fraction.len() > 2 && fraction.ends_with('0') {
        fraction.pop();
    }
    format!("${dollars}.{fraction} USD")
}

/// Render a titled body paragraph shared by workspace pages.
pub(super) fn body(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    title_key: &str,
    lines: Vec<Line<'static>>,
) {
    body_scrolled(f, app, area, title_key, lines, 0);
}

/// Render a titled body paragraph at one clamped line offset.
pub(super) fn body_scrolled(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    title_key: &str,
    lines: Vec<Line<'static>>,
    scroll: u16,
) {
    let th = theme();
    f.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(th.text).bg(th.base))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((scroll, 0))
            .block(Block::default().borders(Borders::ALL).title(Span::styled(
                crate::i18n::t(app, title_key),
                Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
            ))),
        area,
    );
}

/// Clamp a line offset against unwrapped content and the current inner height.
pub(super) fn clamp_line_scroll(offset: u16, lines: &[Line<'static>], area: Rect) -> u16 {
    let viewport = usize::from(area.height.saturating_sub(2)).max(1);
    let width = usize::from(area.width.saturating_sub(2).max(1));
    let wrapped_lines = lines
        .iter()
        .map(|line| wrapped_line_count(line, width))
        .sum::<usize>();
    let maximum = wrapped_lines.saturating_sub(viewport);
    offset.min(u16::try_from(maximum).unwrap_or(u16::MAX))
}

/// Count Ratatui-style word-wrapped display rows for scrolling and mouse hit seams.
pub(super) fn wrapped_line_count(line: &Line<'static>, width: usize) -> usize {
    let text = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    if text.is_empty() {
        return 1;
    }
    let mut count = 1usize;
    let mut used = 0usize;
    let mut pending_space = 0usize;
    let mut word = 0usize;
    for character in text.chars().chain(std::iter::once(' ')) {
        let character_width = character.width().unwrap_or(0);
        if character.is_whitespace() {
            if word > 0 {
                pack_word(width, &mut count, &mut used, pending_space, word);
                pending_space = character_width;
                word = 0;
            } else {
                pending_space = pending_space.saturating_add(character_width);
            }
        } else {
            word = word.saturating_add(character_width);
        }
    }
    count
}

/// Pack one display-width word into a wrapped line counter.
const fn pack_word(
    width: usize,
    count: &mut usize,
    used: &mut usize,
    pending_space: usize,
    word: usize,
) {
    if *used > 0 && used.saturating_add(pending_space).saturating_add(word) > width {
        *count = count.saturating_add(1);
        *used = 0;
    }
    let total = used.saturating_add(if *used == 0 { 0 } else { pending_space });
    let total = total.saturating_add(word);
    *count = count.saturating_add(total.saturating_sub(1) / width);
    *used = total % width;
    if *used == 0 && total > 0 {
        *used = width;
    }
}

#[cfg(test)]
mod tests {
    use super::{format_microusd, format_token_count, short_identity};

    /// Human-facing identities use twelve characters without splitting Unicode values.
    #[test]
    fn short_identity_is_deterministic_and_unicode_safe() {
        assert_eq!(short_identity("0123456789abcdef"), "0123456789ab");
        assert_eq!(short_identity("abc"), "abc");
        assert_eq!(short_identity("áéíóöőúüűxyzmore"), "áéíóöőúüűxyz");
    }

    /// Token counts remain readable at zero and across grouping boundaries.
    #[test]
    fn token_count_uses_decimal_grouping() {
        assert_eq!(format_token_count(0), "0");
        assert_eq!(format_token_count(999), "999");
        assert_eq!(format_token_count(1_000), "1,000");
        assert_eq!(format_token_count(500_000), "500,000");
        assert_eq!(format_token_count(u64::MAX), "18,446,744,073,709,551,615");
    }

    /// Micro-dollar formatting stays exact while omitting meaningless trailing zeros.
    #[test]
    fn microusd_is_displayed_as_exact_usd() {
        assert_eq!(format_microusd(0), "$0.00 USD");
        assert_eq!(format_microusd(125), "$0.000125 USD");
        assert_eq!(format_microusd(600_000), "$0.60 USD");
        assert_eq!(format_microusd(25_000_000), "$25.00 USD");
    }
}
