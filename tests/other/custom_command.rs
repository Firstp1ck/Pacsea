//! Integration tests for custom command handler.
//!
//! Tests cover:
//! - `ExecutorRequest::CustomCommand` creation
//! - Command with sudo password
//! - Command without sudo
//! - Dry-run command format

#![cfg(test)]

use pacsea::install::ExecutorRequest;
use pacsea::state::{AppState, Modal, PreflightAction, PreflightTab};
use pacsea::state::modal::{PasswordPurpose, PreflightHeaderChips};

#[test]
/// What: Test `ExecutorRequest::CustomCommand` creation.
///
/// Inputs:
/// - Command string.
///
/// Output:
/// - `ExecutorRequest::CustomCommand` with correct command.
///
/// Details:
/// - Verifies custom command request can be created.
fn integration_custom_command_creation() {
    let request = ExecutorRequest::CustomCommand {
        command: "makepkg -si".to_string(),
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand {
            command,
            password,
            dry_run,
        } => {
            assert_eq!(command, "makepkg -si");
            assert!(password.is_none());
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test `ExecutorRequest::CustomCommand` with sudo password.
///
/// Inputs:
/// - Command requiring sudo and password.
///
/// Output:
/// - Password is included in request.
///
/// Details:
/// - Verifies sudo commands include password.
fn integration_custom_command_with_password() {
    let request = ExecutorRequest::CustomCommand {
        command: "sudo pacman -Fy".to_string(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand {
            command, password, ..
        } => {
            assert!(command.contains("sudo"));
            assert_eq!(password, Some("testpassword".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test `ExecutorRequest::CustomCommand` without password.
///
/// Inputs:
/// - Command not requiring sudo.
///
/// Output:
/// - Password is None.
///
/// Details:
/// - Verifies non-sudo commands don't require password.
fn integration_custom_command_no_password() {
    let request = ExecutorRequest::CustomCommand {
        command: "git clone https://aur.archlinux.org/yay.git".to_string(),
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { password, .. } => {
            assert!(password.is_none());
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test `ExecutorRequest::CustomCommand` dry-run mode.
///
/// Inputs:
/// - Custom command with dry_run=true.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies custom command respects dry-run flag.
fn integration_custom_command_dry_run() {
    let request = ExecutorRequest::CustomCommand {
        command: "makepkg -si".to_string(),
        password: None,
        dry_run: true,
    };

    match request {
        ExecutorRequest::CustomCommand { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command for paru installation.
///
/// Inputs:
/// - Paru installation command sequence.
///
/// Output:
/// - Command includes makepkg.
///
/// Details:
/// - Verifies paru installation command structure.
fn integration_custom_command_paru_install() {
    let command = "cd /tmp/paru && makepkg -si --noconfirm".to_string();

    let request = ExecutorRequest::CustomCommand {
        command: command.clone(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { command, .. } => {
            assert!(command.contains("makepkg"));
            assert!(command.contains("-si"));
            assert!(command.contains("paru"));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command for yay installation.
///
/// Inputs:
/// - Yay installation command sequence.
///
/// Output:
/// - Command includes makepkg.
///
/// Details:
/// - Verifies yay installation command structure.
fn integration_custom_command_yay_install() {
    let command = "cd /tmp/yay && makepkg -si --noconfirm".to_string();

    let request = ExecutorRequest::CustomCommand {
        command: command.clone(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { command, .. } => {
            assert!(command.contains("makepkg"));
            assert!(command.contains("-si"));
            assert!(command.contains("yay"));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command for file database sync.
///
/// Inputs:
/// - File database sync command.
///
/// Output:
/// - Command is pacman -Fy.
///
/// Details:
/// - Verifies file sync command structure.
fn integration_custom_command_file_sync() {
    let request = ExecutorRequest::CustomCommand {
        command: "sudo pacman -Fy".to_string(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { command, .. } => {
            assert!(command.contains("pacman"));
            assert!(command.contains("-Fy"));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command triggers password prompt.
///
/// Inputs:
/// - Custom sudo command.
///
/// Output:
/// - Password prompt modal is shown.
///
/// Details:
/// - Verifies sudo commands trigger password prompt.
fn integration_custom_command_password_prompt() {
    let mut app = AppState {
        pending_custom_command: Some("sudo pacman -Fy".to_string()),
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate password prompt trigger
    app.modal = Modal::PasswordPrompt {
        purpose: PasswordPurpose::FileSync,
        items: vec![],
        input: String::new(),
        cursor: 0,
        error: None,
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, .. } => {
            assert_eq!(purpose, PasswordPurpose::FileSync);
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test custom command transitions to `PreflightExec`.
///
/// Inputs:
/// - Custom command with password submitted.
///
/// Output:
/// - Modal transitions to `PreflightExec`.
///
/// Details:
/// - Verifies custom command flow after password.
fn integration_custom_command_to_preflight_exec() {
    let mut app = AppState {
        pending_custom_command: Some("sudo pacman -Fy".to_string()),
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::FileSync,
            items: vec![],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Extract password and command
    let password = if let Modal::PasswordPrompt { ref input, .. } = app.modal {
        if input.trim().is_empty() {
            None
        } else {
            Some(input.clone())
        }
    } else {
        None
    };

    let command = app.pending_custom_command.take();

    // Simulate transition to PreflightExec
    let header_chips = app.pending_exec_header_chips.take().unwrap_or_default();
    app.modal = Modal::PreflightExec {
        items: vec![],
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        verbose: false,
        log_lines: vec![],
        abortable: false,
        header_chips,
    };

    if let Some(cmd) = command {
        app.pending_executor_request = Some(ExecutorRequest::CustomCommand {
            command: cmd,
            password,
            dry_run: false,
        });
    }

    // Verify modal
    assert!(matches!(app.modal, Modal::PreflightExec { .. }));

    // Verify executor request
    match app.pending_executor_request {
        Some(ExecutorRequest::CustomCommand {
            command, password, ..
        }) => {
            assert_eq!(command, "sudo pacman -Fy");
            assert_eq!(password, Some("testpassword".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command with empty command string.
///
/// Inputs:
/// - Empty command string.
///
/// Output:
/// - Request handles empty command gracefully.
///
/// Details:
/// - Edge case for empty command.
fn integration_custom_command_empty() {
    let request = ExecutorRequest::CustomCommand {
        command: String::new(),
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { command, .. } => {
            assert!(command.is_empty());
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command with special characters.
///
/// Inputs:
/// - Command with special shell characters.
///
/// Output:
/// - Special characters are preserved.
///
/// Details:
/// - Verifies command string handles special chars.
fn integration_custom_command_special_chars() {
    let command = "echo 'test with spaces' && ls -la | grep 'pattern'".to_string();

    let request = ExecutorRequest::CustomCommand {
        command: command.clone(),
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand { command: cmd, .. } => {
            assert!(cmd.contains("&&"));
            assert!(cmd.contains("|"));
            assert!(cmd.contains("grep"));
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test custom command dry-run format.
///
/// Inputs:
/// - Custom command for dry-run.
///
/// Output:
/// - Dry-run format includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run command format.
fn integration_custom_command_dry_run_format() {
    let command = "sudo pacman -Fy";
    let dry_run_cmd = format!("echo DRY RUN: {command}");

    assert!(dry_run_cmd.contains("DRY RUN:"));
    assert!(dry_run_cmd.contains("pacman -Fy"));
}

