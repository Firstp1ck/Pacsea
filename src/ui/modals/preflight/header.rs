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

/// What: Represents the completion and loading status of a tab.
///
/// Inputs: None (struct definition).
///
/// Output: None (struct definition).
///
/// Details: Used to track whether a tab is complete and/or loading.
struct TabStatus {
    complete: bool,
    loading: bool,
}

/// What: Calculate completion status for the summary tab.
///
/// Inputs:
/// - `summary`: Summary data option.
/// - `summary_loading`: Whether summary is currently loading.
///
/// Output: Returns `TabStatus` with completion and loading flags.
///
/// Details: Summary is complete if data exists.
const fn calculate_summary_status(
    summary: Option<&crate::state::modal::PreflightSummaryData>,
    summary_loading: bool,
) -> TabStatus {
    TabStatus {
        complete: summary.is_some(),
        loading: summary_loading,
    }
}

/// What: Calculate completion status for the dependencies tab.
///
/// Inputs:
/// - `app`: Application state (for loading flags).
/// - `item_names`: Set of all package names.
/// - `dependency_info`: Dependency information array.
///
/// Output: Returns `TabStatus` with completion and loading flags.
///
/// Details: Dependencies are complete when not loading and all packages are represented
/// (either all have deps or all have 0 deps).
fn calculate_deps_status(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
    dependency_info: &[crate::state::modal::DependencyInfo],
) -> TabStatus {
    let loading =
        app.preflight_deps_resolving || app.deps_resolving || app.preflight_deps_items.is_some();

    if loading {
        return TabStatus {
            complete: false,
            loading: true,
        };
    }

    let packages_with_deps: std::collections::HashSet<String> = dependency_info
        .iter()
        .flat_map(|d| d.required_by.iter())
        .cloned()
        .collect();

    let all_packages_have_deps = packages_with_deps.len() == item_names.len();
    let complete = item_names.is_empty()
        || dependency_info.is_empty() // All have 0 deps
        || all_packages_have_deps; // All have deps and all are represented

    TabStatus {
        complete,
        loading: false,
    }
}

/// What: Calculate completion status for the files tab.
///
/// Inputs:
/// - `app`: Application state (for loading flags).
/// - `item_names`: Set of all package names.
/// - `file_info`: File information array.
///
/// Output: Returns `TabStatus` with completion and loading flags.
///
/// Details: Files are complete when not loading and all packages have file info.
fn calculate_files_status(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
    file_info: &[crate::state::modal::PackageFileInfo],
) -> TabStatus {
    let loading =
        app.preflight_files_resolving || app.files_resolving || app.preflight_files_items.is_some();

    if loading {
        return TabStatus {
            complete: false,
            loading: true,
        };
    }

    let file_info_names: std::collections::HashSet<String> =
        file_info.iter().map(|f| f.name.clone()).collect();

    let complete = (!item_names.is_empty() && file_info_names.len() == item_names.len())
        || (item_names.is_empty() && !app.install_list_files.is_empty());

    TabStatus {
        complete,
        loading: false,
    }
}

/// What: Calculate completion status for the services tab.
///
/// Inputs:
/// - `app`: Application state (for loading flags).
/// - `services_loaded`: Whether services are loaded.
///
/// Output: Returns `TabStatus` with completion and loading flags.
///
/// Details: Services are complete if marked as loaded or if install list has services.
const fn calculate_services_status(app: &AppState, services_loaded: bool) -> TabStatus {
    let loading = app.preflight_services_resolving || app.services_resolving;
    let complete = services_loaded || (!loading && !app.install_list_services.is_empty());

    TabStatus { complete, loading }
}

/// What: Calculate completion status for the sandbox tab.
///
/// Inputs:
/// - `app`: Application state (for loading flags).
/// - `aur_items`: Set of AUR package names.
/// - `sandbox_info`: Sandbox information array.
/// - `sandbox_loaded`: Whether sandbox is loaded.
///
/// Output: Returns `TabStatus` with completion and loading flags.
///
/// Details: Sandbox is complete when not loading and all AUR packages have sandbox info.
fn calculate_sandbox_status(
    app: &AppState,
    aur_items: &std::collections::HashSet<String>,
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_loaded: bool,
) -> TabStatus {
    let loading = app.preflight_sandbox_resolving || app.sandbox_resolving;

    if loading {
        return TabStatus {
            complete: false,
            loading: true,
        };
    }

    let sandbox_info_names: std::collections::HashSet<String> = sandbox_info
        .iter()
        .map(|s| s.package_name.clone())
        .collect();

    let complete = sandbox_loaded
        || (aur_items.is_empty() || sandbox_info_names.len() == aur_items.len())
        || (aur_items.is_empty() && !app.install_list_sandbox.is_empty());

    TabStatus {
        complete,
        loading: false,
    }
}

/// What: Get status icon and color for a tab based on its status.
///
/// Inputs:
/// - `status`: Tab status (complete/loading).
/// - `th`: Theme reference.
///
/// Output: Returns tuple of (icon string, color).
///
/// Details: Returns loading icon if loading, checkmark if complete, empty otherwise.
const fn get_status_icon(
    status: &TabStatus,
    th: &crate::theme::Theme,
) -> (&'static str, ratatui::style::Color) {
    if status.loading {
        ("⟳ ", th.sapphire)
    } else if status.complete {
        ("✓ ", th.green)
    } else {
        ("", th.overlay1)
    }
}

/// What: Build completion order vector from tab statuses.
///
/// Inputs:
/// - `statuses`: Array of tab statuses.
///
/// Output: Returns vector of tab indices in completion order.
///
/// Details: Only includes tabs that are complete and not loading.
fn build_completion_order(statuses: &[TabStatus]) -> Vec<usize> {
    statuses
        .iter()
        .enumerate()
        .filter_map(|(i, status)| {
            if status.complete && !status.loading {
                Some(i)
            } else {
                None
            }
        })
        .collect()
}

/// What: Get completion highlight color based on position in completion order.
///
/// Inputs:
/// - `order_idx`: Position in completion order (0 = first, 1 = second, etc.).
/// - `th`: Theme reference.
///
/// Output: Returns color for the highlight.
///
/// Details: Uses different colors for different completion positions.
const fn get_completion_highlight_color(
    order_idx: usize,
    th: &crate::theme::Theme,
) -> ratatui::style::Color {
    match order_idx {
        0 => th.green,    // First completed
        1 => th.sapphire, // Second completed
        2 => th.mauve,    // Third completed
        _ => th.overlay1, // Others
    }
}

/// What: Check if a tab index matches the current active tab.
///
/// Inputs:
/// - `tab_idx`: Index of the tab (0-4).
/// - `current_tab`: Currently active tab enum.
///
/// Output: Returns true if the tab is active.
///
/// Details: Maps tab indices to `PreflightTab` enum variants.
const fn is_tab_active(tab_idx: usize, current_tab: PreflightTab) -> bool {
    matches!(
        (tab_idx, current_tab),
        (0, PreflightTab::Summary)
            | (1, PreflightTab::Deps)
            | (2, PreflightTab::Files)
            | (3, PreflightTab::Services)
            | (4, PreflightTab::Sandbox)
    )
}

/// What: Calculate the color for a tab based on its state.
///
/// Inputs:
/// - `is_active`: Whether the tab is currently active.
/// - `tab_idx`: Index of the tab.
/// - `completion_order`: Vector of completed tab indices in order.
/// - `th`: Theme reference.
///
/// Output: Returns the color for the tab.
///
/// Details: Active tabs use mauve, completed tabs use highlight colors, others use overlay1.
fn calculate_tab_color(
    is_active: bool,
    tab_idx: usize,
    completion_order: &[usize],
    th: &crate::theme::Theme,
) -> ratatui::style::Color {
    if is_active {
        return th.mauve;
    }

    completion_order
        .iter()
        .position(|&x| x == tab_idx)
        .map_or(th.overlay1, |order_idx| {
            get_completion_highlight_color(order_idx, th)
        })
}

/// What: Calculate the width of a tab for rectangle storage.
///
/// Inputs:
/// - `label`: Tab label text.
/// - `status_icon`: Status icon string.
/// - `is_active`: Whether the tab is active.
///
/// Output: Returns the width in characters as u16.
///
/// Details: Active tabs include brackets, adding 2 characters to the width.
fn calculate_tab_width(label: &str, status_icon: &str, is_active: bool) -> u16 {
    let base_width = label.len() + status_icon.len();
    if is_active {
        u16::try_from(base_width + 2).unwrap_or(u16::MAX) // +2 for brackets
    } else {
        u16::try_from(base_width).unwrap_or(u16::MAX)
    }
}

/// What: Create status icon span if icon exists.
///
/// Inputs:
/// - `status_icon`: Status icon string (must be static).
/// - `status_color`: Color for the icon.
///
/// Output: Returns optional span for the status icon.
///
/// Details: Returns None if icon is empty, otherwise returns styled span.
fn create_status_icon_span(
    status_icon: &'static str,
    status_color: ratatui::style::Color,
) -> Option<Span<'static>> {
    if status_icon.is_empty() {
        return None;
    }

    Some(Span::styled(
        status_icon,
        Style::default()
            .fg(status_color)
            .add_modifier(Modifier::BOLD),
    ))
}

/// What: Create tab label span based on active state.
///
/// Inputs:
/// - `label`: Tab label text.
/// - `is_active`: Whether the tab is active.
/// - `is_completed`: Whether the tab is completed.
/// - `tab_color`: Color for the label.
///
/// Output: Returns styled span for the tab label.
///
/// Details: Active tabs have brackets and bold, completed tabs have bold modifier.
fn create_tab_label_span(
    label: &str,
    is_active: bool,
    is_completed: bool,
    tab_color: ratatui::style::Color,
) -> Span<'static> {
    let text = if is_active {
        format!("[{label}]")
    } else {
        label.to_string()
    };

    let modifier = if is_active || is_completed {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };

    Span::styled(text, Style::default().fg(tab_color).add_modifier(modifier))
}

/// What: Render spans for a single tab and store its rectangle.
///
/// Inputs:
/// - `tab_idx`: Index of the tab.
/// - `label`: Tab label text.
/// - `status`: Tab status (complete/loading).
/// - `is_active`: Whether the tab is active.
/// - `completion_order`: Vector of completed tab indices in order.
/// - `tab_x`: Current X position for the tab.
/// - `tab_y`: Y position for the tab.
/// - `th`: Theme reference.
/// - `tab_rects`: Mutable reference to tab rectangles array.
///
/// Output: Returns tuple of (spans for this tab, new `tab_x` position).
///
/// Details: Builds status icon and label spans, stores rectangle for mouse clicks.
#[allow(clippy::too_many_arguments)]
fn render_single_tab(
    tab_idx: usize,
    label: &str,
    status: &TabStatus,
    is_active: bool,
    completion_order: &[usize],
    tab_x: u16,
    tab_y: u16,
    th: &crate::theme::Theme,
    tab_rects: &mut [Option<(u16, u16, u16, u16)>; 5],
) -> (Vec<Span<'static>>, u16) {
    let (status_icon, status_color) = get_status_icon(status, th);
    let tab_color = calculate_tab_color(is_active, tab_idx, completion_order, th);
    let tab_width = calculate_tab_width(label, status_icon, is_active);

    // Store rectangle for this tab
    tab_rects[tab_idx] = Some((tab_x, tab_y, tab_width, 1));
    let new_tab_x = tab_x + tab_width;

    let mut spans = Vec::new();

    // Add status icon if present
    if let Some(icon_span) = create_status_icon_span(status_icon, status_color) {
        spans.push(icon_span);
    }

    // Add tab label
    let is_completed = completion_order.contains(&tab_idx);
    spans.push(create_tab_label_span(
        label,
        is_active,
        is_completed,
        tab_color,
    ));

    (spans, new_tab_x)
}

/// What: Extract package names and AUR items from package list.
///
/// Inputs:
/// - `items`: Package items to extract from.
///
/// Output: Returns tuple of (all item names, AUR item names).
///
/// Details: Separates all packages from AUR-only packages for different completion checks.
fn extract_package_sets(
    items: &[crate::state::PackageItem],
) -> (
    std::collections::HashSet<String>,
    std::collections::HashSet<String>,
) {
    let item_names: std::collections::HashSet<String> =
        items.iter().map(|i| i.name.clone()).collect();

    let aur_items: std::collections::HashSet<String> = items
        .iter()
        .filter(|p| matches!(p.source, crate::state::Source::Aur))
        .map(|i| i.name.clone())
        .collect();

    (item_names, aur_items)
}

/// What: Build tab labels array from i18n strings.
///
/// Inputs:
/// - `app`: Application state (for i18n).
///
/// Output: Returns array of 5 tab label strings.
///
/// Details: Creates localized tab labels for all preflight tabs.
fn build_tab_labels(app: &AppState) -> [String; 5] {
    [
        i18n::t(app, "app.modals.preflight.tabs.summary"),
        i18n::t(app, "app.modals.preflight.tabs.deps"),
        i18n::t(app, "app.modals.preflight.tabs.files"),
        i18n::t(app, "app.modals.preflight.tabs.services"),
        i18n::t(app, "app.modals.preflight.tabs.sandbox"),
    ]
}

/// What: Calculate completion status for all tabs.
///
/// Inputs:
/// - `app`: Application state.
/// - `item_names`: All package names.
/// - `aur_items`: AUR package names.
/// - `summary`: Summary data.
/// - `dependency_info`: Dependency info.
/// - `file_info`: File info.
/// - `services_loaded`: Services loaded flag.
/// - `sandbox_info`: Sandbox info.
/// - `sandbox_loaded`: Sandbox loaded flag.
///
/// Output: Returns array of tab statuses.
///
/// Details: Centralizes all status calculation logic.
#[allow(clippy::too_many_arguments)]
fn calculate_all_tab_statuses(
    app: &AppState,
    item_names: &std::collections::HashSet<String>,
    aur_items: &std::collections::HashSet<String>,
    summary: Option<&crate::state::modal::PreflightSummaryData>,
    dependency_info: &[crate::state::modal::DependencyInfo],
    file_info: &[crate::state::modal::PackageFileInfo],
    services_loaded: bool,
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_loaded: bool,
) -> [TabStatus; 5] {
    [
        calculate_summary_status(summary, app.preflight_summary_resolving),
        calculate_deps_status(app, item_names, dependency_info),
        calculate_files_status(app, item_names, file_info),
        calculate_services_status(app, services_loaded),
        calculate_sandbox_status(app, aur_items, sandbox_info, sandbox_loaded),
    ]
}

/// What: Build tab header spans and calculate tab rectangles.
///
/// Inputs:
/// - `tab_labels`: Array of tab label strings.
/// - `statuses`: Array of tab statuses.
/// - `current_tab`: Currently active tab.
/// - `completion_order`: Order of completed tabs.
/// - `content_rect`: Content area rectangle.
/// - `th`: Theme reference.
/// - `tab_rects`: Mutable reference to store tab rectangles.
///
/// Output: Returns vector of spans for the tab header.
///
/// Details: Handles all tab rendering and rectangle calculation.
fn build_tab_header_spans(
    tab_labels: &[String; 5],
    statuses: &[TabStatus; 5],
    current_tab: PreflightTab,
    completion_order: &[usize],
    content_rect: Rect,
    th: &crate::theme::Theme,
    tab_rects: &mut [Option<(u16, u16, u16, u16)>; 5],
) -> Vec<Span<'static>> {
    let tab_y = content_rect.y + 2; // +1 for top border + 1 for chips line
    let mut tab_x = content_rect.x + 1; // +1 for left border
    let mut tab_spans = Vec::new();

    for (i, lbl) in tab_labels.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::raw("  "));
            tab_x += 2; // Account for spacing
        }

        let is_active = is_tab_active(i, current_tab);
        let status = &statuses[i];
        let (spans, new_tab_x) = render_single_tab(
            i,
            lbl,
            status,
            is_active,
            completion_order,
            tab_x,
            tab_y,
            th,
            tab_rects,
        );

        tab_spans.extend(spans);
        tab_x = new_tab_x;
    }

    tab_spans
}

/// What: Calculate and store content area rectangle.
///
/// Inputs:
/// - `app`: Application state to store rectangle in.
/// - `content_rect`: Original content rectangle.
///
/// Output: None (stores in app state).
///
/// Details: Calculates content area after header (3 lines: chips + tabs + empty).
#[allow(clippy::missing_const_for_fn)]
fn store_content_rect(app: &mut AppState, content_rect: Rect) {
    app.preflight_content_rect = Some((
        content_rect.x + 1,                    // +1 for left border
        content_rect.y + 4,                    // +1 for top border + 3 for header lines
        content_rect.width.saturating_sub(2),  // -2 for borders
        content_rect.height.saturating_sub(4), // -1 for top border - 3 for header lines
    ));
}

/// What: Context data for rendering tab header.
///
/// Inputs: None (struct definition).
///
/// Output: None (struct definition).
///
/// Details: Groups all data needed for tab header rendering.
/// This struct is available for future refactoring to reduce parameter count.
#[allow(dead_code)]
struct TabHeaderContext<'a> {
    app: &'a mut AppState,
    content_rect: Rect,
    current_tab: PreflightTab,
    header_chips: &'a PreflightHeaderChips,
    items: &'a [crate::state::PackageItem],
    summary: &'a Option<Box<crate::state::modal::PreflightSummaryData>>,
    dependency_info: &'a [crate::state::modal::DependencyInfo],
    file_info: &'a [crate::state::modal::PackageFileInfo],
    services_loaded: bool,
    sandbox_info: &'a [crate::logic::sandbox::SandboxInfo],
    sandbox_loaded: bool,
}

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
    tab: PreflightTab,
    header_chips: &PreflightHeaderChips,
    items: &[crate::state::PackageItem],
    summary: Option<&crate::state::modal::PreflightSummaryData>,
    dependency_info: &[crate::state::modal::DependencyInfo],
    file_info: &[crate::state::modal::PackageFileInfo],
    services_loaded: bool,
    sandbox_info: &[crate::logic::sandbox::SandboxInfo],
    sandbox_loaded: bool,
) -> (Line<'static>, Line<'static>) {
    let th = theme();
    let current_tab = tab;

    // Option 1: Extract data preparation
    let (item_names, aur_items) = extract_package_sets(items);

    // Build tab labels
    let tab_labels = build_tab_labels(app);

    // Option 2: Extract status calculation
    let statuses = calculate_all_tab_statuses(
        app,
        &item_names,
        &aur_items,
        summary,
        dependency_info,
        file_info,
        services_loaded,
        sandbox_info,
        sandbox_loaded,
    );

    // Track completion order (for highlighting)
    let completion_order = build_completion_order(&statuses);

    // Initialize tab rectangles
    app.preflight_tab_rects = [None; 5];

    // Option 3: Extract tab rendering loop
    let tab_spans = build_tab_header_spans(
        &tab_labels,
        &statuses,
        current_tab,
        &completion_order,
        content_rect,
        &th,
        &mut app.preflight_tab_rects,
    );

    // Option 5: Extract rectangle storage
    store_content_rect(app, content_rect);

    let header_chips_line = render_header_chips(app, header_chips);
    let tab_header_line = Line::from(tab_spans);

    (header_chips_line, tab_header_line)
}
