use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::AppState;
use crate::state::modal::{ServiceImpact, ServiceRestartDecision};
use crate::theme::theme;

/// What: Render the Services tab content for the preflight modal.
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `service_info`: Service impact information.
/// - `service_selected`: Currently selected service index (mutable).
/// - `services_loaded`: Whether services are loaded.
/// - `services_error`: Optional error message.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows service impacts with restart decisions.
/// - Supports viewport-based rendering for large service lists.
#[allow(clippy::too_many_arguments)]
pub fn render_services_tab(
    app: &AppState,
    service_info: &[ServiceImpact],
    service_selected: &mut usize,
    services_loaded: bool,
    services_error: &Option<String>,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    if app.services_resolving {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.services.updating"),
            Style::default().fg(th.yellow),
        )));
    } else if let Some(err_msg) = services_error {
        // Display error with retry hint
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.preflight.services.error", err_msg),
            Style::default().fg(th.red),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.services.retry_hint"),
            Style::default().fg(th.subtext1),
        )));
    } else if !services_loaded {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.services.resolving"),
            Style::default().fg(th.subtext1),
        )));
    } else if service_info.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.services.no_services"),
            Style::default().fg(th.green),
        )));
    } else {
        // Performance optimization: Only render visible items (viewport-based rendering)
        // This prevents performance issues with large service lists
        let available_height = content_rect.height.saturating_sub(6) as usize;
        let visible = available_height.max(1);
        let selected = (*service_selected).min(service_info.len().saturating_sub(1));
        if *service_selected != selected {
            *service_selected = selected;
        }
        let start = if service_info.len() <= visible {
            0
        } else {
            selected
                .saturating_sub(visible / 2)
                .min(service_info.len() - visible)
        };
        let end = (start + visible).min(service_info.len());
        // Render only visible services (viewport-based rendering)
        for (idx, svc) in service_info
            .iter()
            .enumerate()
            .skip(start)
            .take(end - start)
        {
            let is_selected = idx == selected;
            let mut spans = Vec::new();
            let name_style = if is_selected {
                Style::default()
                    .fg(th.crust)
                    .bg(th.sapphire)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(th.text)
            };
            spans.push(Span::styled(svc.unit_name.clone(), name_style));
            spans.push(Span::raw(" "));
            let status_span = if svc.is_active {
                if svc.needs_restart {
                    Span::styled(
                        i18n::t(
                            app,
                            "app.modals.preflight.services.active_restart_recommended",
                        ),
                        Style::default().fg(th.yellow),
                    )
                } else {
                    Span::styled(
                        i18n::t(app, "app.modals.preflight.services.active"),
                        Style::default().fg(th.green),
                    )
                }
            } else {
                Span::styled(
                    i18n::t(app, "app.modals.preflight.services.inactive"),
                    Style::default().fg(th.subtext1),
                )
            };
            spans.push(status_span);
            spans.push(Span::raw(" "));
            let decision_span = match svc.restart_decision {
                ServiceRestartDecision::Restart => Span::styled(
                    i18n::t(app, "app.modals.preflight.services.restart"),
                    Style::default().fg(th.green),
                ),
                ServiceRestartDecision::Defer => Span::styled(
                    i18n::t(app, "app.modals.preflight.services.defer"),
                    Style::default().fg(th.yellow),
                ),
            };
            spans.push(decision_span);
            if !svc.providers.is_empty() {
                spans.push(Span::raw(" • "));
                spans.push(Span::styled(
                    svc.providers.join(", "),
                    Style::default().fg(th.overlay1),
                ));
            }
            lines.push(Line::from(spans));
        }
        if end < service_info.len() {
            lines.push(Line::from(Span::styled(
                i18n::t_fmt1(
                    app,
                    "app.modals.preflight.services.more_services",
                    service_info.len() - end,
                ),
                Style::default().fg(th.subtext1),
            )));
        }
    }

    lines
}
