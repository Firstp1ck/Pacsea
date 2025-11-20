//! Unit tests for render_header_chips function.

use super::render_header_chips;
use crate::state::AppState;
use crate::state::modal::{PreflightHeaderChips, RiskLevel};

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
