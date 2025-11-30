//! Integration tests for auto-scrolling logs in `PreflightExec` modal.
//!
//! Tests cover:
//! - `PreflightExec` modal `log_lines` append behavior
//! - Log panel state after multiple line additions
//! - Progress bar line replacement in `log_lines`

#![cfg(test)]

use pacsea::state::{
    AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source,
    modal::PreflightHeaderChips,
};

/// What: Create a test package item.
///
/// Inputs:
/// - `name`: Package name
///
/// Output:
/// - `PackageItem` ready for testing
///
/// Details:
/// - Helper to create test packages
fn create_test_package(name: &str) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

#[test]
/// What: Test `PreflightExec` modal initial state with empty `log_lines`.
///
/// Inputs:
/// - `PreflightExec` modal with empty `log_lines`.
///
/// Output:
/// - `log_lines` is empty.
///
/// Details:
/// - Verifies initial state before output starts.
fn integration_preflight_exec_empty_log_lines() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert!(log_lines.is_empty());
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `log_lines` append for single line.
///
/// Inputs:
/// - Single output line added to `log_lines`.
///
/// Output:
/// - `log_lines` contains the line.
///
/// Details:
/// - Simulates receiving one output line.
fn integration_preflight_exec_append_single_line() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        log_lines.push(":: Synchronizing package databases...".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 1);
            assert_eq!(log_lines[0], ":: Synchronizing package databases...");
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `log_lines` append for multiple lines.
///
/// Inputs:
/// - Multiple output lines added sequentially.
///
/// Output:
/// - All lines are stored in order.
///
/// Details:
/// - Simulates receiving multiple output lines.
fn integration_preflight_exec_append_multiple_lines() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    let lines = vec![
        ":: Synchronizing package databases...",
        " core is up to date",
        " extra is up to date",
        ":: Starting full system upgrade...",
        "resolving dependencies...",
        "looking for conflicting packages...",
    ];

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        for line in &lines {
            log_lines.push((*line).to_string());
        }
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 6);
            for (i, expected) in lines.iter().enumerate() {
                assert_eq!(log_lines[i], *expected);
            }
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test progress bar line replacement.
///
/// Inputs:
/// - Progress bar updates replacing the last line.
///
/// Output:
/// - Last line is replaced, not appended.
///
/// Details:
/// - Simulates `ReplaceLastLine` behavior for progress bars.
fn integration_preflight_exec_progress_bar_replace() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec!["Downloading package...".to_string()],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    // Simulate progress bar updates
    let progress_updates = vec![
        "[#-------] 10%",
        "[##------] 25%",
        "[####----] 50%",
        "[######--] 75%",
        "[########] 100%",
    ];

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        for progress in progress_updates {
            // Replace last line (progress bar behavior)
            if !log_lines.is_empty() {
                log_lines.pop();
            }
            log_lines.push(progress.to_string());
        }
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            // Only the final progress should remain
            assert_eq!(log_lines.len(), 1);
            assert_eq!(log_lines[0], "[########] 100%");
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test mixed regular lines and progress bar updates.
///
/// Inputs:
/// - Regular output lines followed by progress bar updates.
///
/// Output:
/// - Regular lines preserved, progress bar replaces its line.
///
/// Details:
/// - Simulates realistic pacman output pattern.
fn integration_preflight_exec_mixed_output() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        // Regular output
        log_lines.push(":: Retrieving packages...".to_string());

        // Progress bar (multiple updates, only last should remain)
        log_lines.push("[#-------]  10%".to_string());
        log_lines.pop();
        log_lines.push("[####----]  50%".to_string());
        log_lines.pop();
        log_lines.push("[########] 100%".to_string());

        // More regular output after progress
        log_lines.push("downloading ripgrep-14.0.0...".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 3);
            assert_eq!(log_lines[0], ":: Retrieving packages...");
            assert_eq!(log_lines[1], "[########] 100%");
            assert_eq!(log_lines[2], "downloading ripgrep-14.0.0...");
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test large log output handling.
///
/// Inputs:
/// - Many output lines (simulating verbose package operation).
///
/// Output:
/// - All lines are stored correctly.
///
/// Details:
/// - Verifies handling of large output from verbose operations.
fn integration_preflight_exec_large_log_output() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: true,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        for i in 0..500 {
            log_lines.push(format!("Installing file {i}/500: /usr/lib/pkg/file{i}.so"));
        }
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 500);
            assert!(log_lines[0].contains("file 0"));
            assert!(log_lines[499].contains("file 499"));
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `log_lines` with ANSI color codes.
///
/// Inputs:
/// - Output lines containing ANSI escape sequences.
///
/// Output:
/// - Lines are stored as-is (rendering strips ANSI).
///
/// Details:
/// - Verifies colored output is preserved in state.
fn integration_preflight_exec_ansi_color_output() {
    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        // Lines with ANSI color codes
        log_lines.push("\x1b[1;32m::\x1b[0m Synchronizing package databases...".to_string());
        log_lines.push("\x1b[1;34m core\x1b[0m is up to date".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 2);
            assert!(log_lines[0].contains("\x1b["));
            assert!(log_lines[1].contains("\x1b["));
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test verbose mode flag in `PreflightExec`.
///
/// Inputs:
/// - `PreflightExec` modal with verbose=true.
///
/// Output:
/// - verbose flag is correctly set.
///
/// Details:
/// - Verifies verbose mode can be enabled.
fn integration_preflight_exec_verbose_flag() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: true,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { verbose, .. } => {
            assert!(verbose);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test abortable flag in `PreflightExec`.
///
/// Inputs:
/// - `PreflightExec` modal with abortable=true.
///
/// Output:
/// - abortable flag is correctly set.
///
/// Details:
/// - Verifies abort capability can be enabled.
fn integration_preflight_exec_abortable_flag() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: true,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { abortable, .. } => {
            assert!(abortable);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `PreflightExec` with Remove action.
///
/// Inputs:
/// - `PreflightExec` modal for remove operation.
///
/// Output:
/// - action is correctly set to Remove.
///
/// Details:
/// - Verifies removal operations work with `PreflightExec`.
fn integration_preflight_exec_remove_action() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Remove,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { action, .. } => {
            assert_eq!(action, PreflightAction::Remove);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `PreflightExec` with Downgrade action.
///
/// Inputs:
/// - `PreflightExec` modal for downgrade operation.
///
/// Output:
/// - action is correctly set to Downgrade.
///
/// Details:
/// - Verifies downgrade operations work with `PreflightExec`.
fn integration_preflight_exec_downgrade_action() {
    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package("test-pkg")],
            action: PreflightAction::Downgrade,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { action, .. } => {
            assert_eq!(action, PreflightAction::Downgrade);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}
