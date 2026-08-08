//! End-to-end shell checks for migrated toolkit AUR install plans.

#![cfg(all(test, unix))]

use std::ffi::OsString;
use std::fs::{OpenOptions, read_to_string};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::MutexGuard;

use pacsea::install::build_install_command_for_executor;
use pacsea::state::{PackageItem, Source};

/// What: Temporarily replace `PATH` while holding the shared environment lock.
///
/// Inputs:
/// - `path`: Directory containing controlled command probes.
///
/// Output:
/// - A guard that restores the original process environment on drop.
///
/// Details:
/// - Prevents generated commands from reaching host package-management tools.
struct PathGuard {
    /// Original process path restored when the guard drops.
    original: Option<OsString>,
    /// Shared environment lock held for the full override lifetime.
    _env_lock: MutexGuard<'static, ()>,
}

impl PathGuard {
    /// What: Install a controlled process path for one end-to-end test.
    ///
    /// Inputs:
    /// - `path`: Temporary directory containing probe executables.
    ///
    /// Output:
    /// - Active guard holding the global environment lock.
    ///
    /// Details:
    /// - The original path is restored even when the test panics.
    fn new(path: &Path) -> Self {
        let env_lock = crate::env_guard::acquire();
        let original = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", path);
        }
        Self {
            original,
            _env_lock: env_lock,
        }
    }
}

impl Drop for PathGuard {
    /// What: Restore the original process path.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Process environment side effect only.
    ///
    /// Details:
    /// - Restoration occurs before the shared environment lock is released.
    fn drop(&mut self) {
        unsafe {
            if let Some(path) = self.original.as_ref() {
                std::env::set_var("PATH", path);
            } else {
                std::env::remove_var("PATH");
            }
        }
    }
}

/// What: Create a controlled helper executable that records its argv when executed.
///
/// Inputs:
/// - `path`: New executable path inside a temporary directory.
///
/// Output:
/// - An executable shell probe writing its path and arguments to `PACSEA_E2E_MARKER`.
///
/// Details:
/// - `create_new` and mode `0o700` avoid symlink-following races in temporary files.
fn write_helper_probe(path: &Path) {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(path)
        .expect("probe should be created atomically");
    file.write_all(b"#!/bin/sh\nprintf '%s|%s' \"$0\" \"$*\" > \"$PACSEA_E2E_MARKER\"\n")
        .expect("probe should be written");
}

/// What: Create the AUR package fixture used by shell-plan tests.
///
/// Inputs: None.
///
/// Output:
/// - A stable `yay-bin` package row.
///
/// Details:
/// - AUR-only plans avoid privilege-tool resolution and remain fully controlled by `PATH`.
fn aur_item() -> PackageItem {
    PackageItem {
        name: "yay-bin".to_string(),
        version: String::new(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

/// What: Build and execute one AUR plan under a controlled command path.
///
/// Inputs:
/// - `bin_dir`: Directory containing zero or more helper probes.
/// - `marker`: Path receiving probe argv if a helper executes.
/// - `dry_run`: Whether Pacsea should render rather than execute the toolkit plan.
///
/// Output:
/// - Captured `/bin/sh` process output.
///
/// Details:
/// - The path override remains active through shell completion, then is dropped immediately.
fn run_aur_plan(bin_dir: &Path, marker: &Path, dry_run: bool) -> Output {
    let path_guard = PathGuard::new(bin_dir);
    let command = build_install_command_for_executor(&[aur_item()], None, dry_run)
        .expect("AUR command should build");
    let output = Command::new("/bin/sh")
        .arg("-c")
        .arg(&command)
        .env("PACSEA_E2E_MARKER", marker)
        .output()
        .expect("controlled shell should execute");
    drop(path_guard);
    output
}

/// What: Read the helper marker written by a controlled runtime plan.
///
/// Inputs:
/// - `path`: Marker path expected to exist.
///
/// Output:
/// - Recorded helper executable path and argv.
///
/// Details:
/// - Keeps runtime assertions concise and reports missing marker files clearly.
fn helper_record(path: &Path) -> String {
    read_to_string(path).expect("selected helper should record its arguments")
}

/// What: Verify an AUR dry-run plan can be executed without invoking package helpers.
///
/// Inputs:
/// - Controlled paru/yay probes, one AUR package, and executor dry-run mode.
///
/// Output:
/// - Successful shell output describing the plan with no execution marker created.
///
/// Details:
/// - Exercises package classification, toolkit planning, dry-run quoting, and a real shell parse.
#[test]
fn aur_dry_run_shell_never_executes_toolkit_helper_plan() {
    let temp = tempfile::tempdir().expect("temporary directory should exist");
    write_helper_probe(&temp.path().join("paru"));
    write_helper_probe(&temp.path().join("yay"));
    let marker = temp.path().join("helper-executed");

    let output = run_aur_plan(temp.path(), &marker, true);
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "dry-run shell failed: {stdout}");
    assert!(stdout.contains("DRY RUN:"));
    assert!(stdout.contains("paru -S --aur --needed --noconfirm -- yay-bin"));
    assert!(stdout.contains("exit 127"));
    assert!(
        !marker.exists(),
        "dry-run unexpectedly executed an AUR helper"
    );
}

/// What: Verify a non-dry AUR plan prefers paru when both supported helpers exist.
///
/// Inputs:
/// - Controlled paru and yay probes plus one AUR package.
///
/// Output:
/// - Successful execution recorded by paru with exact toolkit arguments.
///
/// Details:
/// - Validates real shell branching rather than relying on command substrings.
#[test]
fn aur_runtime_plan_prefers_paru_and_preserves_arguments() {
    let temp = tempfile::tempdir().expect("temporary directory should exist");
    write_helper_probe(&temp.path().join("paru"));
    write_helper_probe(&temp.path().join("yay"));
    let marker = temp.path().join("helper-executed");

    let output = run_aur_plan(temp.path(), &marker, false);
    let record = helper_record(&marker);

    assert!(output.status.success());
    assert!(record.ends_with("paru|-S --aur --needed --noconfirm -- yay-bin"));
}

/// What: Verify a non-dry AUR plan falls back to yay when paru is unavailable.
///
/// Inputs:
/// - A controlled yay probe with no paru executable.
///
/// Output:
/// - Successful execution recorded by yay with exact toolkit arguments.
///
/// Details:
/// - Protects the second helper branch in the generated shell plan.
#[test]
fn aur_runtime_plan_falls_back_to_yay() {
    let temp = tempfile::tempdir().expect("temporary directory should exist");
    write_helper_probe(&temp.path().join("yay"));
    let marker = temp.path().join("helper-executed");

    let output = run_aur_plan(temp.path(), &marker, false);
    let record = helper_record(&marker);

    assert!(output.status.success());
    assert!(record.ends_with("yay|-S --aur --needed --noconfirm -- yay-bin"));
}

/// What: Verify a non-dry AUR plan fails clearly when no supported helper exists.
///
/// Inputs:
/// - An empty controlled command path and one AUR package.
///
/// Output:
/// - Exit status 127, an actionable stderr message, and no helper marker.
///
/// Details:
/// - Locks the v0.3.0 failure contract so missing helpers cannot be reported as success.
#[test]
fn aur_runtime_plan_reports_missing_helper_as_failure() {
    let temp = tempfile::tempdir().expect("temporary directory should exist");
    let marker = temp.path().join("helper-executed");

    let output = run_aur_plan(temp.path(), &marker, false);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(output.status.code(), Some(127));
    assert!(stderr.contains("No AUR helper (paru/yay) found."));
    assert!(!marker.exists());
}
