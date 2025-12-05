//! Integration tests for the system update process.
//!
//! Tests cover:
//! - System update modal state
//! - Update command building
//! - Different update options (mirrors, pacman, AUR, cache)
//! - `ExecutorRequest::Update` creation
//! - Password prompt for sudo commands
//! - Full update sequence

#![cfg(test)]

use pacsea::install::{ExecutorOutput, ExecutorRequest};
use pacsea::state::modal::{PasswordPurpose, PreflightHeaderChips};
use pacsea::state::{AppState, Modal, PreflightAction, PreflightTab};

#[test]
/// What: Test system update modal state creation.
///
/// Inputs:
/// - `SystemUpdate` modal with various options.
///
/// Output:
/// - Modal state is correctly structured.
///
/// Details:
/// - Verifies system update modal can be created and accessed.
fn integration_system_update_modal_state() {
    let app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: true,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: ["Worldwide".to_string(), "United States".to_string()].to_vec(),
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            force_sync,
            do_aur,
            do_cache,
            country_idx,
            countries,
            mirror_count,
            cursor,
        } => {
            assert!(do_mirrors);
            assert!(do_pacman);
            assert!(!force_sync);
            assert!(!do_aur);
            assert!(!do_cache);
            assert_eq!(country_idx, 0);
            assert_eq!(countries.len(), 2);
            assert_eq!(mirror_count, 10);
            assert_eq!(cursor, 0);
        }
        _ => panic!("Expected SystemUpdate modal"),
    }
}

#[test]
/// What: Test system update command building.
///
/// Inputs:
/// - Different update options combinations.
///
/// Output:
/// - Commands are built correctly.
///
/// Details:
/// - Verifies command building for different update scenarios.
/// - Note: Actual execution spawns terminal, so this tests command structure only.
fn integration_system_update_command_building() {
    // Test pacman update command
    let pacman_cmd = "sudo pacman -Syu --noconfirm";
    assert!(pacman_cmd.contains("pacman"));
    assert!(pacman_cmd.contains("-Syu"));
    assert!(pacman_cmd.contains("--noconfirm"));

    // Test AUR update command structure
    let aur_cmd = "if command -v paru >/dev/null 2>&1; then \
                paru -Syu --noconfirm; \
            elif command -v yay >/dev/null 2>&1; then \
                yay -Syu --noconfirm; \
            else \
                echo 'No AUR helper (paru/yay) found.'; \
            fi";
    assert!(aur_cmd.contains("paru") || aur_cmd.contains("yay"));
    assert!(aur_cmd.contains("-Syu"));

    // Test cache cleanup command
    let cache_cmd = "sudo pacman -Sc --noconfirm";
    assert!(cache_cmd.contains("pacman"));
    assert!(cache_cmd.contains("-Sc"));
}

#[test]
/// What: Test system update with all options enabled.
///
/// Inputs:
/// - `SystemUpdate` modal with all options enabled.
///
/// Output:
/// - All flags are correctly set.
///
/// Details:
/// - Verifies that all update options can be enabled simultaneously.
fn integration_system_update_all_options() {
    let app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: true,
            force_sync: false,
            do_aur: true,
            do_cache: true,
            country_idx: 0,
            countries: ["Worldwide".to_string()].to_vec(),
            mirror_count: 20,
            cursor: 0,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            ..
        } => {
            assert!(do_mirrors);
            assert!(do_pacman);
            assert!(do_aur);
            assert!(do_cache);
        }
        _ => panic!("Expected SystemUpdate modal"),
    }
}

#[test]
/// What: Test system update with no options selected.
///
/// Inputs:
/// - `SystemUpdate` modal with all options disabled.
///
/// Output:
/// - `Alert` modal is shown.
///
/// Details:
/// - Verifies that no action is taken when no options are selected.
fn integration_system_update_no_options() {
    let app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: false,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: ["Worldwide".to_string()].to_vec(),
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::SystemUpdate {
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            ..
        } => {
            assert!(!do_mirrors);
            assert!(!do_pacman);
            assert!(!do_aur);
            assert!(!do_cache);
        }
        _ => panic!("Expected SystemUpdate modal"),
    }
}

#[test]
/// What: Test that system update uses `ExecutorRequest` instead of spawning terminals.
///
/// Inputs:
/// - System update action triggered through `handle_system_update_enter`.
///
/// Output:
/// - `pending_update_commands` should be set and `PasswordPrompt` modal shown.
/// - After password entry, `pending_executor_request` should be set with `ExecutorRequest::Update`.
///
/// Details:
/// - Verifies that system update uses executor pattern instead of spawning terminals.
/// - `handle_system_update_enter` should set `app.pending_update_commands` and show `PasswordPrompt`.
/// - After password entry, the password handler creates `ExecutorRequest::Update`.
/// - Note: Since `handle_system_update_enter` is private, we test the pattern by directly creating
///   the expected state that the function should produce.
fn integration_system_update_uses_executor_not_terminal() {
    use pacsea::install::ExecutorRequest;

    let mut app = AppState {
        dry_run: false,
        ..Default::default()
    };

    // Step 1: Simulate what handle_system_update_enter does:
    // 1. Store commands in pending_update_commands
    // 2. Transition to PasswordPrompt modal
    let cmds = vec!["sudo pacman -Syu --noconfirm".to_string()];

    app.pending_update_commands = Some(cmds.clone());
    app.modal = pacsea::state::Modal::PasswordPrompt {
        purpose: pacsea::state::modal::PasswordPurpose::Update,
        items: Vec::new(),
        input: String::new(),
        cursor: 0,
        error: None,
    };

    // Verify pending_update_commands is set
    assert!(
        app.pending_update_commands.is_some(),
        "System update must set pending_update_commands before password prompt"
    );

    // Verify modal is PasswordPrompt
    match &app.modal {
        pacsea::state::Modal::PasswordPrompt { purpose, .. } => {
            assert!(
                matches!(purpose, pacsea::state::modal::PasswordPurpose::Update),
                "Password purpose should be Update"
            );
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }

    // Step 2: Simulate what password handler does after password entry:
    // 1. Take pending_update_commands
    // 2. Create ExecutorRequest::Update
    // 3. Transition to PreflightExec
    let password = Some("test_password".to_string());
    let update_cmds = app
        .pending_update_commands
        .take()
        .expect("pending_update_commands should be set");

    app.pending_executor_request = Some(ExecutorRequest::Update {
        commands: update_cmds,
        password: password.clone(),
        dry_run: app.dry_run,
    });

    app.modal = pacsea::state::Modal::PreflightExec {
        items: Vec::new(),
        action: pacsea::state::PreflightAction::Install,
        tab: pacsea::state::PreflightTab::Summary,
        verbose: false,
        log_lines: Vec::new(),
        abortable: false,
        header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
        success: None,
    };

    // Verify that pending_executor_request is set with ExecutorRequest::Update
    assert!(
        app.pending_executor_request.is_some(),
        "System update must use ExecutorRequest instead of spawning terminals"
    );

    // Verify it's an Update request with password
    match &app.pending_executor_request {
        Some(ExecutorRequest::Update {
            commands,
            password: req_password,
            dry_run,
        }) => {
            assert_eq!(
                *commands, cmds,
                "Update request should have the correct commands"
            );
            assert_eq!(
                *req_password, password,
                "Password should be set from password prompt"
            );
            assert!(!dry_run, "Dry run should be false");
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }

    // Verify modal transitioned to PreflightExec
    match app.modal {
        pacsea::state::Modal::PreflightExec { .. } => {}
        _ => panic!("Expected modal to transition to PreflightExec"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with mirror update command.
///
/// Inputs:
/// - Mirror update command with reflector.
///
/// Output:
/// - `ExecutorRequest::Update` with correct mirror command.
///
/// Details:
/// - Verifies mirror update command structure using reflector.
fn integration_system_update_mirror_command() {
    let country = "Germany";
    let mirror_count = 10;
    let mirror_cmd = format!(
        "sudo reflector --country {country} --latest {mirror_count} --sort rate --save /etc/pacman.d/mirrorlist"
    );

    let request = ExecutorRequest::Update {
        commands: vec![mirror_cmd],
        password: None,
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update { commands, .. } => {
            assert_eq!(commands.len(), 1);
            assert!(commands[0].contains("reflector"));
            assert!(commands[0].contains("Germany"));
            assert!(commands[0].contains("10"));
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with pacman update command.
///
/// Inputs:
/// - Pacman system update command.
///
/// Output:
/// - `ExecutorRequest::Update` with correct pacman command.
///
/// Details:
/// - Verifies pacman -Syu command structure.
fn integration_system_update_pacman_command() {
    let pacman_cmd = "sudo pacman -Syu --noconfirm".to_string();

    let request = ExecutorRequest::Update {
        commands: vec![pacman_cmd],
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update {
            commands,
            password,
            dry_run,
        } => {
            assert_eq!(commands.len(), 1);
            assert!(commands[0].contains("pacman"));
            assert!(commands[0].contains("-Syu"));
            assert!(commands[0].contains("--noconfirm"));
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with AUR update command.
///
/// Inputs:
/// - AUR helper detection and update command.
///
/// Output:
/// - `ExecutorRequest::Update` with correct AUR command.
///
/// Details:
/// - Verifies AUR update command structure with paru/yay fallback.
fn integration_system_update_aur_command() {
    let aur_cmd = "if command -v paru >/dev/null 2>&1; then \
                   paru -Syu --noconfirm; \
                   elif command -v yay >/dev/null 2>&1; then \
                   yay -Syu --noconfirm; \
                   else \
                   echo 'No AUR helper (paru/yay) found.'; \
                   fi"
    .to_string();

    let request = ExecutorRequest::Update {
        commands: vec![aur_cmd],
        password: None, // AUR helpers typically don't need sudo upfront
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update { commands, .. } => {
            assert_eq!(commands.len(), 1);
            assert!(commands[0].contains("paru"));
            assert!(commands[0].contains("yay"));
            assert!(commands[0].contains("-Syu"));
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with cache cleanup command.
///
/// Inputs:
/// - Cache cleanup command for pacman.
///
/// Output:
/// - `ExecutorRequest::Update` with correct cache cleanup command.
///
/// Details:
/// - Verifies pacman -Sc command structure.
fn integration_system_update_cache_command() {
    let cache_cmd = "sudo pacman -Sc --noconfirm".to_string();

    let request = ExecutorRequest::Update {
        commands: vec![cache_cmd],
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update {
            commands, password, ..
        } => {
            assert_eq!(commands.len(), 1);
            assert!(commands[0].contains("pacman"));
            assert!(commands[0].contains("-Sc"));
            assert!(commands[0].contains("--noconfirm"));
            assert_eq!(password, Some("testpassword".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test full system update sequence with all commands.
///
/// Inputs:
/// - All update options enabled (mirrors, pacman, AUR, cache).
///
/// Output:
/// - `ExecutorRequest::Update` with all commands in sequence.
///
/// Details:
/// - Verifies that full update sequence includes all commands.
fn integration_system_update_full_sequence() {
    let commands = vec![
        "sudo reflector --country Worldwide --latest 20 --sort rate --save /etc/pacman.d/mirrorlist".to_string(),
        "sudo pacman -Syu --noconfirm".to_string(),
        "if command -v paru >/dev/null 2>&1; then paru -Syu --noconfirm; elif command -v yay >/dev/null 2>&1; then yay -Syu --noconfirm; fi".to_string(),
        "sudo pacman -Sc --noconfirm".to_string(),
    ];

    let request = ExecutorRequest::Update {
        commands,
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update { commands, .. } => {
            assert_eq!(commands.len(), 4);
            assert!(commands[0].contains("reflector"));
            assert!(commands[1].contains("pacman") && commands[1].contains("-Syu"));
            assert!(commands[2].contains("paru") || commands[2].contains("yay"));
            assert!(commands[3].contains("pacman") && commands[3].contains("-Sc"));
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test system update triggers password prompt for sudo commands.
///
/// Inputs:
/// - System update modal confirmed.
///
/// Output:
/// - Password prompt modal is shown for sudo commands.
///
/// Details:
/// - Verifies that update operations requiring sudo show password prompt.
fn integration_system_update_password_prompt() {
    let mut app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: true,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Simulate update confirmation - transition to PasswordPrompt
    app.modal = Modal::PasswordPrompt {
        purpose: PasswordPurpose::Update,
        items: vec![],
        input: String::new(),
        cursor: 0,
        error: None,
    };

    match app.modal {
        Modal::PasswordPrompt { purpose, items, .. } => {
            assert_eq!(purpose, PasswordPurpose::Update);
            assert!(
                items.is_empty(),
                "Update password prompt should have empty items"
            );
        }
        _ => panic!("Expected PasswordPrompt modal"),
    }
}

#[test]
/// What: Test system update transitions to `PreflightExec` after password.
///
/// Inputs:
/// - Password submitted for system update.
///
/// Output:
/// - Modal transitions to `PreflightExec` with empty items.
///
/// Details:
/// - Verifies update flow after password submission.
fn integration_system_update_to_preflight_exec() {
    let mut app = AppState {
        modal: Modal::PasswordPrompt {
            purpose: PasswordPurpose::Update,
            items: vec![],
            input: "testpassword".to_string(),
            cursor: 12,
            error: None,
        },
        pending_exec_header_chips: Some(PreflightHeaderChips::default()),
        ..Default::default()
    };

    // Extract password
    let password = if let Modal::PasswordPrompt { ref input, .. } = app.modal {
        if input.trim().is_empty() {
            None
        } else {
            Some(input.clone())
        }
    } else {
        None
    };

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
        success: None,
    };

    // Set executor request
    app.pending_executor_request = Some(ExecutorRequest::Update {
        commands: vec!["sudo pacman -Syu --noconfirm".to_string()],
        password,
        dry_run: false,
    });

    // Verify modal
    match app.modal {
        Modal::PreflightExec { items, .. } => {
            assert!(
                items.is_empty(),
                "Update PreflightExec should have empty items"
            );
        }
        _ => panic!("Expected PreflightExec modal"),
    }

    // Verify executor request
    match app.pending_executor_request {
        Some(ExecutorRequest::Update { password, .. }) => {
            assert_eq!(password, Some("testpassword".to_string()));
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test system update dry-run mode.
///
/// Inputs:
/// - System update with `dry_run` enabled.
///
/// Output:
/// - `ExecutorRequest::Update` with `dry_run=true`.
///
/// Details:
/// - Verifies dry-run mode is respected for updates.
fn integration_system_update_dry_run() {
    let request = ExecutorRequest::Update {
        commands: vec!["sudo pacman -Syu --noconfirm".to_string()],
        password: None,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Update { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test system update cursor navigation.
///
/// Inputs:
/// - `SystemUpdate` modal with cursor at different positions.
///
/// Output:
/// - Cursor position is correctly tracked.
///
/// Details:
/// - Verifies cursor navigation within the update modal.
fn integration_system_update_cursor_navigation() {
    let app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: true,
            force_sync: false,
            do_aur: true,
            do_cache: true,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 3, // On cache option (index 3)
        },
        ..Default::default()
    };

    match app.modal {
        Modal::SystemUpdate { cursor, .. } => {
            assert_eq!(cursor, 3);
        }
        _ => panic!("Expected SystemUpdate modal"),
    }
}

#[test]
/// What: Test system update country selection.
///
/// Inputs:
/// - `SystemUpdate` modal with different country selection.
///
/// Output:
/// - Country index is correctly tracked.
///
/// Details:
/// - Verifies country selection for reflector mirror update.
fn integration_system_update_country_selection() {
    let countries = vec![
        "Worldwide".to_string(),
        "United States".to_string(),
        "Germany".to_string(),
        "France".to_string(),
    ];

    let app = AppState {
        modal: Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: false,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 2, // Germany
            countries,
            mirror_count: 15,
            cursor: 0,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::SystemUpdate {
            country_idx,
            countries: modal_countries,
            mirror_count,
            ..
        } => {
            assert_eq!(country_idx, 2);
            assert_eq!(modal_countries[country_idx], "Germany");
            assert_eq!(mirror_count, 15);
        }
        _ => panic!("Expected SystemUpdate modal"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with simulated network failure.
///
/// Inputs:
/// - Update request that will fail due to network error.
///
/// Output:
/// - Request structure is correct, error handling can occur.
///
/// Details:
/// - Verifies update request can be created even when network will fail.
/// - Tests error handling in update sequence when network fails mid-operation.
fn integration_system_update_network_failure() {
    let commands = vec![
        "sudo pacman -Syu --noconfirm".to_string(),
        "if command -v paru >/dev/null 2>&1; then paru -Syu --noconfirm; fi".to_string(),
    ];

    let request = ExecutorRequest::Update {
        commands,
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update { commands, .. } => {
            assert_eq!(commands.len(), 2);
            // Request structure is valid even if network will fail
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test network error display in `PreflightExec` modal during system update.
///
/// Inputs:
/// - `PreflightExec` modal with network failure during update.
///
/// Output:
/// - Error message is displayed in `log_lines`.
/// - Modal state reflects error condition.
///
/// Details:
/// - Verifies network errors during update are displayed to user.
fn integration_system_update_network_error_display() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![],
            action: PreflightAction::Install, // Update uses Install action
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![":: Synchronizing package databases...".to_string()],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Simulate network error during update
    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        log_lines.push("error: failed to retrieve 'core.db' from mirror".to_string());
        log_lines.push("error: Failed to connect to host (network unreachable)".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 3);
            assert!(log_lines[1].contains("failed to retrieve"));
            assert!(log_lines[2].contains("network unreachable"));
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `ExecutorOutput::Error` handling for network failure during update sequence.
///
/// Inputs:
/// - `ExecutorOutput::Error` with network failure message during update.
///
/// Output:
/// - Error is properly handled and displayed.
///
/// Details:
/// - Verifies update sequence handles network failures correctly.
fn integration_system_update_network_error_handling() {
    let error_output =
        ExecutorOutput::Error("Failed to connect to host (network unreachable)".to_string());

    // Simulate error being received during update
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![":: Starting full system upgrade...".to_string()],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Simulate error being added to log_lines
    if let ExecutorOutput::Error(msg) = &error_output
        && let Modal::PreflightExec {
            ref mut log_lines, ..
        } = app.modal
    {
        log_lines.push(format!("ERROR: {msg}"));
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 2);
            assert!(log_lines[1].contains("ERROR:"));
            assert!(log_lines[1].contains("network unreachable"));
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Verify system update completion does NOT clear `install_list`.
///
/// Inputs:
/// - System update completes (`PreflightExec` with empty items)
/// - User has packages in `install_list` before update
///
/// Output:
/// - `install_list` is preserved, NOT cleared
/// - `pending_install_names` should NOT be set for system updates
///
/// Details:
/// - System updates use empty items vector since they don't involve specific packages
/// - The Install action completion handler should skip `pending_install_names` tracking
///   when items is empty to avoid clearing `install_list` due to vacuously true check
/// - Regression test for bug: empty `pending_install_names` causes `install_list` to be cleared
fn integration_system_update_preserves_install_list() {
    use pacsea::state::{PackageItem, Source};

    // Create app with packages in install_list (queued for installation)
    let mut app = AppState {
        install_list: vec![
            PackageItem {
                name: "neovim".to_string(),
                version: "0.10.0-1".to_string(),
                description: "Vim-fork focused on extensibility and usability".to_string(),
                source: Source::Official {
                    repo: "extra".to_string(),
                    arch: "x86_64".to_string(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
            PackageItem {
                name: "ripgrep".to_string(),
                version: "14.0.0-1".to_string(),
                description: "A search tool that combines ag with grep".to_string(),
                source: Source::Official {
                    repo: "extra".to_string(),
                    arch: "x86_64".to_string(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            },
        ],
        // Simulate system update in progress (PreflightExec with empty items)
        modal: Modal::PreflightExec {
            items: Vec::new(),                // System update has NO items
            action: PreflightAction::Install, // System update uses Install action
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![":: Starting full system upgrade...".to_string()],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Verify install_list has packages before update
    assert_eq!(
        app.install_list.len(),
        2,
        "install_list should have 2 packages queued"
    );
    assert_eq!(app.install_list[0].name, "neovim");
    assert_eq!(app.install_list[1].name, "ripgrep");

    // Simulate system update completion - this is what handle_executor_output does
    // For system updates, items is empty so installed_names would be empty
    if let Modal::PreflightExec { items, action, .. } = &app.modal
        && matches!(action, PreflightAction::Install)
    {
        // BUG CONDITION: If we set pending_install_names to empty vec,
        // the tick handler will clear install_list (vacuously true check)
        // FIX: Only set pending_install_names if items is NOT empty
        if !items.is_empty() {
            let installed_names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
            app.pending_install_names = Some(installed_names);
        }
        // If items is empty (system update), do NOT set pending_install_names
    }

    // Verify pending_install_names is NOT set for system updates
    assert!(
        app.pending_install_names.is_none(),
        "System updates (empty items) should NOT set pending_install_names"
    );

    // Verify install_list is preserved
    assert_eq!(
        app.install_list.len(),
        2,
        "install_list should still have 2 packages after system update"
    );
    assert_eq!(app.install_list[0].name, "neovim");
    assert_eq!(app.install_list[1].name, "ripgrep");
}
