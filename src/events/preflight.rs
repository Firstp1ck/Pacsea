//! Preflight modal event handling.

use crossterm::event::{KeyCode, KeyEvent};
use std::collections::{HashMap, HashSet};

use crate::state::{AppState, PackageItem};

/// Compute the length of the display_items list for the Deps tab.
///   This matches the logic in ui/modals.rs that builds display_items with headers and dependencies.
///   Accounts for folded groups (only counts dependencies if package is expanded).
pub(crate) fn compute_display_items_len(
    items: &[PackageItem],
    dependency_info: &[crate::state::modal::DependencyInfo],
    dep_tree_expanded: &std::collections::HashSet<String>,
) -> usize {
    // Group dependencies by the packages that require them (same as UI code)
    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> = HashMap::new();
    for dep in dependency_info.iter() {
        for req_by in &dep.required_by {
            grouped.entry(req_by.clone()).or_default().push(dep);
        }
    }

    // Count display items: 1 header + unique deps per package (only if expanded)
    let mut count = 0;
    for pkg_name in items.iter().map(|p| &p.name) {
        if let Some(pkg_deps) = grouped.get(pkg_name) {
            count += 1; // Header
            // Count unique dependencies only if package is expanded
            if dep_tree_expanded.contains(pkg_name) {
                let mut seen_deps = HashSet::new();
                for dep in pkg_deps.iter() {
                    if seen_deps.insert(dep.name.as_str()) {
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

/// Handle key events for the Preflight modal.
pub(crate) fn handle_preflight_key(ke: KeyEvent, app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight {
        tab,
        items,
        action,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        file_info,
        file_selected,
        file_tree_expanded,
    } = &mut app.modal
    {
        match ke.code {
            KeyCode::Esc => {
                app.previous_modal = None; // Clear previous modal when closing Preflight
                app.modal = crate::state::Modal::None;
            }
            KeyCode::Enter => {
                // In Deps tab, Enter toggles expand/collapse; in Files tab, Enter toggles expand/collapse; otherwise closes modal
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    // Find which package header is selected
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_name in items.iter().map(|p| &p.name) {
                        if grouped.contains_key(pkg_name) {
                            display_items.push((true, pkg_name.clone()));
                            if dep_tree_expanded.contains(pkg_name) {
                                // Add placeholder entries for dependencies (we just need to count them)
                                let mut seen_deps = HashSet::new();
                                if let Some(pkg_deps) = grouped.get(pkg_name) {
                                    for dep in pkg_deps.iter() {
                                        if seen_deps.insert(dep.name.as_str()) {
                                            display_items.push((false, String::new()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*dep_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    // Build display items list to find which package header is selected
                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_info in file_info.iter() {
                        if !pkg_info.files.is_empty() {
                            display_items.push((true, pkg_info.name.clone()));
                            if file_tree_expanded.contains(&pkg_info.name) {
                                // Add placeholder entries for files (we just need to count them)
                                for _file in pkg_info.files.iter() {
                                    display_items.push((false, String::new()));
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*file_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else {
                    // Close modal on Enter when not in Deps/Files tab or no data
                    app.previous_modal = None;
                    app.modal = crate::state::Modal::None;
                }
            }
            KeyCode::Left => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Summary,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Services,
                };
                // Resolve dependencies when switching to Deps tab
                if *tab == crate::state::PreflightTab::Deps
                    && dependency_info.is_empty()
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    *dependency_info = crate::logic::deps::resolve_dependencies(items);
                    *dep_selected = 0;
                }
                // Resolve files when switching to Files tab (only if not cached)
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    tracing::info!(
                        "[Events] File cache empty, resolving files when switching to Files tab (Left)"
                    );
                    *file_info = crate::logic::files::resolve_file_changes(items, *action);
                    *file_selected = 0;
                }
            }
            KeyCode::Right => {
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                // Resolve dependencies when switching to Deps tab
                if *tab == crate::state::PreflightTab::Deps
                    && dependency_info.is_empty()
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    *dependency_info = crate::logic::deps::resolve_dependencies(items);
                    *dep_selected = 0;
                }
                // Resolve files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    tracing::info!(
                        "[Events] Auto-refreshing files when switching to Files tab (Right)"
                    );
                    *file_info = crate::logic::files::resolve_file_changes(items, *action);
                    *file_selected = 0;
                }
            }
            KeyCode::Tab => {
                // Cycle forward through tabs (same as Right)
                *tab = match tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                };
                // Resolve dependencies when switching to Deps tab
                if *tab == crate::state::PreflightTab::Deps
                    && dependency_info.is_empty()
                    && matches!(*action, crate::state::PreflightAction::Install)
                {
                    *dependency_info = crate::logic::deps::resolve_dependencies(items);
                    *dep_selected = 0;
                }
                // Resolve files when switching to Files tab
                if *tab == crate::state::PreflightTab::Files && file_info.is_empty() {
                    tracing::info!(
                        "[Events] Auto-refreshing files when switching to Files tab (Tab)"
                    );
                    *file_info = crate::logic::files::resolve_file_changes(items, *action);
                    *file_selected = 0;
                }
            }
            KeyCode::Up => {
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    if *dep_selected > 0 {
                        *dep_selected -= 1;
                    }
                } else if *tab == crate::state::PreflightTab::Files
                    && !file_info.is_empty()
                    && *file_selected > 0
                {
                    *file_selected -= 1;
                }
            }
            KeyCode::Down => {
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    let display_len =
                        compute_display_items_len(items, dependency_info, dep_tree_expanded);
                    if *dep_selected < display_len.saturating_sub(1) {
                        *dep_selected += 1;
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    // Compute total display items length for Files tab (accounting for collapsed packages)
                    let mut display_len = 0;
                    for pkg_info in file_info.iter() {
                        if !pkg_info.files.is_empty() {
                            display_len += 1; // Package header
                            if file_tree_expanded.contains(&pkg_info.name) {
                                display_len += pkg_info.files.len(); // Files only if expanded
                            }
                        }
                    }
                    if *file_selected < display_len.saturating_sub(1) {
                        *file_selected += 1;
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle expand/collapse for selected package group (Space key)
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    // Find which package header is selected
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_name in items.iter().map(|p| &p.name) {
                        if grouped.contains_key(pkg_name) {
                            display_items.push((true, pkg_name.clone()));
                            if dep_tree_expanded.contains(pkg_name) {
                                // Add placeholder entries for dependencies (we just need to count them)
                                let mut seen_deps = HashSet::new();
                                if let Some(pkg_deps) = grouped.get(pkg_name) {
                                    for dep in pkg_deps.iter() {
                                        if seen_deps.insert(dep.name.as_str()) {
                                            display_items.push((false, String::new()));
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*dep_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if dep_tree_expanded.contains(pkg_name) {
                            dep_tree_expanded.remove(pkg_name);
                        } else {
                            dep_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    // Build display items list to find which package header is selected
                    let mut display_items: Vec<(bool, String)> = Vec::new();
                    for pkg_info in file_info.iter() {
                        if !pkg_info.files.is_empty() {
                            display_items.push((true, pkg_info.name.clone()));
                            if file_tree_expanded.contains(&pkg_info.name) {
                                // Add placeholder entries for files (we just need to count them)
                                for _file in pkg_info.files.iter() {
                                    display_items.push((false, String::new()));
                                }
                            }
                        }
                    }

                    if let Some((is_header, pkg_name)) = display_items.get(*file_selected)
                        && *is_header
                    {
                        // Toggle this package's expanded state
                        if file_tree_expanded.contains(pkg_name) {
                            file_tree_expanded.remove(pkg_name);
                        } else {
                            file_tree_expanded.insert(pkg_name.clone());
                        }
                    }
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                // Expand/collapse all package groups
                if *tab == crate::state::PreflightTab::Deps && !dependency_info.is_empty() {
                    let mut grouped: HashMap<String, Vec<&crate::state::modal::DependencyInfo>> =
                        HashMap::new();
                    for dep in dependency_info.iter() {
                        for req_by in &dep.required_by {
                            grouped.entry(req_by.clone()).or_default().push(dep);
                        }
                    }

                    let all_expanded = items.iter().all(|p| dep_tree_expanded.contains(&p.name));
                    if all_expanded {
                        // Collapse all
                        dep_tree_expanded.clear();
                    } else {
                        // Expand all
                        for pkg_name in items.iter().map(|p| &p.name) {
                            if grouped.contains_key(pkg_name) {
                                dep_tree_expanded.insert(pkg_name.clone());
                            }
                        }
                    }
                } else if *tab == crate::state::PreflightTab::Files && !file_info.is_empty() {
                    // Expand/collapse all packages in Files tab
                    let all_expanded = file_info
                        .iter()
                        .filter(|p| !p.files.is_empty())
                        .all(|p| file_tree_expanded.contains(&p.name));
                    if all_expanded {
                        // Collapse all
                        file_tree_expanded.clear();
                    } else {
                        // Expand all
                        for pkg_info in file_info.iter() {
                            if !pkg_info.files.is_empty() {
                                file_tree_expanded.insert(pkg_info.name.clone());
                            }
                        }
                    }
                }
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                // File database sync (Files tab only)
                if *tab == crate::state::PreflightTab::Files {
                    // Launch terminal with sudo pacman -Fy
                    let sync_cmd = "sudo pacman -Fy".to_string();
                    let cmds = vec![sync_cmd];
                    std::thread::spawn(move || {
                        crate::install::spawn_shell_commands_in_terminal(&cmds);
                    });
                    // Clear file_info to trigger re-resolution after sync completes
                    *file_info = Vec::new();
                    *file_selected = 0;
                    app.toast_message = Some(
                        "File database sync started. Switch away and back to Files tab to refresh."
                            .to_string(),
                    );
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
                    return false;
                }
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                // Build AUR package name list to scan
                let mut names: Vec<String> = Vec::new();
                for it in items.iter() {
                    if matches!(it.source, crate::state::Source::Aur) {
                        names.push(it.name.clone());
                    }
                }
                if names.is_empty() {
                    app.modal = crate::state::Modal::Alert {
                        message: "No AUR packages selected to scan.\nAdd AUR packages to scan, then press 's'.".into(),
                    };
                } else {
                    app.pending_install_names = Some(names);
                    // Open Scan Configuration modal initialized from settings.conf
                    let prefs = crate::theme::settings();
                    // Store current Preflight modal state before opening ScanConfig
                    app.previous_modal = Some(app.modal.clone());
                    app.modal = crate::state::Modal::ScanConfig {
                        do_clamav: prefs.scan_do_clamav,
                        do_trivy: prefs.scan_do_trivy,
                        do_semgrep: prefs.scan_do_semgrep,
                        do_shellcheck: prefs.scan_do_shellcheck,
                        do_virustotal: prefs.scan_do_virustotal,
                        do_custom: prefs.scan_do_custom,
                        do_sleuth: prefs.scan_do_sleuth,
                        cursor: 0,
                    };
                }
            }
            KeyCode::Char('d') => {
                // toggle dry-run globally pre-apply
                app.dry_run = !app.dry_run;
                app.toast_message = Some(format!(
                    "Dry-run: {}",
                    if app.dry_run { "ON" } else { "OFF" }
                ));
            }
            KeyCode::Char('p') => {
                // Directly trigger installation/removal and close Preflight modal
                match action {
                    crate::state::PreflightAction::Install => {
                        crate::install::spawn_install_all(items, app.dry_run);
                    }
                    crate::state::PreflightAction::Remove => {
                        let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
                        crate::install::spawn_remove_all(&names, app.dry_run);
                    }
                }
                app.previous_modal = None;
                app.modal = crate::state::Modal::None;
            }
            KeyCode::Char('c') => {
                // Snapshot placeholder
                app.toast_message = Some("Snapshot (placeholder)".to_string());
            }
            KeyCode::Char('q') => {
                app.previous_modal = None; // Clear previous modal when closing Preflight
                app.modal = crate::state::Modal::None;
            }
            KeyCode::Char('?') => {
                // Show Deps tab help when on Deps tab, otherwise show general Preflight help
                let help_message = if *tab == crate::state::PreflightTab::Deps {
                    "Dependencies Tab Help\n\n\
                        This tab shows all dependencies required for the selected packages.\n\n\
                        Status Indicators:\n\
                        • ✓ (green) - Already installed\n\
                        • + (yellow) - Needs to be installed\n\
                        • ↑ (yellow) - Needs upgrade\n\
                        • ⚠ (red) - Conflict detected\n\
                        • ? (red) - Missing/unavailable\n\n\
                        Repository Badges:\n\
                        • [core] - Core repository (fundamental system packages)\n\
                        • [extra] - Extra repository\n\
                        • [AUR] - Arch User Repository\n\n\
                        Markers:\n\
                        • [CORE] (red) - Package from core repository\n\
                        • [SYSTEM] (yellow) - Critical system package\n\n\
                        Navigation:\n\
                        • Up/Down - Navigate dependency list\n\
                        • Left/Right - Switch tabs\n\
                        • ? - Show this help\n\
                        • q/Esc - Close preflight\n\n\
                        Dependencies are automatically resolved when you navigate to this tab.\n\
                        For AUR packages, dependencies are fetched from the AUR API."
                        .to_string()
                } else {
                    "Preflight Help\n\n\
                        Navigation:\n\
                        • Left/Right - Switch between tabs\n\
                        • Up/Down - Navigate lists (Deps tab)\n\
                        • ? - Show help for current tab\n\
                        • q/Esc/Enter - Close preflight\n\n\
                        Actions:\n\
                        • s - Scan AUR packages (if AUR packages selected)\n\
                        • d - Toggle dry-run mode\n\
                        • p - Proceed with installation/removal\n\
                        • c - Create snapshot (placeholder)\n\n\
                        Tabs:\n\
                        • Summary - Overview of packages\n\
                        • Deps - Dependency information\n\
                        • Files - File changes preview\n\
                        • Services - Systemd service impact\n\
                        • Sandbox - AUR build checks"
                        .to_string()
                };
                // Store current Preflight modal state before opening Alert
                app.previous_modal = Some(app.modal.clone());
                app.modal = crate::state::Modal::Alert {
                    message: help_message,
                };
            }
            _ => {}
        }
        return false;
    }
    false
}
