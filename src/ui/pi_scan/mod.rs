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
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    app.pi_scan.budget_dialog_rect = app.pi_scan.budget_dialog.as_ref().map(|_| {
        let rect = budget_dialog_area(area);
        (rect.x, rect.y, rect.width, rect.height)
    });
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
fn render_budget_dialog(f: &mut Frame, app: &mut AppState, area: Rect) {
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
    lines.push(Line::from(Span::styled(
        format!(
            "⚠ {}",
            crate::i18n::t(app, "app.pi_scan.budget_dialog.unlimited_warning")
        ),
        semantic_style(SemanticTone::Warning),
    )));
    push_budget_dialog_status(&mut lines, app, dialog);
    let body = Rect::new(
        overlay.x.saturating_add(1),
        overlay.y.saturating_add(1),
        overlay.width.saturating_sub(2),
        overlay.height.saturating_sub(6),
    );
    f.render_widget(Clear, overlay);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .title(crate::i18n::t(app, "app.pi_scan.budget_dialog.title")),
        overlay,
    );
    let choices = vec![
        budget_choice_line(
            app,
            dialog,
            crate::state::pi_scan::PiScanBudgetAdjustment::Double,
            "app.pi_scan.budget_dialog.double",
        ),
        budget_choice_line(
            app,
            dialog,
            crate::state::pi_scan::PiScanBudgetAdjustment::Unlimited,
            "app.pi_scan.budget_dialog.unlimited",
        ),
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.budget_dialog.hint_navigation",
        )),
        Line::from(crate::i18n::t(
            app,
            "app.pi_scan.budget_dialog.hint_actions",
        )),
    ];
    let pinned = Rect::new(
        body.x,
        overlay.y.saturating_add(overlay.height.saturating_sub(5)),
        body.width,
        4,
    );
    let body_width = usize::from(body.width.max(1));
    let line_count: usize = lines
        .iter()
        .map(|line| wrapped_line_count(line, body_width))
        .sum();
    let body_widget = Paragraph::new(lines).wrap(Wrap { trim: false });
    let max_scroll =
        u16::try_from(line_count.saturating_sub(usize::from(body.height))).unwrap_or(u16::MAX);
    app.pi_scan.budget_body_scroll = app.pi_scan.budget_body_scroll.min(max_scroll);
    app.pi_scan.budget_body_overflows = max_scroll > 0;
    f.render_widget(
        body_widget.scroll((app.pi_scan.budget_body_scroll, 0)),
        body,
    );
    f.render_widget(Paragraph::new(choices), pinned);
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

/// What: Draw the numbered workflow tab bar and the workspace status pill.
///
/// Inputs:
/// - `f`: Target frame.
/// - `app`: Workspace state; tab hit rectangles are recorded here.
/// - `area`: Two-row tab-bar rectangle.
///
/// Output:
/// - Renders tabs and records mouse hit rectangles in tab-bar order.
///
/// Details:
/// - Full labels are preferred, then short labels, then numbers only with the active
///   page name on a second row. The active tab carries a `▸` marker so the selection
///   does not depend on color alone.
fn render_tabs(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let tabs = PiScanView::tabs();
    let mut full: Vec<String> = tabs
        .iter()
        .map(|view| crate::i18n::t(app, &format!("app.pi_scan.tabs.{}", view.tab_key())))
        .collect();
    let mut short: Vec<String> = tabs
        .iter()
        .map(|view| crate::i18n::t(app, &format!("app.pi_scan.tabs_short.{}", view.tab_key())))
        .collect();
    let results_tab = PiScanView::Results.tab_position();
    if app.pi_scan.unseen_result_count > 0 {
        let badge = format!(" ({})", app.pi_scan.unseen_result_count);
        full[results_tab].push_str(&badge);
        short[results_tab].push_str(&badge);
    }
    let active = app.pi_scan.view.tab_position();
    let status = workspace_status(app);
    let (pills, status_tone) = (status.variants, status.tone);
    let available = usize::from(area.width);
    let row_width =
        |labels: &[String]| -> usize { labels.iter().map(|label| label.width() + 5).sum() };
    let numbers_width = tabs.len() * 4;
    let pill_widths: Vec<usize> = pills.iter().map(|pill| pill.width()).collect();
    let (tier, pill) = choose_tab_layout(
        [row_width(&full), row_width(&short), numbers_width],
        &pill_widths,
        status.reports_work,
        available,
    );
    let compact = tier == TabLabelTier::Numbers;
    let active_label = full[active].clone();
    let labels = match tier {
        TabLabelTier::Full => full,
        TabLabelTier::Short => short,
        TabLabelTier::Numbers => vec![String::new(); tabs.len()],
    };
    let mut spans = Vec::new();
    let mut x = area.x;
    for (index, label) in labels.iter().enumerate() {
        let marker = if active == index { "▸" } else { " " };
        let unseen = index == results_tab && app.pi_scan.unseen_result_count > 0;
        let text = if label.is_empty() {
            format!("{marker}{}{} ", index + 1, if unseen { "•" } else { "" })
        } else {
            format!("{marker}{} {label} ", index + 1)
        };
        let width = u16::try_from(text.width()).unwrap_or(u16::MAX);
        app.pi_scan.tab_rects[index] = (x.saturating_add(width)
            <= area.x.saturating_add(area.width))
        .then_some((x, area.y, width, 1));
        let style = if active == index {
            Style::default()
                .fg(th.base)
                .bg(th.sapphire)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.subtext1)
        };
        spans.push(Span::styled(text, style));
        spans.push(Span::raw(" "));
        x = x.saturating_add(width).saturating_add(1);
    }
    let used = usize::from(x.saturating_sub(area.x));
    if let Some(status) = pill.and_then(|index| pills.get(index)) {
        spans.push(Span::raw(
            " ".repeat(available.saturating_sub(used + status.width())),
        ));
        spans.push(Span::styled(
            status.clone(),
            semantic_style(status_tone).add_modifier(Modifier::BOLD),
        ));
    }
    if compact {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(spans),
                Line::from(Span::styled(
                    active_label,
                    Style::default()
                        .fg(th.sapphire)
                        .add_modifier(Modifier::BOLD),
                )),
            ]),
            area,
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(spans)).block(Block::default().borders(Borders::BOTTOM)),
            area,
        );
    }
}

/// Label length used for the workspace tab bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TabLabelTier {
    /// Full localized page names.
    Full,
    /// Short localized page names.
    Short,
    /// Page numbers only, with the active name on a second row.
    Numbers,
}

/// What: Pick the tab label length and status-pill variant that fit one row.
///
/// Inputs:
/// - `rows`: Tab-row widths for full, short, and numbers-only labels, including gaps.
/// - `pills`: Status-pill widths, most detailed first.
/// - `pill_essential`: Whether the pill reports running, waiting, or paused work.
/// - `available`: Tab-bar width.
///
/// Output:
/// - Chosen label tier and index of the pill that fits, if any.
///
/// Details:
/// - A work pill is the only always-visible scan progress, so tabs shorten and the pill
///   drops its detail suffix before the pill itself is dropped. Readable tab names win
///   over pill detail at the same tier.
/// - An idle readiness pill never costs tab readability; it is shown only if it fits.
fn choose_tab_layout(
    rows: [usize; 3],
    pills: &[usize],
    pill_essential: bool,
    available: usize,
) -> (TabLabelTier, Option<usize>) {
    let tiers = [
        TabLabelTier::Full,
        TabLabelTier::Short,
        TabLabelTier::Numbers,
    ];
    let pill_fitting = |row: usize| pills.iter().position(|pill| row + 1 + pill <= available);
    if pill_essential {
        for (tier, row) in tiers.into_iter().zip(rows) {
            if let Some(index) = pill_fitting(row).filter(|_| row <= available) {
                return (tier, Some(index));
            }
        }
    }
    let (tier, row) = tiers
        .into_iter()
        .zip(rows)
        .find(|(_, row)| *row <= available)
        .unwrap_or((TabLabelTier::Numbers, rows[2]));
    (tier, pill_fitting(row))
}

/// What: Summarize what Pi Scan is doing right now in a few plain words.
///
/// Inputs:
/// - `app`: Workspace runtime projection and availability.
///
/// Output:
/// - Symbol-prefixed status variants, most detailed first, with tone and work flag.
///
/// Details:
/// - Running, paused, and waiting work wins over readiness so scan progress stays
///   visible from every page. Each state pairs a symbol and a word with color.
fn workspace_status(app: &AppState) -> WorkspaceStatus {
    let runtime = &app.pi_scan.runtime;
    let waiting = runtime.queue.len();
    let has_work = runtime.active.is_some() || waiting > 0;
    if has_work && !runtime.pause_reasons.is_empty() {
        return WorkspaceStatus::work(
            vec![format!(
                "⏸ {}",
                crate::i18n::t(app, "app.pi_scan.status.paused")
            )],
            SemanticTone::Warning,
        );
    }
    if let Some(active) = runtime.active.as_ref() {
        let scanning = format!(
            "⟳ {}",
            crate::i18n::t_fmt1(
                app,
                "app.pi_scan.status.scanning",
                &active.request.key.package_base
            )
        );
        if waiting == 0 {
            return WorkspaceStatus::work(vec![scanning], SemanticTone::Active);
        }
        let detailed = format!(
            "{scanning} · {}",
            crate::i18n::t_fmt1(app, "app.pi_scan.status.waiting", waiting)
        );
        return WorkspaceStatus::work(vec![detailed, scanning], SemanticTone::Active);
    }
    if waiting > 0 {
        return WorkspaceStatus::work(
            vec![format!(
                "… {}",
                crate::i18n::t_fmt1(app, "app.pi_scan.status.waiting", waiting)
            )],
            SemanticTone::Active,
        );
    }
    let (symbol, key, tone) = match app.pi_scan.availability {
        PiScanAvailability::RuntimeConnected => {
            ("✔", "app.pi_scan.status.connected", SemanticTone::Success)
        }
        PiScanAvailability::Disabled => ("○", "app.pi_scan.status.disabled", SemanticTone::Warning),
        PiScanAvailability::Unsupported => {
            ("✖", "app.pi_scan.status.unsupported", SemanticTone::Error)
        }
        PiScanAvailability::MissingBinary => {
            ("✖", "app.pi_scan.status.missing_pi", SemanticTone::Error)
        }
        PiScanAvailability::RuntimeDisconnected => {
            ("✖", "app.pi_scan.status.disconnected", SemanticTone::Error)
        }
    };
    WorkspaceStatus {
        variants: vec![format!("{symbol} {}", crate::i18n::t(app, key))],
        tone,
        reports_work: false,
    }
}

/// Status-pill text variants for the tab bar.
struct WorkspaceStatus {
    /// Pill texts, most detailed first; later entries are shorter fallbacks.
    variants: Vec<String>,
    /// Semantic tone shared by every variant.
    tone: SemanticTone,
    /// Whether the pill reports running, waiting, or paused work rather than readiness.
    reports_work: bool,
}

impl WorkspaceStatus {
    /// Build a pill that reports active, waiting, or paused scan work.
    const fn work(variants: Vec<String>, tone: SemanticTone) -> Self {
        Self {
            variants,
            tone,
            reports_work: true,
        }
    }
}

/// What: Draw the footer: current feedback or a next-step tip, then contextual keys.
///
/// Inputs:
/// - `f`: Target frame.
/// - `app`: Workspace state.
/// - `area`: Footer rectangle of [`FOOTER_HEIGHT`] rows.
///
/// Output:
/// - Renders one feedback row and one key-hint row below a separator.
///
/// Details:
/// - A foreground notice wins; otherwise a page-specific tip tells the user what to do
///   next, so the footer never repeats static disclaimers.
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
        (
            format!("» {}", crate::i18n::t(app, footer_tip_key(app))),
            th.subtext1,
        )
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
    let hints = footer_hints(app, usize::from(area.width.saturating_sub(2)));
    f.render_widget(
        Paragraph::new(vec![Line::from(notice_spans), hints])
            .block(Block::default().borders(Borders::TOP)),
        area,
    );
}

/// What: Choose the next-step tip for the current page and state.
///
/// Inputs:
/// - `app`: Workspace state.
///
/// Output:
/// - Translation key under `app.pi_scan.footer.tips`.
///
/// Details:
/// - Empty pages point to the page or action that fills them.
fn footer_tip_key(app: &AppState) -> &'static str {
    let pi = &app.pi_scan;
    match pi.view {
        PiScanView::Targets if pi.targets.is_empty() => "app.pi_scan.footer.tips.scan_empty",
        PiScanView::Targets => "app.pi_scan.footer.tips.scan",
        PiScanView::Progress if pi.runtime.active.is_some() || !pi.runtime.queue.is_empty() => {
            "app.pi_scan.footer.tips.progress_running"
        }
        PiScanView::Progress => "app.pi_scan.footer.tips.progress_idle",
        PiScanView::Results if pi.results.is_empty() => "app.pi_scan.footer.tips.results_empty",
        PiScanView::Results => "app.pi_scan.footer.tips.results",
        PiScanView::Details => "app.pi_scan.footer.tips.details",
        PiScanView::Overview => "app.pi_scan.footer.tips.status",
        PiScanView::Setup => "app.pi_scan.footer.tips.settings",
    }
}

/// What: Describe one localized, indivisible key hint.
///
/// Inputs: Accepted key, localized label suffix, and priority.
///
/// Output: A segment for the shared footer line.
///
/// Details: Priority zero survives width-based dropping.
#[derive(Clone, Copy)]
struct FooterSegment {
    /// Literal accepted key or key group.
    key: &'static str,
    /// Suffix of the localized footer label key.
    label: &'static str,
    /// Lower number means more important; zero is mandatory.
    priority: u8,
}

/// What: Build state-aware key hints for the active view or wizard focus.
///
/// Inputs: Workspace state and terminal footer width.
///
/// Output: Styled line of whole hints that fit the available width.
///
/// Details: Esc and Help are mandatory and appear last.
fn footer_hints(app: &AppState, width: usize) -> Line<'static> {
    if app.pi_scan.budget_dialog.is_some() {
        let segments = [
            FooterSegment {
                key: "←→",
                label: "choose",
                priority: 1,
            },
            FooterSegment {
                key: "Enter",
                label: "apply",
                priority: 1,
            },
            FooterSegment {
                key: "Esc",
                label: "cancel_dialog",
                priority: 0,
            },
            FooterSegment {
                key: "?",
                label: "help",
                priority: 0,
            },
        ];
        return fit_footer_segments(app, &segments, width);
    }
    if let Some(wizard) = &app.pi_scan.wizard {
        let mut segments = wizard_footer_segments(wizard);
        segments.push(FooterSegment {
            key: "Esc",
            label: "cancel_dialog",
            priority: 0,
        });
        if !wizard.focuses_text_field() {
            segments.push(FooterSegment {
                key: "?",
                label: "help",
                priority: 0,
            });
        }
        return fit_footer_segments(app, &segments, width);
    }
    let mut segments = Vec::new();
    let mut add = |key, label, priority| {
        segments.push(FooterSegment {
            key,
            label,
            priority,
        });
    };
    match app.pi_scan.view {
        PiScanView::Setup => {
            add("Enter", "activate", 1);
            add("w", "wizard", 1);
            add("v", "verify", 2);
            add("B", "background", 2);
        }
        PiScanView::Overview | PiScanView::Progress => {
            use crate::state::pi_scan::PiScanPauseReason;
            let runtime = &app.pi_scan.runtime;
            if runtime.active.is_some()
                || !runtime.queue.is_empty()
                || runtime.pause_reasons.contains(&PiScanPauseReason::User)
            {
                add(
                    "p",
                    if runtime.pause_reasons.contains(&PiScanPauseReason::User) {
                        "resume"
                    } else {
                        "pause"
                    },
                    1,
                );
            }
            if runtime.active.is_some() {
                add("x x", "cancel", 1);
            }
            if app.pi_scan.view == PiScanView::Progress
                && !app.pi_scan.retry_target_names().is_empty()
            {
                add("r", "retry_failed", 2);
            }
            if app.pi_scan.budget_adjustment_eligible_at(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_secs()),
            ) {
                add("b", "budget", 1);
            }
            if app.pi_scan.view == PiScanView::Overview {
                add("B", "background", 2);
            }
        }
        PiScanView::Targets => {
            if !app.pi_scan.targets.is_empty() {
                add("Enter", "queue", 1);
                add("Space", "select", 1);
            }
        }
        PiScanView::Results => {
            if !app.pi_scan.results.is_empty() {
                add("Enter", "details", 1);
            }
        }
        PiScanView::Details => {
            if let Some(result) = app.pi_scan.selected_result() {
                add("Enter", "expand", 1);
                let binding = result.binding();
                if result.needs_finding_acknowledgement()
                    && !app.pi_scan.finding_acknowledgements.contains(&binding)
                {
                    add("a", "acknowledge", 1);
                }
                if result.stale && !app.pi_scan.stale_acknowledgements.contains(&binding) {
                    add("s", "stale", 1);
                }
                add("c", "continue", 1);
                if app.pi_scan.selected_result_acknowledged() {
                    add("b", "baseline", 2);
                }
                add("t", "raw", 2);
            }
        }
    }
    if matches!(app.pi_scan.view, PiScanView::Setup | PiScanView::Details)
        || app.pi_scan.view == PiScanView::Targets && !app.pi_scan.targets.is_empty()
        || app.pi_scan.view == PiScanView::Results && !app.pi_scan.results.is_empty()
    {
        add("↑↓", "move", 2);
    }
    add("←→", "pages", 2);
    add("PgUp/PgDn", "scroll", 3);
    if app.pi_scan.view == PiScanView::Details {
        add("Esc", "back_results", 0);
    } else {
        add("Esc", "back_workspace", 0);
    }
    add("?", "help", 0);
    fit_footer_segments(app, &segments, width)
}

/// What: Build hints for the currently focused wizard control.
///
/// Inputs: Wizard step, focus, and visible footer targets.
///
/// Output: Action and navigation segments for the current focus.
///
/// Details: Text fields advertise editing instead of letter shortcuts.
fn wizard_footer_segments(
    wizard: &crate::state::pi_scan_setup::PiScanSetupWizardState,
) -> Vec<FooterSegment> {
    use crate::state::pi_scan_setup::PiScanSetupHitTarget;
    let mut segments = Vec::new();
    let mut add = |key, label, priority| {
        segments.push(FooterSegment {
            key,
            label,
            priority,
        });
    };

    if wizard.focuses_text_field() {
        add("Tab", "next_field", 1);
        add("Enter", "confirm_field", 1);
    } else {
        if wizard.focus >= wizard.focus_count() {
            add("Enter", "activate", 1);
        } else if matches!(
            wizard.step,
            crate::state::pi_scan_setup::PiScanSetupStep::PiReadiness
        ) && wizard.focus == 1
        {
            add("Enter", "verify", 1);
        } else if matches!(
            wizard.step,
            crate::state::pi_scan_setup::PiScanSetupStep::Route
        ) || matches!(
            wizard.step,
            crate::state::pi_scan_setup::PiScanSetupStep::OptionalBehavior
        ) && (3..=6).contains(&wizard.focus)
        {
            add("Enter/←/→/h/l", "adjust", 1);
        } else if matches!(
            wizard.step,
            crate::state::pi_scan_setup::PiScanSetupStep::PricingPrivacy
                | crate::state::pi_scan_setup::PiScanSetupStep::OptionalBehavior
        ) {
            add("Enter/Space", "activate", 1);
        } else {
            add("Enter", "activate", 1);
        }
        let targets = wizard.footer_targets();
        if targets.contains(&PiScanSetupHitTarget::Next) {
            add("n", "next", 1);
        }
        if targets.contains(&PiScanSetupHitTarget::Back) {
            add("b", "back", 1);
        }
        if targets.contains(&PiScanSetupHitTarget::Apply) {
            add("a", "apply", 1);
        }
        if targets.contains(&PiScanSetupHitTarget::Retry) {
            add("r", "retry", 1);
        }
    }
    add("Shift+Tab", "previous_field", 1);
    add("PgUp/PgDn", "scroll", 3);
    segments
}

/// What: Render complete localized key segments within a footer width.
///
/// Inputs: Localization state, ordered segments, and available columns.
///
/// Output: Styled key/label spans with no split or truncated segment.
///
/// Details: Drops optional segments by priority and keeps Esc/Help last.
fn fit_footer_segments(app: &AppState, segments: &[FooterSegment], width: usize) -> Line<'static> {
    let label = |segment: &FooterSegment| {
        crate::i18n::t(app, &format!("app.pi_scan.footer.labels.{}", segment.label))
    };
    let size = |segment: &FooterSegment| segment.key.width() + 1 + label(segment).width();
    let mandatory: usize = segments
        .iter()
        .filter(|segment| segment.priority == 0)
        .map(|segment| size(segment) + 3)
        .sum();
    if mandatory.saturating_sub(3) > width {
        let th = theme();
        if !segments.iter().any(|segment| segment.key == "?") {
            return Line::from(Span::styled(
                "Esc",
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        return Line::from(vec![
            Span::styled(
                "Esc",
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" · ", Style::default().fg(th.overlay1)),
            Span::styled(
                "?",
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);
    }
    let mut remaining = width.saturating_sub(mandatory.saturating_sub(3));
    let mut included = vec![false; segments.len()];
    for priority in 1..=3 {
        for (index, segment) in segments.iter().enumerate() {
            let length = size(segment) + 3;
            if segment.priority == priority && length <= remaining {
                included[index] = true;
                remaining -= length;
            }
        }
    }
    let th = theme();
    let mut spans = Vec::new();
    for (index, segment) in segments.iter().enumerate() {
        if segment.priority != 0 && !included[index] {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::styled(" · ", Style::default().fg(th.overlay1)));
        }
        spans.push(Span::styled(
            segment.key,
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!(" {}", label(segment))));
    }
    Line::from(spans)
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
        "Guided setup cancelled — press w to restart it, Esc to leave" => {
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
    body_with_title(f, area, &crate::i18n::t(app, title_key), lines, scroll);
}

/// What: Render a bordered body paragraph with an already-localized title.
///
/// Inputs:
/// - `f`: Target frame.
/// - `area`: Body rectangle.
/// - `title`: Localized title, for example a `Results › package` breadcrumb.
/// - `lines`: Body content.
/// - `scroll`: Clamped line offset.
///
/// Output:
/// - Renders the wrapped, scrolled paragraph.
///
/// Details:
/// - Shared by every workspace page so titles and wrapping look the same.
pub(super) fn body_with_title(
    f: &mut Frame,
    area: Rect,
    title: &str,
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
                format!(" {title} "),
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

/// What: Detect later observed commits without changing persisted result staleness.
///
/// Inputs:
/// - `app`: Ordered target history.
/// - `result`: Exact commit of the displayed validated result.
///
/// Output:
/// - Whether a different, later target supersedes the result for display.
///
/// Details:
/// - Results without a retained target use any different observed commit as a fallback.
pub(super) fn result_has_newer_version(
    app: &crate::state::AppState,
    result: &crate::state::PiScanDisplayResult,
) -> bool {
    let base = result.validated.identity.package_base.as_str();
    let oid = result.validated.identity.commit_oid.as_str();
    if let Some(index) = app.pi_scan.targets.iter().rposition(|target| {
        target.package_base == base && target.commit_oid.as_deref() == Some(oid)
    }) {
        return app.pi_scan.targets[index + 1..].iter().any(|target| {
            target.package_base == base
                && target.commit_oid.as_deref().is_some_and(|next| next != oid)
        });
    }
    app.pi_scan.targets.iter().any(|target| {
        target.package_base == base && target.commit_oid.as_deref().is_some_and(|next| next != oid)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TabLabelTier, choose_tab_layout, format_microusd, format_token_count, short_identity,
    };

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

    #[test]
    fn tab_layout_keeps_work_pills_but_not_idle_readiness() {
        let rows = [58, 43, 20];
        assert_eq!(
            choose_tab_layout(rows, &[40, 28], true, 80),
            (TabLabelTier::Short, Some(1)),
            "work pill drops its suffix and tabs shorten before the pill disappears"
        );
        assert_eq!(
            choose_tab_layout(rows, &[40, 28], true, 64),
            (TabLabelTier::Numbers, Some(0))
        );
        assert_eq!(
            choose_tab_layout(rows, &[12], false, 55),
            (TabLabelTier::Short, None),
            "idle readiness never costs readable tab names"
        );
        assert_eq!(
            choose_tab_layout(rows, &[12], false, 120),
            (TabLabelTier::Full, Some(0))
        );
        assert_eq!(
            choose_tab_layout(rows, &[40], true, 10),
            (TabLabelTier::Numbers, None)
        );
    }
}
