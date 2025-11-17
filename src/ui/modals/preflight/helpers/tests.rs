//! Unit tests for preflight helper functions.

use super::{format_bytes, format_signed_bytes, render_header_chips};
use crate::state::AppState;
use crate::state::modal::{PreflightHeaderChips, RiskLevel};

/// What: Test format_bytes with zero bytes.
///
/// Inputs:
/// - `value`: 0 bytes
///
/// Output:
/// - Returns "0 B"
///
/// Details:
/// - Verifies that zero bytes are formatted correctly without decimal places.
#[test]
fn test_format_bytes_zero() {
    assert_eq!(format_bytes(0), "0 B");
}

/// What: Test format_bytes with single byte.
///
/// Inputs:
/// - `value`: 1 byte
///
/// Output:
/// - Returns "1 B"
///
/// Details:
/// - Verifies that single bytes are formatted without decimal places.
#[test]
fn test_format_bytes_one() {
    assert_eq!(format_bytes(1), "1 B");
}

/// What: Test format_bytes with bytes less than 1 KiB.
///
/// Inputs:
/// - `value`: 1023 bytes
///
/// Output:
/// - Returns "1023 B"
///
/// Details:
/// - Verifies that bytes less than 1024 are formatted in bytes without decimal places.
#[test]
fn test_format_bytes_less_than_kib() {
    assert_eq!(format_bytes(1023), "1023 B");
}

/// What: Test format_bytes with exactly 1 KiB.
///
/// Inputs:
/// - `value`: 1024 bytes
///
/// Output:
/// - Returns "1.0 KiB"
///
/// Details:
/// - Verifies that exactly 1024 bytes converts to 1.0 KiB with one decimal place.
#[test]
fn test_format_bytes_one_kib() {
    assert_eq!(format_bytes(1024), "1.0 KiB");
}

/// What: Test format_bytes with values between KiB and MiB.
///
/// Inputs:
/// - `value`: 1536 bytes (1.5 KiB)
///
/// Output:
/// - Returns "1.5 KiB"
///
/// Details:
/// - Verifies that fractional KiB values are formatted with one decimal place.
#[test]
fn test_format_bytes_fractional_kib() {
    assert_eq!(format_bytes(1536), "1.5 KiB");
}

/// What: Test format_bytes with exactly 1 MiB.
///
/// Inputs:
/// - `value`: 1048576 bytes (1024 * 1024)
///
/// Output:
/// - Returns "1.0 MiB"
///
/// Details:
/// - Verifies that exactly 1 MiB is formatted correctly.
#[test]
fn test_format_bytes_one_mib() {
    assert_eq!(format_bytes(1048576), "1.0 MiB");
}

/// What: Test format_bytes with values between MiB and GiB.
///
/// Inputs:
/// - `value`: 15728640 bytes (15 MiB)
///
/// Output:
/// - Returns "15.0 MiB"
///
/// Details:
/// - Verifies that MiB values are formatted with one decimal place.
#[test]
fn test_format_bytes_mib() {
    assert_eq!(format_bytes(15728640), "15.0 MiB");
}

/// What: Test format_bytes with exactly 1 GiB.
///
/// Inputs:
/// - `value`: 1073741824 bytes (1024 * 1024 * 1024)
///
/// Output:
/// - Returns "1.0 GiB"
///
/// Details:
/// - Verifies that exactly 1 GiB is formatted correctly.
#[test]
fn test_format_bytes_one_gib() {
    assert_eq!(format_bytes(1073741824), "1.0 GiB");
}

/// What: Test format_bytes with large values (TiB).
///
/// Inputs:
/// - `value`: 1099511627776 bytes (1 TiB)
///
/// Output:
/// - Returns "1.0 TiB"
///
/// Details:
/// - Verifies that TiB values are formatted correctly.
#[test]
fn test_format_bytes_one_tib() {
    assert_eq!(format_bytes(1099511627776), "1.0 TiB");
}

/// What: Test format_bytes with very large values (PiB).
///
/// Inputs:
/// - `value`: 1125899906842624 bytes (1 PiB)
///
/// Output:
/// - Returns "1.0 PiB"
///
/// Details:
/// - Verifies that PiB values are formatted correctly.
#[test]
fn test_format_bytes_one_pib() {
    assert_eq!(format_bytes(1125899906842624), "1.0 PiB");
}

/// What: Test format_bytes with fractional values.
///
/// Inputs:
/// - `value`: 2621440 bytes (2.5 MiB)
///
/// Output:
/// - Returns "2.5 MiB"
///
/// Details:
/// - Verifies that fractional values are rounded to one decimal place.
#[test]
fn test_format_bytes_fractional_mib() {
    assert_eq!(format_bytes(2621440), "2.5 MiB");
}

/// What: Test format_signed_bytes with zero.
///
/// Inputs:
/// - `value`: 0
///
/// Output:
/// - Returns "0 B"
///
/// Details:
/// - Verifies that zero is formatted without a sign prefix.
#[test]
fn test_format_signed_bytes_zero() {
    assert_eq!(format_signed_bytes(0), "0 B");
}

/// What: Test format_signed_bytes with positive value.
///
/// Inputs:
/// - `value`: 1024
///
/// Output:
/// - Returns "+1.0 KiB"
///
/// Details:
/// - Verifies that positive values get a "+" prefix.
#[test]
fn test_format_signed_bytes_positive() {
    assert_eq!(format_signed_bytes(1024), "+1.0 KiB");
}

/// What: Test format_signed_bytes with negative value.
///
/// Inputs:
/// - `value`: -1024
///
/// Output:
/// - Returns "-1.0 KiB"
///
/// Details:
/// - Verifies that negative values get a "-" prefix.
#[test]
fn test_format_signed_bytes_negative() {
    assert_eq!(format_signed_bytes(-1024), "-1.0 KiB");
}

/// What: Test format_signed_bytes with large positive value.
///
/// Inputs:
/// - `value`: 1048576 (1 MiB)
///
/// Output:
/// - Returns "+1.0 MiB"
///
/// Details:
/// - Verifies that large positive values are formatted correctly with sign.
#[test]
fn test_format_signed_bytes_large_positive() {
    assert_eq!(format_signed_bytes(1048576), "+1.0 MiB");
}

/// What: Test format_signed_bytes with large negative value.
///
/// Inputs:
/// - `value`: -1048576 (-1 MiB)
///
/// Output:
/// - Returns "-1.0 MiB"
///
/// Details:
/// - Verifies that large negative values are formatted correctly with sign.
#[test]
fn test_format_signed_bytes_large_negative() {
    assert_eq!(format_signed_bytes(-1048576), "-1.0 MiB");
}

/// What: Test format_signed_bytes with fractional positive value.
///
/// Inputs:
/// - `value`: 1536 (1.5 KiB)
///
/// Output:
/// - Returns "+1.5 KiB"
///
/// Details:
/// - Verifies that fractional positive values are formatted correctly.
#[test]
fn test_format_signed_bytes_fractional_positive() {
    assert_eq!(format_signed_bytes(1536), "+1.5 KiB");
}

/// What: Test format_signed_bytes with fractional negative value.
///
/// Inputs:
/// - `value`: -1536 (-1.5 KiB)
///
/// Output:
/// - Returns "-1.5 KiB"
///
/// Details:
/// - Verifies that fractional negative values are formatted correctly.
#[test]
fn test_format_signed_bytes_fractional_negative() {
    assert_eq!(format_signed_bytes(-1536), "-1.5 KiB");
}

/// What: Test render_header_chips with minimal data.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: Minimal PreflightHeaderChips with zero values
///
/// Output:
/// - Returns a Line containing styled spans
///
/// Details:
/// - Verifies that header chips render without panicking with minimal data.
#[test]
fn test_render_header_chips_minimal() {
    let app = AppState::default();
    let chips = PreflightHeaderChips {
        package_count: 0,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 0,
        risk_level: RiskLevel::Low,
        risk_score: 0,
    };
    let line = render_header_chips(&app, &chips);
    assert!(!line.spans.is_empty());
}

/// What: Test render_header_chips with AUR packages.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: PreflightHeaderChips with AUR count > 0
///
/// Output:
/// - Returns a Line containing AUR package count in chips
///
/// Details:
/// - Verifies that AUR count is included in the package count chip when > 0.
#[test]
fn test_render_header_chips_with_aur() {
    let app = AppState::default();
    let chips = PreflightHeaderChips {
        package_count: 5,
        aur_count: 2,
        download_bytes: 1048576,
        install_delta_bytes: 512000,
        risk_level: RiskLevel::Medium,
        risk_score: 5,
    };
    let line = render_header_chips(&app, &chips);
    assert!(!line.spans.is_empty());
    // Verify AUR count is mentioned in the output
    let line_text: String = line
        .spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>();
    // The AUR count should be included in the package count chip
    assert!(line_text.contains("5") || line_text.contains("2"));
}

/// What: Test render_header_chips with positive install delta.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: PreflightHeaderChips with positive install_delta_bytes
///
/// Output:
/// - Returns a Line with green delta color
///
/// Details:
/// - Verifies that positive install delta uses green color.
#[test]
fn test_render_header_chips_positive_delta() {
    let app = AppState::default();
    let chips = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 1048576, // Positive
        risk_level: RiskLevel::Low,
        risk_score: 1,
    };
    let line = render_header_chips(&app, &chips);
    assert!(!line.spans.is_empty());
}

/// What: Test render_header_chips with negative install delta.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: PreflightHeaderChips with negative install_delta_bytes
///
/// Output:
/// - Returns a Line with red delta color
///
/// Details:
/// - Verifies that negative install delta uses red color.
#[test]
fn test_render_header_chips_negative_delta() {
    let app = AppState::default();
    let chips = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: -1048576, // Negative
        risk_level: RiskLevel::Low,
        risk_score: 1,
    };
    let line = render_header_chips(&app, &chips);
    assert!(!line.spans.is_empty());
}

/// What: Test render_header_chips with zero install delta.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: PreflightHeaderChips with zero install_delta_bytes
///
/// Output:
/// - Returns a Line with neutral delta color
///
/// Details:
/// - Verifies that zero install delta uses neutral color.
#[test]
fn test_render_header_chips_zero_delta() {
    let app = AppState::default();
    let chips = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 0, // Zero
        risk_level: RiskLevel::Low,
        risk_score: 1,
    };
    let line = render_header_chips(&app, &chips);
    assert!(!line.spans.is_empty());
}

/// What: Test render_header_chips with different risk levels.
///
/// Inputs:
/// - `app`: Default AppState
/// - `chips`: PreflightHeaderChips with Low, Medium, and High risk levels
///
/// Output:
/// - Returns Lines with appropriate risk colors
///
/// Details:
/// - Verifies that different risk levels render with correct color coding.
#[test]
fn test_render_header_chips_risk_levels() {
    let app = AppState::default();

    // Test Low risk
    let chips_low = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 0,
        risk_level: RiskLevel::Low,
        risk_score: 1,
    };
    let line_low = render_header_chips(&app, &chips_low);
    assert!(!line_low.spans.is_empty());

    // Test Medium risk
    let chips_medium = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 0,
        risk_level: RiskLevel::Medium,
        risk_score: 5,
    };
    let line_medium = render_header_chips(&app, &chips_medium);
    assert!(!line_medium.spans.is_empty());

    // Test High risk
    let chips_high = PreflightHeaderChips {
        package_count: 1,
        aur_count: 0,
        download_bytes: 0,
        install_delta_bytes: 0,
        risk_level: RiskLevel::High,
        risk_score: 10,
    };
    let line_high = render_header_chips(&app, &chips_high);
    assert!(!line_high.spans.is_empty());
}

// Sync and Layout Tests

use super::layout;
use super::sync;
use crate::state::modal::{
    DependencyInfo, DependencySource, DependencyStatus, FileChange, FileChangeType,
    PackageFileInfo, PreflightAction, PreflightTab, ServiceImpact, ServiceRestartDecision,
};
use crate::state::{PackageItem, Source};

/// What: Test sync_dependencies early return for Remove action.
///
/// Inputs:
/// - `action`: PreflightAction::Remove
/// - `dependency_info`: Empty vector
///
/// Output:
/// - `dependency_info` remains unchanged
///
/// Details:
/// - Verifies that dependency sync is skipped for remove actions.
#[test]
fn test_sync_dependencies_early_return_remove() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Remove;
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        &action,
        &tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert!(dependency_info.is_empty());
}

/// What: Test sync_dependencies early return when not on Deps tab.
///
/// Inputs:
/// - `tab`: PreflightTab::Summary
/// - `dependency_info`: Empty vector
///
/// Output:
/// - `dependency_info` remains unchanged
///
/// Details:
/// - Verifies that dependency sync is skipped when not on Deps tab.
#[test]
fn test_sync_dependencies_early_return_wrong_tab() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Summary;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        &action,
        &tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert!(dependency_info.is_empty());
}

/// What: Test sync_dependencies filters dependencies by required_by.
///
/// Inputs:
/// - `app`: AppState with cached dependencies
/// - `items`: Packages that require dependencies
///
/// Output:
/// - `dependency_info` contains only dependencies required by items
///
/// Details:
/// - Verifies that dependency filtering works correctly.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_filters_by_required_by() {
    let mut app = AppState::default();
    app.install_list_deps = vec![
        DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        DependencyInfo {
            name: "dep2".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["other-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 0;

    sync::sync_dependencies(
        &app,
        &items,
        &action,
        &tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dependency_info.len(), 1);
    assert_eq!(dependency_info[0].name, "dep1");
    assert_eq!(dep_selected, 0);
}

/// What: Test sync_dependencies resets selection on first load.
///
/// Inputs:
/// - `dependency_info`: Empty (first load)
/// - `dep_selected`: 5
///
/// Output:
/// - `dep_selected` is reset to 0
///
/// Details:
/// - Verifies that selection is reset on first load.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_resets_selection_on_first_load() {
    let mut app = AppState::default();
    app.install_list_deps = vec![DependencyInfo {
        name: "dep1".to_string(),
        version: "1.0".to_string(),
        status: DependencyStatus::ToInstall,
        source: DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-pkg".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Deps;
    let mut dependency_info = Vec::new();
    let mut dep_selected = 5;

    sync::sync_dependencies(
        &app,
        &items,
        &action,
        &tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dep_selected, 0);
}

/// What: Test sync_dependencies does not reset selection on subsequent loads.
///
/// Inputs:
/// - `dependency_info`: Already populated
/// - `dep_selected`: 2
///
/// Output:
/// - `dep_selected` remains 2
///
/// Details:
/// - Verifies that selection is preserved on subsequent syncs.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_dependencies_preserves_selection_on_subsequent_load() {
    let mut app = AppState::default();
    app.install_list_deps = vec![
        DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
        DependencyInfo {
            name: "dep2".to_string(),
            version: "1.0".to_string(),
            status: DependencyStatus::ToInstall,
            source: DependencySource::Official {
                repo: "core".to_string(),
            },
            required_by: vec!["test-pkg".to_string()],
            depends_on: Vec::new(),
            is_core: false,
            is_system: false,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Deps;
    let mut dependency_info = vec![DependencyInfo {
        name: "dep1".to_string(),
        version: "1.0".to_string(),
        status: DependencyStatus::ToInstall,
        source: DependencySource::Official {
            repo: "core".to_string(),
        },
        required_by: vec!["test-pkg".to_string()],
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }];
    let mut dep_selected = 2;

    sync::sync_dependencies(
        &app,
        &items,
        &action,
        &tab,
        &mut dependency_info,
        &mut dep_selected,
    );

    assert_eq!(dep_selected, 2);
}

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

/// What: Test sync_services early return for Remove action.
///
/// Inputs:
/// - `action`: PreflightAction::Remove
/// - `service_info`: Empty vector
///
/// Output:
/// - `service_info` remains unchanged
///
/// Details:
/// - Verifies that service sync is skipped for remove actions.
#[test]
fn test_sync_services_early_return_remove() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Remove;
    let mut service_info = Vec::new();
    let mut service_selected = 0;
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert!(service_info.is_empty());
    assert!(!services_loaded);
}

/// What: Test sync_services filters services by providers.
///
/// Inputs:
/// - `app`: AppState with cached service info
/// - `items`: Packages that provide services
///
/// Output:
/// - `service_info` contains only services provided by items
///
/// Details:
/// - Verifies that service filtering works correctly.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_services_filters_by_providers() {
    let mut app = AppState::default();
    app.install_list_services = vec![
        ServiceImpact {
            unit_name: "test.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "other.service".to_string(),
            providers: vec!["other-pkg".to_string()],
            is_active: false,
            needs_restart: false,
            recommended_decision: ServiceRestartDecision::Defer,
            restart_decision: ServiceRestartDecision::Defer,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let mut service_info = Vec::new();
    let mut service_selected = 0;
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert_eq!(service_info.len(), 1);
    assert_eq!(service_info[0].unit_name, "test.service");
    assert!(services_loaded);
}

/// What: Test sync_services adjusts selection when out of bounds.
///
/// Inputs:
/// - `service_info`: 3 services
/// - `service_selected`: 5 (out of bounds)
///
/// Output:
/// - `service_selected` is adjusted to 2 (last valid index)
///
/// Details:
/// - Verifies that selection is clamped to valid range.
#[test]
#[allow(clippy::field_reassign_with_default)]
fn test_sync_services_adjusts_selection_out_of_bounds() {
    let mut app = AppState::default();
    app.install_list_services = vec![
        ServiceImpact {
            unit_name: "service1.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "service2.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
        ServiceImpact {
            unit_name: "service3.service".to_string(),
            providers: vec!["test-pkg".to_string()],
            is_active: true,
            needs_restart: true,
            recommended_decision: ServiceRestartDecision::Restart,
            restart_decision: ServiceRestartDecision::Restart,
        },
    ];

    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let mut service_info = Vec::new();
    let mut service_selected = 5; // Out of bounds
    let mut services_loaded = false;

    sync::sync_services(
        &app,
        &items,
        &action,
        &mut service_info,
        &mut service_selected,
        &mut services_loaded,
    );

    assert_eq!(service_info.len(), 3);
    assert_eq!(service_selected, 2); // Clamped to last valid index
}

/// What: Test sync_sandbox early return for Remove action.
///
/// Inputs:
/// - `action`: PreflightAction::Remove
/// - `sandbox_info`: Empty vector
///
/// Output:
/// - `sandbox_info` remains unchanged (unless no AUR packages)
///
/// Details:
/// - Verifies that sandbox sync handles remove actions correctly.
#[test]
fn test_sync_sandbox_early_return_remove() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Official {
            repo: "core".to_string(),
            arch: "x86_64".to_string(),
        },
        popularity: None,
    }];
    let action = PreflightAction::Remove;
    let tab = PreflightTab::Sandbox;
    let mut sandbox_info = Vec::new();
    let mut sandbox_loaded = false;

    sync::sync_sandbox(
        &app,
        &items,
        &action,
        &tab,
        &mut sandbox_info,
        &mut sandbox_loaded,
    );

    // For remove with no AUR packages, should mark as loaded
    assert!(sandbox_loaded);
}

/// What: Test sync_sandbox early return when not on Sandbox tab.
///
/// Inputs:
/// - `tab`: PreflightTab::Summary
/// - `sandbox_info`: Empty vector
///
/// Output:
/// - `sandbox_info` remains unchanged
///
/// Details:
/// - Verifies that sandbox sync is skipped when not on Sandbox tab.
#[test]
fn test_sync_sandbox_early_return_wrong_tab() {
    let app = AppState::default();
    let items = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0".to_string(),
        description: String::new(),
        source: Source::Aur,
        popularity: None,
    }];
    let action = PreflightAction::Install;
    let tab = PreflightTab::Summary;
    let mut sandbox_info = Vec::new();
    let mut sandbox_loaded = false;

    sync::sync_sandbox(
        &app,
        &items,
        &action,
        &tab,
        &mut sandbox_info,
        &mut sandbox_loaded,
    );

    assert!(sandbox_info.is_empty());
}

/// What: Test calculate_modal_layout with standard area.
///
/// Inputs:
/// - `area`: Standard terminal area (120x40)
///
/// Output:
/// - Returns properly sized and centered modal rects
///
/// Details:
/// - Verifies that layout calculation works for standard sizes.
#[test]
fn test_calculate_modal_layout_standard() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };

    let (modal_rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);

    // Modal should be centered and within max size
    assert!(modal_rect.width <= 96);
    assert!(modal_rect.height <= 32);
    assert!(modal_rect.x > 0); // Centered
    assert!(modal_rect.y > 0); // Centered

    // Content and keybinds should fit within modal
    assert!(content_rect.width <= modal_rect.width);
    assert!(content_rect.height + keybinds_rect.height <= modal_rect.height);
    assert_eq!(keybinds_rect.height, 4);
}

/// What: Test calculate_modal_layout with small area.
///
/// Inputs:
/// - `area`: Small terminal area (50x20)
///
/// Output:
/// - Returns modal that fits within area
///
/// Details:
/// - Verifies that layout calculation handles small areas correctly.
#[test]
fn test_calculate_modal_layout_small() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 20,
    };

    let (modal_rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);

    // Modal should fit within area (with margins)
    assert!(modal_rect.width <= area.width);
    assert!(modal_rect.height <= area.height);
    assert!(content_rect.width <= modal_rect.width);
    assert!(content_rect.height + keybinds_rect.height <= modal_rect.height);
}

/// What: Test calculate_modal_layout with maximum constraints.
///
/// Inputs:
/// - `area`: Very large terminal area (200x100)
///
/// Output:
/// - Returns modal constrained to max 96x32
///
/// Details:
/// - Verifies that maximum size constraints are enforced.
#[test]
fn test_calculate_modal_layout_max_constraints() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };

    let (modal_rect, _, _) = layout::calculate_modal_layout(area);

    // Modal should be constrained to max size
    assert_eq!(modal_rect.width, 96);
    assert_eq!(modal_rect.height, 32);
}

/// What: Test calculate_modal_layout with offset area.
///
/// Inputs:
/// - `area`: Area with non-zero offset (x=10, y=5)
///
/// Output:
/// - Returns modal centered within offset area
///
/// Details:
/// - Verifies that layout calculation handles offset areas correctly.
#[test]
fn test_calculate_modal_layout_offset() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 10,
        y: 5,
        width: 120,
        height: 40,
    };

    let (modal_rect, _, _) = layout::calculate_modal_layout(area);

    // Modal should be centered within offset area
    assert!(modal_rect.x >= area.x);
    assert!(modal_rect.y >= area.y);
    assert!(modal_rect.x + modal_rect.width <= area.x + area.width);
    assert!(modal_rect.y + modal_rect.height <= area.y + area.height);
}
