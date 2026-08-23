use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::{AppState, types::AppMode};
use crate::theme::theme;
use crate::ui::results::{
    clear_top_bar_menu_rects, render_top_bar_menu_cluster, top_bar_menu_cluster_width,
};

/// Minimum width reserved for the Updates/News label before the menu cluster.
const MIN_UPDATES_LABEL_SLOT: u16 = 8;

/// What: Column gap between the two config-editor top labels when both strings are non-empty.
///
/// Inputs: None.
///
/// Output: Gap width in terminal columns.
///
/// Details: Keeps the pair readable when centered as a group.
const CONFIG_EDITOR_DUAL_LABEL_GAP: u16 = 2;

/// What: Build the package-mode updates strip label for the top bar.
///
/// Inputs:
/// - `app`: Application state (`updates_loading`, `updates_count`, `updates_last_check_authoritative`).
///
/// Output:
/// - Localized string matching the Package-mode updates button.
///
/// Details:
/// - Mirrors the non-News branch of `render_updates_button` so Config editor can show the same text
///   alongside the news strip without duplicating conditionals.
fn package_updates_top_bar_label(app: &AppState) -> String {
    let updates = if app.updates_loading {
        i18n::t(app, "app.updates_button.loading")
    } else if let Some(count) = app.updates_count {
        if count > 0 {
            if app.updates_last_check_authoritative == Some(false) {
                i18n::t_fmt1(app, "app.updates_button.available_degraded", count)
            } else {
                i18n::t_fmt1(app, "app.updates_button.available", count)
            }
        } else if app.updates_last_check_authoritative == Some(false) {
            i18n::t(app, "app.updates_button.none_maybe_stale")
        } else {
            i18n::t(app, "app.updates_button.none")
        }
    } else {
        i18n::t(app, "app.updates_button.none")
    };
    if !app.pi_scan.settings.enabled {
        return updates;
    }
    let pi_status = if app.pi_scan.runtime.active.is_some() {
        Some(i18n::t(app, "app.pi_scan.top_bar.running"))
    } else if app.pi_scan.unseen_result_count > 0 {
        Some(i18n::t_fmt1(
            app,
            "app.pi_scan.top_bar.new_results",
            app.pi_scan.unseen_result_count,
        ))
    } else {
        None
    };
    pi_status.map_or_else(|| updates.clone(), |status| format!("{updates} · {status}"))
}

/// Build the compact app-wide unattended Pi scan preference indicator.
fn background_indicator_label(app: &AppState) -> String {
    let key = if app.pi_scan.background_toggle_pending.is_some() {
        "app.pi_scan.top_bar.background_pending"
    } else if app.pi_scan.settings.background_enabled {
        if app.pi_scan.background_toggle_ready() {
            "app.pi_scan.top_bar.background_on"
        } else {
            "app.pi_scan.top_bar.background_setup"
        }
    } else if app.pi_scan.runtime.active.as_ref().is_some_and(|active| {
        active.request.priority == crate::state::pi_scan::PiScanPriority::Background
    }) {
        "app.pi_scan.top_bar.background_finishing"
    } else {
        "app.pi_scan.top_bar.background_off"
    };
    i18n::t(app, key)
}

/// Draw the reserved app-wide unattended Pi status segment.
fn render_background_indicator(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    label: &str,
    th: &crate::theme::Theme,
) {
    let attention = app.pi_scan.background_toggle_pending.is_some()
        || (app.pi_scan.settings.background_enabled && !app.pi_scan.background_toggle_ready())
        || (!app.pi_scan.settings.background_enabled
            && app.pi_scan.runtime.active.as_ref().is_some_and(|active| {
                active.request.priority == crate::state::pi_scan::PiScanPriority::Background
            }));
    let color = if attention {
        th.yellow
    } else if app.pi_scan.settings.background_enabled {
        th.green
    } else {
        th.overlay1
    };
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label.to_string(),
            Style::default()
                .fg(color)
                .bg(th.base)
                .add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right)
        .block(Block::default().style(Style::default().bg(th.base))),
        area,
    );
}

/// What: Build the news-mode top bar strip label.
///
/// Inputs:
/// - `app`: Application state (`news_loading`, `news_ready`).
///
/// Output:
/// - Localized string matching the News-mode news button.
///
/// Details:
/// - Used by News mode and by Config editor when showing both top strips.
fn news_top_bar_label(app: &AppState) -> String {
    if app.news_loading {
        i18n::t(app, "app.news_button.loading")
    } else if app.news_ready {
        i18n::t(app, "app.news_button.ready")
    } else {
        i18n::t(app, "app.news_button.none")
    }
}

/// Build the Pi Scan top status label without claiming runtime readiness.
fn pi_scan_top_bar_label(app: &AppState) -> String {
    let queued = app.pi_scan.runtime.queue.len();
    let state = match app.pi_scan.availability {
        crate::state::PiScanAvailability::Disabled => i18n::t(app, "app.pi_scan.status.disabled"),
        crate::state::PiScanAvailability::Unsupported => {
            i18n::t(app, "app.pi_scan.status.unsupported")
        }
        crate::state::PiScanAvailability::MissingBinary => {
            i18n::t(app, "app.pi_scan.status.missing_pi")
        }
        crate::state::PiScanAvailability::RuntimeDisconnected => {
            i18n::t(app, "app.pi_scan.status.disconnected")
        }
        crate::state::PiScanAvailability::RuntimeConnected => {
            i18n::t(app, "app.pi_scan.status.connected")
        }
    };
    format!(
        "{}: {state} · {}",
        i18n::t(app, "app.pi_scan.title"),
        i18n::t_fmt1(app, "app.pi_scan.top_bar.queued", queued)
    )
}

/// What: Draw updates and news top labels as one horizontally centered group (config editor).
///
/// Inputs:
/// - `f`: Ratatui frame.
/// - `app`: Application state (stores clickable rects).
/// - `area`: Label-slot rectangle for the row.
/// - `updates_label` / `news_label`: Localized strings for each strip.
/// - `th`: Active theme.
///
/// Output:
/// - Renders both labels and assigns `updates_button_rect` and `news_button_rect`.
///
/// Details:
/// - When `updates_label` plus gap plus `news_label` fits in `area.width`, the combined block is
///   centered in `area`. If not, falls back to a 50/50 horizontal split so both labels remain usable
///   in narrow terminals.
fn render_config_editor_dual_top_labels(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    updates_label: &str,
    news_label: &str,
    th: &crate::theme::Theme,
) {
    let w_u = updates_label.width();
    let w_n = news_label.width();
    let gap_u16 = if w_u > 0 && w_n > 0 {
        CONFIG_EDITOR_DUAL_LABEL_GAP
    } else {
        0
    };
    let desired = w_u.saturating_add(usize::from(gap_u16)).saturating_add(w_n);
    if desired == 0 {
        app.updates_button_rect = None;
        app.news_button_rect = None;
        return;
    }

    let avail = usize::from(area.width);
    if desired <= avail {
        let left_pad = (avail - desired) / 2;
        let start_x = area.x.saturating_add(u16::try_from(left_pad).unwrap_or(0));
        let w_updates = u16::try_from(w_u).unwrap_or(area.width).min(area.width);
        let updates_rect = Rect {
            x: start_x,
            y: area.y,
            width: w_updates,
            height: area.height,
        };
        let news_x = start_x.saturating_add(w_updates).saturating_add(gap_u16);
        let max_news_w = area.x.saturating_add(area.width).saturating_sub(news_x);
        let w_news = u16::try_from(w_n).unwrap_or(0).min(max_news_w);
        let news_rect = Rect {
            x: news_x,
            y: area.y,
            width: w_news,
            height: area.height,
        };
        render_updates_button_inner(f, app, updates_rect, updates_label, th);
        render_news_button_inner(f, app, news_rect, news_label, th);
        return;
    }

    let halves = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    render_updates_button_inner(f, app, halves[0], updates_label, th);
    render_news_button_inner(f, app, halves[1], news_label, th);
}

/// What: Render the updates available button at the top of the window and lockout status on the right.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state containing updates count, loading state, and faillock status
/// - `area`: Target rectangle for the updates button (should be 1 line high)
///
/// Output:
/// - Draws the updates button and lockout status, records clickable rectangle in `app.updates_button_rect`
///
/// Details:
/// - In Package mode: Shows "Updates available (X)" if count > 0, "No updates available" if count is 0,
///   or "Checking updates..." if still loading
/// - In News mode: Shows "News Ready" if news are available, "No News available" if no news,
///   or "Loading news..." if still loading
/// - In Config editor mode: Shows both package updates and news labels as one centered group (with a
///   narrow-terminal fallback split); two click targets.
/// - The Pi background indicator is reserved at the left edge before mode-specific labels.
/// - Config/Lists, Panels, and Options (or collapsed Menu) render on this same row to the right of the
///   updates/news label (before lockout text when present).
/// - Shows lockout status on the right if user is locked out
/// - Button is styled similar to other buttons in the UI
/// - Records clickable rectangle for mouse interaction
pub fn render_updates_button(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    let background_label = background_indicator_label(app);
    let background_width = u16::try_from(background_label.width())
        .unwrap_or(area.width)
        .min(area.width);
    if matches!(app.app_mode, AppMode::PiScan) {
        clear_top_bar_menu_rects(app);
        app.news_button_rect = None;
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(background_width), Constraint::Min(0)])
            .split(area);
        render_background_indicator(f, app, chunks[0], &background_label, &th);
        let label = pi_scan_top_bar_label(app);
        render_updates_button_inner(f, app, chunks[1], &label, &th);
        app.updates_button_rect = None;
        return;
    }

    // Check if lockout status should be displayed
    let lockout_text: Option<String> = if app.faillock_locked {
        app.faillock_remaining_minutes.map_or_else(
            || Some(i18n::t(app, "app.updates_button.locked")),
            |remaining| {
                if remaining > 0 {
                    Some(i18n::t_fmt1(
                        app,
                        "app.updates_button.locked_with_time",
                        remaining,
                    ))
                } else {
                    Some(i18n::t(app, "app.updates_button.locked"))
                }
            },
        )
    } else {
        None
    };

    let lockout_w = lockout_text.as_ref().map_or(0u16, |lockout| {
        u16::try_from(lockout.width()).unwrap_or(20).min(
            area.width
                .saturating_sub(MIN_UPDATES_LABEL_SLOT.saturating_add(1)),
        )
    });

    let width_for_menus_and_label = area
        .width
        .saturating_sub(lockout_w)
        .saturating_sub(background_width);
    let updates_label_slot = width_for_menus_and_label.min(MIN_UPDATES_LABEL_SLOT);
    let max_menu_w = width_for_menus_and_label.saturating_sub(updates_label_slot);
    let menu_cluster_w = top_bar_menu_cluster_width(max_menu_w, app);

    if menu_cluster_w == 0 {
        clear_top_bar_menu_rects(app);
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(background_width),
            Constraint::Min(updates_label_slot),
            Constraint::Length(menu_cluster_w),
            Constraint::Length(lockout_w),
        ])
        .split(area);

    let updates_chunk = chunks[1];
    match app.app_mode {
        AppMode::ConfigEditor => {
            let updates_label = package_updates_top_bar_label(app);
            let news_label = news_top_bar_label(app);
            render_config_editor_dual_top_labels(
                f,
                app,
                updates_chunk,
                &updates_label,
                &news_label,
                &th,
            );
        }
        AppMode::News => {
            let label = news_top_bar_label(app);
            render_news_button_inner(f, app, updates_chunk, &label, &th);
        }
        AppMode::PiScan => unreachable!("Pi Scan returns before package controls are laid out"),
        AppMode::Package => {
            let label = package_updates_top_bar_label(app);
            render_updates_button_inner(f, app, updates_chunk, &label, &th);
        }
    }

    render_background_indicator(f, app, chunks[0], &background_label, &th);
    if menu_cluster_w > 0 {
        render_top_bar_menu_cluster(f, app, chunks[2]);
    }

    if let Some(lockout) = lockout_text.filter(|_| lockout_w > 0) {
        let lockout_style = Style::default()
            .fg(th.red)
            .bg(th.base)
            .add_modifier(Modifier::BOLD);
        let lockout_line = Line::from(Span::styled(lockout, lockout_style));
        let lockout_paragraph = Paragraph::new(lockout_line)
            .alignment(Alignment::Right)
            .block(
                Block::default()
                    .borders(ratatui::widgets::Borders::NONE)
                    .style(Style::default().bg(th.base)),
            );
        f.render_widget(lockout_paragraph, chunks[3]);
    }
}

/// What: Render the updates button inner content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Target rectangle
/// - `button_text`: Button text to display
/// - `th`: Theme
///
/// Output:
/// - Draws the updates button and records clickable rectangle
///
/// Details:
/// - Center-aligns the label within `area` (matches the historical full-row center when no menus fit)
///   and sizes the hit target to the displayed text width at the centered position.
fn render_updates_button_inner(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    button_text: &str,
    th: &crate::theme::Theme,
) {
    // Style the button (similar to other buttons)
    let button_style = Style::default()
        .fg(th.mauve)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);

    // Create button with underlined first character
    let mut spans = Vec::new();
    if let Some(first) = button_text.chars().next() {
        let rest = &button_text[first.len_utf8()..];
        spans.push(Span::styled(
            first.to_string(),
            button_style.add_modifier(Modifier::UNDERLINED),
        ));
        spans.push(Span::styled(rest.to_string(), button_style));
    } else {
        spans.push(Span::styled(button_text.to_string(), button_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(th.base)),
    );

    // Render the button
    f.render_widget(paragraph, area);

    let button_width = u16::try_from(button_text.width())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.updates_button_rect = Some((button_x, area.y, button_width, area.height));
}

/// What: Render the news button inner content.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Target rectangle
/// - `button_text`: Button text to display
/// - `th`: Theme
///
/// Output:
/// - Draws the news button and records clickable rectangle
///
/// Details:
/// - Center-aligns the label within `area` and sizes the hit target to the displayed text width at the
///   centered position.
fn render_news_button_inner(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    button_text: &str,
    th: &crate::theme::Theme,
) {
    // Style the button (similar to other buttons)
    let button_style = Style::default()
        .fg(th.mauve)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);

    // Create button with underlined first character
    let mut spans = Vec::new();
    if let Some(first) = button_text.chars().next() {
        let rest = &button_text[first.len_utf8()..];
        spans.push(Span::styled(
            first.to_string(),
            button_style.add_modifier(Modifier::UNDERLINED),
        ));
        spans.push(Span::styled(rest.to_string(), button_style));
    } else {
        spans.push(Span::styled(button_text.to_string(), button_style));
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center).block(
        Block::default()
            .borders(ratatui::widgets::Borders::NONE)
            .style(Style::default().bg(th.base)),
    );

    // Render the button
    f.render_widget(paragraph, area);

    let button_width = u16::try_from(button_text.width())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let button_x = area
        .x
        .saturating_add(area.width.saturating_sub(button_width) / 2);
    app.news_button_rect = Some((button_x, area.y, button_width, area.height));
}

#[cfg(test)]
mod tests {
    use super::{background_indicator_label, render_updates_button};
    use crate::state::types::AppMode;
    use crate::state::{AppState, PiScanAvailability, PiScanReadiness};
    use ratatui::{Terminal, backend::TestBackend};

    /// Build a fully consented connected projection with background scans enabled.
    fn ready_app() -> AppState {
        let mut app = AppState::default();
        let locales = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("config/locales");
        app.translations =
            crate::i18n::load_locale_file("en-US", &locales).expect("English locale");
        app.pi_scan.settings.enabled = true;
        app.pi_scan.settings.background_enabled = true;
        app.pi_scan.availability = PiScanAvailability::RuntimeConnected;
        app.pi_scan.setup_facts_verified = true;
        app.pi_scan.disclosure_confirmed = true;
        app.pi_scan.runtime.consent.background_observation = true;
        app.pi_scan.runtime.consent.paid_execution = true;
        app.pi_scan.background_paid_execution_confirmed = true;
        app.pi_scan.readiness = PiScanReadiness::Confirmed;
        app
    }

    /// Render one top-bar row into plain text.
    fn render_row(width: u16, app: &mut AppState) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_updates_button(frame, app, frame.area()))
            .expect("top bar render");
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    /// Build one active background item for the finishing-indicator case.
    fn background_active_item() -> crate::state::pi_scan::PiScanActiveItem {
        crate::state::pi_scan::PiScanActiveItem {
            correlation_id: 1,
            request: crate::state::pi_scan::PiScanJobRequest {
                request_id: 1,
                key: crate::state::pi_scan::PiScanQueueKey {
                    package_base: crate::logic::pi_scan::identity::PackageBase::new("demo")
                        .expect("base"),
                    commit_oid: crate::logic::pi_scan::identity::CommitOid::new("a".repeat(40))
                        .expect("oid"),
                },
                priority: crate::state::pi_scan::PiScanPriority::Background,
                reservation: crate::state::pi_scan::PiScanReservation {
                    tokens: 1,
                    cost_microusd: 1,
                },
                manual_budget_override_confirmed: false,
            },
            started_at_unix: 1,
            cancellation_suppressed: false,
        }
    }

    /// Full label, narrow distinguishing token, and state mutator for one indicator case.
    type IndicatorCase = (&'static str, &'static str, fn(&mut AppState));

    /// Every background state remains distinguishable in every mode and supported top-bar width.
    #[test]
    fn background_indicator_renders_state_mode_and_width_matrix() {
        let cases: [IndicatorCase; 5] = [
            ("Pi BG: ON", "ON", |_| {}),
            ("Pi BG: OFF", "OFF", |app| {
                app.pi_scan.settings.background_enabled = false;
            }),
            ("Pi BG: OFF · finishing", "·", |app| {
                app.pi_scan.settings.background_enabled = false;
                app.pi_scan.runtime.active = Some(background_active_item());
            }),
            ("Pi BG: SETUP", "SETUP", |app| {
                app.pi_scan.setup_facts_verified = false;
            }),
            ("Pi BG: …", "…", |app| {
                app.pi_scan.background_toggle_pending = Some(false);
            }),
        ];
        for mode in [
            AppMode::Package,
            AppMode::News,
            AppMode::ConfigEditor,
            AppMode::PiScan,
        ] {
            for (full_label, narrow_token, configure) in cases {
                let mut app = ready_app();
                app.app_mode = mode;
                configure(&mut app);
                assert_eq!(background_indicator_label(&app), full_label);
                let normal = render_row(42, &mut app);
                assert!(
                    normal.contains(full_label),
                    "{mode:?} normal row did not contain {full_label:?}: {normal:?}"
                );
                assert!(
                    normal.starts_with(full_label),
                    "{mode:?} normal row did not place {full_label:?} on the left: {normal:?}"
                );
                let narrow = render_row(14, &mut app);
                assert!(
                    narrow.contains(narrow_token),
                    "{mode:?} narrow row did not distinguish {full_label:?}: {narrow:?}"
                );
            }
        }
    }
}
