use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::modal::{
    CascadeMode, DependencyInfo, PackageFileInfo, PreflightHeaderChips, PreflightSummaryData,
    ServiceImpact,
};
use crate::state::{AppState, PackageItem, PreflightAction, PreflightTab};
use crate::theme::theme;
use std::collections::HashSet;

mod footer;
mod header;
mod helpers;
mod tabs;

use footer::render_footer;
use header::render_tab_header;
use tabs::{
    render_deps_tab, render_files_tab, render_sandbox_tab, render_services_tab, render_summary_tab,
};

/// What: Render the preflight modal summarizing dependency/file checks before install/remove.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `app`: Mutable application state (stores tab rects/content rects)
/// - `items`: Packages under review
/// - `action`: Whether the preflight is for install or removal
/// - `tab`: Active tab (Summary/Deps/Files/Services/Sandbox)
/// - `dependency_info`, `dep_selected`, `dep_tree_expanded`: Mutable dependency state/cache
/// - `file_info`, `file_selected`, `file_tree_expanded`: Mutable file analysis state/cache
/// - `cascade_mode`: Removal cascade mode when uninstalling
///
/// Output:
/// - Draws the modal content for the chosen tab and updates cached data along with clickable rects.
///
/// Details:
/// - Lazily resolves dependencies/files when first accessed, lays out tab headers, records tab
///   rectangles for mouse navigation, and tailors summaries per tab with theming cues.
#[allow(clippy::too_many_arguments)]
pub fn render_preflight(
    f: &mut Frame,
    area: Rect,
    app: &mut AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    tab: &PreflightTab,
    summary: &mut Option<Box<PreflightSummaryData>>,
    header_chips: &mut PreflightHeaderChips,
    dependency_info: &mut Vec<DependencyInfo>,
    dep_selected: &mut usize,
    dep_tree_expanded: &HashSet<String>,
    deps_error: &mut Option<String>,
    file_info: &mut Vec<PackageFileInfo>,
    file_selected: &mut usize,
    file_tree_expanded: &HashSet<String>,
    files_error: &mut Option<String>,
    service_info: &mut Vec<ServiceImpact>,
    service_selected: &mut usize,
    services_loaded: &mut bool,
    services_error: &mut Option<String>,
    sandbox_info: &mut Vec<crate::logic::sandbox::SandboxInfo>,
    sandbox_selected: &mut usize,
    sandbox_tree_expanded: &HashSet<String>,
    sandbox_loaded: &mut bool,
    sandbox_error: &mut Option<String>,
    selected_optdepends: &mut std::collections::HashMap<String, std::collections::HashSet<String>>,
    cascade_mode: CascadeMode,
) {
    let render_start = std::time::Instant::now();
    let th = theme();
    tracing::info!(
        "[UI] render_preflight START: tab={:?}, items={}, deps={}, files={}, services={}, sandbox={}",
        tab,
        items.len(),
        dependency_info.len(),
        file_info.len(),
        service_info.len(),
        sandbox_info.len()
    );
    // Load dependencies from cache - SIMPLIFIED: Always load when on Deps tab or when empty
    // Auto-resolve if cache is empty and we're on Deps tab
    if matches!(*action, PreflightAction::Install) {
        let should_load = dependency_info.is_empty() || matches!(*tab, PreflightTab::Deps);

        if should_load && matches!(*tab, PreflightTab::Deps) {
            if !app.install_list_deps.is_empty() {
                // Get set of current package names for filtering
                let item_names: std::collections::HashSet<String> =
                    items.iter().map(|i| i.name.clone()).collect();

                // Filter to only show dependencies required by current items
                let filtered: Vec<DependencyInfo> = app
                    .install_list_deps
                    .iter()
                    .filter(|dep| {
                        // Show dependency if any current item requires it
                        dep.required_by
                            .iter()
                            .any(|req_by| item_names.contains(req_by))
                    })
                    .cloned()
                    .collect();

                tracing::debug!(
                    "[UI] Deps tab: cache={}, filtered={}, items={:?}, resolving={}, current={}",
                    app.install_list_deps.len(),
                    filtered.len(),
                    item_names,
                    app.deps_resolving,
                    dependency_info.len()
                );

                // Always update when on Deps tab, but only reset selection if dependencies were empty (first load)
                // Don't reset on every render - that would break navigation
                let was_empty = dependency_info.is_empty();
                if !filtered.is_empty() || dependency_info.is_empty() {
                    *dependency_info = filtered;
                    // Only reset selection if this is the first load (was empty), not on every render
                    if was_empty {
                        *dep_selected = 0;
                    }
                }
            } else if dependency_info.is_empty() {
                // Check if background resolution is in progress
                if app.preflight_deps_resolving || app.deps_resolving {
                    // Background resolution in progress - UI will show loading state
                    tracing::debug!(
                        "[UI] Deps tab: background resolution in progress, items={:?}",
                        items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                } else {
                    // Cache is empty and no resolution in progress - trigger background resolution
                    // This will be handled by the event handler when switching to Deps tab
                    tracing::debug!(
                        "[UI] Deps tab: cache is empty, will auto-resolve, items={:?}",
                        items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                }
            }
        }
    }
    // Use cached file info if available
    // Note: Cached files are populated in background when packages are added to install list
    // Note: File resolution is triggered asynchronously in event handlers, not during rendering
    if matches!(*tab, PreflightTab::Files) {
        // Check if we have cached files from app state that match the current items
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_files: Vec<PackageFileInfo> = app
            .install_list_files
            .iter()
            .filter(|file_info| item_names.contains(&file_info.name))
            .cloned()
            .collect();
        // Sync results from background resolution if available
        if !cached_files.is_empty()
            && (file_info.is_empty() || cached_files.len() != file_info.len())
        {
            tracing::debug!(
                "[UI] Syncing {} file infos from background resolution to Preflight modal",
                cached_files.len()
            );
            *file_info = cached_files;
            if *file_selected >= file_info.len() {
                *file_selected = 0;
            }
        } else if file_info.is_empty() {
            // Check if background resolution is in progress
            if app.preflight_files_resolving || app.files_resolving {
                // Background resolution in progress - UI will show loading state
                tracing::debug!(
                    "[UI] Files tab: background resolution in progress, items={:?}",
                    items.iter().map(|i| &i.name).collect::<Vec<_>>()
                );
            }
            // If no cached files available, resolution will be triggered by event handlers when user navigates to Files tab
        }
    }
    // Use cached service info if available
    // Note: Cached services are pre-populated when modal opens, so this only runs if cache was empty
    // Sync services from background resolution if available (similar to sandbox)
    // Always sync when services are loaded and modal is open, regardless of current tab
    if matches!(*action, PreflightAction::Install)
        && !app.services_resolving
        && !app.preflight_services_resolving
    {
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_services: Vec<_> = app
            .install_list_services
            .iter()
            .filter(|s| s.providers.iter().any(|p| item_names.contains(p)))
            .cloned()
            .collect();

        // Sync results from background resolution if available
        if !cached_services.is_empty() {
            let needs_update = service_info.is_empty()
                || cached_services.len() != service_info.len()
                || cached_services.iter().any(|cached| {
                    !service_info
                        .iter()
                        .any(|existing| existing.unit_name == cached.unit_name)
                });
            if needs_update {
                tracing::debug!(
                    "[UI] Syncing {} services from background resolution to Preflight modal",
                    cached_services.len()
                );
                *service_info = cached_services;
                *services_loaded = true;
            }
        } else if service_info.is_empty() && !*services_loaded {
            // Check if cache file exists with matching signature (even if empty)
            let cache_check_start = std::time::Instant::now();
            let cache_exists = if !items.is_empty() {
                let signature = crate::app::services_cache::compute_signature(items);
                let result =
                    crate::app::services_cache::load_cache(&app.services_cache_path, &signature)
                        .is_some();
                let cache_duration = cache_check_start.elapsed();
                if cache_duration.as_millis() > 10 {
                    tracing::warn!(
                        "[UI] Services cache check took {:?} (slow!)",
                        cache_duration
                    );
                }
                result
            } else {
                false
            };

            if cache_exists {
                // Cache exists but is empty - this is valid, means no services found
                tracing::debug!("[UI] Using cached service impacts (empty - no services found)");
                *services_loaded = true;
            }
        }
    }
    if !service_info.is_empty() && *service_selected >= service_info.len() {
        *service_selected = service_info.len().saturating_sub(1);
    }

    // Sync sandbox info if on Sandbox tab
    if matches!(*action, PreflightAction::Install) && matches!(*tab, PreflightTab::Sandbox) {
        // Show all packages, but only analyze AUR packages
        let aur_items: Vec<_> = items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .collect();

        // Check if we have cached sandbox info from app state that matches current items
        let item_names: std::collections::HashSet<String> =
            items.iter().map(|i| i.name.clone()).collect();
        let cached_sandbox: Vec<_> = app
            .install_list_sandbox
            .iter()
            .filter(|s| item_names.contains(&s.package_name))
            .cloned()
            .collect();
        // Sync results from background resolution if available (always sync when on Sandbox tab)
        // Always sync cached data to sandbox_info when available
        if !cached_sandbox.is_empty() {
            // Always update if sandbox_info is empty, or if content differs
            let needs_update = sandbox_info.is_empty()
                || cached_sandbox.len() != sandbox_info.len()
                || cached_sandbox.iter().any(|cached| {
                    !sandbox_info
                        .iter()
                        .any(|existing| existing.package_name == cached.package_name)
                });
            if needs_update {
                tracing::debug!(
                    "[UI] Syncing {} sandbox info entries from background resolution to Preflight modal",
                    cached_sandbox.len()
                );
                *sandbox_info = cached_sandbox;
                *sandbox_loaded = true;
            }
        }
        // If sandbox_info is empty and we haven't loaded yet, check cache or trigger resolution
        if sandbox_info.is_empty()
            && !*sandbox_loaded
            && !app.preflight_sandbox_resolving
            && !app.sandbox_resolving
        {
            // Check if cache file exists with matching signature (even if empty)
            let sandbox_cache_start = std::time::Instant::now();
            let signature = crate::app::sandbox_cache::compute_signature(items);
            let sandbox_cache_exists =
                crate::app::sandbox_cache::load_cache(&app.sandbox_cache_path, &signature)
                    .is_some();
            let sandbox_cache_duration = sandbox_cache_start.elapsed();
            if sandbox_cache_duration.as_millis() > 10 {
                tracing::warn!(
                    "[UI] Sandbox cache check took {:?} (slow!)",
                    sandbox_cache_duration
                );
            }
            if sandbox_cache_exists {
                // Cache exists but is empty - this is valid, means no sandbox info found
                // But don't mark as loaded if resolution is still in progress
                if !app.preflight_sandbox_resolving && !app.sandbox_resolving {
                    tracing::debug!(
                        "[UI] Using cached sandbox info (empty - no sandbox info found)"
                    );
                    *sandbox_loaded = true;
                }
            } else if aur_items.is_empty() {
                // No AUR packages, mark as loaded
                *sandbox_loaded = true;
            } else {
                // Check if background resolution is in progress
                if app.preflight_sandbox_resolving || app.sandbox_resolving {
                    // Background resolution in progress - UI will show loading state
                    tracing::debug!(
                        "[UI] Sandbox tab: background resolution in progress, items={:?}",
                        items.iter().map(|i| &i.name).collect::<Vec<_>>()
                    );
                    // Don't mark as loaded - keep showing loading state
                }
                // If no cached sandbox info available, resolution will be triggered by event handlers when user navigates to Sandbox tab
                // Don't mark as loaded yet - wait for resolution to complete
            }
        }
        // Also check if we have sandbox_info already populated (from previous sync or initial load)
        // This ensures we show data even if cached_sandbox is empty but sandbox_info has data
        // But don't mark as loaded if resolution is still in progress
        if !sandbox_info.is_empty()
            && !*sandbox_loaded
            && !app.preflight_sandbox_resolving
            && !app.sandbox_resolving
        {
            *sandbox_loaded = true;
        }
    } else if matches!(*action, PreflightAction::Remove) {
        // For remove actions, no sandbox analysis needed
        let aur_items: Vec<_> = items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .collect();
        if aur_items.is_empty() {
            // No AUR packages, mark as loaded
            *sandbox_loaded = true;
        }
    }

    // Calculate modal size and position
    let w = area.width.saturating_sub(6).min(96);
    let h = area.height.saturating_sub(8).min(22);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    // Split rect into content area and keybinds pane (reserve 4 lines for keybinds to account for borders)
    // With double borders, we need: 1 top border + 2 content lines + 1 bottom border = 4 lines minimum
    let keybinds_height = 4;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(keybinds_height)])
        .split(rect);
    let content_rect = layout[0];
    let keybinds_rect = layout[1];

    let title = match action {
        PreflightAction::Install => i18n::t(app, "app.modals.preflight.title_install"),
        PreflightAction::Remove => i18n::t(app, "app.modals.preflight.title_remove"),
    };
    let border_color = th.lavender;
    let bg_color = th.crust;

    // Render tab header
    let (header_chips_line, tab_header_line) = render_tab_header(
        app,
        content_rect,
        tab,
        header_chips,
        summary,
        dependency_info,
        file_info,
        service_info,
        *services_loaded,
        sandbox_info,
        *sandbox_loaded,
    );

    let mut lines: Vec<Line<'static>> = Vec::new();
    // Header chips line
    lines.push(header_chips_line);
    // Tab header line with progress indicators
    lines.push(tab_header_line);
    lines.push(Line::from(""));

    // Render tab content
    match tab {
        PreflightTab::Summary => {
            let tab_lines = render_summary_tab(
                app,
                items,
                action,
                summary,
                header_chips,
                dependency_info,
                cascade_mode,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Deps => {
            let tab_lines = render_deps_tab(
                app,
                items,
                action,
                dependency_info,
                dep_selected,
                dep_tree_expanded,
                deps_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Files => {
            let tab_lines = render_files_tab(
                app,
                items,
                file_info,
                file_selected,
                file_tree_expanded,
                files_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Services => {
            let tab_lines = render_services_tab(
                app,
                service_info,
                service_selected,
                *services_loaded,
                services_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Sandbox => {
            let tab_lines = render_sandbox_tab(
                app,
                items,
                sandbox_info,
                sandbox_selected,
                sandbox_tree_expanded,
                *sandbox_loaded,
                sandbox_error,
                selected_optdepends,
                content_rect,
            );
            lines.extend(tab_lines);
        }
    }

    // Render content area (no bottom border - keybinds pane will have top border)
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(ratatui::text::Span::styled(
                    title,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(boxw, content_rect);

    // Render footer
    render_footer(
        f,
        app,
        items,
        action,
        tab,
        content_rect,
        keybinds_rect,
        bg_color,
        border_color,
    );

    let render_duration = render_start.elapsed();
    if render_duration.as_millis() > 50 {
        tracing::warn!("[UI] render_preflight took {:?} (slow!)", render_duration);
    } else {
        tracing::debug!("[UI] render_preflight completed in {:?}", render_duration);
    }
}
