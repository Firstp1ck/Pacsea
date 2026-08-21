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
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
    if app.pi_scan.budget_dialog.is_some() {
        render_budget_dialog(f, app, area);
    }
}

/// What: Render the focused responsive Double/Unlimited budget choice above the workspace.
///
/// Inputs:
/// - `f`: Target frame.
/// - `app`: Workspace with deterministic dialog state and localized copy.
/// - `area`: Full Pi Scan workspace rectangle.
///
/// Output:
/// - A centered overlay containing affected limits, old-to-proposed values, warning, and status.
///
/// Details:
/// - The overlay remains bounded at narrow dimensions and never mutates scheduler projection.
fn render_budget_dialog(f: &mut Frame, app: &AppState, area: Rect) {
    let Some(dialog) = app.pi_scan.budget_dialog.as_ref() else {
        return;
    };
    let overlay = budget_dialog_area(area);
    let mut lines = vec![Line::from(crate::i18n::t(
        app,
        "app.pi_scan.budget_dialog.prompt",
    ))];
    for dimension in &dialog.affected {
        lines.push(budget_change_line(app, dialog, *dimension));
    }
    lines.push(Line::from(""));
    lines.push(budget_choice_line(
        app,
        dialog,
        crate::state::pi_scan::PiScanBudgetAdjustment::Double,
        "app.pi_scan.budget_dialog.double",
    ));
    lines.push(budget_choice_line(
        app,
        dialog,
        crate::state::pi_scan::PiScanBudgetAdjustment::Unlimited,
        "app.pi_scan.budget_dialog.unlimited",
    ));
    lines.push(Line::from(Span::styled(
        format!(
            "⚠ {}",
            crate::i18n::t(app, "app.pi_scan.budget_dialog.unlimited_warning")
        ),
        semantic_style(SemanticTone::Warning),
    )));
    push_budget_dialog_status(&mut lines, app, dialog);
    lines.push(Line::from(crate::i18n::t(
        app,
        "app.pi_scan.budget_dialog.keys",
    )));
    f.render_widget(Clear, overlay);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .title(crate::i18n::t(app, "app.pi_scan.budget_dialog.title")),
        ),
        overlay,
    );
}

/// What: Compute a centered budget-dialog rectangle bounded by the current terminal.
///
/// Inputs:
/// - `area`: Full available workspace.
///
/// Output:
/// - Centered rectangle using at most 76 columns and 16 rows.
///
/// Details:
/// - Saturating arithmetic keeps 20x10 and smaller renders valid.
const fn budget_dialog_area(area: Rect) -> Rect {
    let width = if area.width < 76 { area.width } else { 76 };
    let height = if area.height < 16 { area.height } else { 16 };
    Rect {
        x: area.x.saturating_add(area.width.saturating_sub(width) / 2),
        y: area
            .y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    }
}

/// What: Render one scheduler-classified limit with its visible old and proposed value.
///
/// Inputs:
/// - `app`: Localization state.
/// - `dialog`: Open-time limit snapshot and selected policy.
/// - `dimension`: One affected budget dimension.
///
/// Output:
/// - Localized label and `old → proposed` line.
///
/// Details:
/// - Checked-overflow proposals remain explicit instead of silently clamping.
fn budget_change_line(
    app: &AppState,
    dialog: &crate::state::pi_scan_ui::PiScanBudgetDialogState,
    dimension: crate::state::pi_scan::PiScanBudgetDimension,
) -> Line<'static> {
    let old = budget_limit_value(app, dimension, dialog.previous_limits);
    let proposed = proposed_budget_limit_value(app, dialog, dimension);
    labeled_line(
        crate::i18n::t(app, budget_dimension_key(dimension)),
        format!("{old} → {proposed}"),
        SemanticTone::Normal,
    )
}

/// What: Format the currently selected proposal for one affected limit.
///
/// Inputs:
/// - `app`: Localization state.
/// - `dialog`: Selected policy and open-time limits.
/// - `dimension`: Affected dimension.
///
/// Output:
/// - Exact doubled value, Unlimited, or explicit overflow text.
///
/// Details:
/// - This is presentation-only; WS1 remains authoritative for Apply-time arithmetic.
fn proposed_budget_limit_value(
    app: &AppState,
    dialog: &crate::state::pi_scan_ui::PiScanBudgetDialogState,
    dimension: crate::state::pi_scan::PiScanBudgetDimension,
) -> String {
    use crate::state::pi_scan::{PiScanBudgetAdjustment, PiScanBudgetDimension};
    if dialog.selection == PiScanBudgetAdjustment::Unlimited {
        return crate::i18n::t(app, "app.pi_scan.common.unlimited");
    }
    let mut proposed = dialog.previous_limits;
    let doubled = match dimension {
        PiScanBudgetDimension::Starts => proposed
            .starts_per_hour
            .checked_mul(2)
            .map(|value| proposed.starts_per_hour = value),
        PiScanBudgetDimension::Tokens => proposed
            .tokens_per_24h
            .checked_mul(2)
            .map(|value| proposed.tokens_per_24h = value),
        PiScanBudgetDimension::Cost => proposed
            .cost_microusd_per_24h
            .checked_mul(2)
            .map(|value| proposed.cost_microusd_per_24h = value),
    };
    doubled.map_or_else(
        || crate::i18n::t(app, "app.pi_scan.budget_dialog.overflow"),
        |()| budget_limit_value(app, dimension, proposed),
    )
}

/// What: Render one selectable adjustment option with visible keyboard focus.
///
/// Inputs:
/// - `app`: Localization state.
/// - `dialog`: Current selection/status.
/// - `choice`: Option represented by this line.
/// - `key`: Localized option label key.
///
/// Output:
/// - Focus-marked option line.
///
/// Details:
/// - Focus remains visible while submitting or showing a rejection.
fn budget_choice_line(
    app: &AppState,
    dialog: &crate::state::pi_scan_ui::PiScanBudgetDialogState,
    choice: crate::state::pi_scan::PiScanBudgetAdjustment,
    key: &str,
) -> Line<'static> {
    let selected = dialog.selection == choice;
    Line::from(Span::styled(
        format!(
            "  {} {}",
            if selected { "▶" } else { " " },
            crate::i18n::t(app, key)
        ),
        semantic_style(if selected {
            SemanticTone::Active
        } else {
            SemanticTone::Muted
        }),
    ))
}

/// What: Append pending or rejected runtime status to the budget dialog.
///
/// Inputs:
/// - `lines`: Overlay output buffer.
/// - `app`: Localization state.
/// - `dialog`: Current submission status and optional rejection.
///
/// Output:
/// - No line while choosing, or a visible pending/rejected status line.
///
/// Details:
/// - Rejection detail remains actionable and does not close the choice.
fn push_budget_dialog_status(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    dialog: &crate::state::pi_scan_ui::PiScanBudgetDialogState,
) {
    use crate::state::pi_scan_ui::PiScanBudgetDialogStatus;
    match dialog.status {
        PiScanBudgetDialogStatus::Choosing => {}
        PiScanBudgetDialogStatus::Submitting => lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.pi_scan.budget_dialog.submitting"),
            semantic_style(SemanticTone::Active),
        ))),
        PiScanBudgetDialogStatus::Rejected => lines.push(Line::from(Span::styled(
            format!(
                "{}: {}",
                crate::i18n::t(app, "app.pi_scan.budget_dialog.rejected"),
                dialog.rejection.as_deref().unwrap_or_default()
            ),
            semantic_style(SemanticTone::Error),
        ))),
    }
}

/// Map one budget dimension to its shared localized label.
const fn budget_dimension_key(
    dimension: crate::state::pi_scan::PiScanBudgetDimension,
) -> &'static str {
    match dimension {
        crate::state::pi_scan::PiScanBudgetDimension::Starts => "app.pi_scan.progress.limit_starts",
        crate::state::pi_scan::PiScanBudgetDimension::Tokens => "app.pi_scan.progress.limit_tokens",
        crate::state::pi_scan::PiScanBudgetDimension::Cost => "app.pi_scan.progress.limit_cost",
    }
}

/// What: Format one runtime-owned budget limit with truthful Unlimited semantics.
///
/// Inputs:
/// - `app`: Localization state.
/// - `dimension`: Starts, tokens, or cost.
/// - `limits`: Current authoritative runtime limits.
///
/// Output:
/// - Grouped finite value or localized Unlimited for numeric zero.
///
/// Details:
/// - Cost remains exact integer micro-USD and finite starts include the rolling unit.
pub(super) fn budget_limit_value(
    app: &AppState,
    dimension: crate::state::pi_scan::PiScanBudgetDimension,
    limits: crate::state::pi_scan::PiScanBudgetLimits,
) -> String {
    use crate::state::pi_scan::PiScanBudgetDimension;
    let unlimited = match dimension {
        PiScanBudgetDimension::Starts => limits.starts_per_hour == 0,
        PiScanBudgetDimension::Tokens => limits.tokens_per_24h == 0,
        PiScanBudgetDimension::Cost => limits.cost_microusd_per_24h == 0,
    };
    if unlimited {
        return crate::i18n::t(app, "app.pi_scan.common.unlimited");
    }
    match dimension {
        PiScanBudgetDimension::Starts => format!("{}/h", limits.starts_per_hour),
        PiScanBudgetDimension::Tokens => format_token_count(limits.tokens_per_24h),
        PiScanBudgetDimension::Cost => format_microusd(limits.cost_microusd_per_24h),
    }
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
