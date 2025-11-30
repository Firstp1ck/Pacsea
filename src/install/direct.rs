//! Direct install/remove operations using integrated processes (bypassing preflight).

use crate::state::{AppState, PackageItem, PreflightAction, modal::CascadeMode};

/// What: Start integrated install process for a single package (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `item`: Package to install
/// - `dry_run`: Whether to run in dry-run mode
///
/// Output:
/// - Transitions to `PasswordPrompt` if password needed, or `PreflightExec` if not
///
/// Details:
/// - Checks if password is needed (for official packages)
/// - Shows `PasswordPrompt` modal if password needed, otherwise goes directly to `PreflightExec`
/// - Uses `ExecutorRequest::Install` for execution
pub fn start_integrated_install(app: &mut AppState, item: &PackageItem, dry_run: bool) {
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;

    // Check if password is needed (for official packages)
    let has_official = matches!(item.source, crate::state::Source::Official { .. });
    if has_official {
        // Check faillock status before showing password prompt
        let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        if let Some(lockout_msg) =
            crate::logic::faillock::get_lockout_message_if_locked(&username, app)
        {
            // User is locked out - show warning and don't show password prompt
            app.modal = crate::state::Modal::Alert {
                message: lockout_msg,
            };
            return;
        }
        // Show password prompt
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Install,
            items: vec![item.clone()],
            input: String::new(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(PreflightHeaderChips::default());
    } else {
        // No password needed (AUR package), go directly to execution
        start_execution_internal(
            app,
            std::slice::from_ref(item),
            PreflightAction::Install,
            PreflightHeaderChips::default(),
            None,
        );
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
/// - Transitions to `PasswordPrompt` if password needed, or `PreflightExec` if not
///
/// Details:
/// - Checks if password is needed (for official packages)
/// - Shows `PasswordPrompt` modal if password needed, otherwise goes directly to `PreflightExec`
/// - Uses `ExecutorRequest::Install` for execution
pub fn start_integrated_install_all(app: &mut AppState, items: &[PackageItem], dry_run: bool) {
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;

    // Check if password is needed (for official packages)
    let has_official = items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Official { .. }));
    if has_official {
        // Check faillock status before showing password prompt
        let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
        if let Some(lockout_msg) =
            crate::logic::faillock::get_lockout_message_if_locked(&username, app)
        {
            // User is locked out - show warning and don't show password prompt
            app.modal = crate::state::Modal::Alert {
                message: lockout_msg,
            };
            return;
        }
        // Show password prompt
        app.modal = crate::state::Modal::PasswordPrompt {
            purpose: crate::state::modal::PasswordPurpose::Install,
            items: items.to_vec(),
            input: String::new(),
            cursor: 0,
            error: None,
        };
        app.pending_exec_header_chips = Some(PreflightHeaderChips::default());
    } else {
        // No password needed (AUR packages only), go directly to execution
        start_execution_internal(
            app,
            items,
            PreflightAction::Install,
            PreflightHeaderChips::default(),
            None,
        );
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
/// - Transitions to `PasswordPrompt` (remove always needs sudo)
///
/// Details:
/// - Remove operations always need sudo, so always show `PasswordPrompt`
/// - Uses `ExecutorRequest::Remove` for execution
pub fn start_integrated_remove_all(
    app: &mut AppState,
    names: &[String],
    dry_run: bool,
    cascade_mode: CascadeMode,
) {
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;
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

    // Remove operations always need sudo (pacman -R requires sudo regardless of package source)
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
    // Always show password prompt - user can press Enter if passwordless sudo is configured
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Remove,
        items,
        input: String::new(),
        cursor: 0,
        error: None,
    };
    app.pending_exec_header_chips = Some(PreflightHeaderChips::default());
}

/// What: Internal helper to start execution (duplicated from `preflight::keys::action_keys::start_execution`).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Packages to install/remove
/// - `action`: Install or Remove action
/// - `header_chips`: Header chip metrics
/// - `password`: Optional password (if already obtained from password prompt)
///
/// Details:
/// - Transitions to `PreflightExec` modal and stores `ExecutorRequest` for processing in tick handler
/// - Duplicated from `preflight::keys::action_keys::start_execution` since that module is private
fn start_execution_internal(
    app: &mut AppState,
    items: &[PackageItem],
    action: PreflightAction,
    header_chips: crate::state::modal::PreflightHeaderChips,
    password: Option<String>,
) {
    use crate::install::ExecutorRequest;

    // Transition to PreflightExec modal
    app.modal = crate::state::Modal::PreflightExec {
        items: items.to_vec(),
        action,
        tab: crate::state::PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips,
    };

    // Store executor request for processing in tick handler
    app.pending_executor_request = Some(match action {
        PreflightAction::Install => ExecutorRequest::Install {
            items: items.to_vec(),
            password,
            dry_run: app.dry_run,
        },
        PreflightAction::Remove => {
            let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
            ExecutorRequest::Remove {
                names,
                password,
                cascade: app.remove_cascade_mode,
                dry_run: app.dry_run,
            }
        }
        PreflightAction::Downgrade => {
            let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
            ExecutorRequest::Downgrade {
                names,
                password,
                dry_run: app.dry_run,
            }
        }
    });
}
