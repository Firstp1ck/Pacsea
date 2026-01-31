//! Integration tests for opening config files in the user's preferred editor (VISUAL/EDITOR).
//!
//! Verifies that the shell command built by `editor_open_config_command` respects VISUAL/EDITOR
//! and passes the config path as a single argument.

#![cfg(test)]
#![cfg(not(target_os = "windows"))]

use std::path::Path;
use std::process::Command;

/// What: When VISUAL is set to echo, running the editor command prints the config path to stdout.
///
/// Inputs:
/// - A test config path (e.g. `/tmp/editor_config_integration_test.conf`).
/// - VISUAL=echo in the environment.
///
/// Output:
/// - Running the command via bash -lc produces stdout that contains the path.
///
/// Details:
/// - Confirms that VISUAL is used and the path is passed as one argument.
/// - Does not spawn a real terminal; runs the command directly and captures output.
#[test]
fn integration_editor_config_visual_echo_prints_path() {
    let path = Path::new("/tmp/editor_config_integration_test.conf");
    let path_str = path.display().to_string();
    let cmd = pacsea::install::editor_open_config_command(path);

    let output = Command::new("bash")
        .arg("-lc")
        .arg(&cmd)
        .env("VISUAL", "echo")
        .env_remove("EDITOR")
        .output()
        .expect("bash -lc must run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(path_str.as_str()),
        "stdout should contain config path when VISUAL=echo; got: {stdout:?}"
    );
}

/// What: When EDITOR is set to echo (and VISUAL unset), running the editor command prints the path.
///
/// Inputs:
/// - A test config path.
/// - EDITOR=echo, VISUAL unset.
///
/// Output:
/// - Stdout contains the path.
///
/// Details:
/// - Confirms that EDITOR is used when VISUAL is not set.
#[test]
fn integration_editor_config_editor_echo_prints_path() {
    let path = Path::new("/tmp/editor_config_editor_fallback.conf");
    let path_str = path.display().to_string();
    let cmd = pacsea::install::editor_open_config_command(path);

    let output = Command::new("bash")
        .arg("-lc")
        .arg(&cmd)
        .env_remove("VISUAL")
        .env("EDITOR", "echo")
        .output()
        .expect("bash -lc must run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(path_str.as_str()),
        "stdout should contain config path when EDITOR=echo; got: {stdout:?}"
    );
}
