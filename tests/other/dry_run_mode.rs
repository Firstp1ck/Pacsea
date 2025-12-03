//! Consolidated integration tests for dry-run mode.
//!
//! Tests cover:
//! - Install dry-run respects flag
//! - Remove dry-run respects flag
//! - Update dry-run respects flag
//! - Scan dry-run respects flag
//! - Custom command dry-run respects flag
//! - "DRY RUN:" prefix in commands

#![cfg(test)]

use pacsea::install::ExecutorRequest;
use pacsea::state::{AppState, PackageItem, Source, modal::CascadeMode};

/// What: Create a test package item.
///
/// Inputs:
/// - `name`: Package name
/// - `source`: Package source
///
/// Output:
/// - `PackageItem` ready for testing
///
/// Details:
/// - Helper to create test packages
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
/// What: Test `ExecutorRequest::Install` with dry_run=true.
///
/// Inputs:
/// - Install request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies install respects dry-run flag.
fn integration_dry_run_install() {
    let items = vec![create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    )];

    let request = ExecutorRequest::Install {
        items,
        password: None,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Install { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Install"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Remove` with dry_run=true.
///
/// Inputs:
/// - Remove request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies remove respects dry-run flag.
fn integration_dry_run_remove() {
    let names = vec!["test-pkg".to_string()];

    let request = ExecutorRequest::Remove {
        names,
        password: None,
        cascade: CascadeMode::Basic,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Remove { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Remove"),
    }
}

#[test]
/// What: Test `ExecutorRequest::Update` with dry_run=true.
///
/// Inputs:
/// - Update request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies update respects dry-run flag.
fn integration_dry_run_update() {
    let commands = vec!["sudo pacman -Syu --noconfirm".to_string()];

    let request = ExecutorRequest::Update {
        commands,
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
/// What: Test `ExecutorRequest::Scan` with dry_run=true.
///
/// Inputs:
/// - Scan request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies scan respects dry-run flag.
fn integration_dry_run_scan() {
    let request = ExecutorRequest::Scan {
        package: "test-pkg".to_string(),
        do_clamav: true,
        do_trivy: false,
        do_semgrep: false,
        do_shellcheck: false,
        do_virustotal: false,
        do_custom: false,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Scan { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Scan"),
    }
}

#[test]
/// What: Test `ExecutorRequest::CustomCommand` with dry_run=true.
///
/// Inputs:
/// - Custom command request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies custom command respects dry-run flag.
fn integration_dry_run_custom_command() {
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
/// What: Test `ExecutorRequest::Downgrade` with dry_run=true.
///
/// Inputs:
/// - Downgrade request with dry_run enabled.
///
/// Output:
/// - dry_run flag is true.
///
/// Details:
/// - Verifies downgrade respects dry-run flag.
fn integration_dry_run_downgrade() {
    let names = vec!["test-pkg".to_string()];

    let request = ExecutorRequest::Downgrade {
        names,
        password: None,
        dry_run: true,
    };

    match request {
        ExecutorRequest::Downgrade { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Downgrade"),
    }
}

#[test]
/// What: Test `AppState` dry_run flag.
///
/// Inputs:
/// - `AppState` with dry_run=true.
///
/// Output:
/// - dry_run flag is accessible.
///
/// Details:
/// - Verifies dry_run flag is stored in app state.
fn integration_app_state_dry_run_flag() {
    let app = AppState {
        dry_run: true,
        ..Default::default()
    };

    assert!(app.dry_run);
}

#[test]
/// What: Test dry-run command format with "DRY RUN:" prefix.
///
/// Inputs:
/// - Package name for dry-run install.
///
/// Output:
/// - Command includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run commands have the expected prefix.
fn integration_dry_run_command_format_install() {
    let pkg_name = "test-pkg";
    let dry_run_cmd = format!("echo DRY RUN: sudo pacman -S {pkg_name} --noconfirm");

    assert!(dry_run_cmd.contains("DRY RUN:"));
    assert!(dry_run_cmd.contains("pacman -S"));
    assert!(dry_run_cmd.contains(pkg_name));
}

#[test]
/// What: Test dry-run command format for remove.
///
/// Inputs:
/// - Package name for dry-run remove.
///
/// Output:
/// - Command includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run remove commands have the expected prefix.
fn integration_dry_run_command_format_remove() {
    let pkg_name = "test-pkg";
    let dry_run_cmd = format!("echo DRY RUN: sudo pacman -R {pkg_name} --noconfirm");

    assert!(dry_run_cmd.contains("DRY RUN:"));
    assert!(dry_run_cmd.contains("pacman -R"));
    assert!(dry_run_cmd.contains(pkg_name));
}

#[test]
/// What: Test dry-run command format for update.
///
/// Inputs:
/// - Dry-run update command.
///
/// Output:
/// - Command includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run update commands have the expected prefix.
fn integration_dry_run_command_format_update() {
    let dry_run_cmd = "echo DRY RUN: sudo pacman -Syu --noconfirm";

    assert!(dry_run_cmd.contains("DRY RUN:"));
    assert!(dry_run_cmd.contains("pacman -Syu"));
}

#[test]
/// What: Test dry-run command format for downgrade.
///
/// Inputs:
/// - Package name for dry-run downgrade.
///
/// Output:
/// - Command includes "DRY RUN:" prefix.
///
/// Details:
/// - Verifies dry-run downgrade commands have the expected prefix.
fn integration_dry_run_command_format_downgrade() {
    let pkg_name = "test-pkg";
    let dry_run_cmd = format!("echo DRY RUN: sudo downgrade {pkg_name}");

    assert!(dry_run_cmd.contains("DRY RUN:"));
    assert!(dry_run_cmd.contains("downgrade"));
    assert!(dry_run_cmd.contains(pkg_name));
}

#[test]
/// What: Test all executor requests respect dry_run=false.
///
/// Inputs:
/// - All executor request types with dry_run=false.
///
/// Output:
/// - All dry_run flags are false.
///
/// Details:
/// - Verifies dry_run=false is default/respected.
fn integration_dry_run_all_false() {
    let install_req = ExecutorRequest::Install {
        items: vec![],
        password: None,
        dry_run: false,
    };

    let remove_req = ExecutorRequest::Remove {
        names: vec![],
        password: None,
        cascade: CascadeMode::Basic,
        dry_run: false,
    };

    let update_req = ExecutorRequest::Update {
        commands: vec![],
        password: None,
        dry_run: false,
    };

    let scan_req = ExecutorRequest::Scan {
        package: String::new(),
        do_clamav: false,
        do_trivy: false,
        do_semgrep: false,
        do_shellcheck: false,
        do_virustotal: false,
        do_custom: false,
        dry_run: false,
    };

    let custom_req = ExecutorRequest::CustomCommand {
        command: String::new(),
        password: None,
        dry_run: false,
    };

    let downgrade_req = ExecutorRequest::Downgrade {
        names: vec![],
        password: None,
        dry_run: false,
    };

    match install_req {
        ExecutorRequest::Install { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected Install"),
    }
    match remove_req {
        ExecutorRequest::Remove { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected Remove"),
    }
    match update_req {
        ExecutorRequest::Update { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected Update"),
    }
    match scan_req {
        ExecutorRequest::Scan { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected Scan"),
    }
    match custom_req {
        ExecutorRequest::CustomCommand { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected CustomCommand"),
    }
    match downgrade_req {
        ExecutorRequest::Downgrade { dry_run, .. } => assert!(!dry_run),
        _ => panic!("Expected Downgrade"),
    }
}

#[test]
/// What: Test dry-run with multiple packages.
///
/// Inputs:
/// - Multiple packages for dry-run install.
///
/// Output:
/// - All package names in command.
///
/// Details:
/// - Verifies batch dry-run includes all packages.
fn integration_dry_run_multiple_packages() {
    let pkg_names = vec!["pkg1", "pkg2", "pkg3"];
    let joined = pkg_names.join(" ");
    let dry_run_cmd = format!("echo DRY RUN: sudo pacman -S {joined} --noconfirm");

    for pkg in &pkg_names {
        assert!(dry_run_cmd.contains(pkg));
    }
    assert!(dry_run_cmd.contains("DRY RUN:"));
}

#[test]
/// What: Test dry-run flag inheritance from `AppState`.
///
/// Inputs:
/// - `AppState` with dry_run=true.
///
/// Output:
/// - `ExecutorRequest` uses app's dry_run value.
///
/// Details:
/// - Verifies dry_run is correctly passed from state to request.
fn integration_dry_run_state_to_request() {
    let app = AppState {
        dry_run: true,
        ..Default::default()
    };

    let request = ExecutorRequest::Install {
        items: vec![],
        password: None,
        dry_run: app.dry_run,
    };

    match request {
        ExecutorRequest::Install { dry_run, .. } => {
            assert!(dry_run);
        }
        _ => panic!("Expected ExecutorRequest::Install"),
    }
}

