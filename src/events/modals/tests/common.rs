//! Common test utilities for modal key event handling tests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::state::{AppState, PreflightAction, PreflightTab, modal::PreflightHeaderChips};

/// What: Create a baseline `AppState` for modal tests.
///
/// Inputs:
/// - None
///
/// Output:
/// - Fresh `AppState` ready for modal testing
///
/// Details:
/// - Provides a clean starting state for each test case
pub(super) fn new_app() -> AppState {
    AppState::default()
}

/// What: Create a key event with Press kind.
///
/// Inputs:
/// - `code`: Key code
/// - `modifiers`: Key modifiers
///
/// Output:
/// - `KeyEvent` with Press kind
pub(super) fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    let mut ke = KeyEvent::new(code, modifiers);
    ke.kind = crossterm::event::KeyEventKind::Press;
    ke
}

/// What: Create test modals for global keybind blocking tests.
///
/// Inputs:
/// - None
///
/// Output:
/// - Vector of (modal, name) tuples for testing
///
/// Details:
/// - Returns all modals that should block global keybinds (excludes None and Preflight)
pub(super) fn create_test_modals() -> Vec<(crate::state::Modal, &'static str)> {
    vec![
        (
            crate::state::Modal::Alert {
                message: "Test alert".to_string(),
            },
            "Alert",
        ),
        (
            crate::state::Modal::Loading {
                message: "Loading...".to_string(),
            },
            "Loading",
        ),
        (
            crate::state::Modal::ConfirmInstall { items: vec![] },
            "ConfirmInstall",
        ),
        (
            crate::state::Modal::ConfirmReinstall {
                items: vec![],
                all_items: vec![],
                header_chips: PreflightHeaderChips::default(),
            },
            "ConfirmReinstall",
        ),
        (
            crate::state::Modal::ConfirmBatchUpdate {
                items: vec![],
                dry_run: false,
            },
            "ConfirmBatchUpdate",
        ),
        (
            crate::state::Modal::PreflightExec {
                items: vec![],
                action: PreflightAction::Install,
                tab: PreflightTab::Summary,
                verbose: false,
                log_lines: vec![],
                abortable: false,
                header_chips: PreflightHeaderChips::default(),
                success: None,
            },
            "PreflightExec",
        ),
        (
            crate::state::Modal::PostSummary {
                success: true,
                changed_files: 0,
                pacnew_count: 0,
                pacsave_count: 0,
                services_pending: vec![],
                snapshot_label: None,
            },
            "PostSummary",
        ),
        (crate::state::Modal::Help, "Help"),
        (
            crate::state::Modal::ConfirmRemove { items: vec![] },
            "ConfirmRemove",
        ),
        (
            crate::state::Modal::SystemUpdate {
                do_mirrors: false,
                do_pacman: false,
                force_sync: false,
                do_aur: false,
                do_cache: false,
                country_idx: 0,
                countries: vec!["US".to_string()],
                mirror_count: 10,
                cursor: 0,
            },
            "SystemUpdate",
        ),
        (
            crate::state::Modal::News {
                items: vec![crate::state::types::NewsFeedItem {
                    id: "https://example.com".to_string(),
                    date: "2025-01-01".to_string(),
                    title: "Test".to_string(),
                    summary: None,
                    url: Some("https://example.com".to_string()),
                    source: crate::state::types::NewsFeedSource::ArchNews,
                    severity: None,
                    packages: Vec::new(),
                }],
                selected: 0,
                scroll: 0,
            },
            "News",
        ),
        (
            crate::state::Modal::Updates {
                entries: vec![("pkg".to_string(), "1.0".to_string(), "2.0".to_string())],
                scroll: 0,
                selected: 0,
            },
            "Updates",
        ),
        (
            crate::state::Modal::OptionalDeps {
                rows: vec![],
                selected: 0,
            },
            "OptionalDeps",
        ),
        (
            crate::state::Modal::ScanConfig {
                do_clamav: false,
                do_trivy: false,
                do_semgrep: false,
                do_shellcheck: false,
                do_virustotal: false,
                do_custom: false,
                do_sleuth: false,
                cursor: 0,
            },
            "ScanConfig",
        ),
        (
            crate::state::Modal::VirusTotalSetup {
                input: String::new(),
                cursor: 0,
            },
            "VirusTotalSetup",
        ),
        (
            crate::state::Modal::PasswordPrompt {
                purpose: crate::state::modal::PasswordPurpose::Install,
                items: vec![],
                input: String::new(),
                cursor: 0,
                error: None,
            },
            "PasswordPrompt",
        ),
        (
            crate::state::Modal::GnomeTerminalPrompt,
            "GnomeTerminalPrompt",
        ),
        (crate::state::Modal::ImportHelp, "ImportHelp"),
    ]
}
