//! Unit tests for sync_files function.

use super::sync;
use crate::state::AppState;
use crate::state::modal::{FileChange, FileChangeType, PackageFileInfo, PreflightTab};
use crate::state::{PackageItem, Source};

/// What: Test sync_files early return when not on Files tab.
///
/// Inputs:
/// - `tab`: PreflightTab::Summary
/// - `file_info`: Empty vector
///
/// Output:
/// - `file_info` remains unchanged
///
/// Details:
/// - Verifies that file sync is skipped when not on Files tab.
#[test]
fn test_sync_files_early_return_wrong_tab() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let tab = PreflightTab::Summary;
    let mut file_info = Vec::new();
    let mut file_selected = 0;

    sync::sync_files(&app, &items, &tab, &mut file_info, &mut file_selected);

    assert!(file_info.is_empty());
}

/// What: Test sync_files filters files by package name.
///
/// Inputs:
/// - `app`: AppState with cached file info
/// - `items`: Packages to filter by
///
/// Output:
/// - `file_info` contains only files for matching packages
///
/// Details:
/// - Verifies that file filtering works correctly.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_files_filters_by_package_name() {
    let mut app = AppState::default();
    app.install_list_files = vec![
        PackageFileInfo {
            name: "test-pkg".to_string(),
            files: vec![FileChange {
                path: "/usr/bin/test".to_string(),
                change_type: FileChangeType::New,
                package: "test-pkg".to_string(),
                is_config: false,
                predicted_pacnew: false,
                predicted_pacsave: false,
            }],
            total_count: 1,
            new_count: 1,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        },
        PackageFileInfo {
            name: "other-pkg".to_string(),
            files: vec![],
            total_count: 0,
            new_count: 0,
            changed_count: 0,
            removed_count: 0,
            config_count: 0,
            pacnew_candidates: 0,
            pacsave_candidates: 0,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let tab = PreflightTab::Files;
    let mut file_info = Vec::new();
    let mut file_selected = 0;

    sync::sync_files(&app, &items, &tab, &mut file_info, &mut file_selected);

    assert_eq!(file_info.len(), 1);
    assert_eq!(file_info[0].name, "test-pkg");
    assert_eq!(file_selected, 0);
}

/// What: Test sync_files resets selection when files change.
///
/// Inputs:
/// - `file_info`: Empty
/// - `file_selected`: 5
/// - Cache has files
///
/// Output:
/// - `file_selected` is reset to 0
///
/// Details:
/// - Verifies that selection is reset when files are synced.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_files_resets_selection_when_files_change() {
    let mut app = AppState::default();
    app.install_list_files = vec![PackageFileInfo {
        name: "test-pkg".to_string(),
        files: vec![],
        total_count: 0,
        new_count: 0,
        changed_count: 0,
        removed_count: 0,
        config_count: 0,
        pacnew_candidates: 0,
        pacsave_candidates: 0,
    }];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let tab = PreflightTab::Files;
    let mut file_info = Vec::new();
    let mut file_selected = 5;

    sync::sync_files(&app, &items, &tab, &mut file_info, &mut file_selected);

    assert_eq!(file_selected, 0);
}
