//! Direct install/remove operations using integrated processes (bypassing preflight).

use crate::state::{AppState, PackageItem, modal::CascadeMode};

/// What: Show one-time long-run auth preflight guidance when readiness is at risk.
fn maybe_show_long_run_auth_preflight(app: &mut AppState) {
    if app.long_run_auth_preflight_warned {
        return;
    }
    let settings = crate::theme::settings();
    let readiness = crate::logic::long_run_auth::evaluate_long_run_auth_readiness(&settings);
    if readiness.should_warn {
        app.long_run_auth_preflight_warned = true;
        app.toast_message = Some(crate::logic::long_run_auth::build_long_run_warning_message(
            app,
        ));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(8));
    }
}

/// What: Start integrated install process for a single package (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `item`: Package to install
/// - `dry_run`: Whether to run in dry-run mode
///
/// Output:
/// - If passwordless sudo is available: Proceeds directly to `PreflightExec` modal.
/// - If passwordless sudo is not available: Transitions to `PasswordPrompt` modal.
///
/// Details:
/// - Checks for passwordless sudo first to skip password prompt if available.
/// - Both official packages (sudo pacman) and AUR packages (paru/yay need sudo for final step)
///   require sudo, but password may not be needed if passwordless sudo is configured.
/// - Uses `ExecutorRequest::Install` for execution.
pub fn start_integrated_install(app: &mut AppState, item: &PackageItem, dry_run: bool) {
    use crate::events::start_execution;
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;
    maybe_show_long_run_auth_preflight(app);
    let items = vec![item.clone()];
    let header_chips = PreflightHeaderChips::default();

    // Check faillock status before proceeding
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    if let Some(lockout_msg) = crate::logic::faillock::get_lockout_message_if_locked(&username, app)
    {
        // User is locked out - show warning
        app.modal = crate::state::Modal::Alert {
            message: lockout_msg,
        };
        return;
    }

    let settings = crate::theme::settings();
    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => start_execution(
                app,
                &items,
                crate::state::PreflightAction::Install,
                header_chips,
                None,
            ),
            Ok(false) => {
                app.modal = crate::state::Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.modal = crate::state::Modal::Alert { message: e };
            }
        }
    } else if crate::logic::password::resolve_auth_mode(&settings)
        == crate::logic::privilege::AuthMode::PasswordlessOnly
        && crate::logic::password::should_use_passwordless_sudo(&settings)
    {
        start_execution(
            app,
            &items,
            crate::state::PreflightAction::Install,
            header_chips,
            None,
        );
    } else {
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Install,
            items,
            input: crate::state::SecureString::default(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(header_chips);
    }
}

/// What: Start integrated install process for multiple packages (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Packages to install
/// - `dry_run`: Whether to run in dry-run mode
///
/// Output:
/// - If passwordless sudo is available: Proceeds directly to `PreflightExec` modal.
/// - If passwordless sudo is not available: Transitions to `PasswordPrompt` modal.
///
/// Details:
/// - Checks for passwordless sudo first to skip password prompt if available.
/// - Both official packages (sudo pacman) and AUR packages (paru/yay need sudo for final step)
///   require sudo, but password may not be needed if passwordless sudo is configured.
/// - Uses `ExecutorRequest::Install` for execution.
pub fn start_integrated_install_all(app: &mut AppState, items: &[PackageItem], dry_run: bool) {
    use crate::events::start_execution;
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;
    maybe_show_long_run_auth_preflight(app);
    let items_vec = items.to_vec();
    let header_chips = PreflightHeaderChips::default();

    // Check faillock status before proceeding
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    if let Some(lockout_msg) = crate::logic::faillock::get_lockout_message_if_locked(&username, app)
    {
        // User is locked out - show warning
        app.modal = crate::state::Modal::Alert {
            message: lockout_msg,
        };
        return;
    }

    let settings = crate::theme::settings();
    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => start_execution(
                app,
                &items_vec,
                crate::state::PreflightAction::Install,
                header_chips,
                None,
            ),
            Ok(false) => {
                app.modal = crate::state::Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.modal = crate::state::Modal::Alert { message: e };
            }
        }
    } else if crate::logic::password::resolve_auth_mode(&settings)
        == crate::logic::privilege::AuthMode::PasswordlessOnly
        && crate::logic::password::should_use_passwordless_sudo(&settings)
    {
        start_execution(
            app,
            &items_vec,
            crate::state::PreflightAction::Install,
            header_chips,
            None,
        );
    } else {
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Install,
            items: items_vec,
            input: crate::state::SecureString::default(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(header_chips);
    }
}

/// What: Start integrated remove process (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `names`: Package names to remove
/// - `dry_run`: Whether to run in dry-run mode
/// - `cascade_mode`: Cascade removal mode
///
/// Output:
/// - In interactive mode: Proceeds directly to `PreflightExec` without password modal.
/// - Otherwise: Transitions to `PasswordPrompt`.
///
/// Details:
/// - Remove operations always need privilege escalation.
/// - In `auth_mode = interactive`, Pacsea performs terminal handoff auth and then starts
///   execution without collecting a password in-app.
/// - Outside interactive mode, remove keeps using the in-app password prompt.
/// - Uses `ExecutorRequest::Remove` for execution.
pub fn start_integrated_remove_all(
    app: &mut AppState,
    names: &[String],
    dry_run: bool,
    cascade_mode: CascadeMode,
) {
    use crate::events::start_execution;
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;
    maybe_show_long_run_auth_preflight(app);
    app.remove_cascade_mode = cascade_mode;

    // Convert names to PackageItem for password prompt (we only need names, so create minimal items)
    let items: Vec<PackageItem> = names
        .iter()
        .map(|name| PackageItem {
            name: name.clone(),
            version: String::new(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: String::new(),
                arch: String::new(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        })
        .collect();

    // Remove operations always need sudo (pacman -R requires sudo regardless of package source).
    // Remove is intentionally never passwordless for safety (see docstring).
    // Check faillock status before showing password prompt
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    if let Some(lockout_msg) = crate::logic::faillock::get_lockout_message_if_locked(&username, app)
    {
        // User is locked out - show warning and don't show password prompt
        app.modal = crate::state::Modal::Alert {
            message: lockout_msg,
        };
        return;
    }
    let header_chips = PreflightHeaderChips::default();
    let settings = crate::theme::settings();
    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => {
                start_execution(
                    app,
                    &items,
                    crate::state::PreflightAction::Remove,
                    header_chips,
                    None,
                );
            }
            Ok(false) => {
                app.modal = crate::state::Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.modal = crate::state::Modal::Alert { message: e };
            }
        }
        return;
    }

    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Remove,
        items,
        input: crate::state::SecureString::default(),
        cursor: 0,
        error: None,
    };
    app.pending_exec_header_chips = Some(header_chips);
}

#[cfg(test)]
mod tests {
    use super::maybe_show_long_run_auth_preflight;

    #[test]
    fn preflight_warning_is_latched_once_per_session() {
        let _guard = crate::global_test_mutex_lock();
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "none");
        }
        let mut app = crate::state::AppState::default();
        assert!(!app.long_run_auth_preflight_warned);

        maybe_show_long_run_auth_preflight(&mut app);
        let first_toast = app.toast_message.clone();
        assert!(app.long_run_auth_preflight_warned);
        assert!(first_toast.is_some());

        maybe_show_long_run_auth_preflight(&mut app);
        assert_eq!(app.toast_message, first_toast);

        unsafe {
            std::env::remove_var("PACSEA_TEST_PRIVILEGE_AVAILABLE");
            std::env::remove_var("PACSEA_INTEGRATION_TEST");
        }
    }
}
