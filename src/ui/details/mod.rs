use ratatui::{Frame, prelude::Rect};

use crate::state::AppState;

mod footer;
mod layout;
mod package_info;
mod pkgbuild;

/// Render the bottom details pane and optional PKGBUILD viewer.
///
/// Updates geometry fields on [`AppState`] for mouse hit-testing and draws a
/// contextual footer with keybindings. When `app.pkgb_visible` is true, splits
/// the area to show the PKGBUILD content with scroll support.
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
    /// What: Details render sets URL/PKGBUILD rects and mouse flags
    ///
    /// - Input: AppState with URL present and PKGBUILD visible
    /// - Output: details/url/PKGBUILD rects are Some; mouse is disabled in details
    #[test]
    fn details_sets_url_and_pkgb_rects() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();

        let mut app = crate::state::AppState {
            ..Default::default()
        };
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
        .unwrap();

        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());
        assert!(app.pkgb_button_rect.is_some());
        assert!(app.pkgb_check_button_rect.is_some());
        assert!(app.pkgb_rect.is_some());
        assert!(app.mouse_disabled_in_details);
    }
}
