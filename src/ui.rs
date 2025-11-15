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

use crate::i18n;
use crate::{state::AppState, theme::theme};

mod details;
pub mod helpers;
mod middle;
mod modals;
mod results;

/// What: Render a full frame of the Pacsea TUI.
///
/// Inputs:
/// - `f`: `ratatui` frame to render into
/// - `app`: Mutable application state; updated during rendering for selection offsets,
///   cursor position, and clickable geometry
///
/// Output:
/// - Draws the entire interface and updates hit-test rectangles used by mouse handlers.
///
/// Details:
/// - Applies global theme/background; renders Results (top), Middle (left/center/right), Details
///   (bottom), and Modal overlays.
/// - Keeps results selection centered by adjusting list offset.
/// - Computes and records clickable rects (URL, Sort/Filters, Options/Config/Panels, status label).
pub fn ui(f: &mut Frame, app: &mut AppState) {
    let th = theme();
    let area = f.area();

    // Background
    let bg = Block::default().style(Style::default().bg(th.base));
    f.render_widget(bg, area);

    let total_h = area.height;

    // Minimum heights required (including borders: 2 lines for top/bottom borders)
    const MIN_RESULTS_H: u16 = 3; // 1 visible line + 2 borders
    const MIN_MIDDLE_H: u16 = 3; // 1 visible line + 2 borders
    const MIN_PACKAGE_INFO_H: u16 = 3; // 1 visible line + 2 borders

    // Maximum heights (including borders)
    const MAX_RESULTS_H: u16 = 17; // Maximum height for Results pane
    const MAX_MIDDLE_H: u16 = 5; // Maximum height for Middle (three-pane) section

    // Allocate space in priority order:
    // 1. Keybinds vanish first (handled by details.rs)
    // 2. Results and Middle shrink proportionally together (they can grow when space is available)
    // 3. Package Info pane gets remaining space, vanishes if not enough

    // Allocate space to Results and Middle first (they can grow beyond minimum)
    // Reserve some space for Package Info if there's enough
    let min_top_middle_total = MIN_RESULTS_H + MIN_MIDDLE_H;
    let space_after_min = total_h.saturating_sub(min_top_middle_total);

    // If there's space beyond minimums, allocate it to Results and Middle
    // Package Info only gets space if there's enough left after Results and Middle grow
    let (top_h, search_h, bottom_h) = if space_after_min >= MIN_PACKAGE_INFO_H {
        // Enough space for all three: Results and Middle get most of the space (75%), Package Info gets remainder (25%)
        let top_middle_share = (total_h * 3) / 4; // 75% for Results + Middle

        // First, ensure Middle gets its maximum (5 lines) if possible
        // Need at least MAX_MIDDLE_H + MIN_RESULTS_H = 5 + 3 = 8 lines to give Middle its max
        let search_h_initial = if top_middle_share >= MAX_MIDDLE_H + MIN_RESULTS_H {
            MAX_MIDDLE_H
        } else if top_middle_share >= MIN_MIDDLE_H + MIN_RESULTS_H {
            // Enough for both minimums, but not enough for Middle's max
            top_middle_share.saturating_sub(MIN_RESULTS_H)
        } else {
            MIN_MIDDLE_H
        };

        // Results gets the remaining space within top_middle_share, up to its maximum
        let remaining_for_results = top_middle_share.saturating_sub(search_h_initial);
        let top_h = remaining_for_results.clamp(MIN_RESULTS_H, MAX_RESULTS_H);

        // If Results didn't use all its allocated space, give the extra back to Middle (up to its max)
        let unused_results_space = remaining_for_results.saturating_sub(top_h);
        let search_h = (search_h_initial + unused_results_space).min(MAX_MIDDLE_H);

        // Package Info gets what's left, but at least minimum if possible
        let remaining_for_package = total_h.saturating_sub(top_h).saturating_sub(search_h);
        if remaining_for_package >= MIN_PACKAGE_INFO_H {
            (top_h, search_h, remaining_for_package)
        } else {
            // Not enough space for Package Info: redistribute to Results and Middle
            // Ensure Middle gets its maximum first, then Results gets the rest
            let remaining = total_h;
            let search_h_final = if remaining >= MAX_MIDDLE_H + MIN_RESULTS_H {
                MAX_MIDDLE_H
            } else if remaining >= MIN_MIDDLE_H + MIN_RESULTS_H {
                remaining.saturating_sub(MIN_RESULTS_H)
            } else {
                MIN_MIDDLE_H
            };
            let remaining_for_results = remaining.saturating_sub(search_h_final);
            let top_h_final = remaining_for_results.clamp(MIN_RESULTS_H, MAX_RESULTS_H);
            (top_h_final, search_h_final, 0)
        }
    } else {
        // Not enough space for Package Info: Results and Middle share all space
        // Ensure Middle gets its maximum first, then Results gets the rest
        let search_h = if total_h >= MAX_MIDDLE_H + MIN_RESULTS_H {
            MAX_MIDDLE_H
        } else if total_h >= MIN_MIDDLE_H + MIN_RESULTS_H {
            total_h.saturating_sub(MIN_RESULTS_H)
        } else {
            MIN_MIDDLE_H
        };
        let remaining_for_results = total_h.saturating_sub(search_h);
        let mut top_h = remaining_for_results.clamp(MIN_RESULTS_H, MAX_RESULTS_H);

        // If enforcing minimums exceeded space, adjust
        if top_h + search_h > total_h {
            top_h = total_h
                .saturating_sub(MIN_MIDDLE_H)
                .clamp(MIN_RESULTS_H, MAX_RESULTS_H);
            let search_h_adjusted = total_h
                .saturating_sub(top_h)
                .clamp(MIN_MIDDLE_H, MAX_MIDDLE_H);
            (top_h, search_h_adjusted, 0)
        } else {
            (top_h, search_h, 0)
        }
    };

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

    // Render dropdowns last to ensure they appear on top layer
    results::render_dropdowns(f, app, chunks[0]);

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
            i18n::t(app, "app.toasts.title_news")
        } else {
            i18n::t(app, "app.toasts.title_clipboard")
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
    /// What: Ensure the top-level UI renderer draws successfully and records key rectangles.
    ///
    /// Inputs:
    /// - `app`: Minimal [`AppState`] seeded with one result, URL, and optional toast message.
    ///
    /// Output:
    /// - Rendering completes twice (with and without toast) and critical rects become `Some`.
    ///
    /// Details:
    /// - Uses `TestBackend` to render `ui`, verifying toast handling and rect bookkeeping without
    ///   panics across successive draws.
    ///
    /// What: Initialize minimal English translations for tests.
    ///
    /// Inputs:
    /// - `app`: AppState to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with minimal English translations
    ///
    /// Details:
    /// - Sets up only the translations needed for tests to pass
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        // Details
        translations.insert("app.details.fields.url".to_string(), "URL".to_string());
        translations.insert("app.details.url_label".to_string(), "URL:".to_string());
        // Results
        translations.insert("app.results.title".to_string(), "Results".to_string());
        translations.insert("app.results.buttons.sort".to_string(), "Sort".to_string());
        translations.insert(
            "app.results.buttons.options".to_string(),
            "Options".to_string(),
        );
        translations.insert(
            "app.results.buttons.panels".to_string(),
            "Panels".to_string(),
        );
        translations.insert(
            "app.results.buttons.config_lists".to_string(),
            "Config/Lists".to_string(),
        );
        translations.insert("app.results.filters.aur".to_string(), "AUR".to_string());
        translations.insert("app.results.filters.core".to_string(), "core".to_string());
        translations.insert("app.results.filters.extra".to_string(), "extra".to_string());
        translations.insert(
            "app.results.filters.multilib".to_string(),
            "multilib".to_string(),
        );
        translations.insert("app.results.filters.eos".to_string(), "EOS".to_string());
        translations.insert(
            "app.results.filters.cachyos".to_string(),
            "CachyOS".to_string(),
        );
        translations.insert("app.results.filters.artix".to_string(), "Artix".to_string());
        translations.insert(
            "app.results.filters.artix_omniverse".to_string(),
            "omniverse".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_universe".to_string(),
            "universe".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_lib32".to_string(),
            "lib32".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_galaxy".to_string(),
            "galaxy".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_world".to_string(),
            "world".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_system".to_string(),
            "system".to_string(),
        );
        translations.insert(
            "app.results.filters.manjaro".to_string(),
            "Manjaro".to_string(),
        );
        // Toasts
        translations.insert(
            "app.toasts.copied_to_clipboard".to_string(),
            "Copied to clipboard".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    #[test]
    fn ui_renders_frame_and_sets_rects_and_toast() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(120, 40);
        let mut term = Terminal::new(backend).unwrap();
        let mut app = crate::state::AppState {
            ..Default::default()
        };
        init_test_translations(&mut app);
        // Seed minimal data to exercise all three sections
        app.results = vec![crate::state::PackageItem {
            name: "pkg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        app.all_results = app.results.clone();
        app.selected = 0;
        app.list_state.select(Some(0));
        app.details.url = "https://example.com".into();
        app.toast_message = Some(crate::i18n::t(&app, "app.toasts.copied_to_clipboard"));

        term.draw(|f| {
            super::ui(f, &mut app);
        })
        .unwrap();

        // Expect rects set by sub-renderers
        assert!(app.results_rect.is_some());
        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());

        // Verify buffer was rendered with correct dimensions
        let buffer = term.backend().buffer();
        assert_eq!(buffer.area.width, 120);
        assert_eq!(buffer.area.height, 40);

        // Second render without toast should still work
        app.toast_message = None;
        term.draw(|f| {
            super::ui(f, &mut app);
        })
        .unwrap();

        // Verify rects are still set after second render
        assert!(app.results_rect.is_some());
        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());

        // Verify buffer dimensions remain correct
        let buffer = term.backend().buffer();
        assert_eq!(buffer.area.width, 120);
        assert_eq!(buffer.area.height, 40);
    }
}
