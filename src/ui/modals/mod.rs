use ratatui::{Frame, prelude::Rect, style::Style, widgets::Block};

use crate::state::AppState;
use crate::theme::theme;

mod alert;
mod common;
mod confirm;
mod help;
mod misc;
mod news;
mod password;
mod post_summary;
mod preflight;
mod preflight_exec;
mod renderer;
mod system_update;
mod updates;

/// What: Render modal overlays (`Alert`, `ConfirmInstall`, `ConfirmRemove`, `SystemUpdate`, `Help`, `News`).
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (modal state, rects)
/// - `area`: Full available area; modals are centered within it
///
/// Output:
/// - Draws the active modal overlay and updates any modal-specific rects for hit-testing.
///
/// Details:
/// - Clears the area behind the modal; draws a styled centered box; content varies by modal.
/// - Help dynamically reflects keymap; News draws a selectable list and records list rect.
/// - Uses trait-based rendering to reduce cognitive complexity.
pub fn render_modals(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();
    // Draw a full-screen scrim behind any active modal to avoid underlying text bleed/concatenation
    if !matches!(app.modal, crate::state::Modal::None) {
        let scrim = Block::default().style(Style::default().bg(th.mantle));
        f.render_widget(scrim, area);
    }

    // Extract modal to avoid borrow conflicts
    let modal = std::mem::replace(&mut app.modal, crate::state::Modal::None);

    // Use trait-based renderer to handle all modal variants
    app.modal = renderer::render_modal(modal, f, app, area);
}

#[cfg(test)]
mod tests {
    /// What: Render each modal variant to ensure layout rects and state assignments succeed without panic.
    ///
    /// Inputs:
    /// - Iterates through Alert, `ConfirmInstall`, `ConfirmRemove` (core item), Help, and News variants.
    ///
    /// Output:
    /// - Rendering completes without error, with Help and News variants setting their associated rectangles.
    ///
    /// Details:
    /// - Uses a `TestBackend` terminal to capture layout side effects while mutating `app.modal` as each branch runs.
    #[test]
    fn modals_set_rects_and_render_variants() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(100, 28);
        let mut term = Terminal::new(backend).expect("Failed to create terminal for test");
        // Alert
        let mut app = crate::state::AppState {
            modal: crate::state::Modal::Alert {
                message: "Test".into(),
            },
            ..Default::default()
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area);
        })
        .expect("Failed to render Alert modal");

        // ConfirmInstall
        app.modal = crate::state::Modal::ConfirmInstall { items: vec![] };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area);
        })
        .expect("Failed to render ConfirmInstall modal");

        // ConfirmRemove with core warn
        app.modal = crate::state::Modal::ConfirmRemove {
            items: vec![crate::state::PackageItem {
                name: "glibc".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Official {
                    repo: "core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            }],
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area);
        })
        .expect("Failed to render ConfirmRemove modal");

        // Help
        app.modal = crate::state::Modal::Help;
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area);
        })
        .expect("Failed to render Help modal");
        assert!(app.help_rect.is_some());

        // News
        app.modal = crate::state::Modal::News {
            items: vec![crate::state::NewsItem {
                date: "2025-10-11".into(),
                title: "Test".into(),
                url: String::new(),
            }],
            selected: 0,
        };
        term.draw(|f| {
            let area = f.area();
            super::render_modals(f, &mut app, area);
        })
        .expect("Failed to render News modal");
        assert!(app.news_rect.is_some());
        assert!(app.news_list_rect.is_some());
    }
}
