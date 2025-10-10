//! TUI rendering for Pacsea.
//!
//! This module renders the full terminal user interface using `ratatui`.
//! The layout is split vertically into three regions:
//!
//! 1) Results list (top): shows search matches and keeps the current selection
//!    centered when possible
//! 2) Middle row (three columns): Recent (left), Search input (center), and
//!    Install list (right), each styled based on focus
//! 3) Details pane (bottom): rich package information with a clickable URL and
//!    a contextual help footer displaying keybindings
//!
//! The renderer also draws modal overlays for alerts and install confirmation.
//! It updates `app.url_button_rect` to make the URL clickable when available.
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::Block,
};

use crate::{state::AppState, theme::theme};

mod details;
mod middle;
mod modals;
mod results;

/// Render a full frame of the Pacsea TUI.
///
/// This function is the single entry point for drawing the interface and is
/// meant to be called each tick or after state changes.
///
/// Arguments:
///
/// - `f`: `ratatui` frame to render into
/// - `app`: mutable application state; updated during rendering for selection
///   offsets, cursor position, and clickable URL geometry
///
/// Behavior summary:
///
/// - Applies the global theme and background
/// - Ensures the results selection remains centered within the list viewport
///   by adjusting the list state's internal offset
/// - Renders the top results list with source badges and install markers
/// - Renders the middle row with Recent, Search (with visible cursor), and
///   Install panes; titles and colors reflect focus
/// - Computes and stores a clickable rectangle for the details URL when it is
///   present, enabling mouse interactions handled elsewhere
/// - Shows a help footer with keybindings inside the details area
/// - Draws modal overlays for network alerts and install confirmations
pub fn ui(f: &mut Frame, app: &mut AppState) {
    let th = theme();
    let area = f.area();

    // Background
    let bg = Block::default().style(Style::default().bg(th.base));
    f.render_widget(bg, area);

    let total_h = area.height;
    let search_h: u16 = 5; // give a bit more room for history pane
    let bottom_h: u16 = total_h.saturating_mul(2) / 3; // 2/3 of full height
    let top_h: u16 = total_h.saturating_sub(search_h).saturating_sub(bottom_h);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_h),
            Constraint::Length(search_h),
            Constraint::Length(bottom_h),
        ])
        .split(area);

    results::render_results(f, app, chunks[0]);
    middle::render_middle(f, app, chunks[1]);
    details::render_details(f, app, chunks[2]);
    modals::render_modals(f, app, area);
}
