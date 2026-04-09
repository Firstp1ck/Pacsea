//! Handlers for foreign↔sync overlap wizard and AUR duplicate-results warning.

use std::collections::HashSet;

use crossterm::event::{KeyCode, KeyEvent};

use crate::state::modal::{ForeignRepoOverlapPhase, PasswordPurpose, PreflightHeaderChips};
use crate::state::{AppState, Modal, PackageItem};

/// What: Scroll the overlap list on warning steps 0 and 1.
///
/// Inputs:
/// - `scroll`: Current scroll offset.
/// - `down`: Move down when `true`, up when `false`.
/// - `total_rows`: Number of rows in the list.
/// - `viewport`: Max visible rows.
///
/// Output:
/// - Updated scroll offset clamped to content.
fn scroll_overlap_list(scroll: u16, down: bool, total_rows: usize, viewport: usize) -> u16 {
    if total_rows <= viewport {
        return 0;
    }
    let max_scroll = total_rows.saturating_sub(viewport);
    let mut s = usize::from(scroll);
    if down {
        s = s.saturating_add(1).min(max_scroll);
    } else {
        s = s.saturating_sub(1);
    }
    u16::try_from(s).unwrap_or(0)
}

/// What: Run privileged foreign→sync migration using the same paths as repository apply.
///
/// Inputs:
/// - `app`: Application state.
/// - `cmds`: Shell commands from [`crate::logic::repos::build_foreign_to_sync_migrate_bundle`].
/// - `summary_lines`: Lines to seed `PreflightExec`.
///
/// Output:
/// - None (sets modals and pending fields).
///
/// Details:
/// - Mirrors [`crate::events::modals::repositories::queue_repo_apply_execution`] semantics including `PACSEA_TEST_OUT`.
fn queue_foreign_migrate_execution(
    app: &mut AppState,
    cmds: Vec<String>,
    summary_lines: Vec<String>,
) {
    app.pending_repositories_modal_resume = None;
    if cmds.is_empty() {
        app.pending_foreign_migrate_summary = None;
        app.pending_foreign_migrate_commands = None;
        app.modal = Modal::Alert {
            message: crate::i18n::t(app, "app.modals.foreign_overlap.migrate_empty_plan"),
        };
        return;
    }

    if std::env::var("PACSEA_TEST_OUT").is_ok() {
        crate::install::spawn_shell_commands_in_terminal(&cmds);
        app.modal = Modal::None;
        app.pending_foreign_migrate_summary = None;
        app.pending_foreign_migrate_commands = None;
        return;
    }

    let settings = crate::theme::settings();

    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => {
                app.modal = Modal::PreflightExec {
                    items: Vec::new(),
                    action: crate::state::PreflightAction::Install,
                    tab: crate::state::PreflightTab::Summary,
                    verbose: false,
                    log_lines: summary_lines,
                    abortable: false,
                    header_chips: PreflightHeaderChips::default(),
                    success: None,
                };
                app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
                    commands: cmds,
                    password: None,
                    dry_run: app.dry_run,
                });
            }
            Ok(false) => {
                app.pending_foreign_migrate_summary = None;
                app.pending_foreign_migrate_commands = None;
                app.modal = Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.pending_foreign_migrate_summary = None;
                app.pending_foreign_migrate_commands = None;
                app.modal = Modal::Alert { message: e };
            }
        }
        return;
    }

    if crate::logic::password::resolve_auth_mode(&settings)
        == crate::logic::privilege::AuthMode::PasswordlessOnly
        && crate::logic::password::should_use_passwordless_sudo(&settings)
    {
        app.modal = Modal::PreflightExec {
            items: Vec::new(),
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines: summary_lines,
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        };
        app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
            commands: cmds,
            password: None,
            dry_run: app.dry_run,
        });
        return;
    }

    app.pending_foreign_migrate_commands = Some(cmds);
    app.pending_foreign_migrate_summary = Some(summary_lines);
    app.modal = Modal::PasswordPrompt {
        purpose: PasswordPurpose::RepoForeignMigrate,
        items: Vec::new(),
        input: crate::state::SecureString::default(),
        cursor: 0,
        error: None,
    };
}

/// What: Handle keys for [`Modal::WarnAurRepoDuplicate`].
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Application state.
/// - `dup_names`: Duplicate names (for parity with renderer; unused here).
/// - `packages`: Install set to resume.
/// - `header_chips`: Preflight chips for [`crate::events::preflight::keys::handle_proceed_install`].
///
/// Output:
/// - `true` when the event was consumed.
///
/// Details:
/// - Enter sets `skip_aur_repo_dup_warning_once` indirectly via proceed handler clearing it on next entry; we set the flag before calling proceed.
pub(super) fn handle_warn_aur_repo_duplicate_modal(
    ke: KeyEvent,
    app: &mut AppState,
    _dup_names: &[String],
    packages: &[PackageItem],
    header_chips: &PreflightHeaderChips,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            app.modal = Modal::None;
            true
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            app.skip_aur_repo_dup_warning_once = true;
            let packages = packages.to_vec();
            let chips = header_chips.clone();
            app.modal = Modal::None;
            crate::events::preflight::keys::handle_proceed_install(app, packages, chips);
            true
        }
        _ => false,
    }
}

/// What: Handle keys for [`Modal::ForeignRepoOverlap`].
///
/// Inputs:
/// - `ke`: Key event.
/// - `app`: Application state.
/// - `repo_name`: Applied repository name.
/// - `entries`: Overlapping package rows.
/// - `phase`: Current wizard phase (updated in place via reassigned modal).
///
/// Output:
/// - `true` when the key was consumed.
pub(super) fn handle_foreign_repo_overlap_modal(
    ke: KeyEvent,
    app: &mut AppState,
    repo_name: &str,
    entries: &[(String, String)],
    phase: ForeignRepoOverlapPhase,
) -> bool {
    const VIEWPORT: usize = 8;
    match phase {
        ForeignRepoOverlapPhase::WarnAck { step, list_scroll } => {
            handle_overlap_warn_ack(ke, app, repo_name, entries, step, list_scroll, VIEWPORT)
        }
        ForeignRepoOverlapPhase::Select {
            cursor,
            list_scroll,
            selected,
        } => handle_overlap_select(
            ke,
            app,
            repo_name,
            entries,
            cursor,
            list_scroll,
            selected,
            VIEWPORT,
        ),
        ForeignRepoOverlapPhase::FinalConfirm {
            select_cursor,
            select_scroll,
            selected,
        } => handle_overlap_final_confirm(
            ke,
            app,
            repo_name,
            entries,
            select_cursor,
            select_scroll,
            selected,
        ),
    }
}

/// What: Keys for the two-step red warning before selection.
fn handle_overlap_warn_ack(
    ke: KeyEvent,
    app: &mut AppState,
    repo_name: &str,
    entries: &[(String, String)],
    step: u8,
    list_scroll: u16,
    viewport: usize,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            if step == 0 && crate::logic::repos::repositories_linux_actions_supported() {
                match super::repositories::queue_repo_disable_apply_after_overlap_warn_cancel(
                    app, repo_name,
                ) {
                    Ok(()) => return true,
                    Err(message) => {
                        app.modal = Modal::Alert { message };
                        return true;
                    }
                }
            }
            app.modal = Modal::None;
            super::repositories::reopen_repositories_modal_if_pending(app);
            true
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            if step == 0 {
                app.modal = Modal::ForeignRepoOverlap {
                    repo_name: repo_name.to_string(),
                    entries: entries.to_vec(),
                    phase: ForeignRepoOverlapPhase::WarnAck {
                        step: 1,
                        list_scroll,
                    },
                };
            } else {
                app.modal = Modal::ForeignRepoOverlap {
                    repo_name: repo_name.to_string(),
                    entries: entries.to_vec(),
                    phase: ForeignRepoOverlapPhase::Select {
                        cursor: 0,
                        list_scroll: 0,
                        selected: HashSet::new(),
                    },
                };
            }
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            let ns = scroll_overlap_list(list_scroll, false, entries.len(), viewport);
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::WarnAck {
                    step,
                    list_scroll: ns,
                },
            };
            true
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let ns = scroll_overlap_list(list_scroll, true, entries.len(), viewport);
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::WarnAck {
                    step,
                    list_scroll: ns,
                },
            };
            true
        }
        _ => false,
    }
}

/// What: Keys for multi-select migration targets.
#[allow(clippy::too_many_arguments)] // Mirrors wizard state split across modal fields; bundling adds indirection
fn handle_overlap_select(
    ke: KeyEvent,
    app: &mut AppState,
    repo_name: &str,
    entries: &[(String, String)],
    mut cursor: usize,
    list_scroll: u16,
    mut selected: HashSet<String>,
    viewport: usize,
) -> bool {
    let n = entries.len().max(1);
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            app.modal = Modal::None;
            super::repositories::reopen_repositories_modal_if_pending(app);
            true
        }
        KeyCode::Up | KeyCode::Char('k') => {
            cursor = cursor.saturating_sub(1);
            let mut ls = list_scroll;
            let cur = usize::from(ls);
            if cursor < cur {
                ls = u16::try_from(cursor).unwrap_or(0);
            }
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::Select {
                    cursor,
                    list_scroll: ls,
                    selected,
                },
            };
            true
        }
        KeyCode::Down | KeyCode::Char('j') => {
            cursor = (cursor + 1).min(n.saturating_sub(1));
            let mut ls = list_scroll;
            let cur = usize::from(ls);
            if cursor >= cur + viewport {
                ls = u16::try_from(cursor.saturating_sub(viewport.saturating_sub(1))).unwrap_or(0);
            }
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::Select {
                    cursor,
                    list_scroll: ls,
                    selected,
                },
            };
            true
        }
        KeyCode::Char(' ') => {
            if let Some(name) = entries.get(cursor).map(|e| e.0.clone()) {
                if selected.contains(&name) {
                    selected.remove(&name);
                } else {
                    selected.insert(name);
                }
            }
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::Select {
                    cursor,
                    list_scroll,
                    selected,
                },
            };
            true
        }
        KeyCode::Enter | KeyCode::Char('\n' | '\r') => {
            if selected.is_empty() {
                app.modal = Modal::None;
                super::repositories::reopen_repositories_modal_if_pending(app);
                return true;
            }
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::FinalConfirm {
                    select_cursor: cursor,
                    select_scroll: list_scroll,
                    selected,
                },
            };
            true
        }
        _ => false,
    }
}

/// What: Final confirmation before queuing privileged migration.
fn handle_overlap_final_confirm(
    ke: KeyEvent,
    app: &mut AppState,
    repo_name: &str,
    entries: &[(String, String)],
    select_cursor: usize,
    select_scroll: u16,
    selected: HashSet<String>,
) -> bool {
    match ke.code {
        KeyCode::Esc | KeyCode::Char('q' | 'Q') => {
            app.modal = Modal::ForeignRepoOverlap {
                repo_name: repo_name.to_string(),
                entries: entries.to_vec(),
                phase: ForeignRepoOverlapPhase::Select {
                    cursor: select_cursor,
                    list_scroll: select_scroll,
                    selected,
                },
            };
            true
        }
        KeyCode::Char('y' | 'Y') => {
            let mut names: Vec<String> = selected.into_iter().collect();
            names.sort();
            let tool = match crate::logic::privilege::active_tool() {
                Ok(t) => t,
                Err(msg) => {
                    app.modal = Modal::Alert { message: msg };
                    return true;
                }
            };
            let bundle = crate::logic::repos::build_foreign_to_sync_migrate_bundle(
                tool,
                app.dry_run,
                &names,
            );
            match bundle {
                Ok((summary, cmds)) => queue_foreign_migrate_execution(app, cmds, summary),
                Err(e) => {
                    app.modal = Modal::Alert { message: e };
                }
            }
            true
        }
        _ => false,
    }
}
