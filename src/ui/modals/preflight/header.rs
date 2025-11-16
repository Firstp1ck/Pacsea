use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::PreflightHeaderChips;
use crate::state::{AppState, PreflightTab};
use crate::theme::theme;

use super::helpers::render_header_chips;

/// What: Render tab header with progress indicators and calculate tab rectangles.
///
/// Inputs:
/// - `app`: Application state (for i18n and storing tab rects).
/// - `content_rect`: Content area rectangle.
/// - `tab`: Current active tab.
/// - `header_chips`: Header chip data.
/// - `items`: Packages in the preflight modal (for checking completion).
/// - `summary`: Summary data (for completion status).
/// - `dependency_info`: Dependency info (for completion status).
/// - `file_info`: File info (for completion status).
/// - `service_info`: Service info (for completion status).
/// - `services_loaded`: Whether services are loaded.
/// - `sandbox_info`: Sandbox info (for completion status).
/// - `sandbox_loaded`: Whether sandbox is loaded.
///
/// Output:
/// - Returns a `Line` containing the tab header with progress indicators.
///
/// Details:
/// - Calculates completion status for each tab and displays progress indicators.
/// - Checks if data is complete for ALL packages, not just if any data exists.
/// - Stores tab rectangles in `app.preflight_tab_rects` for mouse click detection.
/// - Stores content area rectangle in `app.preflight_content_rect`.
#[allow(clippy::too_many_arguments)]
pub fn render_tab_header(
    app: &mut AppState,
    content_rect: Rect,
    tab: &PreflightTab,
    header_chips: &PreflightHeaderChips,
    items: &[crate::state::PackageItem],
    summary: &Option<Box<crate::state::modal::PreflightSummaryData>>,
    dependency_info: &[crate::state::modal::DependencyInfo],
    file_info: &[crate::state::modal::PackageFileInfo],
    _service_info: &[crate::state::modal::ServiceImpact],
    services_loaded: bool,
    _sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_loaded: bool,
) -> (Line<'static>, Line<'static>) {
    let th = theme();
    let current_tab = *tab;

    // Build header tab labels and calculate tab rectangles for mouse clicks
    let tab_labels = [
        i18n::t(app, "app.modals.preflight.tabs.summary"),
        i18n::t(app, "app.modals.preflight.tabs.deps"),
        i18n::t(app, "app.modals.preflight.tabs.files"),
        i18n::t(app, "app.modals.preflight.tabs.services"),
        i18n::t(app, "app.modals.preflight.tabs.sandbox"),
    ];

    // Determine completion status for each stage
    // A stage is complete if it has data for ALL packages OR if resolution has finished (even if empty)
    let summary_complete = summary.is_some();
    let summary_loading = app.preflight_summary_resolving;

    // Get item names for file completion check
    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();

    // Deps is complete if:
    // 1. We're not resolving (resolution has finished)
    // 2. AND we can verify all packages are represented:
    //    - All packages have dependencies (packages_with_deps.len() == item_names.len())
    //    - OR all packages have 0 deps (dependency_info is empty)
    // Note: Packages with 0 dependencies won't appear in required_by, so we can't
    // easily verify all packages are represented when using cached data with mixed deps.
    // We only mark complete if we can definitively verify all packages are represented:
    // - All packages appear in required_by (all have deps), OR
    // - dependency_info is empty (all have 0 deps)
    // This is conservative but correct - when resolution finishes, if all packages have deps,
    // they'll all appear in required_by, so it will be marked complete.
    // Show loading if resolution is in progress OR if items are queued (waiting to start)
    let deps_loading =
        app.preflight_deps_resolving || app.deps_resolving || app.preflight_deps_items.is_some();
    let packages_with_deps: std::collections::HashSet<String> = dependency_info
        .iter()
        .flat_map(|d| d.required_by.iter())
        .cloned()
        .collect();
    // Check if all packages are represented in dependency data
    let all_packages_have_deps = packages_with_deps.len() == item_names.len();
    // Only mark as complete if:
    // - Not loading AND
    // - (All packages have deps AND all are represented, OR all have 0 deps)
    // Note: We don't check preflight_deps_items because it's None both when using cached data
    // and when resolution finishes, so we can't distinguish. Instead, we only mark complete
    // when we can verify all packages are represented.
    let deps_complete = !deps_loading
        && (item_names.is_empty()
            || dependency_info.is_empty() // All have 0 deps
            || all_packages_have_deps); // All have deps and all are represented

    // Check if all packages have file data
    let file_info_names: std::collections::HashSet<String> =
        file_info.iter().map(|f| f.name.clone()).collect();
    // Files is complete if:
    // 1. We're not resolving
    // 2. AND all packages have file info (file_info.len() == items.len())
    // Show loading if resolution is in progress OR if items are queued (waiting to start)
    let files_loading =
        app.preflight_files_resolving || app.files_resolving || app.preflight_files_items.is_some();
    let files_complete = !files_loading
        && ((!item_names.is_empty() && file_info_names.len() == item_names.len())
            || (item_names.is_empty() && !app.install_list_files.is_empty()));

    // Services is complete if marked as loaded OR if install list has services and we're not resolving
    let services_complete = services_loaded
        || (!app.preflight_services_resolving
            && !app.services_resolving
            && !app.install_list_services.is_empty());
    let services_loading = app.preflight_services_resolving || app.services_resolving;

    // Check if all AUR packages have sandbox data
    let aur_items: std::collections::HashSet<String> = items
        .iter()
        .filter(|p| matches!(p.source, crate::state::Source::Aur))
        .map(|i| i.name.clone())
        .collect();
    let sandbox_info_names: std::collections::HashSet<String> = _sandbox_info
        .iter()
        .map(|s| s.package_name.clone())
        .collect();
    // Sandbox is complete if:
    // 1. We're not resolving
    // 2. AND (sandbox_loaded flag is set OR all AUR packages have sandbox info)
    let sandbox_loading = app.preflight_sandbox_resolving || app.sandbox_resolving;
    let sandbox_complete = !sandbox_loading
        && ((sandbox_loaded
            || (aur_items.is_empty() || sandbox_info_names.len() == aur_items.len()))
            || (aur_items.is_empty() && !app.install_list_sandbox.is_empty()));

    // Track completion order (for highlighting)
    let mut completion_order = Vec::new();
    if summary_complete && !summary_loading {
        completion_order.push(0);
    }
    if deps_complete && !deps_loading {
        completion_order.push(1);
    }
    if files_complete && !files_loading {
        completion_order.push(2);
    }
    if services_complete && !services_loading {
        completion_order.push(3);
    }
    if sandbox_complete && !sandbox_loading {
        completion_order.push(4);
    }

    // Calculate tab rectangles for mouse click detection
    // Tab header is on the second line of content_rect (after border + chips line)
    let tab_y = content_rect.y + 2; // +1 for top border + 1 for chips line
    let mut tab_x = content_rect.x + 1; // +1 for left border
    app.preflight_tab_rects = [None; 5];

    // Build tab header with progress indicators
    let mut tab_spans: Vec<Span> = Vec::new();

    for (i, lbl) in tab_labels.iter().enumerate() {
        let is_active = matches!(
            (i, current_tab),
            (0, PreflightTab::Summary)
                | (1, PreflightTab::Deps)
                | (2, PreflightTab::Files)
                | (3, PreflightTab::Services)
                | (4, PreflightTab::Sandbox)
        );

        if i > 0 {
            tab_spans.push(Span::raw("  "));
            tab_x += 2; // Account for spacing
        }

        // Determine status indicator
        let (status_icon, status_color) = match i {
            0 => {
                if summary_loading {
                    ("⟳ ", th.sapphire)
                } else if summary_complete {
                    ("✓ ", th.green)
                } else {
                    ("", th.overlay1)
                }
            }
            1 => {
                if deps_loading {
                    ("⟳ ", th.sapphire)
                } else if deps_complete {
                    ("✓ ", th.green)
                } else {
                    ("", th.overlay1)
                }
            }
            2 => {
                if files_loading {
                    ("⟳ ", th.sapphire)
                } else if files_complete {
                    ("✓ ", th.green)
                } else {
                    ("", th.overlay1)
                }
            }
            3 => {
                if services_loading {
                    ("⟳ ", th.sapphire)
                } else if services_complete {
                    ("✓ ", th.green)
                } else {
                    ("", th.overlay1)
                }
            }
            4 => {
                if sandbox_loading {
                    ("⟳ ", th.sapphire)
                } else if sandbox_complete {
                    ("✓ ", th.green)
                } else {
                    ("", th.overlay1)
                }
            }
            _ => ("", th.overlay1),
        };

        // Highlight completed stages (show completion order)
        let completion_highlight = if completion_order.contains(&i) {
            let order_idx = completion_order.iter().position(|&x| x == i).unwrap_or(0);
            // Use different colors for different completion positions
            match order_idx {
                0 => th.green,    // First completed
                1 => th.sapphire, // Second completed
                2 => th.mauve,    // Third completed
                _ => th.overlay1, // Others
            }
        } else {
            th.overlay1
        };

        let tab_color = if is_active {
            th.mauve
        } else if completion_order.contains(&i) {
            completion_highlight
        } else {
            th.overlay1
        };

        // Calculate tab width (with brackets if active, plus status icon)
        let tab_width = if is_active {
            lbl.len() + status_icon.len() + 2 // [icon label]
        } else {
            lbl.len() + status_icon.len()
        } as u16;

        // Store rectangle for this tab
        app.preflight_tab_rects[i] = Some((tab_x, tab_y, tab_width, 1));
        tab_x += tab_width;

        // Add status icon
        if !status_icon.is_empty() {
            tab_spans.push(Span::styled(
                status_icon,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Add tab label
        if is_active {
            tab_spans.push(Span::styled(
                format!("[{}]", lbl),
                Style::default().fg(tab_color).add_modifier(Modifier::BOLD),
            ));
        } else {
            tab_spans.push(Span::styled(
                lbl.to_string(),
                Style::default()
                    .fg(tab_color)
                    .add_modifier(if completion_order.contains(&i) {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
            ));
        }
    }

    // Store content area rectangle for package group click detection
    // Content area starts after the header (3 lines: chips + tabs + empty line)
    app.preflight_content_rect = Some((
        content_rect.x + 1,                    // +1 for left border
        content_rect.y + 4,                    // +1 for top border + 3 for header lines
        content_rect.width.saturating_sub(2),  // -2 for borders
        content_rect.height.saturating_sub(4), // -1 for top border - 3 for header lines
    ));

    let header_chips_line = render_header_chips(app, header_chips);
    let tab_header_line = Line::from(tab_spans);

    (header_chips_line, tab_header_line)
}
