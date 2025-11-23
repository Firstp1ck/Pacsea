use ratatui::{Frame, prelude::Rect};

use crate::state::AppState;

mod footer;
mod layout;
mod package_info;
mod pkgbuild;
mod pkgbuild_highlight;

/// What: Render the bottom details pane, footer, and optional PKGBUILD viewer.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (details, PKGBUILD, footer flags)
/// - `area`: Target rectangle for the details section
///
/// Output:
/// - Draws package information, optional PKGBUILD, and footer while updating mouse hit-test rects.
///
/// Details:
/// - Computes layout splits for details, PKGBUILD, and footer; records rects on [`AppState`] for
///   URL/PKGBUILD interaction and toggles footer visibility based on available height.
pub fn render_details(f: &mut Frame, app: &mut AppState, area: Rect) {
    // Calculate footer height and layout areas
    let footer_height = layout::calculate_footer_height(app, area);
    let (_content_container, details_area, pkgb_area_opt, show_keybinds) =
        layout::calculate_layout_areas(app, area, footer_height);

    // Render Package Info pane
    package_info::render_package_info(f, app, details_area);

    // Render PKGBUILD pane if visible
    if let Some(pkgb_area) = pkgb_area_opt {
        pkgbuild::render_pkgbuild(f, app, pkgb_area);
    }

    // Render footer/keybinds if enabled and there's space
    if show_keybinds {
        footer::render_footer(f, app, area, footer_height);
    }
}

#[cfg(test)]
mod tests {
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
        translations.insert("app.details.fields.url".to_string(), "URL".to_string());
        translations.insert("app.details.url_label".to_string(), "URL:".to_string());
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Confirm rendering the details pane records hit-test rectangles and disables mouse interactions when appropriate.
    ///
    /// Inputs:
    /// - `AppState` containing package details with a URL and an expanded PKGBUILD view.
    ///
    /// Output:
    /// - Details, URL button, PKGBUILD toggle, and PKGBUILD area rectangles become `Some`, and the mouse flag toggles off.
    ///
    /// Details:
    /// - Uses a `TestBackend` terminal to drive layout without user interaction, ensuring the renderer updates state.
    #[test]
    fn details_sets_url_and_pkgb_rects() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");

        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.details = crate::state::PackageDetails {
            repository: "extra".into(),
            name: "ripgrep".into(),
            version: "14".into(),
            description: String::new(),
            architecture: "x86_64".into(),
            url: "https://example.com".into(),
            licenses: vec![],
            groups: vec![],
            provides: vec![],
            depends: vec![],
            opt_depends: vec![],
            required_by: vec![],
            optional_for: vec![],
            conflicts: vec![],
            replaces: vec![],
            download_size: None,
            install_size: None,
            owner: String::new(),
            build_date: String::new(),
            popularity: None,
        };
        // Show PKGBUILD area
        app.pkgb_visible = true;
        app.pkgb_text = Some("line1\nline2\nline3".into());

        term.draw(|f| {
            let area = f.area();
            super::render_details(f, &mut app, area);
        })
        .expect("failed to draw test terminal");

        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());
        assert!(app.pkgb_button_rect.is_some());
        assert!(app.pkgb_check_button_rect.is_some());
        assert!(app.pkgb_rect.is_some());
        assert!(app.mouse_disabled_in_details);
    }
}
