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
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::{state::AppState, theme::theme};

mod details;
pub mod helpers;
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

    // Render transient toast (bottom-right) if present (framed)
    if let Some(msg) = &app.toast_message {
        let th = theme();
        let inner_w = (msg.len() as u16).min(area.width.saturating_sub(4)); // leave room for borders
        let w = inner_w.saturating_add(2 + 2); // borders + small padding
        let h: u16 = 3; // single-line message with top/bottom padding
        let x = area.x + area.width.saturating_sub(w).saturating_sub(1);
        let y = area.y + area.height.saturating_sub(h).saturating_sub(1);
        let rect = ratatui::prelude::Rect {
            x,
            y,
            width: w,
            height: h,
        };
        let title_text = if msg.to_lowercase().contains("news") {
            " News "
        } else {
            " Clipboard "
        };
        let content = Span::styled(msg.clone(), Style::default().fg(th.text));
        let p = Paragraph::new(content)
            .block(
                ratatui::widgets::Block::default()
                    .title(Span::styled(title_text, Style::default().fg(th.overlay1)))
                    .borders(ratatui::widgets::Borders::ALL)
                    .border_style(Style::default().fg(th.overlay1))
                    .style(Style::default().bg(th.mantle)),
            )
            .style(Style::default().bg(th.mantle));
        f.render_widget(p, rect);
    }
}

#[cfg(test)]
mod tests {
    /// What: Render full UI, set rects, and render toast without panic
    ///
    /// - Input: Minimal app state; first with a toast message, then without
    /// - Output: Key rects (results/details/url) are Some; no rendering errors
    #[test]
    fn ui_renders_frame_and_sets_rects_and_toast() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(120, 40);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        // Seed minimal data to exercise all three sections
        app.results = vec![crate::state::PackageItem {
            name: "pkg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.details.url = "https://example.com".into();
        app.toast_message = Some("Copied to clipboard".into());

        term.draw(|f| {
            super::ui(f, &mut app);
        })
        .unwrap();

        // Expect rects set by sub-renderers
        assert!(app.results_rect.is_some());
        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());

        // Second render without toast should still work
        app.toast_message = None;
        term.draw(|f| {
            super::ui(f, &mut app);
        })
        .unwrap();
    }
}
