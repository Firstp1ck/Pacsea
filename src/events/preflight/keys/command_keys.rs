//! Command key handlers for Preflight modal.

use crate::state::{AppState, PackageItem};

use crate::events::preflight::modal::close_preflight_modal;

/// What: Handle F key - sync file database.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false` to continue event processing.
///
/// Details:
/// - Runs the sync operation in a background thread to avoid blocking the UI.
/// - If sync requires root privileges, launches a terminal with sudo command.
pub(super) fn handle_f_key(app: &mut AppState) -> bool {
    if let crate::state::Modal::Preflight { tab, .. } = &app.modal {
        if *tab != crate::state::PreflightTab::Files {
            return false;
        }
    } else {
        return false;
    }

    // Show initial message that sync is starting
    app.toast_message = Some("File database sync starting...".to_string());
    app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));

    // Run sync in background thread to avoid blocking the UI
    // Use catch_unwind to prevent panics from crashing the TUI
    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Use the new ensure_file_db_synced function with force=true
            // This will attempt to sync regardless of timestamp
            let sync_result = crate::logic::files::ensure_file_db_synced(true, 7);
            match sync_result {
                Ok(synced) => {
                    if synced {
                        tracing::info!("File database sync completed successfully");
                    } else {
                        tracing::info!("File database is already fresh");
                    }
                }
                Err(e) => {
                    // Sync failed (likely requires root), launch terminal with sudo
                    tracing::warn!(
                        "File database sync failed: {}, launching terminal with sudo",
                        e
                    );
                    let sync_cmd = "sudo pacman -Fy".to_string();
                    let cmds = vec![sync_cmd];
                    crate::install::spawn_shell_commands_in_terminal(&cmds);
                }
            }
        }));

        if let Err(panic_info) = result {
            tracing::error!("File database sync thread panicked: {:?}", panic_info);
        }
    });

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

    false
}

/// What: Handle S key - open scan configuration modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_s_key(app: &mut AppState) -> bool {
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
pub(super) fn handle_dry_run_key(app: &mut AppState) -> bool {
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
pub(super) fn handle_m_key(app: &mut AppState) -> bool {
    let next_mode_opt = if let crate::state::Modal::Preflight {
        action: crate::state::PreflightAction::Remove,
        cascade_mode,
        ..
    } = &mut app.modal
    {
        let next_mode = cascade_mode.next();
        *cascade_mode = next_mode;
        Some(next_mode)
    } else {
        None
    };

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

/// What: Extract install targets from preflight modal.
///
/// Inputs:
/// - `items`: Items from preflight modal
/// - `selected_optdepends`: Selected optional dependencies
///
/// Output: Packages to install including optional dependencies.
///
/// Details: Adds selected optional dependencies to the install list.
fn extract_install_targets(
    items: &[PackageItem],
    selected_optdepends: &std::collections::HashMap<String, std::collections::HashSet<String>>,
) -> Vec<PackageItem> {
    let mut packages = items.to_vec();
    // Add selected optional dependencies as additional packages to install
    for optdeps in selected_optdepends.values() {
        for optdep in optdeps {
            let optdep_pkg_name = crate::logic::sandbox::extract_package_name(optdep);
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
                    out_of_date: None,
                    orphaned: false,
                });
            }
        }
    }
    packages
}

/// What: Handle proceed action for install targets.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `packages`: Packages to install
/// - `header_chips`: Header chip metrics
///
/// Output: `false` to keep TUI open.
///
/// Details: Checks for reinstalls first, then batch updates (only if update available), handles password prompt if needed, or starts execution.
pub(super) fn handle_proceed_install(
    app: &mut AppState,
    packages: Vec<PackageItem>,
    header_chips: crate::state::modal::PreflightHeaderChips,
) -> bool {
    // First, check if we're installing packages that are already installed (reinstall scenario)
    // This check happens BEFORE password prompt
    // BUT exclude packages that have updates available (those should go through normal update flow)
    let installed_set = crate::logic::deps::get_installed_packages();
    let provided_set = crate::logic::deps::get_provided_packages(&installed_set);
    let upgradable_set = crate::logic::deps::get_upgradable_packages();

    let installed_packages: Vec<crate::state::PackageItem> = packages
        .iter()
        .filter(|item| {
            // Check if package is installed or provided by an installed package
            let is_installed = crate::logic::deps::is_package_installed_or_provided(
                &item.name,
                &installed_set,
                &provided_set,
            );

            if !is_installed {
                return false;
            }

            // Check if package has an update available
            // For official packages: check if it's in upgradable_set
            // For AUR packages: check if target version is different/newer than installed version
            let has_update = if upgradable_set.contains(&item.name) {
                // Official package with update available
                true
            } else if matches!(item.source, crate::state::Source::Aur) && !item.version.is_empty() {
                // AUR package: compare target version with installed version
                // Use simple string comparison for AUR packages
                // If target version is different from installed, it's an update
                crate::logic::deps::get_installed_version(&item.name)
                    .is_ok_and(|installed_version| item.version != installed_version)
            } else {
                // No update available
                false
            };

            // Only show reinstall confirmation if installed AND no update available
            // If update is available, it should go through normal update flow
            !has_update
        })
        .cloned()
        .collect();

    if !installed_packages.is_empty() {
        // Show reinstall confirmation modal (before password prompt)
        app.modal = crate::state::Modal::ConfirmReinstall {
            items: installed_packages,
            header_chips,
        };
        return false; // Don't close modal yet, wait for confirmation
    }

    // Check if this is a batch update scenario requiring confirmation
    // Only show if there's actually an update available (package is upgradable)
    // AND the package has installed packages in its "Required By" field (dependency risk)
    let upgradable_set = crate::logic::deps::get_upgradable_packages();
    let has_versions = packages.iter().any(|item| {
        matches!(item.source, crate::state::Source::Official { .. }) && !item.version.is_empty()
    });
    let has_upgrade_available = packages.iter().any(|item| {
        matches!(item.source, crate::state::Source::Official { .. })
            && upgradable_set.contains(&item.name)
    });

    // Only show warning if package has installed packages in "Required By" (dependency risk)
    let has_installed_required_by = packages.iter().any(|item| {
        matches!(item.source, crate::state::Source::Official { .. })
            && crate::index::is_installed(&item.name)
            && crate::logic::deps::has_installed_required_by(&item.name)
    });

    if has_versions && has_upgrade_available && has_installed_required_by {
        // Show confirmation modal for batch updates (only if update is actually available
        // AND package has installed dependents that could be affected)
        app.modal = crate::state::Modal::ConfirmBatchUpdate {
            items: packages,
            dry_run: app.dry_run,
        };
        return false; // Don't close modal yet, wait for confirmation
    }

    // Check if password is needed
    let has_official = packages
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Official { .. }));
    if has_official {
        // Show password prompt
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Install,
            items: packages,
            input: String::new(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(header_chips);
    } else {
        // No password needed, go directly to execution
        super::action_keys::start_execution(
            app,
            &packages,
            crate::state::PreflightAction::Install,
            header_chips,
            None,
        );
    }
    false // Keep TUI open
}

/// What: Handle proceed action for remove targets.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Items to remove
/// - `mode`: Cascade removal mode
/// - `header_chips`: Header chip metrics
///
/// Output: `false` to keep TUI open.
///
/// Details: Handles password prompt if needed, or starts execution.
pub(super) fn handle_proceed_remove(
    app: &mut AppState,
    items: Vec<PackageItem>,
    mode: crate::state::modal::CascadeMode,
    header_chips: crate::state::modal::PreflightHeaderChips,
) -> bool {
    // Store cascade mode for executor (needed in both password and non-password paths)
    app.remove_cascade_mode = mode;

    // Remove operations always need sudo (pacman -R requires sudo regardless of package source)
    // Always show password prompt - user can press Enter if passwordless sudo is configured
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Remove,
        items,
        input: String::new(),
        cursor: 0,
        error: None,
    };
    app.pending_exec_header_chips = Some(header_chips);
    false // Keep TUI open
}

/// What: Handle p key - proceed with install/remove.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false` to continue event processing.
///
/// Details:
/// - Closes the modal if install/remove is triggered, but TUI remains open.
#[allow(clippy::too_many_lines)] // Function handles complex preflight proceed logic
pub(super) fn handle_p_key(app: &mut AppState) -> bool {
    let new_summary: Option<Vec<crate::state::modal::ReverseRootSummary>> = None;
    let mut blocked_dep_count: Option<usize> = None;
    let mut removal_names: Option<Vec<String>> = None;
    let mut removal_mode: Option<crate::state::modal::CascadeMode> = None;
    let mut install_targets: Option<Vec<PackageItem>> = None;
    let mut service_info_for_plan: Option<Vec<crate::state::modal::ServiceImpact>> = None;
    let mut deps_not_resolved_message: Option<String> = None;

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
                    let packages = extract_install_targets(&*items, selected_optdepends);
                    install_targets = Some(packages);
                }
                crate::state::PreflightAction::Remove => {
                    // If dependency_info is empty, dependencies haven't been resolved yet.
                    // Show a warning but allow removal to proceed (don't block).
                    if dependency_info.is_empty() {
                        // Show warning if cascade mode is Basic (might have dependents we don't know about)
                        if !cascade_mode.allows_dependents() {
                            deps_not_resolved_message = Some(
                                "Warning: Dependencies not resolved yet. Package may have dependents. Switch to Dependencies tab to check."
                                    .to_string(),
                            );
                        }
                        // Always allow removal to proceed, even if dependencies aren't resolved
                        removal_names = Some(items.iter().map(|p| p.name.clone()).collect());
                        removal_mode = Some(*cascade_mode);
                    } else if cascade_mode.allows_dependents() {
                        // Cascade mode allows dependents, proceed with removal
                        removal_names = Some(items.iter().map(|p| p.name.clone()).collect());
                        removal_mode = Some(*cascade_mode);
                    } else {
                        // Dependencies are resolved, cascade mode is Basic, and there are dependents
                        // Block removal since Basic mode doesn't allow removing packages with dependents
                        blocked_dep_count = Some(dependency_info.len());
                    }
                }
                crate::state::PreflightAction::Downgrade => {
                    // For downgrade, we don't need to check dependencies
                    // Downgrade tool handles its own logic
                    // Just allow downgrade to proceed - handled separately below
                }
            }

            if !service_info.is_empty() {
                service_info_for_plan = Some(service_info.clone());
            }
        }
    }

    // Set toast message if dependencies not resolved
    if let Some(msg) = deps_not_resolved_message {
        app.toast_message = Some(msg);
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(5));
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
        // Get header_chips and service_info before closing modal
        let header_chips = if let crate::state::Modal::Preflight { header_chips, .. } = &app.modal {
            header_chips.clone()
        } else {
            crate::state::modal::PreflightHeaderChips::default()
        };
        let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
            service_info.clone()
        } else {
            Vec::new()
        };

        // Close preflight modal
        crate::events::preflight::modal::close_preflight_modal(app, &service_info);

        return handle_proceed_install(app, packages, header_chips);
    } else if let Some(names) = removal_names {
        let mode = removal_mode.unwrap_or(crate::state::modal::CascadeMode::Basic);

        // Get header_chips and service_info before closing modal
        let header_chips = if let crate::state::Modal::Preflight { header_chips, .. } = &app.modal {
            header_chips.clone()
        } else {
            crate::state::modal::PreflightHeaderChips::default()
        };
        let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
            service_info.clone()
        } else {
            Vec::new()
        };

        // Get items for removal
        let items = if let crate::state::Modal::Preflight { items, .. } = &app.modal {
            items
                .iter()
                .filter(|p| names.contains(&p.name))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        // Close preflight modal
        crate::events::preflight::modal::close_preflight_modal(app, &service_info);

        return handle_proceed_remove(app, items, mode, header_chips);
    }

    // Check if this is a downgrade action
    let is_downgrade = if let crate::state::Modal::Preflight { action, .. } = &app.modal {
        matches!(action, crate::state::PreflightAction::Downgrade)
    } else {
        false
    };

    if is_downgrade {
        // Get items, action, header_chips, and cascade_mode before closing modal
        let (items, _action, header_chips, cascade_mode) = if let crate::state::Modal::Preflight {
            action,
            items,
            header_chips,
            cascade_mode,
            ..
        } = &app.modal
        {
            (items.clone(), *action, header_chips.clone(), *cascade_mode)
        } else {
            return false;
        };

        // Downgrade operations always need sudo (downgrade tool requires sudo)
        // Always show password prompt - user can press Enter if passwordless sudo is configured
        // Store cascade mode for consistency (though downgrade doesn't use it)
        app.remove_cascade_mode = cascade_mode;

        // Get service_info before closing modal
        let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
            service_info.clone()
        } else {
            Vec::new()
        };

        // Close preflight modal
        crate::events::preflight::modal::close_preflight_modal(app, &service_info);

        // Show password prompt for downgrade
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Downgrade,
            items,
            input: String::new(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(header_chips);
        return false;
    }

    if let Some(count) = blocked_dep_count {
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

    false
}

/// What: Handle c key - snapshot placeholder.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_c_key(app: &mut AppState) -> bool {
    // TODO: implement Logic for creating a snapshot
    app.toast_message = Some(crate::i18n::t(app, "app.toasts.snapshot_placeholder"));
    false
}

/// What: Handle q key - close modal.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false` to continue event processing.
///
/// Details:
/// - Closes the modal but keeps the TUI open.
pub(super) fn handle_q_key(app: &mut AppState) -> bool {
    let service_info = if let crate::state::Modal::Preflight { service_info, .. } = &app.modal {
        service_info.clone()
    } else {
        Vec::new()
    };
    close_preflight_modal(app, &service_info);
    // Return false to keep TUI open - modal is closed but app continues
    false
}

/// What: Handle ? key - show help.
///
/// Inputs:
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_help_key(app: &mut AppState) -> bool {
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
