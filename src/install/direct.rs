//! Direct install/remove operations using integrated processes (bypassing preflight).

use crate::state::{AppState, PackageItem, modal::CascadeMode};

/// What: Start integrated install process for a single package (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `item`: Package to install
/// - `dry_run`: Whether to run in dry-run mode
///
/// Output:
/// - Transitions to `PasswordPrompt` (all installs need password for sudo)
///
/// Details:
/// - Both official packages (sudo pacman) and AUR packages (paru/yay need sudo for final step)
///   require password, so always show `PasswordPrompt`
/// - Uses `ExecutorRequest::Install` for execution
pub fn start_integrated_install(app: &mut AppState, item: &PackageItem, dry_run: bool) {
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;

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
    // Show password prompt for all installs (official and AUR both need sudo)
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Install,
        items: vec![item.clone()],
        input: String::new(),
        cursor: 0,
        error: None,
    };
    app.pending_exec_header_chips = Some(PreflightHeaderChips::default());
}

/// What: Start integrated install process for multiple packages (bypassing preflight).
///
/// Inputs:
/// - `app`: Mutable application state
/// - `items`: Packages to install
/// - `dry_run`: Whether to run in dry-run mode
///
/// Output:
/// - Transitions to `PasswordPrompt` (all installs need password for sudo)
///
/// Details:
/// - Both official packages (sudo pacman) and AUR packages (paru/yay need sudo for final step)
///   require password, so always show `PasswordPrompt`
/// - Uses `ExecutorRequest::Install` for execution
pub fn start_integrated_install_all(app: &mut AppState, items: &[PackageItem], dry_run: bool) {
    use crate::state::modal::PreflightHeaderChips;

    app.dry_run = dry_run;

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
    // Show password prompt for all installs (official and AUR both need sudo)
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::Install,
        items: items.to_vec(),
        input: String::new(),
        cursor: 0,
        error: None,
    };
    app.pending_exec_header_chips = Some(PreflightHeaderChips::default());
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
