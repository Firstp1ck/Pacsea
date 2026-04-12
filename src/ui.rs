//! TUI rendering for Pacsea.
//!
//! This module renders the full terminal user interface using `ratatui`.
//! The main content (below the one-row updates bar) is split vertically into three **bands**
//! whose **order** is configurable (`main_pane_order` in `settings.conf`):
//!
//! 1) **Results** — search matches list (title row + list), selection centering when possible
//! 2) **Middle** — three columns: Recent (left), Search input (center), Install list (right)
//! 3) **Package info** — package details or news body, URL affordances, contextual keybind footer
//!
//! Default order is results → middle → package info. Row min/max limits apply to each **role**,
//! not to a fixed screen position, so limits move with the pane when reordered.
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
use crate::state::MainVerticalPane;
use crate::state::types::AppMode;
use crate::{state::AppState, theme::theme};

/// Details pane rendering module.
mod details;
pub mod helpers;
/// Middle row rendering module.
mod middle;
/// Modal overlays rendering module.
mod modals;
/// Search results rendering module.
mod results;
/// Updates pane rendering module.
mod updates;

/// What: Advance the PKGBUILD viewer to the next section (body, `ShellCheck`, `Namcap`) and align scroll.
///
/// Inputs:
/// - `app`: Mutable application state (`pkgb_visible` should be true when the user triggers this).
///
/// Output:
/// - Updates `pkgb_section_cycle` and `pkgb_scroll` in `app`.
///
/// Details:
/// - Forwards into the details-pane PKGBUILD renderer so event handlers do not reach into private
///   `details` submodules directly.
pub fn cycle_pkgbuild_view_section(app: &mut AppState) {
    details::cycle_pkgbuild_view_section(app);
}

/// What: Layout height constraints for UI panes.
///
/// Inputs: None (struct definition)
///
/// Output: None (struct definition)
///
/// Details:
/// - Groups minimum and maximum height constraints to reduce data flow complexity.
struct LayoutConstraints {
    /// Minimum height for results pane.
    min_results: u16,
    /// Minimum height for middle pane.
    min_middle: u16,
    /// Minimum height for package info pane.
    min_package_info: u16,
    /// Maximum height for results pane.
    max_results: u16,
    /// Maximum height for middle pane.
    max_middle: u16,
}

impl LayoutConstraints {
    /// What: Build constraints from user-tuned vertical limits.
    ///
    /// Inputs:
    /// - `limits`: Normalized semantic min/max row counts from settings.
    ///
    /// Output:
    /// - `LayoutConstraints` for the allocator.
    ///
    /// Details:
    /// - Values are expected already normalized (`max >= min`, within cap).
    const fn from_limits(limits: &crate::state::VerticalLayoutLimits) -> Self {
        Self {
            min_results: limits.min_results,
            min_middle: limits.min_middle,
            min_package_info: limits.min_package_info,
            max_results: limits.max_results,
            max_middle: limits.max_middle,
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
    /// Height for results pane.
    results: u16,
    /// Height for middle pane.
    middle: u16,
    /// Height for details pane.
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
/// - `constraints`: Semantic min/max row limits from settings
///
/// Output:
/// - Returns `LayoutHeights` with calculated heights for all panes
///
/// Details:
/// - Implements priority-based layout allocation with min/max constraints.
/// - Uses match expression to choose allocation strategy based on available space.
fn calculate_layout_heights(available_h: u16, constraints: &LayoutConstraints) -> LayoutHeights {
    let min_top_middle_total = constraints.min_results + constraints.min_middle;
    let space_after_min = available_h.saturating_sub(min_top_middle_total);

    match space_after_min {
        s if s >= constraints.min_package_info => {
            allocate_with_package_info(available_h, constraints)
        }
        _ => allocate_without_package_info(available_h, constraints),
    }
}

/// What: Map semantic pane heights into top-to-bottom band lengths for `main_pane_order`.
///
/// Inputs:
/// - `order`: User-configured vertical permutation.
/// - `heights`: Allocator output keyed by pane role.
///
/// Output:
/// - Three band heights matching `order[0]` → top through `order[2]` → bottom.
///
/// Details:
/// - Used by the main `Layout::split` and by unit tests in this module.
fn vertical_band_lengths_for_order(
    order: [MainVerticalPane; 3],
    heights: &LayoutHeights,
) -> [u16; 3] {
    let mut out = [0u16; 3];
    for (i, pane) in order.iter().enumerate() {
        out[i] = match pane {
            MainVerticalPane::Results => heights.results,
            MainVerticalPane::Middle => heights.middle,
            MainVerticalPane::PackageInfo => heights.details,
        };
    }
    out
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

    // Determine toast type by checking against all known news-related translation keys
    // This is language-agnostic as it compares the actual translated text
    // List of all news-related toast translation keys (add new ones here as needed)
    let news_keys = ["app.toasts.no_new_news", "app.news_button.loading"];
    let is_news_toast = news_keys.iter().any(|key| {
        let translated = i18n::t(app, key);
        msg == &translated
    });

    // Check for news age messages by comparing against translation keys or content pattern
    let translated_all = i18n::t(app, "app.results.options_menu.news_age_all");
    let is_news_age_toast = msg == &translated_all
        || msg.starts_with("News age:")
        || msg.to_lowercase().contains("news age");

    // Check for clipboard messages by content (language-agnostic pattern matching)
    let msg_lower = msg.to_lowercase();
    let is_clipboard_toast = msg_lower.contains("clipboard")
        || msg_lower.contains("wl-copy")
        || msg_lower.contains("xclip")
        || msg_lower.contains("copied")
        || msg_lower.contains("copying");

    let title_text = if is_news_toast || is_news_age_toast {
        i18n::t(app, "app.toasts.title_news")
    } else if is_clipboard_toast {
        i18n::t(app, "app.toasts.title_clipboard")
    } else {
        i18n::t(app, "app.toasts.title_notification")
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
/// - Applies global theme/background; renders the three main vertical bands in `main_pane_order`,
///   then modal overlays.
/// - Keeps results selection centered by adjusting list offset.
/// - Computes and records clickable rects (URL, Sort/Filters, Options/Config/Panels, status label).
pub fn ui(f: &mut Frame, app: &mut AppState) {
    const UPDATES_H: u16 = 1;
    let th = theme();
    let area = f.area();

    // Background
    let bg = Block::default().style(Style::default().bg(th.base));
    f.render_widget(bg, area);
    let available_h = area.height.saturating_sub(UPDATES_H);
    let constraints = LayoutConstraints::from_limits(&app.vertical_layout_limits);
    let layout = calculate_layout_heights(available_h, &constraints);
    let band_lengths = vertical_band_lengths_for_order(app.main_pane_order, &layout);

    // Split area into updates row and main content
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(UPDATES_H),
            Constraint::Length(band_lengths[0] + band_lengths[1] + band_lengths[2]),
        ])
        .split(area);

    // Render updates button in the top row
    updates::render_updates_button(f, app, main_chunks[0]);

    // Split main content into the three vertical bands (visual order from settings)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(band_lengths[0]),
            Constraint::Length(band_lengths[1]),
            Constraint::Length(band_lengths[2]),
        ])
        .split(main_chunks[1]);

    let order = app.main_pane_order;
    let mut results_band: Option<ratatui::prelude::Rect> = None;
    for (slot, role) in order.iter().enumerate() {
        let chunk = chunks[slot];
        match role {
            MainVerticalPane::Results => {
                results::render_results(f, app, chunk);
                results_band = Some(chunk);
            }
            MainVerticalPane::Middle => middle::render_middle(f, app, chunk),
            MainVerticalPane::PackageInfo => {
                if matches!(app.app_mode, AppMode::News) {
                    details::render_news_details(f, app, chunk);
                } else {
                    details::render_details(f, app, chunk);
                }
            }
        }
    }
    modals::render_modals(f, app, area);

    // Render dropdowns last to ensure they appear on top layer (now for both modes)
    if let Some(r) = results_band {
        results::render_dropdowns(f, app, r);
    }

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
        translations.insert("app.results.buttons.menu".to_string(), "Menu".to_string());
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
            "app.results.filters.blackarch".to_string(),
            "BlackArch".to_string(),
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
            out_of_date: None,
            orphaned: false,
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

    #[test]
    fn vertical_band_lengths_cover_all_six_permutations() {
        use crate::state::MainVerticalPane::{Middle, PackageInfo, Results};
        let heights = super::LayoutHeights {
            results: 11,
            middle: 22,
            details: 33,
        };
        let cases: [([crate::state::MainVerticalPane; 3], [u16; 3]); 6] = [
            ([Results, Middle, PackageInfo], [11, 22, 33]),
            ([Results, PackageInfo, Middle], [11, 33, 22]),
            ([Middle, Results, PackageInfo], [22, 11, 33]),
            ([Middle, PackageInfo, Results], [22, 33, 11]),
            ([PackageInfo, Results, Middle], [33, 11, 22]),
            ([PackageInfo, Middle, Results], [33, 22, 11]),
        ];
        for (order, expected) in cases {
            assert_eq!(
                super::vertical_band_lengths_for_order(order, &heights),
                expected,
                "order mismatch for {order:?}"
            );
        }
    }

    #[test]
    fn default_vertical_limits_match_historical_layout_heights() {
        let constraints =
            super::LayoutConstraints::from_limits(&crate::state::VerticalLayoutLimits::default());
        let h39 = super::calculate_layout_heights(39, &constraints);
        assert_eq!((h39.results, h39.middle, h39.details), (17, 5, 17));
        let h10 = super::calculate_layout_heights(10, &constraints);
        assert_eq!((h10.results, h10.middle, h10.details), (3, 4, 3));
        let h5 = super::calculate_layout_heights(5, &constraints);
        assert_eq!((h5.results, h5.middle, h5.details), (3, 3, 0));
    }

    #[test]
    fn ui_renders_when_main_pane_order_puts_results_last() {
        use crate::state::MainVerticalPane::{Middle, PackageInfo, Results};
        use ratatui::{Terminal, backend::TestBackend};

        let backend = TestBackend::new(120, 40);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");
        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.main_pane_order = [PackageInfo, Middle, Results];
        app.results = vec![crate::state::PackageItem {
            name: "pkg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }];
        app.all_results = app.results.clone();
        app.selected = 0;
        app.list_state.select(Some(0));
        app.details.url = "https://example.com".into();

        term.draw(|f| {
            super::ui(f, &mut app);
        })
        .expect("draw with reordered panes");

        assert!(app.results_rect.is_some());
        assert!(app.details_rect.is_some());
    }
}
