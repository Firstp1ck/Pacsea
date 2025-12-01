//! Integration tests for executor output streaming.
//!
//! Tests cover:
//! - `ExecutorOutput::Line` message generation
//! - `ExecutorOutput::ReplaceLastLine` for progress bars
//! - `ExecutorOutput::Finished` with success/failure states
//! - `ExecutorOutput::Error` handling
//! - Large output handling in PreflightExec modal

#![cfg(test)]

use pacsea::install::{ExecutorOutput, ExecutorRequest};
use pacsea::state::{
    AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source,
    modal::PreflightHeaderChips,
};

/// What: Create a test package item with specified source.
///
/// Inputs:
/// - `name`: Package name
/// - `source`: Package source (Official or AUR)
///
/// Output:
/// - `PackageItem` ready for testing
///
/// Details:
/// - Helper to create test packages with consistent structure
fn create_test_package(name: &str, source: Source) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: String::new(),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

#[test]
/// What: Test ExecutorOutput::Line variant creation.
///
/// Inputs:
/// - Line of output text.
///
/// Output:
/// - ExecutorOutput::Line with the text.
///
/// Details:
/// - Verifies Line variant can be created and matched.
fn integration_executor_output_line() {
    let output = ExecutorOutput::Line("Downloading packages...".to_string());

    match output {
        ExecutorOutput::Line(text) => {
            assert_eq!(text, "Downloading packages...");
        }
        _ => panic!("Expected ExecutorOutput::Line"),
    }
}

#[test]
/// What: Test ExecutorOutput::ReplaceLastLine variant for progress bars.
///
/// Inputs:
/// - Progress bar text with carriage return semantics.
///
/// Output:
/// - ExecutorOutput::ReplaceLastLine with the text.
///
/// Details:
/// - Used for progress bars that overwrite the previous line.
fn integration_executor_output_replace_last_line() {
    let output = ExecutorOutput::ReplaceLastLine("[####----] 50%".to_string());

    match output {
        ExecutorOutput::ReplaceLastLine(text) => {
            assert_eq!(text, "[####----] 50%");
        }
        _ => panic!("Expected ExecutorOutput::ReplaceLastLine"),
    }
}

#[test]
/// What: Test ExecutorOutput::Finished variant with success.
///
/// Inputs:
/// - Finished state with success=true and exit_code=0.
///
/// Output:
/// - ExecutorOutput::Finished with correct fields.
///
/// Details:
/// - Indicates successful command completion.
fn integration_executor_output_finished_success() {
    let output = ExecutorOutput::Finished {
        success: true,
        exit_code: Some(0),
    };

    match output {
        ExecutorOutput::Finished { success, exit_code } => {
            assert!(success);
            assert_eq!(exit_code, Some(0));
        }
        _ => panic!("Expected ExecutorOutput::Finished"),
    }
}

#[test]
/// What: Test ExecutorOutput::Finished variant with failure.
///
/// Inputs:
/// - Finished state with success=false and exit_code=1.
///
/// Output:
/// - ExecutorOutput::Finished with failure state.
///
/// Details:
/// - Indicates failed command completion.
fn integration_executor_output_finished_failure() {
    let output = ExecutorOutput::Finished {
        success: false,
        exit_code: Some(1),
    };

    match output {
        ExecutorOutput::Finished { success, exit_code } => {
            assert!(!success);
            assert_eq!(exit_code, Some(1));
        }
        _ => panic!("Expected ExecutorOutput::Finished"),
    }
}

#[test]
/// What: Test ExecutorOutput::Finished with no exit code.
///
/// Inputs:
/// - Finished state with exit_code=None.
///
/// Output:
/// - ExecutorOutput::Finished with None exit_code.
///
/// Details:
/// - Some processes may not provide an exit code.
fn integration_executor_output_finished_no_exit_code() {
    let output = ExecutorOutput::Finished {
        success: false,
        exit_code: None,
    };

    match output {
        ExecutorOutput::Finished { success, exit_code } => {
            assert!(!success);
            assert!(exit_code.is_none());
        }
        _ => panic!("Expected ExecutorOutput::Finished"),
    }
}

#[test]
/// What: Test ExecutorOutput::Error variant.
///
/// Inputs:
/// - Error message string.
///
/// Output:
/// - ExecutorOutput::Error with the message.
///
/// Details:
/// - Used for PTY or command execution errors.
fn integration_executor_output_error() {
    let output = ExecutorOutput::Error("Failed to create PTY".to_string());

    match output {
        ExecutorOutput::Error(msg) => {
            assert_eq!(msg, "Failed to create PTY");
        }
        _ => panic!("Expected ExecutorOutput::Error"),
    }
}

#[test]
/// What: Test PreflightExec modal log_lines append behavior.
///
/// Inputs:
/// - PreflightExec modal with log_lines.
///
/// Output:
/// - log_lines correctly stores output.
///
/// Details:
/// - Simulates output being appended to log panel.
fn integration_preflight_exec_log_lines_append() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![pkg],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Simulate receiving output lines
    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        log_lines.push(":: Synchronizing package databases...".to_string());
        log_lines.push(" core is up to date".to_string());
        log_lines.push(" extra is up to date".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 3);
            assert_eq!(log_lines[0], ":: Synchronizing package databases...");
            assert_eq!(log_lines[1], " core is up to date");
            assert_eq!(log_lines[2], " extra is up to date");
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test PreflightExec progress bar replacement in log_lines.
///
/// Inputs:
/// - PreflightExec modal with progress bar updates.
///
/// Output:
/// - Last line is replaced for progress bar updates.
///
/// Details:
/// - Simulates ReplaceLastLine behavior for progress bars.
fn integration_preflight_exec_progress_bar_replacement() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![pkg],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec!["Downloading ripgrep...".to_string()],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Simulate progress bar updates (ReplaceLastLine behavior)
    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        // First progress update
        if !log_lines.is_empty() {
            log_lines.pop();
        }
        log_lines.push("[###-----] 25%".to_string());

        // Second progress update
        if !log_lines.is_empty() {
            log_lines.pop();
        }
        log_lines.push("[######--] 75%".to_string());

        // Final progress update
        if !log_lines.is_empty() {
            log_lines.pop();
        }
        log_lines.push("[########] 100%".to_string());
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            // Only the final progress bar should remain
            assert_eq!(log_lines.len(), 1);
            assert_eq!(log_lines[0], "[########] 100%");
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test large output handling in PreflightExec.
///
/// Inputs:
/// - PreflightExec modal with many output lines.
///
/// Output:
/// - All lines are stored correctly.
///
/// Details:
/// - Verifies handling of large output from package operations.
fn integration_preflight_exec_large_output() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let mut app = AppState {
        modal: Modal::PreflightExec {
            items: vec![pkg],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
        },
        ..Default::default()
    };

    // Simulate large output (e.g., verbose package list)
    if let Modal::PreflightExec {
        ref mut log_lines, ..
    } = app.modal
    {
        for i in 0..1000 {
            log_lines.push(format!("Processing file {i}/1000: /usr/lib/package/file{i}.so"));
        }
    }

    match app.modal {
        Modal::PreflightExec { log_lines, .. } => {
            assert_eq!(log_lines.len(), 1000);
            assert!(log_lines[0].contains("file 0"));
            assert!(log_lines[999].contains("file 999"));
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test ExecutorRequest::Install creation.
///
/// Inputs:
/// - Package items and optional password.
///
/// Output:
/// - ExecutorRequest::Install with correct fields.
///
/// Details:
/// - Verifies Install request can be created for executor.
fn integration_executor_request_install() {
    let items = vec![
        create_test_package(
            "ripgrep",
            Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
        ),
        create_test_package("yay-bin", Source::Aur),
    ];

    let request = ExecutorRequest::Install {
        items: items.clone(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Install {
            items,
            password,
            dry_run,
        } => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].name, "ripgrep");
            assert_eq!(items[1].name, "yay-bin");
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Install"),
    }
}

#[test]
/// What: Test ExecutorRequest::Remove creation.
///
/// Inputs:
/// - Package names, password, and cascade mode.
///
/// Output:
/// - ExecutorRequest::Remove with correct fields.
///
/// Details:
/// - Verifies Remove request can be created for executor.
fn integration_executor_request_remove() {
    use pacsea::state::modal::CascadeMode;

    let names = vec!["pkg1".to_string(), "pkg2".to_string()];

    let request = ExecutorRequest::Remove {
        names: names.clone(),
        password: Some("testpassword".to_string()),
        cascade: CascadeMode::Cascade,
        dry_run: false,
    };

    match request {
        ExecutorRequest::Remove {
            names,
            password,
            cascade,
            dry_run,
        } => {
            assert_eq!(names.len(), 2);
            assert_eq!(names[0], "pkg1");
            assert_eq!(password, Some("testpassword".to_string()));
            assert_eq!(cascade, CascadeMode::Cascade);
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Remove"),
    }
}

#[test]
/// What: Test ExecutorRequest::Downgrade creation.
///
/// Inputs:
/// - Package names and optional password.
///
/// Output:
/// - ExecutorRequest::Downgrade with correct fields.
///
/// Details:
/// - Verifies Downgrade request can be created for executor.
fn integration_executor_request_downgrade() {
    let names = vec!["pkg1".to_string()];

    let request = ExecutorRequest::Downgrade {
        names: names.clone(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Downgrade {
            names,
            password,
            dry_run,
        } => {
            assert_eq!(names.len(), 1);
            assert_eq!(names[0], "pkg1");
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test ExecutorRequest::CustomCommand creation.
///
/// Inputs:
/// - Command string and optional password.
///
/// Output:
/// - ExecutorRequest::CustomCommand with correct fields.
///
/// Details:
/// - Verifies CustomCommand request can be created for executor.
fn integration_executor_request_custom_command() {
    let request = ExecutorRequest::CustomCommand {
        command: "makepkg -si".to_string(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::CustomCommand {
            command,
            password,
            dry_run,
        } => {
            assert_eq!(command, "makepkg -si");
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::CustomCommand"),
    }
}

#[test]
/// What: Test ExecutorRequest::Update creation.
///
/// Inputs:
/// - Commands array and optional password.
///
/// Output:
/// - ExecutorRequest::Update with correct fields.
///
/// Details:
/// - Verifies Update request can be created for executor.
fn integration_executor_request_update() {
    let commands = vec![
        "sudo pacman -Syu --noconfirm".to_string(),
        "paru -Syu --noconfirm".to_string(),
    ];

    let request = ExecutorRequest::Update {
        commands: commands.clone(),
        password: Some("testpassword".to_string()),
        dry_run: false,
    };

    match request {
        ExecutorRequest::Update {
            commands,
            password,
            dry_run,
        } => {
            assert_eq!(commands.len(), 2);
            assert!(commands[0].contains("pacman"));
            assert!(commands[1].contains("paru"));
            assert_eq!(password, Some("testpassword".to_string()));
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Update"),
    }
}

#[test]
/// What: Test ExecutorRequest::Scan creation.
///
/// Inputs:
/// - Package name and scanner flags.
///
/// Output:
/// - ExecutorRequest::Scan with correct fields.
///
/// Details:
/// - Verifies Scan request can be created for executor.
fn integration_executor_request_scan() {
    let request = ExecutorRequest::Scan {
        package: "test-pkg".to_string(),
        do_clamav: true,
        do_trivy: true,
        do_semgrep: false,
        do_shellcheck: true,
        do_virustotal: false,
        do_custom: false,
        dry_run: false,
    };

    match request {
        ExecutorRequest::Scan {
            package,
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            dry_run,
        } => {
            assert_eq!(package, "test-pkg");
            assert!(do_clamav);
            assert!(do_trivy);
            assert!(!do_semgrep);
            assert!(do_shellcheck);
            assert!(!do_virustotal);
            assert!(!do_custom);
            assert!(!dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Scan"),
    }
}

#[test]
/// What: Test verbose mode in PreflightExec modal.
///
/// Inputs:
/// - PreflightExec modal with verbose=true.
///
/// Output:
/// - verbose flag is correctly set.
///
/// Details:
/// - Verifies verbose mode can be toggled for detailed output.
fn integration_preflight_exec_verbose_mode() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![pkg],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: true,
            log_lines: vec![],
            abortable: false,
            header_chips: PreflightHeaderChips::default(),
            success: None,
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
/// What: Test abortable flag in PreflightExec modal.
///
/// Inputs:
/// - PreflightExec modal with abortable=true.
///
/// Output:
/// - abortable flag is correctly set.
///
/// Details:
/// - Verifies abort capability can be enabled for long operations.
fn integration_preflight_exec_abortable() {
    let pkg = create_test_package(
        "ripgrep",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![pkg],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: true,
            header_chips: PreflightHeaderChips::default(),
            success: None,
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

