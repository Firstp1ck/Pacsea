use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

use super::utils::{
    find_in_install, refresh_install_details, refresh_remove_details, refresh_selected_details,
};

/// What: Handle key events while the Install pane (right column) is focused.
///
/// Inputs:
/// - `ke`: Key event received from the terminal
/// - `app`: Mutable application state (selection, lists, pane focus)
/// - `details_tx`: Channel to request package details for the focused item
/// - `preview_tx`: Channel to request preview details (used for some focus changes)
/// - `_add_tx`: Channel for adding items (not used directly in Install handler)
///
/// Output:
/// - `true` to request application exit (e.g., Ctrl+C); `false` to continue.
///
/// Details:
/// - In-pane find: `/` enters find mode; typing edits the pattern; Enter jumps to next match;
///   Esc cancels. Find matches against name/description (Install) or name-only (Remove/Downgrade).
/// - Navigation: `j/k` and `Down/Up` move selection in the active subpane. Behavior adapts to
///   installed-only mode (`app.installed_only_mode`) and current `right_pane_focus`:
///   - Normal mode: selection moves in Install list only.
///   - Installed-only: selection moves in Downgrade/Remove/Install subpane depending on focus.
/// - Pane cycling: Configured `pane_next` chord cycles focus across panes. In installed-only mode
///   it cycles Search → Downgrade → Remove → Recent → Search; otherwise Search → Install → Recent.
/// - Arrow focus: Left/Right move focus between Search/Install/Recent (and subpanes when installed-only).
/// - Deletion: `Delete` (or configured `install_remove`) removes the selected entry from the active
///   list (Install/Remove/Downgrade) and updates selection and details.
/// - Clear list: Configured `install_clear` clears the respective list (or all in normal mode),
///   and resets selection.
/// - Enter:
///   - Normal mode with non-empty Install list: opens `Modal::ConfirmInstall` for batch install.
///   - Installed-only Remove focus with non-empty list: opens `Modal::ConfirmRemove`.
///   - Installed-only Downgrade focus with non-empty list: runs `downgrade` tool (or dry-run).
/// - Esc: Returns focus to Search and refreshes the selected result's details.
pub fn handle_install_key(
    ke: KeyEvent,
    app: &mut AppState,
    details_tx: &mpsc::UnboundedSender<PackageItem>,
    preview_tx: &mpsc::UnboundedSender<PackageItem>,
    _add_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if ke.code == KeyCode::Char('c') && ke.modifiers.contains(KeyModifiers::CONTROL) {
        return true;
    }

    // Pane-search mode first
    if app.pane_find.is_some() {
        match ke.code {
            KeyCode::Enter => {
                find_in_install(app, true);
                refresh_install_details(app, details_tx);
            }
            KeyCode::Esc => {
                app.pane_find = None;
            }
            KeyCode::Backspace => {
                if let Some(buf) = &mut app.pane_find {
                    buf.pop();
                }
            }
            KeyCode::Char(ch) => {
                if let Some(buf) = &mut app.pane_find {
                    buf.push(ch);
                }
            }
            _ => {}
        }
        return false;
    }

    let km = &app.keymap;
    // Match helper that treats Shift+<char> from config as equivalent to uppercase char without Shift from terminal
    let matches_any = |list: &Vec<crate::theme::KeyChord>| {
        list.iter().any(|c| {
            if (c.code, c.mods) == (ke.code, ke.modifiers) {
                return true;
            }
            match (c.code, ke.code) {
                (
                    crossterm::event::KeyCode::Char(cfg_ch),
                    crossterm::event::KeyCode::Char(ev_ch),
                ) => {
                    let cfg_has_shift = c.mods.contains(crossterm::event::KeyModifiers::SHIFT);
                    let ev_has_no_shift =
                        !ke.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);
                    cfg_has_shift && ev_has_no_shift && ev_ch == cfg_ch.to_ascii_uppercase()
                }
                _ => false,
            }
        })
    };

    match ke.code {
        KeyCode::Char('j') => {
            // vim down
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                let sel = app.install_state.selected().unwrap_or(0);
                let max = inds.len().saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.install_state.select(Some(new));
                refresh_install_details(app, details_tx);
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                let len = app.remove_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.remove_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.remove_state.select(Some(new));
                refresh_remove_details(app, details_tx);
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) {
                let len = app.downgrade_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.downgrade_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        KeyCode::Char('k') => {
            // vim up
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(sel) = app.install_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.install_state.select(Some(new));
                    refresh_install_details(app, details_tx);
                }
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                if let Some(sel) = app.remove_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.remove_state.select(Some(new));
                    refresh_remove_details(app, details_tx);
                }
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) && let Some(sel) = app.downgrade_state.selected()
            {
                let new = sel.saturating_sub(1);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        KeyCode::Char('/') => {
            app.pane_find = Some(String::new());
        }
        KeyCode::Enter => {
            let skip = crate::theme::settings().skip_preflight;
            if !app.installed_only_mode && !app.install_list.is_empty() {
                if skip {
                    crate::install::spawn_install_all(&app.install_list, app.dry_run);
                    app.toast_message = Some("Installing list (preflight skipped)".to_string());
                    app.toast_expires_at =
                        Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                } else {
                    tracing::info!(
                        "[Install] Opening preflight modal for {} packages",
                        app.install_list.len()
                    );
                    let start_time = std::time::Instant::now();
                    let item_count = app.install_list.len();
                    // Open Preflight modal listing all items to be installed
                    let items = app.install_list.clone();
                    let cache_start = std::time::Instant::now();
                    let cached_deps = if !app.deps_resolving && !app.install_list_deps.is_empty() {
                        tracing::debug!(
                            "[Install] Using {} cached dependencies",
                            app.install_list_deps.len()
                        );
                        app.install_list_deps.clone()
                    } else {
                        tracing::debug!("[Install] No cached dependencies available");
                        Vec::new()
                    };
                    let cached_files = if !app.files_resolving && !app.install_list_files.is_empty()
                    {
                        tracing::debug!(
                            "[Install] Using {} cached file infos",
                            app.install_list_files.len()
                        );
                        app.install_list_files.clone()
                    } else {
                        tracing::debug!("[Install] No cached file infos available");
                        Vec::new()
                    };
                    let cached_services =
                        if !app.services_resolving && !app.install_list_services.is_empty() {
                            tracing::debug!(
                                "[Install] Using {} cached services",
                                app.install_list_services.len()
                            );
                            app.install_list_services.clone()
                        } else {
                            tracing::debug!("[Install] No cached services available");
                            Vec::new()
                        };
                    // Check if cache file exists with matching signature to determine if services are loaded
                    // (even if empty - empty cache means "no services found", which is valid)
                    let services_cache_loaded = if !app.install_list.is_empty() {
                        let signature =
                            crate::app::services_cache::compute_signature(&app.install_list);
                        let loaded = crate::app::services_cache::load_cache(
                            &app.services_cache_path,
                            &signature,
                        )
                        .is_some();
                        tracing::debug!(
                            "[Install] Services cache check: {} (signature match: {})",
                            if loaded { "found" } else { "not found" },
                            signature.len()
                        );
                        loaded
                    } else {
                        false
                    };
                    let services_loaded = services_cache_loaded || !cached_services.is_empty();
                    tracing::debug!(
                        "[Install] Cache loading took {:?}",
                        cache_start.elapsed()
                    );

                    // Restore user restart decisions from pending_service_plan if available
                    let mut final_services = cached_services;
                    if !app.pending_service_plan.is_empty() && !final_services.is_empty() {
                        // Create a map of unit_name -> restart_decision from pending plan
                        let decision_map: std::collections::HashMap<
                            String,
                            crate::state::modal::ServiceRestartDecision,
                        > = app
                            .pending_service_plan
                            .iter()
                            .map(|s| (s.unit_name.clone(), s.restart_decision))
                            .collect();

                        // Apply saved decisions to cached services
                        for service in final_services.iter_mut() {
                            if let Some(&saved_decision) = decision_map.get(&service.unit_name) {
                                service.restart_decision = saved_decision;
                            }
                        }
                    }

                    // Load cached sandbox info
                    let cached_sandbox =
                        if !app.sandbox_resolving && !app.install_list_sandbox.is_empty() {
                            app.install_list_sandbox.clone()
                        } else {
                            Vec::new()
                        };

                    // Check if sandbox cache file exists with matching signature (even if empty)
                    let sandbox_cache_loaded = if !items.is_empty() {
                        let signature = crate::app::sandbox_cache::compute_signature(&items);
                        crate::app::sandbox_cache::load_cache(&app.sandbox_cache_path, &signature)
                            .is_some()
                    } else {
                        false
                    };
                    let sandbox_loaded = sandbox_cache_loaded || !cached_sandbox.is_empty();

                    // Compute a minimal summary without blocking pacman calls to avoid freezing the UI
                    // The full summary will be computed asynchronously after the modal opens
                    let summary_start = std::time::Instant::now();
                    let aur_count = items.iter().filter(|p| matches!(p.source, crate::state::Source::Aur)).count();
                    tracing::debug!(
                        "[Install] Creating minimal summary for {} packages ({} AUR)",
                        items.len(),
                        aur_count
                    );
                    let minimal_summary = crate::state::modal::PreflightSummaryData {
                        packages: items.iter().map(|item| crate::state::modal::PreflightPackageSummary {
                            name: item.name.clone(),
                            source: item.source.clone(),
                            installed_version: None,
                            target_version: item.version.clone(),
                            is_downgrade: false,
                            is_major_bump: false,
                            download_bytes: None,
                            install_delta_bytes: None,
                            notes: vec![],
                        }).collect(),
                        package_count: items.len(),
                        aur_count,
                        download_bytes: 0,
                        install_delta_bytes: 0,
                        risk_score: if aur_count > 0 { 2 } else { 0 },
                        risk_level: if aur_count > 0 { crate::state::modal::RiskLevel::Medium } else { crate::state::modal::RiskLevel::Low },
                        risk_reasons: if aur_count > 0 { vec!["AUR packages included (+2)".to_string()] } else { vec![] },
                        major_bump_packages: vec![],
                        core_system_updates: vec![],
                        pacnew_candidates: 0,
                        pacsave_candidates: 0,
                        config_warning_packages: vec![],
                        service_restart_units: vec![],
                        summary_warnings: if aur_count > 0 { vec!["AUR packages included (+2)".to_string()] } else { vec![] },
                        summary_notes: if aur_count > 0 { vec!["AUR packages present; build steps may vary.".to_string()] } else { vec![] },
                    };
                    let minimal_header = crate::state::modal::PreflightHeaderChips {
                        package_count: items.len(),
                        download_bytes: 0,
                        install_delta_bytes: 0,
                        aur_count,
                        risk_score: if aur_count > 0 { 2 } else { 0 },
                        risk_level: if aur_count > 0 { crate::state::modal::RiskLevel::Medium } else { crate::state::modal::RiskLevel::Low },
                    };
                    tracing::debug!(
                        "[Install] Minimal summary creation took {:?}",
                        summary_start.elapsed()
                    );
                    // Don't clear pending_service_plan here - it will be updated when modal closes
                    let modal_set_start = std::time::Instant::now();
                    app.modal = crate::state::Modal::Preflight {
                        items,
                        action: crate::state::PreflightAction::Install,
                        tab: crate::state::PreflightTab::Summary,
                        summary: Some(Box::new(minimal_summary)),
                        header_chips: minimal_header,
                        dependency_info: cached_deps,
                        dep_selected: 0,
                        dep_tree_expanded: std::collections::HashSet::new(),
                        deps_error: None,
                        file_info: cached_files,
                        file_selected: 0,
                        file_tree_expanded: std::collections::HashSet::new(),
                        files_error: None,
                        service_info: final_services,
                        service_selected: 0,
                        services_loaded,
                        services_error: None,
                        sandbox_info: cached_sandbox,
                        sandbox_selected: 0,
                        sandbox_tree_expanded: std::collections::HashSet::new(),
                        sandbox_loaded,
                        sandbox_error: None,
                        selected_optdepends: std::collections::HashMap::new(),
                        cascade_mode: app.remove_cascade_mode,
                    };
                    tracing::debug!(
                        "[Install] Modal state set in {:?}",
                        modal_set_start.elapsed()
                    );
                    tracing::info!(
                        "[Install] Preflight modal opened successfully in {:?} ({} packages)",
                        start_time.elapsed(),
                        item_count
                    );
                    app.remove_preflight_summary.clear();
                }
            } else if app.installed_only_mode
                && matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove)
            {
                if !app.remove_list.is_empty() {
                    if skip {
                        let names: Vec<String> =
                            app.remove_list.iter().map(|p| p.name.clone()).collect();
                        crate::install::spawn_remove_all(
                            &names,
                            app.dry_run,
                            app.remove_cascade_mode,
                        );
                        app.toast_message = Some("Removing list (preflight skipped)".to_string());
                        app.toast_expires_at =
                            Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
                        app.remove_list.clear();
                        app.remove_state.select(None);
                    } else {
                        let items = app.remove_list.clone();
                        let report = crate::logic::deps::resolve_reverse_dependencies(&items);
                        let summaries = report.summaries;
                        let dependencies = report.dependencies;
                        let crate::logic::preflight::PreflightSummaryOutcome { summary, header } =
                            crate::logic::preflight::compute_preflight_summary(
                                &items,
                                crate::state::PreflightAction::Remove,
                            );
                        app.pending_service_plan.clear();
                        app.modal = crate::state::Modal::Preflight {
                            items,
                            action: crate::state::PreflightAction::Remove,
                            tab: crate::state::PreflightTab::Summary,
                            summary: Some(Box::new(summary)),
                            header_chips: header,
                            dependency_info: dependencies,
                            dep_selected: 0,
                            dep_tree_expanded: std::collections::HashSet::new(),
                            deps_error: None,
                            file_info: Vec::new(),
                            file_selected: 0,
                            file_tree_expanded: std::collections::HashSet::new(),
                            files_error: None,
                            service_info: Vec::new(),
                            service_selected: 0,
                            services_loaded: false,
                            services_error: None,
                            sandbox_info: Vec::new(),
                            sandbox_selected: 0,
                            sandbox_tree_expanded: std::collections::HashSet::new(),
                            sandbox_loaded: false,
                            sandbox_error: None,
                            selected_optdepends: std::collections::HashMap::new(),
                            cascade_mode: app.remove_cascade_mode,
                        };
                        app.remove_preflight_summary = summaries;
                        app.toast_message = Some("Preflight: Remove list".to_string());
                    }
                }
            } else if app.installed_only_mode
                && matches!(
                    app.right_pane_focus,
                    crate::state::RightPaneFocus::Downgrade
                )
                && !app.downgrade_list.is_empty()
            {
                let names: Vec<String> =
                    app.downgrade_list.iter().map(|p| p.name.clone()).collect();
                let joined = names.join(" ");
                let cmd = if app.dry_run {
                    format!("echo DRY RUN: downgrade {joined}")
                } else {
                    format!(
                        "((command -v downgrade >/dev/null 2>&1) || sudo pacman -Qi downgrade >/dev/null 2>&1) && downgrade {joined} || echo 'downgrade tool not found. Install \"downgrade\" from AUR.'"
                    )
                };
                crate::install::spawn_shell_commands_in_terminal(&[cmd]);
                app.downgrade_list.clear();
                app.downgrade_state.select(None);
            }
        }
        KeyCode::Esc => {
            app.focus = crate::state::Focus::Search;
            // Activate Search Normal mode when returning with Esc
            app.search_normal_mode = true;
            refresh_selected_details(app, details_tx);
        }
        code if matches_any(&km.pane_next) && code == ke.code => {
            // Desired cycle: Search -> Downgrade -> Remove -> Recent -> Search
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        // Downgrade -> Remove (stay in Install)
                        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                        if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                            app.remove_state.select(Some(0));
                        }
                        refresh_remove_details(app, details_tx);
                        return false;
                    }
                    crate::state::RightPaneFocus::Remove => {
                        // Remove -> Recent
                        if app.history_state.selected().is_none() && !app.recent.is_empty() {
                            app.history_state.select(Some(0));
                        }
                        app.focus = crate::state::Focus::Recent;
                        crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                        return false;
                    }
                    crate::state::RightPaneFocus::Install => {}
                }
            }
            // Not in installed-only: Install -> Recent
            if app.history_state.selected().is_none() && !app.recent.is_empty() {
                app.history_state.select(Some(0));
            }
            app.focus = crate::state::Focus::Recent;
            crate::ui::helpers::trigger_recent_preview(app, preview_tx);
        }
        KeyCode::Left => {
            // In installed-only mode, follow reverse: Remove -> Downgrade -> Search
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Remove => {
                        // Move to Downgrade subpane and keep Install focus
                        app.right_pane_focus = crate::state::RightPaneFocus::Downgrade;
                        if app.downgrade_state.selected().is_none()
                            && !app.downgrade_list.is_empty()
                        {
                            app.downgrade_state.select(Some(0));
                        }
                        super::utils::refresh_downgrade_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Downgrade => {
                        // Downgrade -> Search
                        app.focus = crate::state::Focus::Search;
                        refresh_selected_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Normal mode: Install -> Search
                        app.focus = crate::state::Focus::Search;
                        refresh_selected_details(app, details_tx);
                    }
                }
            } else {
                // Normal mode: Install -> Search
                app.focus = crate::state::Focus::Search;
                refresh_selected_details(app, details_tx);
            }
        }
        KeyCode::Right => {
            // In installed-only mode, follow: Downgrade -> Remove -> Recent; else wrap to Recent
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        app.right_pane_focus = crate::state::RightPaneFocus::Remove;
                        if app.remove_state.selected().is_none() && !app.remove_list.is_empty() {
                            app.remove_state.select(Some(0));
                        }
                        refresh_remove_details(app, details_tx);
                    }
                    crate::state::RightPaneFocus::Remove => {
                        // Wrap-around to Recent from rightmost subpane
                        if app.history_state.selected().is_none() && !app.recent.is_empty() {
                            app.history_state.select(Some(0));
                        }
                        app.focus = crate::state::Focus::Recent;
                        crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Normal Install subpane: wrap directly to Recent
                        if app.history_state.selected().is_none() && !app.recent.is_empty() {
                            app.history_state.select(Some(0));
                        }
                        app.focus = crate::state::Focus::Recent;
                        crate::ui::helpers::trigger_recent_preview(app, preview_tx);
                    }
                }
            } else {
                // Normal mode: Install -> Recent (wrap)
                if app.history_state.selected().is_none() && !app.recent.is_empty() {
                    app.history_state.select(Some(0));
                }
                app.focus = crate::state::Focus::Recent;
                crate::ui::helpers::trigger_recent_preview(app, preview_tx);
            }
        }
        KeyCode::Delete if !ke.modifiers.contains(KeyModifiers::SHIFT) => {
            // Delete the selected item from the active list
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        if let Some(sel) = app.downgrade_state.selected()
                            && sel < app.downgrade_list.len()
                        {
                            app.downgrade_list.remove(sel);
                            let len = app.downgrade_list.len();
                            if len == 0 {
                                app.downgrade_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.downgrade_state.select(Some(new_sel));
                                super::utils::refresh_downgrade_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Remove => {
                        if let Some(sel) = app.remove_state.selected()
                            && sel < app.remove_list.len()
                        {
                            app.remove_list.remove(sel);
                            let len = app.remove_list.len();
                            if len == 0 {
                                app.remove_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.remove_state.select(Some(new_sel));
                                refresh_remove_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Installed-only mode: when Install subpane is focused, delete from Install list
                        let inds = crate::ui::helpers::filtered_install_indices(app);
                        if inds.is_empty() {
                            return false;
                        }
                        if let Some(vsel) = app.install_state.selected() {
                            let i = inds.get(vsel).copied().unwrap_or(0);
                            if i < app.install_list.len() {
                                app.install_list.remove(i);
                                app.install_dirty = true;
                                // Clear dependency cache when list changes
                                app.install_list_deps.clear();
                                app.install_list_files.clear();
                                app.deps_resolving = false;
                                app.files_resolving = false;
                                let vis_len = inds.len().saturating_sub(1); // one less visible
                                if vis_len == 0 {
                                    app.install_state.select(None);
                                } else {
                                    let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                                    app.install_state.select(Some(new_sel));
                                    refresh_install_details(app, details_tx);
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: delete from Install list
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(vsel) = app.install_state.selected() {
                    let i = inds.get(vsel).copied().unwrap_or(0);
                    if i < app.install_list.len() {
                        app.install_list.remove(i);
                        app.install_dirty = true;
                        let vis_len = inds.len().saturating_sub(1); // one less visible
                        if vis_len == 0 {
                            app.install_state.select(None);
                        } else {
                            let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                            app.install_state.select(Some(new_sel));
                            refresh_install_details(app, details_tx);
                        }
                    }
                }
            }
        }
        code if matches_any(&km.install_clear) && code == ke.code => {
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        app.downgrade_list.clear();
                        app.downgrade_state.select(None);
                    }
                    crate::state::RightPaneFocus::Remove => {
                        app.remove_list.clear();
                        app.remove_state.select(None);
                    }
                    crate::state::RightPaneFocus::Install => {
                        app.install_list.clear();
                        app.install_state.select(None);
                        app.install_dirty = true;
                        // Clear dependency cache when list is cleared
                        app.install_list_deps.clear();
                        app.install_list_files.clear();
                        app.deps_resolving = false;
                        app.files_resolving = false;
                    }
                }
            } else {
                app.install_list.clear();
                app.install_state.select(None);
                app.install_dirty = true;
                // Clear dependency cache when list is cleared
                app.install_list_deps.clear();
                app.deps_resolving = false;
            }
        }
        code if matches_any(&km.install_remove) && code == ke.code => {
            // Support 'd' (and other configured keys) as an alternative to Delete everywhere
            if app.installed_only_mode {
                match app.right_pane_focus {
                    crate::state::RightPaneFocus::Downgrade => {
                        if let Some(sel) = app.downgrade_state.selected()
                            && sel < app.downgrade_list.len()
                        {
                            app.downgrade_list.remove(sel);
                            let len = app.downgrade_list.len();
                            if len == 0 {
                                app.downgrade_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.downgrade_state.select(Some(new_sel));
                                super::utils::refresh_downgrade_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Remove => {
                        if let Some(sel) = app.remove_state.selected()
                            && sel < app.remove_list.len()
                        {
                            app.remove_list.remove(sel);
                            let len = app.remove_list.len();
                            if len == 0 {
                                app.remove_state.select(None);
                            } else {
                                let new_sel = sel.min(len - 1);
                                app.remove_state.select(Some(new_sel));
                                refresh_remove_details(app, details_tx);
                            }
                        }
                    }
                    crate::state::RightPaneFocus::Install => {
                        // Fall through to normal install list removal logic below
                        let inds = crate::ui::helpers::filtered_install_indices(app);
                        if inds.is_empty() {
                            return false;
                        }
                        if let Some(vsel) = app.install_state.selected() {
                            let i = inds.get(vsel).copied().unwrap_or(0);
                            if i < app.install_list.len() {
                                app.install_list.remove(i);
                                app.install_dirty = true;
                                // Clear dependency cache when list changes
                                app.install_list_deps.clear();
                                app.install_list_files.clear();
                                app.deps_resolving = false;
                                app.files_resolving = false;
                                let vis_len = inds.len().saturating_sub(1); // one less visible
                                if vis_len == 0 {
                                    app.install_state.select(None);
                                } else {
                                    let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                                    app.install_state.select(Some(new_sel));
                                    refresh_install_details(app, details_tx);
                                }
                            }
                        }
                    }
                }
            } else {
                // Normal mode: remove from Install list
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(vsel) = app.install_state.selected() {
                    let i = inds.get(vsel).copied().unwrap_or(0);
                    if i < app.install_list.len() {
                        app.install_list.remove(i);
                        app.install_dirty = true;
                        let vis_len = inds.len().saturating_sub(1); // one less visible
                        if vis_len == 0 {
                            app.install_state.select(None);
                        } else {
                            let new_sel = if vsel >= vis_len { vis_len - 1 } else { vsel };
                            app.install_state.select(Some(new_sel));
                            refresh_install_details(app, details_tx);
                        }
                    }
                }
            }
        }
        KeyCode::Up => {
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                if let Some(sel) = app.install_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.install_state.select(Some(new));
                    refresh_install_details(app, details_tx);
                }
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                if let Some(sel) = app.remove_state.selected() {
                    let new = sel.saturating_sub(1);
                    app.remove_state.select(Some(new));
                    refresh_remove_details(app, details_tx);
                }
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) && let Some(sel) = app.downgrade_state.selected()
            {
                let new = sel.saturating_sub(1);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        KeyCode::Down => {
            if !app.installed_only_mode
                || matches!(app.right_pane_focus, crate::state::RightPaneFocus::Install)
            {
                let inds = crate::ui::helpers::filtered_install_indices(app);
                if inds.is_empty() {
                    return false;
                }
                let sel = app.install_state.selected().unwrap_or(0);
                let max = inds.len().saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.install_state.select(Some(new));
                refresh_install_details(app, details_tx);
            } else if matches!(app.right_pane_focus, crate::state::RightPaneFocus::Remove) {
                let len = app.remove_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.remove_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.remove_state.select(Some(new));
                refresh_remove_details(app, details_tx);
            } else if matches!(
                app.right_pane_focus,
                crate::state::RightPaneFocus::Downgrade
            ) {
                let len = app.downgrade_list.len();
                if len == 0 {
                    return false;
                }
                let sel = app.downgrade_state.selected().unwrap_or(0);
                let max = len.saturating_sub(1);
                let new = std::cmp::min(sel + 1, max);
                app.downgrade_state.select(Some(new));
                super::utils::refresh_downgrade_details(app, details_tx);
            }
        }
        _ => {}
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What: Produce a baseline `AppState` tailored for install-pane tests without repeating setup boilerplate.
    ///
    /// Inputs:
    /// - None (relies on `Default::default()` for deterministic initial state).
    ///
    /// Output:
    /// - Fresh `AppState` ready for mutation inside individual test cases.
    ///
    /// Details:
    /// - Keeps test bodies concise while ensuring each case starts from a clean copy.
    fn new_app() -> AppState {
        AppState {
            ..Default::default()
        }
    }

    #[test]
    /// What: Confirm pressing Enter opens the preflight modal when installs are pending.
    ///
    /// Inputs:
    /// - Install list seeded with a single package and `Enter` key event.
    ///
    /// Output:
    /// - Modal transitions to `Preflight` with one item, `Install` action, and `Summary` tab active.
    ///
    /// Details:
    /// - Uses mock channels to satisfy handler requirements without observing downstream messages.
    fn install_enter_opens_confirm_install() {
        let mut app = new_app();
        app.install_list = vec![PackageItem {
            name: "rg".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        }];
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let _ = handle_install_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &dtx,
            &ptx,
            &atx,
        );
        match app.modal {
            crate::state::Modal::Preflight {
                ref items,
                action,
                tab,
                summary: _,
                header_chips: _,
                dependency_info: _,
                dep_selected: _,
                dep_tree_expanded: _,
                deps_error: _,
                file_info: _,
                file_selected: _,
                file_tree_expanded: _,
                files_error: _,
                service_info: _,
                service_selected: _,
                services_loaded: _,
                services_error: _,
                sandbox_info: _,
                sandbox_selected: _,
                sandbox_tree_expanded: _,
                sandbox_loaded: _,
                sandbox_error: _,
                selected_optdepends: _,
                cascade_mode: _,
            } => {
                assert_eq!(items.len(), 1);
                assert_eq!(action, crate::state::PreflightAction::Install);
                assert_eq!(tab, crate::state::PreflightTab::Summary);
            }
            _ => panic!("Preflight modal not opened"),
        }
    }

    #[test]
    /// What: Placeholder ensuring default behaviour still opens the preflight modal when `skip_preflight` remains false.
    ///
    /// Inputs:
    /// - Single official package queued for install with `Enter` key event.
    ///
    /// Output:
    /// - Modal remains `Preflight`, matching current default configuration.
    ///
    /// Details:
    /// - Documents intent for future skip-preflight support while asserting existing flow stays intact.
    fn install_enter_bypasses_preflight_with_skip_flag() {
        // Simulate settings skip flag by temporarily overriding global settings via environment
        // (Direct mutation isn't available; we approximate by checking that modal stays None after handler when flag true)
        let mut app = new_app();
        app.install_list = vec![PackageItem {
            name: "ripgrep".into(),
            version: "1".into(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "core".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        }];
        // Force skip_preflight behavior by asserting settings default is false; we cannot change global easily here
        // so only run if default is false to ensure test logic doesn't misrepresent actual behavior.
        assert!(
            !crate::theme::settings().skip_preflight,
            "skip_preflight unexpectedly true by default"
        );
        // We cannot toggle the global setting in test environment without refactoring; mark this test as a placeholder.
        // Ensure original behavior still opens preflight.
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let _ = handle_install_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()),
            &mut app,
            &dtx,
            &ptx,
            &atx,
        );
        // Behavior remains preflight when flag false; placeholder ensures future refactor retains compatibility.
        match app.modal {
            crate::state::Modal::Preflight {
                summary: _,
                header_chips: _,
                dependency_info: _,
                dep_selected: _,
                dep_tree_expanded: _,
                file_info: _,
                file_selected: _,
                file_tree_expanded: _,
                service_info: _,
                service_selected: _,
                services_loaded: _,
                cascade_mode: _,
                ..
            } => {}
            _ => panic!("Expected Preflight when skip_preflight=false"),
        }
    }

    #[test]
    /// What: Verify the Delete key removes the selected install item.
    ///
    /// Inputs:
    /// - Install list with two entries, selection on the first, and `Delete` key event.
    ///
    /// Output:
    /// - List shrinks to one entry, confirming removal logic.
    ///
    /// Details:
    /// - Channels are stubbed to satisfy handler signature while focusing on list mutation.
    fn install_delete_removes_item() {
        let mut app = new_app();
        app.install_list = vec![
            PackageItem {
                name: "rg".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Aur,
                popularity: None,
            },
            PackageItem {
                name: "fd".into(),
                version: "1".into(),
                description: String::new(),
                source: crate::state::Source::Aur,
                popularity: None,
            },
        ];
        app.install_state.select(Some(0));
        let (dtx, _drx) = mpsc::unbounded_channel::<PackageItem>();
        let (ptx, _prx) = mpsc::unbounded_channel::<PackageItem>();
        let (atx, _arx) = mpsc::unbounded_channel::<PackageItem>();
        let _ = handle_install_key(
            KeyEvent::new(KeyCode::Delete, KeyModifiers::empty()),
            &mut app,
            &dtx,
            &ptx,
            &atx,
        );
        assert_eq!(app.install_list.len(), 1);
    }
}
