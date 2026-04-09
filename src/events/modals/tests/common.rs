//! Common test utilities for modal key event handling tests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

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
#[allow(clippy::too_many_lines)] // Exhaustive modal fixture list for keybind-blocking regression tests
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
            crate::state::Modal::WarnAurRepoDuplicate {
                dup_names: vec!["duppkg".to_string()],
                packages: vec![],
                header_chips: PreflightHeaderChips::default(),
            },
            "WarnAurRepoDuplicate",
        ),
        (
            crate::state::Modal::ForeignRepoOverlap {
                repo_name: "extra".to_string(),
                entries: vec![],
                phase: crate::state::modal::ForeignRepoOverlapPhase::WarnAck {
                    step: 0,
                    list_scroll: 0,
                },
            },
            "ForeignRepoOverlap",
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
                filter_active: false,
                filter_query: String::new(),
                filter_caret: 0,
                last_selected_pkg_name: None,
                filtered_indices: vec![0],
                selected_pkg_names: std::collections::HashSet::new(),
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
                input: crate::state::SecureString::default(),
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

#[test]
/// What: Verify updates jump keys are no-ops for an empty updates list.
///
/// Inputs:
/// - Empty `Updates` modal and `Home`/`End`/`g`/`G` key events.
///
/// Output:
/// - Handler preserves a valid empty selection state.
///
/// Details:
/// - Guards edge cases where navigation parity keys are used before data is present.
fn updates_navigation_keys_are_safe_when_empty() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Updates {
        entries: vec![],
        scroll: 0,
        selected: 0,
        filter_active: false,
        filter_query: String::new(),
        filter_caret: 0,
        last_selected_pkg_name: None,
        filtered_indices: vec![],
        selected_pkg_names: std::collections::HashSet::new(),
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel();

    for key in [
        key_event(KeyCode::Home, KeyModifiers::empty()),
        key_event(KeyCode::End, KeyModifiers::empty()),
        key_event(KeyCode::Char('g'), KeyModifiers::empty()),
        key_event(KeyCode::Char('g'), KeyModifiers::empty()),
        key_event(KeyCode::Char('G'), KeyModifiers::empty()),
    ] {
        super::handle_modal_key(key, &mut app, &add_tx);
    }

    match &app.modal {
        crate::state::Modal::Updates {
            entries,
            selected,
            filtered_indices,
            ..
        } => {
            assert!(entries.is_empty());
            assert_eq!(*selected, 0);
            assert!(filtered_indices.is_empty());
        }
        _ => panic!("Expected Updates modal to remain active"),
    }
}

#[test]
/// What: Verify `a` selects only visible filtered rows in updates modal.
///
/// Inputs:
/// - Updates modal with three entries and a filtered subset of two indices.
///
/// Output:
/// - Selected package-name set contains only the filtered subset names.
///
/// Details:
/// - Ensures batch-selection respects current filtered visibility.
fn updates_select_all_respects_filtered_subset() {
    let mut app = new_app();
    app.modal = crate::state::Modal::Updates {
        entries: vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
            ("gamma".to_string(), "1".to_string(), "2".to_string()),
        ],
        scroll: 0,
        selected: 0,
        filter_active: false,
        filter_query: String::new(),
        filter_caret: 0,
        last_selected_pkg_name: None,
        filtered_indices: vec![0, 2],
        selected_pkg_names: std::collections::HashSet::new(),
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel();

    super::handle_modal_key(
        key_event(KeyCode::Char('a'), KeyModifiers::empty()),
        &mut app,
        &add_tx,
    );

    match &app.modal {
        crate::state::Modal::Updates {
            selected_pkg_names, ..
        } => {
            assert!(selected_pkg_names.contains("alpha"));
            assert!(selected_pkg_names.contains("gamma"));
            assert!(!selected_pkg_names.contains("beta"));
        }
        _ => panic!("Expected Updates modal to remain active"),
    }
}

#[test]
/// What: Verify pressing Enter in updates modal transitions away from updates state.
///
/// Inputs:
/// - Updates modal with one selected package-name and Enter key event.
///
/// Output:
/// - Modal transitions to a non-`Updates` state.
///
/// Details:
/// - Intentionally avoids over-constraining internal package-selection precedence semantics.
fn updates_enter_transitions_to_next_modal_state() {
    let mut app = new_app();
    let mut selected_pkg_names = std::collections::HashSet::new();
    selected_pkg_names.insert("alpha".to_string());
    app.modal = crate::state::Modal::Updates {
        entries: vec![
            ("alpha".to_string(), "1".to_string(), "2".to_string()),
            ("beta".to_string(), "1".to_string(), "2".to_string()),
        ],
        scroll: 0,
        selected: 0,
        filter_active: false,
        filter_query: String::new(),
        filter_caret: 0,
        last_selected_pkg_name: Some("alpha".to_string()),
        filtered_indices: vec![0, 1],
        selected_pkg_names,
    };
    let (add_tx, _add_rx) = mpsc::unbounded_channel();

    let consumed = super::handle_modal_key(
        key_event(KeyCode::Enter, KeyModifiers::empty()),
        &mut app,
        &add_tx,
    );

    assert!(consumed);
    assert!(!matches!(app.modal, crate::state::Modal::Updates { .. }));
}
