use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::AppState;
use crate::state::modal::PreflightHeaderChips;
use crate::theme::theme;

pub mod extract;
pub mod layout;
pub mod scroll;
pub mod sync;
pub mod tabs;
pub mod widget;

// Re-export byte formatting functions from helpers
pub use crate::ui::helpers::{format_bytes, format_signed_bytes};

/// What: Format count with incomplete data indicator when data is still resolving.
///
/// Inputs:
/// - `count`: Current count of resolved items.
/// - `total_items`: Total number of items expected.
/// - `is_resolving`: Whether resolution is still in progress.
///
/// Output:
/// - Returns formatted string like "7", "7...", or "7/9" depending on state.
///
/// Details:
/// - Shows plain count if complete or not resolving.
/// - Shows count with "..." if partial and still resolving.
/// - Shows "count/total" if complete but count doesn't match total (shouldn't happen normally).
pub fn format_count_with_indicator(count: usize, total_items: usize, is_resolving: bool) -> String {
    if !is_resolving {
        // Resolution complete, show actual count
        format!("{}", count)
    } else if count < total_items {
        // Still resolving and we have partial data
        format!("{}...", count)
    } else {
        // Resolving but count matches total (shouldn't happen, but be safe)
        format!("{}", count)
    }
}

/// What: Render header chips as a compact horizontal line of metrics.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `chips`: Header chip data containing counts and sizes.
///
/// Output:
/// - Returns a `Line` containing styled chip spans separated by spaces.
///
/// Details:
/// - Formats package count, download size, install delta, AUR count, and risk score
///   as compact chips. Risk score uses color coding (green/yellow/red) based on level.
pub fn render_header_chips(app: &AppState, chips: &PreflightHeaderChips) -> Line<'static> {
    let th = theme();
    let mut spans = Vec::new();

    // Package count chip
    let pkg_text = if chips.aur_count > 0 {
        format!(
            "{}{}",
            chips.package_count,
            i18n::t_fmt1(
                app,
                "app.modals.preflight.header_chips.aur_packages",
                chips.aur_count
            )
        )
    } else {
        format!("{}", chips.package_count)
    };
    spans.push(Span::styled(
        format!("[{}]", pkg_text),
        Style::default()
            .fg(th.sapphire)
            .add_modifier(Modifier::BOLD),
    ));

    // Download size chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    spans.push(Span::styled(
        i18n::t_fmt1(
            app,
            "app.modals.preflight.header_chips.download_label",
            format_bytes(chips.download_bytes),
        ),
        Style::default().fg(th.sapphire),
    ));

    // Install delta chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let delta_color = if chips.install_delta_bytes > 0 {
        th.green
    } else if chips.install_delta_bytes < 0 {
        th.red
    } else {
        th.overlay1 // Neutral color for zero
    };
    spans.push(Span::styled(
        i18n::t_fmt1(
            app,
            "app.modals.preflight.header_chips.size_label",
            format_signed_bytes(chips.install_delta_bytes),
        ),
        Style::default().fg(delta_color),
    ));

    // Risk score chip (always shown)
    spans.push(Span::styled(" ", Style::default()));
    let risk_label = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_low")
        }
        crate::state::modal::RiskLevel::Medium => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_medium")
        }
        crate::state::modal::RiskLevel::High => {
            i18n::t(app, "app.modals.preflight.header_chips.risk_high")
        }
    };
    let risk_color = match chips.risk_level {
        crate::state::modal::RiskLevel::Low => th.green,
        crate::state::modal::RiskLevel::Medium => th.yellow,
        crate::state::modal::RiskLevel::High => th.red,
    };
    spans.push(Span::styled(
        format!("[Risk: {} ({})]", risk_label, chips.risk_score),
        Style::default().fg(risk_color).add_modifier(Modifier::BOLD),
    ));

    Line::from(spans)
}

#[cfg(test)]
mod format_tests;
#[cfg(test)]
mod layout_tests;
#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod sync_dependencies_tests;
#[cfg(test)]
mod sync_files_tests;
#[cfg(test)]
mod sync_sandbox_tests;
#[cfg(test)]
mod sync_services_tests;
