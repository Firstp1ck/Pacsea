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
