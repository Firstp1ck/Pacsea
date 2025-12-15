//! Unit tests for system update modal handlers.

use crate::state::AppState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::handle_system_update;

#[test]
/// What: Verify `SystemUpdate` modal handles Esc to close.
///
/// Inputs:
/// - `SystemUpdate` modal, Esc key event.
///
/// Output:
/// - Modal is closed.
///
/// Details:
/// - Tests that Esc closes the `SystemUpdate` modal.
fn system_update_esc_closes_modal() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: false,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = false;
    let mut force_sync = false;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    match app.modal {
        crate::state::Modal::None => {}
        _ => panic!("Expected modal to be closed"),
    }
}

#[test]
/// What: Verify `SystemUpdate` modal handles navigation.
///
/// Inputs:
/// - `SystemUpdate` modal, Down key event.
///
/// Output:
/// - Cursor moves down.
///
/// Details:
/// - Tests that navigation keys work in `SystemUpdate` modal.
fn system_update_navigation() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: false,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = false;
    let mut force_sync = false;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Down, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    assert_eq!(cursor, 1, "Cursor should move down");
}

#[test]
/// What: Verify `SystemUpdate` modal handles toggle with Space.
///
/// Inputs:
/// - `SystemUpdate` modal, Space key event on first option.
///
/// Output:
/// - `do_mirrors` flag is toggled.
///
/// Details:
/// - Tests that Space toggles options in `SystemUpdate` modal.
fn system_update_toggle() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: false,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = false;
    let mut force_sync = false;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    assert!(do_mirrors, "do_mirrors should be toggled to true");
}

#[test]
/// What: Verify `SystemUpdate` modal handles Enter to show password prompt.
///
/// Inputs:
/// - `SystemUpdate` modal with options selected, Enter key event.
///
/// Output:
/// - Transitions to `PasswordPrompt` modal with pending update commands.
///
/// Details:
/// - Tests that Enter triggers password prompt before system update execution.
/// - Verifies that `pending_update_commands` is set with the update commands.
fn system_update_enter_executes() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
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
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = true;
    let mut force_sync = false;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let result = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    // Should return Some(true) when Enter triggers password prompt
    assert_eq!(result, Some(true));
    // Modal should transition to PasswordPrompt
    match &app.modal {
        crate::state::Modal::PasswordPrompt { purpose, .. } => {
            assert!(
                matches!(purpose, crate::state::modal::PasswordPurpose::Update),
                "Password purpose should be Update"
            );
        }
        _ => panic!("Expected modal to transition to PasswordPrompt"),
    }
    // Verify that pending_update_commands is set
    assert!(
        app.pending_update_commands.is_some(),
        "System update should set pending_update_commands"
    );
    // Verify the commands include pacman update with normal sync (-Syu)
    let commands = app
        .pending_update_commands
        .as_ref()
        .expect("pending_update_commands should be set");
    assert!(
        commands.iter().any(|c| c.contains("pacman -Syu")),
        "Commands should include pacman -Syu for normal sync"
    );
}

#[test]
/// What: Verify force sync option uses `-Syyu` instead of `-Syu`.
///
/// Inputs:
/// - `SystemUpdate` modal with `force_sync` enabled, Enter key event.
///
/// Output:
/// - Commands use `-Syyu` flag.
///
/// Details:
/// - Tests that enabling force sync uses the force database refresh flag.
fn system_update_force_sync_uses_syyu() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: true,
            force_sync: true,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = true;
    let mut force_sync = true;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    // Verify the commands use -Syyu (force sync)
    let commands = app
        .pending_update_commands
        .as_ref()
        .expect("pending_update_commands should be set");
    assert!(
        commands.iter().any(|c| c.contains("-Syyu")),
        "Commands should include -Syyu for force sync"
    );
    assert!(
        !commands
            .iter()
            .any(|c| c.contains("-Syu --noconfirm") && !c.contains("-Syyu")),
        "Commands should not include plain -Syu when force sync is enabled"
    );
}

#[test]
/// What: Verify AUR update uses `-Sua` when pacman is also selected.
///
/// Inputs:
/// - `SystemUpdate` modal with both `do_pacman` and `do_aur` enabled, Enter key event.
///
/// Output:
/// - AUR command uses `-Sua` flag (AUR only, since official packages are updated by pacman).
///
/// Details:
/// - Tests that when both pacman and AUR updates are selected, AUR uses `-Sua` to avoid redundant official package updates.
fn system_update_aur_uses_sua_when_pacman_selected() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: true,
            force_sync: false,
            do_aur: true,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = true;
    let mut force_sync = false;
    let mut do_aur = true;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    // Verify the commands include pacman -Syu
    let commands = app
        .pending_update_commands
        .as_ref()
        .expect("pending_update_commands should be set");
    assert!(
        commands.iter().any(|c| c.contains("pacman -Syu")),
        "Commands should include pacman -Syu"
    );
    // Verify AUR command is stored separately and uses -Sua (not -Syu) when pacman is also selected
    let aur_command = app
        .pending_aur_update_command
        .as_ref()
        .expect("pending_aur_update_command should be set when both pacman and AUR are selected");
    assert!(
        aur_command.contains("paru -Sua") || aur_command.contains("yay -Sua"),
        "AUR command should use -Sua when pacman is also selected"
    );
    assert!(
        !aur_command.contains("paru -Syu") && !aur_command.contains("yay -Syu"),
        "AUR command should not use -Syu when pacman is also selected"
    );
    // Verify AUR command is NOT in the main command list
    assert!(
        !commands
            .iter()
            .any(|c| c.contains("paru") || c.contains("yay")),
        "AUR command should not be in pending_update_commands when pacman is also selected"
    );
}

#[test]
/// What: Verify AUR update uses `-Sua` when only AUR is selected (no pacman).
///
/// Inputs:
/// - `SystemUpdate` modal with only `do_aur` enabled (no pacman), Enter key event.
///
/// Output:
/// - AUR command uses `-Sua` flag (updates only AUR packages).
///
/// Details:
/// - Tests that when only AUR update is selected, AUR uses `-Sua` to update only AUR packages.
/// - AUR helpers will handle dependency resolution and report if newer official packages are needed.
fn system_update_aur_uses_sua_when_only_aur_selected() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: false,
            force_sync: false,
            do_aur: true,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 0,
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = false;
    let mut force_sync = false;
    let mut do_aur = true;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 0;

    let ke = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    // Verify AUR command uses -Sua (not -Syu) when only AUR is selected
    let commands = app
        .pending_update_commands
        .as_ref()
        .expect("pending_update_commands should be set");
    assert!(
        commands
            .iter()
            .any(|c| c.contains("paru -Sua") || c.contains("yay -Sua")),
        "AUR command should use -Sua when only AUR is selected"
    );
    assert!(
        !commands
            .iter()
            .any(|c| (c.contains("paru -Syu") || c.contains("yay -Syu")) && !c.contains("-Sua")),
        "AUR command should not use -Syu when only AUR is selected"
    );
}

#[test]
/// What: Verify left/right/tab keys toggle `force_sync` on pacman row.
///
/// Inputs:
/// - `SystemUpdate` modal on cursor row 1 (pacman), Left/Right/Tab key event.
///
/// Output:
/// - `force_sync` is toggled.
///
/// Details:
/// - Tests that Left/Right/Tab on pacman row toggles sync mode.
fn system_update_left_right_toggles_force_sync() {
    let mut app = AppState {
        modal: crate::state::Modal::SystemUpdate {
            do_mirrors: false,
            do_pacman: true,
            force_sync: false,
            do_aur: false,
            do_cache: false,
            country_idx: 0,
            countries: vec!["Worldwide".to_string()],
            mirror_count: 10,
            cursor: 1, // Pacman row
        },
        ..Default::default()
    };

    let mut do_mirrors = false;
    let mut do_pacman = true;
    let mut force_sync = false;
    let mut do_aur = false;
    let mut do_cache = false;
    let mut country_idx = 0;
    let countries = vec!["Worldwide".to_string()];
    let mut mirror_count = 10;
    let mut cursor = 1; // Pacman row

    // Press Right to toggle force_sync to true
    let ke = KeyEvent::new(KeyCode::Right, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    assert!(
        force_sync,
        "force_sync should be toggled to true with Right"
    );

    // Press Left to toggle force_sync back to false
    let ke = KeyEvent::new(KeyCode::Left, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    assert!(
        !force_sync,
        "force_sync should be toggled back to false with Left"
    );

    // Press Tab to toggle force_sync to true
    let ke = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    let _ = handle_system_update(
        ke,
        &mut app,
        &mut do_mirrors,
        &mut do_pacman,
        &mut force_sync,
        &mut do_aur,
        &mut do_cache,
        &mut country_idx,
        &countries,
        &mut mirror_count,
        &mut cursor,
    );

    assert!(force_sync, "force_sync should be toggled to true with Tab");
}
