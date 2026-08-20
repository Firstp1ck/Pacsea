//! Queue and active-progress page with animated scan feedback.

use crate::state::pi_scan::{
    PiScanActiveItem, PiScanPauseReason, PiScanRuntimeState, PiScanTerminalStatus,
};
use crate::state::{AppState, PiScanExecutionPhase};
use crate::theme::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::time::{SystemTime, UNIX_EPOCH};

/// Braille spinner frames cycled by the periodic redraw tick.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Milliseconds each spinner frame stays visible; the 200 ms UI tick keeps rotation visible.
const SPINNER_FRAME_MS: u128 = 100;

/// Character width of the textual progress bar.
const PROGRESS_BAR_WIDTH: usize = 24;

/// What: Aggregated scan counts projected from the runtime queue and terminal history.
///
/// Inputs:
/// - Populated by [`session_counts`] from queue depth, active state, and terminal records.
///
/// Output:
/// - Per-status counters plus derived done/total sums for the progress bar.
///
/// Details:
/// - Terminal history is bounded by runtime retention, so counters reflect retained work.
struct SessionCounts {
    /// Retained terminal records that completed successfully.
    completed: usize,
    /// Retained terminal records that failed.
    failed: usize,
    /// Retained terminal records cancelled by the user.
    cancelled: usize,
    /// Retained terminal records interrupted by shutdown or recovery.
    interrupted: usize,
    /// Pending queue depth.
    queued: usize,
    /// One when a scan is currently active, zero otherwise.
    running: usize,
}

impl SessionCounts {
    /// Sum of all terminal outcomes.
    const fn done(&self) -> usize {
        self.completed + self.failed + self.cancelled + self.interrupted
    }

    /// Total known work: terminal outcomes plus running and queued items.
    const fn total(&self) -> usize {
        self.done() + self.running + self.queued
    }
}

/// What: Render sequential queue, animated activity, and pause/cancel/retry affordances.
///
/// Inputs:
/// - `f`: Target frame.
/// - `app`: Application state holding the Pi Scan runtime projection.
/// - `area`: Page body area.
///
/// Output:
/// - Draws the progress page and persists the clamped scroll offset.
///
/// Details:
/// - The 200 ms tick worker redraws the UI, so the millisecond-based spinner animates
///   continuously while work is active or queued.
/// - Without an active item but with queued work, an animated waiting line plus any
///   pause reasons explain why nothing is running yet.
pub(super) fn render(f: &mut Frame, app: &mut AppState, area: Rect) {
    let now_millis = unix_now_millis();
    let spinner = spinner_frame(now_millis);
    let pi = &app.pi_scan;
    let counts = session_counts(&pi.runtime);
    let mut lines = Vec::new();
    if counts.total() > 0 {
        lines.push(summary_line(
            app,
            &counts,
            spinner,
            !pi.runtime.pause_reasons.is_empty(),
        ));
        lines.push(Line::from(String::new()));
    }
    let now_secs = u64::try_from(now_millis / 1_000).unwrap_or(0);
    if let Some(active) = &pi.runtime.active {
        push_active_lines(&mut lines, app, active, spinner, now_secs);
    } else if counts.queued > 0 {
        push_waiting_lines(&mut lines, app, spinner, now_secs);
    } else {
        lines.push(Line::from(crate::i18n::t(
            app,
            "app.pi_scan.progress.no_active",
        )));
    }
    push_queue_lines(&mut lines, app);
    let scroll = super::clamp_line_scroll(pi.view_scroll.progress, &lines, area);
    app.pi_scan.view_scroll.progress = scroll;
    super::body_scrolled(f, app, area, "app.pi_scan.tabs.progress", lines, scroll);
}

/// What: Build the animated one-line session summary with bar and per-status counts.
///
/// Inputs:
/// - `app`: Application state for translations.
/// - `counts`: Aggregated session counters.
/// - `spinner`: Current spinner frame.
/// - `paused`: Whether a sticky pause currently blocks queued starts.
///
/// Output:
/// - Styled summary line, e.g. `⠹ [██████░░] 3/70 · 2 completed · 1 running · 67 queued`.
///
/// Details:
/// - Zero-valued categories are omitted to keep the line short. Active or eligible queued work
///   spins, blocked idle work uses a static pause marker, and fully terminal work uses a check.
fn summary_line(
    app: &AppState,
    counts: &SessionCounts,
    spinner: &'static str,
    paused: bool,
) -> Line<'static> {
    let th = theme();
    let pending = counts.running + counts.queued > 0;
    let blocked = paused && counts.running == 0 && counts.queued > 0;
    let mut spans = vec![
        if blocked {
            Span::styled(
                "⏸ ",
                Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
            )
        } else if pending {
            Span::styled(
                format!("{spinner} "),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled("✔ ", Style::default().fg(th.green))
        },
        Span::styled(
            format!(
                "[{}] ",
                progress_bar(counts.done(), counts.total(), PROGRESS_BAR_WIDTH)
            ),
            Style::default().fg(th.green),
        ),
        Span::styled(
            format!("{}/{}", counts.done(), counts.total()),
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ),
    ];
    let categories: [(usize, &str, ratatui::style::Color); 6] = [
        (counts.running, "running", th.sapphire),
        (counts.queued, "queued", th.subtext1),
        (counts.completed, "completed", th.green),
        (counts.failed, "failed", th.red),
        (counts.cancelled, "cancelled", th.yellow),
        (counts.interrupted, "interrupted", th.yellow),
    ];
    for (count, key, color) in categories {
        if count == 0 {
            continue;
        }
        let label = crate::i18n::t(app, &format!("app.pi_scan.target_status.{key}"));
        spans.push(Span::styled(
            format!(" · {count} {label}"),
            Style::default().fg(color),
        ));
    }
    Line::from(spans)
}

/// What: Append the animated active-scan block with identity, elapsed time, and ceiling.
///
/// Inputs:
/// - `lines`: Output line buffer.
/// - `app`: Application state for translations.
/// - `active`: Currently running queue item.
/// - `spinner`: Current spinner frame.
/// - `now_secs`: Current Unix second for elapsed display.
///
/// Output:
/// - Pushes identity, current phase, and elapsed/reservation lines for the running scan.
///
/// Details:
/// - Elapsed time renders as unbounded minutes plus seconds so long scans stay readable.
/// - A transient phase is displayed only when its correlation still owns this active item.
fn push_active_lines(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    active: &PiScanActiveItem,
    spinner: &'static str,
    now_secs: u64,
) {
    let th = theme();
    let elapsed = now_secs.saturating_sub(active.started_at_unix);
    lines.push(Line::from(vec![
        Span::styled(
            format!("{}: ", crate::i18n::t(app, "app.pi_scan.progress.active")),
            Style::default().fg(th.text).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "{} @ {} (#{})",
                active.request.key.package_base,
                active.request.key.commit_oid,
                active.correlation_id
            ),
            Style::default().fg(th.text),
        ),
    ]));
    let phase = active_phase(app, active.correlation_id).map_or_else(
        || crate::i18n::t(app, "app.pi_scan.progress.working"),
        |phase| crate::i18n::t(app, phase_key(phase)),
    );
    lines.push(Line::from(vec![
        Span::styled(
            format!("{spinner} "),
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "{}: {phase}",
                crate::i18n::t(app, "app.pi_scan.progress.current_step")
            ),
            Style::default()
                .fg(th.sapphire)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        format!(
            "   {} {:02}:{:02} · {} {} {} / {}",
            crate::i18n::t(app, "app.pi_scan.progress.running_for"),
            elapsed / 60,
            elapsed % 60,
            crate::i18n::t(app, "app.pi_scan.progress.reservation"),
            super::format_token_count(active.request.reservation.tokens),
            crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
            super::format_microusd(active.request.reservation.cost_microusd),
        ),
        Style::default().fg(th.subtext1),
    )));
}

/// What: Return the transient phase only when it belongs to the active correlation.
///
/// Inputs:
/// - `app`: Application state containing the transient phase projection.
/// - `correlation_id`: Correlation of the active item being rendered.
///
/// Output:
/// - Matching execution phase, or `None` for missing/stale progress.
///
/// Details:
/// - This defensive check prevents a late update from labeling a different active package.
fn active_phase(app: &AppState, correlation_id: u64) -> Option<PiScanExecutionPhase> {
    app.pi_scan
        .active_progress
        .filter(|progress| progress.correlation_id == correlation_id)
        .map(|progress| progress.phase)
}

/// Map one transient execution phase to its localization key.
const fn phase_key(phase: PiScanExecutionPhase) -> &'static str {
    match phase {
        PiScanExecutionPhase::Preparing => "app.pi_scan.progress.phase.preparing",
        PiScanExecutionPhase::ResolvingMetadata => "app.pi_scan.progress.phase.resolving_metadata",
        PiScanExecutionPhase::WaitingToRetry => "app.pi_scan.progress.phase.waiting_to_retry",
        PiScanExecutionPhase::AcquiringSources => "app.pi_scan.progress.phase.acquiring_sources",
        PiScanExecutionPhase::RunningModel => "app.pi_scan.progress.phase.running_model",
        PiScanExecutionPhase::RecheckingIdentity => {
            "app.pi_scan.progress.phase.rechecking_identity"
        }
        PiScanExecutionPhase::ValidatingResult => "app.pi_scan.progress.phase.validating_result",
        PiScanExecutionPhase::Finalizing => "app.pi_scan.progress.phase.finalizing",
    }
}

/// What: Append the animated waiting block shown while queued work has not started.
///
/// Inputs:
/// - `lines`: Output line buffer.
/// - `app`: Application state for translations and pause reasons.
/// - `spinner`: Current spinner frame.
/// - `now_unix`: Current Unix time for rolling-budget accounting.
///
/// Output:
/// - Pushes a waiting line and, when present, a localized pause-reason line.
///
/// Details:
/// - Surfacing sticky pause reasons here explains an idle queue without opening Overview.
fn push_waiting_lines(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    spinner: &'static str,
    now_unix: u64,
) {
    let th = theme();
    let reasons = &app.pi_scan.runtime.pause_reasons;
    let marker = if reasons.is_empty() { spinner } else { "⏸" };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{marker} "),
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            crate::i18n::t(app, "app.pi_scan.progress.waiting"),
            Style::default().fg(th.yellow),
        ),
    ]));
    if reasons.is_empty() {
        return;
    }
    let localized = reasons
        .iter()
        .map(|reason| crate::i18n::t(app, pause_reason_key(*reason)))
        .collect::<Vec<_>>()
        .join(", ");
    lines.push(Line::from(Span::styled(
        format!(
            "   {}: {localized}",
            crate::i18n::t(app, "app.pi_scan.progress.paused")
        ),
        Style::default().fg(th.red),
    )));
    if reasons.contains(&PiScanPauseReason::Budget) {
        lines.push(Line::from(Span::styled(
            format!(
                "   {}: {}",
                crate::i18n::t(app, "app.pi_scan.progress.budget_limits"),
                budget_limit_hit_names(app, now_unix),
            ),
            Style::default().fg(th.yellow),
        )));
        lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.pi_scan.progress.budget_solution"),
            Style::default().fg(th.text),
        )));
    }
}

/// What: Append numbered queue entries with identity, priority, and reservation.
///
/// Inputs:
/// - `lines`: Output line buffer.
/// - `app`: Application state holding the runtime queue.
///
/// Output:
/// - Pushes one line per pending queue item with its 1-based position.
///
/// Details:
/// - Position numbers make the sequential order visible while the dimmed prefix keeps
///   focus on the package identity.
fn push_queue_lines(lines: &mut Vec<Line<'static>>, app: &AppState) {
    let th = theme();
    for (index, request) in app.pi_scan.runtime.queue.iter().enumerate() {
        let priority = crate::i18n::t(
            app,
            match request.priority {
                crate::state::pi_scan::PiScanPriority::Foreground => {
                    "app.pi_scan.priority.foreground"
                }
                crate::state::pi_scan::PiScanPriority::Background => {
                    "app.pi_scan.priority.background"
                }
            },
        );
        lines.push(Line::from(vec![
            Span::styled(
                format!("{:>4}. ", index + 1),
                Style::default().fg(th.overlay1),
            ),
            Span::styled(
                format!(
                    "{} @ {} ({priority}) · {} {} / {}",
                    request.key.package_base,
                    request.key.commit_oid,
                    super::format_token_count(request.reservation.tokens),
                    crate::i18n::t(app, "app.pi_scan.wizard.pricing.tokens"),
                    super::format_microusd(request.reservation.cost_microusd),
                ),
                Style::default().fg(th.text),
            ),
        ]));
    }
}

/// What: Project session counters from the cohesive runtime state.
///
/// Inputs:
/// - `runtime`: Queue, active item, and retained terminal history.
///
/// Output:
/// - [`SessionCounts`] for the summary line and progress bar.
///
/// Details:
/// - Delegates to [`count_outcomes`] so the aggregation stays unit-testable without
///   constructing full runtime fixtures.
fn session_counts(runtime: &PiScanRuntimeState) -> SessionCounts {
    count_outcomes(
        runtime.terminal.iter().map(|record| record.status),
        runtime.queue.len(),
        runtime.active.is_some(),
    )
}

/// What: Aggregate terminal outcomes with pending queue and running state.
///
/// Inputs:
/// - `statuses`: Terminal status per retained record.
/// - `queued`: Pending queue depth.
/// - `running`: Whether a scan is currently active.
///
/// Output:
/// - Fully populated [`SessionCounts`].
///
/// Details:
/// - Pure aggregation; the caller decides which history window is supplied.
fn count_outcomes(
    statuses: impl Iterator<Item = PiScanTerminalStatus>,
    queued: usize,
    running: bool,
) -> SessionCounts {
    let mut counts = SessionCounts {
        completed: 0,
        failed: 0,
        cancelled: 0,
        interrupted: 0,
        queued,
        running: usize::from(running),
    };
    for status in statuses {
        match status {
            PiScanTerminalStatus::Completed => counts.completed += 1,
            PiScanTerminalStatus::Failed => counts.failed += 1,
            PiScanTerminalStatus::Cancelled => counts.cancelled += 1,
            PiScanTerminalStatus::Interrupted => counts.interrupted += 1,
        }
    }
    counts
}

/// What: Render a fixed-width textual progress bar.
///
/// Inputs:
/// - `done`: Finished item count.
/// - `total`: Total known item count.
/// - `width`: Bar width in characters.
///
/// Output:
/// - `width` characters of filled and empty cells; all empty when `total` is zero.
///
/// Details:
/// - `done` is clamped to `total` so late history changes can never overflow the bar.
fn progress_bar(done: usize, total: usize, width: usize) -> String {
    let filled = done
        .min(total)
        .saturating_mul(width)
        .checked_div(total)
        .unwrap_or(0);
    let mut bar = String::with_capacity(width.saturating_mul(3));
    for index in 0..width {
        bar.push(if index < filled { '█' } else { '░' });
    }
    bar
}

/// What: Select the spinner frame for the current time.
///
/// Inputs:
/// - `now_millis`: Current Unix time in milliseconds.
///
/// Output:
/// - One braille frame; rotation wraps smoothly under the periodic redraw tick.
///
/// Details:
/// - Frame index derives from wall time so animation continues across redraw sources.
fn spinner_frame(now_millis: u128) -> &'static str {
    let index = (now_millis / SPINNER_FRAME_MS) % SPINNER_FRAMES.len() as u128;
    SPINNER_FRAMES[usize::try_from(index).unwrap_or(0)]
}

/// Map one sticky pause reason to its localization key.
const fn pause_reason_key(reason: PiScanPauseReason) -> &'static str {
    match reason {
        PiScanPauseReason::User => "app.pi_scan.progress.pause.user",
        PiScanPauseReason::Service => "app.pi_scan.progress.pause.service",
        PiScanPauseReason::Budget => "app.pi_scan.progress.pause.budget",
    }
}

/// Return the current Unix time in milliseconds for redraw-driven animation.
fn unix_now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

/// Return the exact rolling limits that block the next background reservation.
fn budget_limit_hit_names(app: &AppState, now_unix: u64) -> String {
    let Some(next) = app
        .pi_scan
        .runtime
        .queue
        .iter()
        .find(|item| item.priority == crate::state::pi_scan::PiScanPriority::Background)
    else {
        return crate::i18n::t(app, "app.pi_scan.progress.budget_unknown");
    };
    let mut starts = 0_u32;
    let mut tokens = 0_u64;
    let mut cost = 0_u64;
    for record in
        app.pi_scan.runtime.budget.records.iter().filter(|record| {
            record.class == crate::state::pi_scan::PiScanAccountingClass::Background
        })
    {
        let age = now_unix.saturating_sub(record.started_at_unix);
        if age < crate::state::pi_scan::START_WINDOW_SECONDS {
            starts = starts.saturating_add(1);
        }
        if age < crate::state::pi_scan::USAGE_WINDOW_SECONDS {
            tokens = tokens.saturating_add(record.effective_tokens());
            cost = cost.saturating_add(record.effective_cost_microusd());
        }
    }
    let limits = &app.pi_scan.settings;
    let mut hit = Vec::new();
    if starts >= limits.background_starts_per_hour {
        hit.push(crate::i18n::t(app, "app.pi_scan.progress.limit_starts"));
    }
    if tokens.saturating_add(next.reservation.tokens) > limits.background_token_cap_24h {
        hit.push(crate::i18n::t(app, "app.pi_scan.progress.limit_tokens"));
    }
    let cost_limit = parse_cost_microusd(&limits.background_cost_cap_24h);
    if cost.saturating_add(next.reservation.cost_microusd) > cost_limit {
        hit.push(crate::i18n::t(app, "app.pi_scan.progress.limit_cost"));
    }
    if hit.is_empty() {
        crate::i18n::t(app, "app.pi_scan.progress.budget_unknown")
    } else {
        hit.join(", ")
    }
}

/// Parse a decimal USD cap into micro-USD without floating-point rounding.
fn parse_cost_microusd(value: &str) -> u64 {
    let trimmed = value.trim();
    let (whole, fraction) = trimmed.split_once('.').map_or((trimmed, ""), |parts| parts);
    let whole = whole.parse::<u64>().unwrap_or(0);
    let mut fraction_digits = fraction.chars().take(6).collect::<String>();
    while fraction_digits.len() < 6 {
        fraction_digits.push('0');
    }
    whole
        .saturating_mul(1_000_000)
        .saturating_add(fraction_digits.parse::<u64>().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::{
        PROGRESS_BAR_WIDTH, SPINNER_FRAME_MS, SPINNER_FRAMES, active_phase, count_outcomes,
        pause_reason_key, phase_key, progress_bar, spinner_frame,
    };
    use crate::state::pi_scan::{PiScanPauseReason, PiScanTerminalStatus};
    use crate::state::{AppState, PiScanExecutionPhase, PiScanExecutionProgress};

    /// The spinner starts at frame zero, advances per interval, and wraps around.
    #[test]
    fn spinner_frame_cycles_and_wraps() {
        assert_eq!(spinner_frame(0), SPINNER_FRAMES[0]);
        assert_eq!(spinner_frame(SPINNER_FRAME_MS), SPINNER_FRAMES[1]);
        let full_cycle = SPINNER_FRAME_MS * SPINNER_FRAMES.len() as u128;
        assert_eq!(spinner_frame(full_cycle), SPINNER_FRAMES[0]);
        assert_eq!(
            spinner_frame(full_cycle + SPINNER_FRAME_MS * 3),
            SPINNER_FRAMES[3]
        );
    }

    /// The bar stays fixed-width, clamps overflow, and handles empty totals.
    #[test]
    fn progress_bar_renders_bounded_fill() {
        assert_eq!(progress_bar(0, 0, 4), "░░░░");
        assert_eq!(progress_bar(0, 10, 4), "░░░░");
        assert_eq!(progress_bar(5, 10, 4), "██░░");
        assert_eq!(progress_bar(10, 10, 4), "████");
        assert_eq!(progress_bar(99, 10, 4), "████");
        assert_eq!(progress_bar(1, 3, PROGRESS_BAR_WIDTH).chars().count(), 24);
    }

    /// Terminal statuses aggregate into the correct counters and derived sums.
    #[test]
    fn count_outcomes_aggregates_all_statuses() {
        let statuses = [
            PiScanTerminalStatus::Completed,
            PiScanTerminalStatus::Completed,
            PiScanTerminalStatus::Failed,
            PiScanTerminalStatus::Cancelled,
            PiScanTerminalStatus::Interrupted,
        ];
        let counts = count_outcomes(statuses.into_iter(), 7, true);
        assert_eq!(counts.completed, 2);
        assert_eq!(counts.failed, 1);
        assert_eq!(counts.cancelled, 1);
        assert_eq!(counts.interrupted, 1);
        assert_eq!(counts.queued, 7);
        assert_eq!(counts.running, 1);
        assert_eq!(counts.done(), 5);
        assert_eq!(counts.total(), 13);
    }

    /// Matching phase projection is accepted while a stale correlation is ignored.
    #[test]
    fn active_phase_requires_exact_correlation() {
        let mut app = AppState::default();
        app.pi_scan.active_progress = Some(PiScanExecutionProgress {
            correlation_id: 41,
            phase: PiScanExecutionPhase::RunningModel,
        });
        assert_eq!(
            active_phase(&app, 41),
            Some(PiScanExecutionPhase::RunningModel)
        );
        assert_eq!(active_phase(&app, 42), None);
    }

    /// Every execution phase maps to a distinct progress-page localization key.
    #[test]
    fn execution_phases_map_to_localization_keys() {
        let phases = [
            PiScanExecutionPhase::Preparing,
            PiScanExecutionPhase::ResolvingMetadata,
            PiScanExecutionPhase::WaitingToRetry,
            PiScanExecutionPhase::AcquiringSources,
            PiScanExecutionPhase::RunningModel,
            PiScanExecutionPhase::RecheckingIdentity,
            PiScanExecutionPhase::ValidatingResult,
            PiScanExecutionPhase::Finalizing,
        ];
        let keys = phases.map(phase_key);
        assert_eq!(keys.len(), 8);
        assert!(
            keys.into_iter()
                .all(|key| key.starts_with("app.pi_scan.progress.phase."))
        );
    }

    /// Every sticky pause reason maps to a progress-page localization key.
    #[test]
    fn pause_reasons_map_to_localization_keys() {
        assert_eq!(
            pause_reason_key(PiScanPauseReason::User),
            "app.pi_scan.progress.pause.user"
        );
        assert_eq!(
            pause_reason_key(PiScanPauseReason::Service),
            "app.pi_scan.progress.pause.service"
        );
        assert_eq!(
            pause_reason_key(PiScanPauseReason::Budget),
            "app.pi_scan.progress.pause.budget"
        );
    }
}
