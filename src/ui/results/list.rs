use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::ListItem,
};

use crate::state::{AppState, PackageItem, Source};
use crate::theme::{PackageMarker, Theme};

/// What: Check if a package is in any of the install/remove/downgrade lists.
///
/// Inputs:
/// - `package`: Package to check
/// - `app`: Application state containing the lists
///
/// Output:
/// - Struct containing boolean flags for each list type
///
/// Details:
/// - Performs case-insensitive name matching against all three lists.
pub struct PackageListStatus {
    pub in_install: bool,
    pub in_remove: bool,
    pub in_downgrade: bool,
}

/// What: Check if a package is in any of the install/remove/downgrade lists.
///
/// Inputs:
/// - `package`: Package to check
/// - `app`: Application state containing the lists
///
/// Output:
/// - `PackageListStatus` struct with boolean flags for each list type
///
/// Details:
/// - Performs case-insensitive name matching against all three lists.
pub fn check_package_in_lists(package: &PackageItem, app: &AppState) -> PackageListStatus {
    let in_install = app
        .install_list
        .iter()
        .any(|it| it.name.eq_ignore_ascii_case(&package.name));
    let in_remove = app
        .remove_list
        .iter()
        .any(|it| it.name.eq_ignore_ascii_case(&package.name));
    let in_downgrade = app
        .downgrade_list
        .iter()
        .any(|it| it.name.eq_ignore_ascii_case(&package.name));

    PackageListStatus {
        in_install,
        in_remove,
        in_downgrade,
    }
}

/// What: Determine the source label and color for a package.
///
/// Inputs:
/// - `source`: Package source (Official or AUR)
/// - `package_name`: Name of the package
/// - `app`: Application state for accessing details cache
/// - `theme`: Theme for color values
///
/// Output:
/// - Tuple of (label: String, color: Color)
///
/// Details:
/// - For official packages, determines label from repo/owner and selects color
///   based on whether it's an optional repo (sapphire) or standard (green).
/// - For AUR packages, returns "AUR" with yellow color.
pub fn determine_source_label_and_color(
    source: &Source,
    package_name: &str,
    app: &AppState,
    theme: &Theme,
) -> (String, ratatui::style::Color) {
    match source {
        Source::Official { repo, .. } => {
            let owner = app
                .details_cache
                .get(package_name)
                .map(|d| d.owner.clone())
                .unwrap_or_default();
            let label = crate::logic::distro::label_for_official(repo, package_name, &owner);
            let color = if label == "EOS"
                || label == "CachyOS"
                || label == "Artix"
                || label == "OMNI"
                || label == "UNI"
                || label == "LIB32"
                || label == "GALAXY"
                || label == "WORLD"
                || label == "SYSTEM"
                || label == "Manjaro"
            {
                theme.sapphire
            } else {
                theme.green
            };
            (label, color)
        }
        Source::Aur => ("AUR".to_string(), theme.yellow),
    }
}

/// What: Build a `ListItem` with package marker styling applied.
///
/// Inputs:
/// - `segs`: Vector of spans representing the package information
/// - `marker_type`: Type of marker to apply (`FullLine`, `Front`, or `End`)
/// - `label`: Marker label text (e.g., "[+]", "[-]", "[↓]")
/// - `color`: Color for the marker
/// - `in_install`: Whether package is in install list (affects `FullLine` background)
/// - `theme`: Theme for additional color values
///
/// Output:
/// - `ListItem` with marker styling applied
///
/// Details:
/// - `FullLine`: Colors the entire line background with dimmed color for installs
/// - Front: Adds marker at the beginning of the line
/// - End: Adds marker at the end of the line
pub fn build_package_marker_item(
    segs: Vec<Span<'static>>,
    marker_type: PackageMarker,
    label: &str,
    color: ratatui::style::Color,
    in_install: bool,
    theme: &Theme,
) -> ListItem<'static> {
    match marker_type {
        PackageMarker::FullLine => {
            let mut item = ListItem::new(Line::from(segs));
            let bgc = if in_install {
                if let ratatui::style::Color::Rgb(r, g, b) = color {
                    ratatui::style::Color::Rgb(
                        u8::try_from((u16::from(r) * 85) / 100).unwrap_or(255),
                        u8::try_from((u16::from(g) * 85) / 100).unwrap_or(255),
                        u8::try_from((u16::from(b) * 85) / 100).unwrap_or(255),
                    )
                } else {
                    color
                }
            } else {
                color
            };
            item = item.style(Style::default().fg(theme.crust).bg(bgc));
            item
        }
        PackageMarker::Front => {
            let mut new_segs: Vec<Span> = Vec::new();
            new_segs.push(Span::styled(
                label.to_string(),
                Style::default()
                    .fg(theme.crust)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ));
            new_segs.push(Span::raw(" "));
            new_segs.extend(segs);
            ListItem::new(Line::from(new_segs))
        }
        PackageMarker::End => {
            let mut new_segs = segs.clone();
            new_segs.push(Span::raw(" "));
            new_segs.push(Span::styled(
                label.to_string(),
                Style::default()
                    .fg(theme.crust)
                    .bg(color)
                    .add_modifier(Modifier::BOLD),
            ));
            ListItem::new(Line::from(new_segs))
        }
    }
}

/// What: Build a `ListItem` for a package in the results list.
///
/// Inputs:
/// - `package`: Package to render
/// - `app`: Application state for accessing cache and lists
/// - `theme`: Theme for styling
/// - `prefs`: Theme preferences including package marker type
/// - `in_viewport`: Whether this item is in the visible viewport
///
/// Output:
/// - `ListItem` ready for rendering
///
/// Details:
/// - Returns empty item if not in viewport for performance.
/// - Builds spans for popularity, source label, name, version, description, and installed status.
/// - Applies package markers if package is in install/remove/downgrade lists.
pub fn build_list_item(
    package: &PackageItem,
    app: &AppState,
    theme: &Theme,
    prefs: &crate::theme::Settings,
    in_viewport: bool,
) -> ListItem<'static> {
    // For rows outside the viewport, render a cheap empty item
    if !in_viewport {
        return ListItem::new(Line::raw(""));
    }

    let (src, color) = determine_source_label_and_color(&package.source, &package.name, app, theme);

    let desc = if package.description.is_empty() {
        app.details_cache
            .get(&package.name)
            .map(|d| d.description.clone())
            .unwrap_or_default()
    } else {
        package.description.clone()
    };

    let installed = crate::index::is_installed(&package.name);

    // Build the main content spans
    let mut segs: Vec<Span<'static>> = Vec::new();
    if let Some(pop) = package.popularity {
        segs.push(Span::styled(
            format!("Pop: {pop:.2} "),
            Style::default().fg(theme.overlay1),
        ));
    }
    segs.push(Span::styled(format!("{src} "), Style::default().fg(color)));
    segs.push(Span::styled(
        package.name.clone(),
        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
    ));
    segs.push(Span::styled(
        format!("  {}", package.version),
        Style::default().fg(theme.overlay1),
    ));
    if !desc.is_empty() {
        segs.push(Span::raw("  - "));
        segs.push(Span::styled(desc, Style::default().fg(theme.overlay2)));
    }
    if installed {
        segs.push(Span::raw("  "));
        segs.push(Span::styled(
            "[Installed]",
            Style::default()
                .fg(theme.green)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Check if package is in any lists and apply markers if needed
    let list_status = check_package_in_lists(package, app);
    if list_status.in_install || list_status.in_remove || list_status.in_downgrade {
        let (label, marker_color) = if list_status.in_remove {
            ("[-]", theme.red)
        } else if list_status.in_downgrade {
            ("[↓]", theme.yellow)
        } else {
            ("[+]", theme.green)
        };
        build_package_marker_item(
            segs,
            prefs.package_marker,
            label,
            marker_color,
            list_status.in_install,
            theme,
        )
    } else {
        ListItem::new(Line::from(segs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_package_in_lists() {
        let mut app = crate::state::AppState::default();
        let package = PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
        };

        // Initially not in any list
        let status = check_package_in_lists(&package, &app);
        assert!(!status.in_install);
        assert!(!status.in_remove);
        assert!(!status.in_downgrade);

        // Add to install list
        app.install_list.push(crate::state::PackageItem {
            name: "TEST-PKG".to_string(), // Test case-insensitive
            version: "1.0".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
        });
        let status = check_package_in_lists(&package, &app);
        assert!(status.in_install);
        assert!(!status.in_remove);
        assert!(!status.in_downgrade);
    }

    #[test]
    fn test_determine_source_label_and_color_aur() {
        let app = crate::state::AppState::default();
        let theme = crate::theme::theme();
        let source = Source::Aur;

        let (label, color) = determine_source_label_and_color(&source, "test-pkg", &app, &theme);
        assert_eq!(label, "AUR");
        assert_eq!(color, theme.yellow);
    }

    #[test]
    fn test_determine_source_label_and_color_official() {
        let app = crate::state::AppState::default();
        let theme = crate::theme::theme();
        let source = Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        };

        let (label, color) = determine_source_label_and_color(&source, "test-pkg", &app, &theme);
        assert_eq!(label, "core");
        assert_eq!(color, theme.green);
    }

    #[test]
    fn test_build_list_item_not_in_viewport() {
        let app = crate::state::AppState::default();
        let theme = crate::theme::theme();
        let prefs = crate::theme::settings();
        let package = PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: "Test description".to_string(),
            source: Source::Aur,
            popularity: None,
        };

        let item = build_list_item(&package, &app, &theme, &prefs, false);
        // Verify that the item is created (not in viewport returns empty item)
        let _ = item;
    }

    #[test]
    fn test_build_list_item_in_viewport() {
        let app = crate::state::AppState::default();
        let theme = crate::theme::theme();
        let prefs = crate::theme::settings();
        let package = PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0".to_string(),
            description: "Test description".to_string(),
            source: Source::Aur,
            popularity: Some(1.5),
        };

        let item = build_list_item(&package, &app, &theme, &prefs, true);
        // Verify that the item is created (in viewport returns populated item)
        let _ = item;
    }
}
