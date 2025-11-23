//! Unit tests for `format_bytes` and `format_signed_bytes` functions.

use super::{format_bytes, format_signed_bytes};

/// What: Test `format_bytes` with zero bytes.
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

/// What: Test `format_bytes` with single byte.
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

/// What: Test `format_bytes` with bytes less than 1 KiB.
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

/// What: Test `format_bytes` with exactly 1 KiB.
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

/// What: Test `format_bytes` with values between KiB and MiB.
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

/// What: Test `format_bytes` with exactly 1 MiB.
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
    assert_eq!(format_bytes(1_048_576), "1.0 MiB");
}

/// What: Test `format_bytes` with values between MiB and GiB.
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
    assert_eq!(format_bytes(15_728_640), "15.0 MiB");
}

/// What: Test `format_bytes` with exactly 1 GiB.
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
    assert_eq!(format_bytes(1_073_741_824), "1.0 GiB");
}

/// What: Test `format_bytes` with large values (TiB).
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
    assert_eq!(format_bytes(1_099_511_627_776), "1.0 TiB");
}

/// What: Test `format_bytes` with very large values (PiB).
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
    assert_eq!(format_bytes(1_125_899_906_842_624), "1.0 PiB");
}

/// What: Test `format_bytes` with fractional values.
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
    assert_eq!(format_bytes(2_621_440), "2.5 MiB");
}

/// What: Test `format_signed_bytes` with zero.
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

/// What: Test `format_signed_bytes` with positive value.
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

/// What: Test `format_signed_bytes` with negative value.
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

/// What: Test `format_signed_bytes` with large positive value.
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
    assert_eq!(format_signed_bytes(1_048_576), "+1.0 MiB");
}

/// What: Test `format_signed_bytes` with large negative value.
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
    assert_eq!(format_signed_bytes(-1_048_576), "-1.0 MiB");
}

/// What: Test `format_signed_bytes` with fractional positive value.
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

/// What: Test `format_signed_bytes` with fractional negative value.
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
