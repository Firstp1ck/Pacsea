//! Command key handlers for Preflight modal.

use crate::state::{AppState, PackageItem};

use crate::events::preflight::modal::close_preflight_modal;

/// What: Handle F key - sync file database.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if handled (should return early), `false` otherwise.
pub(crate) fn handle_f_key(app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        if *tab != crate::state::PreflightTab::Files {
            return false;
        }
    } else {
        return false;
    }

    // Use the new ensure_file_db_synced function with force=true
    // This will attempt to sync regardless of timestamp
    let sync_result = crate::logic::files::ensure_file_db_synced(true, 7);
    match sync_result {
        Ok(synced) => {
            if synced {
                app.toast_message =
                    Some("File database sync completed. Files tab will refresh.".to_string());
            } else {
                app.toast_message = Some("File database is already fresh.".to_string());
            }
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            // Clear file_info to trigger re-resolution after sync completes
            if let crate::state::Modal::Preflight {
                file_info,
                file_selected,
                ..
            } = &mut app.modal
            {
                *file_info = Vec::new();
                *file_selected = 0;
            }
        }
        Err(e) => {
            // Sync failed (likely requires root), launch terminal with sudo
            let sync_cmd = "sudo pacman -Fy".to_string();
            let cmds = vec![sync_cmd];
            std::thread::spawn(move || {
                crate::install::spawn_shell_commands_in_terminal(&cmds);
            });
            app.toast_message = Some(format!(
                "File database sync started in terminal (requires root). Error: {}",
                e
            ));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
            // Clear file_info to trigger re-resolution after sync completes
            if let crate::state::Modal::Preflight {
                file_info,
                file_selected,
                ..
            } = &mut app.modal
            {
                *file_info = Vec::new();
                *file_selected = 0;
            }
        }
    }
    true
}

/// What: Handle S key - open scan configuration modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_s_key(app: &mut AppState) -> bool {
    // Build AUR package name list to scan
    let names = if let crate::state::Modal::Preflight { items, .. } = &app.modal {
        items
            .iter()
            .filter(|p| matches!(p.source, crate::state::Source::Aur))
            .map(|p| p.name.clone())
            .collect::<Vec<_>>()
    } else {
        return false;
    };

    if names.is_empty() {
        app.modal = crate::state::Modal::Alert {
            message: "No AUR packages selected to scan.\nAdd AUR packages to scan, then press 's'."
                .into(),
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
    false
}

/// What: Handle d key - toggle dry-run mode.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_dry_run_key(app: &mut AppState) -> bool {
    app.dry_run = !app.dry_run;
    let toast_key = if app.dry_run {
        "app.toasts.dry_run_enabled"
    } else {
        "app.toasts.dry_run_disabled"
    };
    app.toast_message = Some(crate::i18n::t(app, toast_key));
    false
}

/// What: Handle m key - cycle cascade mode.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_m_key(app: &mut AppState) -> bool {
    let mut next_mode_opt = None;
    if let crate::state::Modal::Preflight {
        action: crate::state::PreflightAction::Remove,
        cascade_mode,
        ..
    } = &mut app.modal
    {
        let next_mode = cascade_mode.next();
        *cascade_mode = next_mode;
        next_mode_opt = Some(next_mode);
    }

    if let Some(next_mode) = next_mode_opt {
        app.remove_cascade_mode = next_mode;
        app.toast_message = Some(format!(
            "Cascade mode set to {} ({})",
            next_mode.flag(),
            next_mode.description()
        ));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
    }
    false
}

/// What: Handle p key - proceed with install/remove.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
pub(crate) fn handle_p_key(app: &mut AppState) -> bool {
    let mut close_modal = false;
    let mut new_summary: Option<Vec<crate::state::modal::ReverseRootSummary>> = None;
    let mut blocked_dep_count: Option<usize> = None;
    let mut removal_names: Option<Vec<String>> = None;
    let mut removal_mode: Option<crate::state::modal::CascadeMode> = None;
    let mut install_targets: Option<Vec<PackageItem>> = None;
    let mut service_info_for_plan: Option<Vec<crate::state::modal::ServiceImpact>> = None;

    // Scope for borrowing app.modal
    {
        if let crate::state::Modal::Preflight {
            action,
            items,
            dependency_info,
            cascade_mode,
            selected_optdepends,
            service_info,
            ..
        } = &mut app.modal
        {
            match action {
                crate::state::PreflightAction::Install => {
                    let mut packages = items.to_vec();
                    // Add selected optional dependencies as additional packages to install
                    for (_pkg_name, optdeps) in selected_optdepends.iter() {
                        for optdep in optdeps {
                            let optdep_pkg_name =
                                crate::logic::sandbox::extract_package_name(optdep);
                            if !packages.iter().any(|p| p.name == optdep_pkg_name) {
                                packages.push(PackageItem {
                                    name: optdep_pkg_name,
                                    version: String::new(),
                                    description: String::new(),
                                    source: crate::state::Source::Official {
                                        repo: String::new(),
                                        arch: String::new(),
                                    },
                                    popularity: None,
                                });
                            }
                        }
                    }
                    install_targets = Some(packages);
                }
                crate::state::PreflightAction::Remove => {
                    if dependency_info.is_empty() {
                        let report = crate::logic::deps::resolve_reverse_dependencies(items);
                        new_summary = Some(report.summaries);
                        *dependency_info = report.dependencies;
                    }

                    if dependency_info.is_empty() || cascade_mode.allows_dependents() {
                        removal_names = Some(items.iter().map(|p| p.name.clone()).collect());
                        removal_mode = Some(*cascade_mode);
                    } else {
                        blocked_dep_count = Some(dependency_info.len());
                    }
                }
            }

            if !service_info.is_empty() {
                service_info_for_plan = Some(service_info.clone());
            }
        }
    }

    if let Some(summary) = new_summary {
        app.remove_preflight_summary = summary;
    }

    if let Some(plan) = service_info_for_plan {
        app.pending_service_plan = plan;
    } else {
        app.pending_service_plan.clear();
    }

    if let Some(packages) = install_targets {
        crate::install::spawn_install_all(&packages, app.dry_run);
        close_modal = true;
    } else if let Some(names) = removal_names {
        let mode = removal_mode.unwrap_or(crate::state::modal::CascadeMode::Basic);
        crate::install::spawn_remove_all(&names, app.dry_run, mode);
        close_modal = true;
    } else if let Some(count) = blocked_dep_count {
        let root_list: Vec<String> = app
            .remove_preflight_summary
            .iter()
            .filter(|summary| summary.total_dependents > 0)
            .map(|summary| summary.package.clone())
            .collect();
        let subject = if root_list.is_empty() {
            "the selected packages".to_string()
        } else {
            root_list.join(", ")
        };
        app.toast_message = Some(format!(
            "Removal blocked: {count} dependent package(s) rely on {subject}. Enable cascade removal to proceed."
        ));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(6));
    }

    if close_modal {
        let service_info_clone =
            if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
                service_info.clone()
            } else {
                Vec::new()
            };
        close_preflight_modal(app, &service_info_clone);
        return true;
    }
    false
}

/// What: Handle c key - snapshot placeholder.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_c_key(app: &mut AppState) -> bool {
    app.toast_message = Some(crate::i18n::t(app, "app.toasts.snapshot_placeholder"));
    false
}

/// What: Handle q key - close modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if modal should close, `false` otherwise.
pub(crate) fn handle_q_key(app: &mut AppState) -> bool {
    let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
        service_info.clone()
    } else {
        Vec::new()
    };
    close_preflight_modal(app, &service_info);
    true
}

/// What: Handle ? key - show help.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(crate) fn handle_help_key(app: &mut AppState) -> bool {
    let help_message = if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        if *tab == crate::state::PreflightTab::Deps {
            crate::i18n::t(app, "app.modals.preflight.help.deps_tab")
        } else {
            crate::i18n::t(app, "app.modals.preflight.help.general")
        }
    } else {
        return false;
    };

    app.previous_modal = Some(app.modal.clone());
    app.modal = crate::state::Modal::Alert {
        message: help_message,
    };
    false
}
