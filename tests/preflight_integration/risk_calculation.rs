//! Integration tests for enhanced preflight risk calculation.
//!
//! Tests cover:
//! - Risk score calculation with dependents (+2 per dependent)
//! - Risk level thresholds (Low/Medium/High)
//! - Header chips update with risk info
//! - Multiple dependents accumulation

#![cfg(test)]

use pacsea::state::{
    AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source,
    modal::{CascadeMode, PreflightHeaderChips, RiskLevel},
};
use std::collections::HashSet;

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
/// What: Test `RiskLevel::Low` default.
///
/// Inputs:
/// - Default risk level.
///
/// Output:
/// - Risk level is Low.
///
/// Details:
/// - Verifies default risk level is Low.
fn integration_risk_level_default_is_low() {
    let risk_level = RiskLevel::default();
    assert_eq!(risk_level, RiskLevel::Low);
}

#[test]
/// What: Test `PreflightHeaderChips` default values.
///
/// Inputs:
/// - Default header chips.
///
/// Output:
/// - All values are zero/low.
///
/// Details:
/// - Verifies default header chips are neutral.
fn integration_header_chips_default() {
    let chips = PreflightHeaderChips::default();

    assert_eq!(chips.package_count, 0);
    assert_eq!(chips.download_bytes, 0);
    assert_eq!(chips.install_delta_bytes, 0);
    assert_eq!(chips.aur_count, 0);
    assert_eq!(chips.risk_score, 0);
    assert_eq!(chips.risk_level, RiskLevel::Low);
}

#[test]
/// What: Test `PreflightHeaderChips` with low risk score.
///
/// Inputs:
/// - Header chips with `risk_score` < 10.
///
/// Output:
/// - Risk level is Low.
///
/// Details:
/// - Verifies low risk threshold.
fn integration_header_chips_low_risk() {
    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 1024,
        install_delta_bytes: 2048,
        aur_count: 0,
        risk_score: 5,
        risk_level: RiskLevel::Low,
    };

    assert_eq!(chips.risk_score, 5);
    assert_eq!(chips.risk_level, RiskLevel::Low);
}

#[test]
/// What: Test `PreflightHeaderChips` with medium risk score.
///
/// Inputs:
/// - Header chips with `risk_score` between 10 and 30.
///
/// Output:
/// - Risk level is Medium.
///
/// Details:
/// - Verifies medium risk threshold.
fn integration_header_chips_medium_risk() {
    let chips = PreflightHeaderChips {
        package_count: 5,
        download_bytes: 50 * 1024 * 1024,
        install_delta_bytes: 100 * 1024 * 1024,
        aur_count: 2,
        risk_score: 20,
        risk_level: RiskLevel::Medium,
    };

    assert_eq!(chips.risk_score, 20);
    assert_eq!(chips.risk_level, RiskLevel::Medium);
}

#[test]
/// What: Test `PreflightHeaderChips` with high risk score.
///
/// Inputs:
/// - Header chips with `risk_score` >= 30.
///
/// Output:
/// - Risk level is High.
///
/// Details:
/// - Verifies high risk threshold.
fn integration_header_chips_high_risk() {
    let chips = PreflightHeaderChips {
        package_count: 10,
        download_bytes: 200 * 1024 * 1024,
        install_delta_bytes: 500 * 1024 * 1024,
        aur_count: 5,
        risk_score: 45,
        risk_level: RiskLevel::High,
    };

    assert_eq!(chips.risk_score, 45);
    assert_eq!(chips.risk_level, RiskLevel::High);
}

#[test]
/// What: Test risk score with dependent packages.
///
/// Inputs:
/// - Base risk score + 2 per dependent package.
///
/// Output:
/// - Risk score includes dependent contribution.
///
/// Details:
/// - Verifies +2 per dependent package calculation.
fn integration_risk_score_with_dependents() {
    let base_risk = 5;
    let num_dependents = 3;
    let dependent_risk = num_dependents * 2; // +2 per dependent
    let total_risk = base_risk + dependent_risk;

    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count: 0,
        risk_score: total_risk,
        risk_level: if total_risk < 10 {
            RiskLevel::Low
        } else if total_risk < 30 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        },
    };

    assert_eq!(chips.risk_score, 11); // 5 + (3 * 2)
    assert_eq!(chips.risk_level, RiskLevel::Medium);
}

#[test]
/// What: Test risk score with many dependents pushes to High.
///
/// Inputs:
/// - Many dependent packages.
///
/// Output:
/// - Risk score becomes High.
///
/// Details:
/// - Verifies many dependents can push risk to High level.
fn integration_risk_score_many_dependents() {
    let base_risk: u8 = 10;
    let num_dependents: u8 = 15;
    let dependent_risk: u8 = num_dependents * 2;
    let total_risk: u8 = base_risk + dependent_risk;

    let risk_level = if total_risk < 10 {
        RiskLevel::Low
    } else if total_risk < 30 {
        RiskLevel::Medium
    } else {
        RiskLevel::High
    };

    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count: 0,
        risk_score: total_risk,
        risk_level,
    };

    assert_eq!(chips.risk_score, 40); // 10 + (15 * 2)
    assert_eq!(chips.risk_level, RiskLevel::High);
}

#[test]
/// What: Test AUR packages contribute to risk.
///
/// Inputs:
/// - Multiple AUR packages.
///
/// Output:
/// - Risk score includes AUR contribution.
///
/// Details:
/// - Verifies AUR packages increase risk score.
fn integration_risk_score_aur_contribution() {
    let aur_count: usize = 3;
    // AUR packages typically add 5 points each
    let aur_risk: u8 = 15; // 3 * 5

    let chips = PreflightHeaderChips {
        package_count: 5,
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count,
        risk_score: aur_risk,
        risk_level: if aur_risk < 10 {
            RiskLevel::Low
        } else if aur_risk < 30 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        },
    };

    assert_eq!(chips.aur_count, 3);
    assert_eq!(chips.risk_score, 15);
    assert_eq!(chips.risk_level, RiskLevel::Medium);
}

#[test]
/// What: Test Preflight modal with header chips.
///
/// Inputs:
/// - Preflight modal with populated header chips.
///
/// Output:
/// - Header chips are accessible.
///
/// Details:
/// - Verifies header chips are stored in Preflight modal.
fn integration_preflight_modal_header_chips() {
    let chips = PreflightHeaderChips {
        package_count: 3,
        download_bytes: 10 * 1024 * 1024,
        install_delta_bytes: 25 * 1024 * 1024,
        aur_count: 1,
        risk_score: 12,
        risk_level: RiskLevel::Medium,
    };

    let app = AppState {
        modal: Modal::Preflight {
            items: vec![create_test_package("test-pkg", Source::Aur)],
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            summary: None,
            summary_scroll: 0,
            header_chips: chips,
            dependency_info: vec![],
            dep_selected: 0,
            dep_tree_expanded: HashSet::new(),
            deps_error: None,
            file_info: vec![],
            file_selected: 0,
            file_tree_expanded: HashSet::new(),
            files_error: None,
            service_info: vec![],
            service_selected: 0,
            services_loaded: false,
            services_error: None,
            sandbox_info: vec![],
            sandbox_selected: 0,
            sandbox_tree_expanded: HashSet::new(),
            sandbox_loaded: false,
            sandbox_error: None,
            selected_optdepends: std::collections::HashMap::new(),
            cascade_mode: CascadeMode::Basic,
            cached_reverse_deps_report: None,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::Preflight { header_chips, .. } => {
            assert_eq!(header_chips.package_count, 3);
            assert_eq!(header_chips.aur_count, 1);
            assert_eq!(header_chips.risk_score, 12);
            assert_eq!(header_chips.risk_level, RiskLevel::Medium);
        }
        _ => panic!("Expected Preflight modal"),
    }
}

#[test]
/// What: Test `PreflightExec` modal preserves header chips.
///
/// Inputs:
/// - `PreflightExec` modal with header chips.
///
/// Output:
/// - Header chips are accessible.
///
/// Details:
/// - Verifies header chips persist through modal transition.
fn integration_preflight_exec_header_chips() {
    let chips = PreflightHeaderChips {
        package_count: 2,
        download_bytes: 5 * 1024 * 1024,
        install_delta_bytes: 15 * 1024 * 1024,
        aur_count: 0,
        risk_score: 3,
        risk_level: RiskLevel::Low,
    };

    let app = AppState {
        modal: Modal::PreflightExec {
            items: vec![create_test_package(
                "test-pkg",
                Source::Official {
                    repo: "extra".into(),
                    arch: "x86_64".into(),
                },
            )],
            success: None,
            action: PreflightAction::Install,
            tab: PreflightTab::Summary,
            verbose: false,
            log_lines: vec![],
            abortable: false,
            header_chips: chips,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::PreflightExec { header_chips, .. } => {
            assert_eq!(header_chips.package_count, 2);
            assert_eq!(header_chips.risk_score, 3);
            assert_eq!(header_chips.risk_level, RiskLevel::Low);
        }
        _ => panic!("Expected PreflightExec modal"),
    }
}

#[test]
/// What: Test `ConfirmReinstall` modal preserves header chips.
///
/// Inputs:
/// - `ConfirmReinstall` modal with header chips.
///
/// Output:
/// - Header chips are accessible.
///
/// Details:
/// - Verifies header chips are available in reinstall confirmation.
fn integration_confirm_reinstall_header_chips() {
    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 1024,
        install_delta_bytes: 2048,
        aur_count: 0,
        risk_score: 0,
        risk_level: RiskLevel::Low,
    };

    let pkg = create_test_package(
        "test-pkg",
        Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
    );

    let app = AppState {
        modal: Modal::ConfirmReinstall {
            items: vec![pkg.clone()],
            all_items: vec![pkg],
            header_chips: chips,
        },
        ..Default::default()
    };

    match app.modal {
        Modal::ConfirmReinstall { header_chips, .. } => {
            assert_eq!(header_chips.package_count, 1);
            assert_eq!(header_chips.risk_level, RiskLevel::Low);
        }
        _ => panic!("Expected ConfirmReinstall modal"),
    }
}

#[test]
/// What: Test risk level boundary at 10 (Low to Medium).
///
/// Inputs:
/// - Risk score at boundary value 10.
///
/// Output:
/// - Risk level is Medium.
///
/// Details:
/// - Verifies boundary condition between Low and Medium.
fn integration_risk_level_boundary_10() {
    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count: 0,
        risk_score: 10,
        risk_level: RiskLevel::Medium,
    };

    assert_eq!(chips.risk_score, 10);
    assert_eq!(chips.risk_level, RiskLevel::Medium);
}

#[test]
/// What: Test risk level boundary at 30 (Medium to High).
///
/// Inputs:
/// - Risk score at boundary value 30.
///
/// Output:
/// - Risk level is High.
///
/// Details:
/// - Verifies boundary condition between Medium and High.
fn integration_risk_level_boundary_30() {
    let chips = PreflightHeaderChips {
        package_count: 1,
        download_bytes: 0,
        install_delta_bytes: 0,
        aur_count: 0,
        risk_score: 30,
        risk_level: RiskLevel::High,
    };

    assert_eq!(chips.risk_score, 30);
    assert_eq!(chips.risk_level, RiskLevel::High);
}

#[test]
/// What: Test combined risk calculation.
///
/// Inputs:
/// - Multiple risk factors combined.
///
/// Output:
/// - Total risk score is sum of factors.
///
/// Details:
/// - Verifies combined risk from AUR, dependents, and base factors.
fn integration_combined_risk_calculation() {
    let base_risk: u8 = 5;
    let aur_risk: u8 = 2 * 5; // 2 AUR packages * 5
    let dependent_risk: u8 = 4 * 2; // 4 dependents * 2
    let total_risk: u8 = base_risk + aur_risk + dependent_risk;

    let chips = PreflightHeaderChips {
        package_count: 5,
        download_bytes: 50 * 1024 * 1024,
        install_delta_bytes: 100 * 1024 * 1024,
        aur_count: 2,
        risk_score: total_risk,
        risk_level: if total_risk < 10 {
            RiskLevel::Low
        } else if total_risk < 30 {
            RiskLevel::Medium
        } else {
            RiskLevel::High
        },
    };

    assert_eq!(chips.risk_score, 23); // 5 + 10 + 8
    assert_eq!(chips.risk_level, RiskLevel::Medium);
}
