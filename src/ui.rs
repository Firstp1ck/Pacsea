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
mod updates;

/// What: Layout height constraints for UI panes.
///
/// Inputs: None (struct definition)
///
/// Output: None (struct definition)
///
/// Details:
/// - Groups minimum and maximum height constraints to reduce data flow complexity.
struct LayoutConstraints {
    min_results: u16,
    min_middle: u16,
    min_package_info: u16,
    max_results: u16,
    max_middle: u16,
}

impl LayoutConstraints {
    /// What: Create default layout constraints.
    ///
    /// Inputs: None
    ///
    /// Output: `LayoutConstraints` with default values
    ///
    /// Details:
    /// - Returns constraints with standard minimum and maximum heights for all panes.
    const fn new() -> Self {
        Self {
            min_results: 3,
            min_middle: 3,
            min_package_info: 3,
            max_results: 17,
            max_middle: 5,
        }
    }
}

/// What: Calculated layout heights for UI panes.
///
/// Inputs: None (struct definition)
///
/// Output: None (struct definition)
///
/// Details:
/// - Groups related layout parameters to reduce data flow complexity by grouping related fields.
struct LayoutHeights {
    results: u16,
    middle: u16,
    details: u16,
}

/// What: Calculate middle pane height based on available space and constraints.
///
/// Inputs:
/// - `available_h`: Available height for middle pane
/// - `min_results_h`: Minimum height required for results pane
/// - `constraints`: Layout constraints
///
/// Output:
/// - Returns calculated middle pane height
///
/// Details:
/// - Uses match expression to determine height based on available space thresholds.
const fn calculate_middle_height(
    available_h: u16,
    min_results_h: u16,
    constraints: &LayoutConstraints,
) -> u16 {
    match available_h {
        h if h >= constraints.max_middle + min_results_h => constraints.max_middle,
        h if h >= constraints.min_middle + min_results_h => h.saturating_sub(min_results_h),
        _ => constraints.min_middle,
    }
}

/// What: Calculate results pane height based on available space and middle height.
///
/// Inputs:
/// - `available_h`: Available height for results pane
/// - `middle_h`: Height allocated to middle pane
/// - `constraints`: Layout constraints
///
/// Output:
/// - Returns calculated results pane height
///
/// Details:
/// - Clamps results height between minimum and maximum constraints.
fn calculate_results_height(
    available_h: u16,
    middle_h: u16,
    constraints: &LayoutConstraints,
) -> u16 {
    available_h
        .saturating_sub(middle_h)
        .clamp(constraints.min_results, constraints.max_results)
}

/// What: Allocate layout heights when package info pane can be shown.
///
/// Inputs:
/// - `available_h`: Total available height
/// - `constraints`: Layout constraints
///
/// Output:
/// - Returns `LayoutHeights` with allocated heights
///
/// Details:
/// - Allocates 75% of space to Results and Middle, remainder to Package Info.
/// - Redistributes if Package Info doesn't have minimum space.
fn allocate_with_package_info(available_h: u16, constraints: &LayoutConstraints) -> LayoutHeights {
    let top_middle_share = (available_h * 3) / 4;

    let search_h_initial =
        calculate_middle_height(top_middle_share, constraints.min_results, constraints);
    let remaining_for_results = top_middle_share.saturating_sub(search_h_initial);
    let top_h = remaining_for_results.clamp(constraints.min_results, constraints.max_results);

    let unused_results_space = remaining_for_results.saturating_sub(top_h);
    let search_h = (search_h_initial + unused_results_space).min(constraints.max_middle);

    let remaining_for_package = available_h.saturating_sub(top_h).saturating_sub(search_h);

    match remaining_for_package {
        h if h >= constraints.min_package_info => LayoutHeights {
            results: top_h,
            middle: search_h,
            details: remaining_for_package,
        },
        _ => {
            // Redistribute: Middle gets max first, then Results gets the rest
            let search_h_final =
                calculate_middle_height(available_h, constraints.min_results, constraints);
            let top_h_final = calculate_results_height(available_h, search_h_final, constraints);

            LayoutHeights {
                results: top_h_final,
                middle: search_h_final,
                details: 0,
            }
        }
    }
}

/// What: Allocate layout heights when package info pane cannot be shown.
///
/// Inputs:
/// - `available_h`: Total available height
/// - `constraints`: Layout constraints
///
/// Output:
/// - Returns `LayoutHeights` with allocated heights (details = 0)
///
/// Details:
/// - Allocates all space between Results and Middle panes.
/// - Adjusts if minimum constraints exceed available space.
fn allocate_without_package_info(
    available_h: u16,
    constraints: &LayoutConstraints,
) -> LayoutHeights {
    let search_h = calculate_middle_height(available_h, constraints.min_results, constraints);
    let mut top_h = calculate_results_height(available_h, search_h, constraints);

    match (top_h + search_h).cmp(&available_h) {
        std::cmp::Ordering::Greater => {
            top_h = available_h
                .saturating_sub(constraints.min_middle)
                .clamp(constraints.min_results, constraints.max_results);
            let search_h_adjusted = available_h
                .saturating_sub(top_h)
                .clamp(constraints.min_middle, constraints.max_middle);

            LayoutHeights {
                results: top_h,
                middle: search_h_adjusted,
                details: 0,
            }
        }
        _ => LayoutHeights {
            results: top_h,
            middle: search_h,
            details: 0,
        },
    }
}

/// What: Calculate layout heights for Results, Middle, and Details panes.
///
/// Inputs:
/// - `available_h`: Available height after reserving space for updates button
///
/// Output:
/// - Returns `LayoutHeights` with calculated heights for all panes
///
/// Details:
/// - Implements priority-based layout allocation with min/max constraints.
/// - Uses match expression to choose allocation strategy based on available space.
fn calculate_layout_heights(available_h: u16) -> LayoutHeights {
    let constraints = LayoutConstraints::new();
    let min_top_middle_total = constraints.min_results + constraints.min_middle;
    let space_after_min = available_h.saturating_sub(min_top_middle_total);

    match space_after_min {
        s if s >= constraints.min_package_info => {
            allocate_with_package_info(available_h, &constraints)
        }
        _ => allocate_without_package_info(available_h, &constraints),
    }
}

/// What: Render toast message overlay in bottom-right corner.
///
/// Inputs:
/// - `f`: `ratatui` frame to render into
/// - `app`: Application state containing toast message
/// - `area`: Full terminal area for positioning
///
/// Output:
/// - Renders toast widget if message is present
///
/// Details:
/// - Positions toast in bottom-right corner with appropriate sizing.
/// - Uses match expression to determine toast title based on message content.
#[allow(clippy::many_single_char_names)]
fn render_toast(f: &mut Frame, app: &AppState, area: ratatui::prelude::Rect) {
    let Some(msg) = &app.toast_message else {
        return;
    };

    let th = theme();
    let inner_w = u16::try_from(msg.len())
        .unwrap_or(u16::MAX)
        .min(area.width.saturating_sub(4));
    let w = inner_w.saturating_add(2 + 2);
    let h: u16 = 3;
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

    const UPDATES_H: u16 = 1;
    let available_h = area.height.saturating_sub(UPDATES_H);
    let layout = calculate_layout_heights(available_h);

    // Split area into updates row and main content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(UPDATES_H),
            Constraint::Length(layout.results + layout.middle + layout.details),
        ])
        .split(area);

    // Render updates button in the top row
    updates::render_updates_button(f, app, main_chunks[0]);

    // Split main content into results, middle, and details
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(layout.results),
            Constraint::Length(layout.middle),
            Constraint::Length(layout.details),
        ])
        .split(main_chunks[1]);

    results::render_results(f, app, chunks[0]);
    middle::render_middle(f, app, chunks[1]);
    details::render_details(f, app, chunks[2]);
    modals::render_modals(f, app, area);

    // Render dropdowns last to ensure they appear on top layer
    results::render_dropdowns(f, app, chunks[0]);

    // Render transient toast (bottom-right) if present
    render_toast(f, app, area);
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
    /// - `app`: `AppState` to populate with translations
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
            "OMNI".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_universe".to_string(),
            "UNI".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_lib32".to_string(),
            "LIB32".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_galaxy".to_string(),
            "GALAXY".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_world".to_string(),
            "WORLD".to_string(),
        );
        translations.insert(
            "app.results.filters.artix_system".to_string(),
            "SYSTEM".to_string(),
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
        translations.insert("app.toasts.title_news".to_string(), "News".to_string());
        translations.insert(
            "app.toasts.title_clipboard".to_string(),
            "Clipboard".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    #[test]
    fn ui_renders_frame_and_sets_rects_and_toast() {
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(120, 40);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");
        let mut app = crate::state::AppState::default();
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
        .expect("failed to draw test terminal");

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
        .expect("failed to draw test terminal second time");

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
