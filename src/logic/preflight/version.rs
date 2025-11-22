//! Version comparison utilities for preflight analysis.
//!
//! This module provides functions to compare version strings and detect
//! major version bumps.

use std::cmp::Ordering;

/// What: Compare dotted version strings numerically.
///
/// Inputs:
/// - `a`: Left-hand version.
/// - `b`: Right-hand version.
///
/// Output:
/// - `Ordering` indicating which version is greater.
///
/// Details:
/// - Splits on `.` and `-`, comparing numeric segments when possible and
///   falling back to lexicographical comparison.
pub(super) fn compare_versions(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<&str> = a.split(['.', '-']).collect();
    let b_parts: Vec<&str> = b.split(['.', '-']).collect();
    let len = a_parts.len().max(b_parts.len());

    for idx in 0..len {
        let a_seg = a_parts.get(idx).copied().unwrap_or("0");
        let b_seg = b_parts.get(idx).copied().unwrap_or("0");

        match (a_seg.parse::<i64>(), b_seg.parse::<i64>()) {
            (Ok(a_num), Ok(b_num)) => match a_num.cmp(&b_num) {
                Ordering::Equal => {}
                ord => return ord,
            },
            _ => match a_seg.cmp(b_seg) {
                Ordering::Equal => {}
                ord => return ord,
            },
        }
    }

    Ordering::Equal
}

/// What: Determine whether `new` constitutes a major version bump relative to
/// `old`.
///
/// Inputs:
/// - `old`: Currently installed version.
/// - `new`: Target version.
///
/// Output:
/// - `true` when the major component increased; `false` otherwise.
///
/// Details:
/// - Parses the first numeric segment (before `.`/`-`) for comparison.
pub(super) fn is_major_version_bump(old: &str, new: &str) -> bool {
    match (extract_major_component(old), extract_major_component(new)) {
        (Some(old_major), Some(new_major)) => new_major > old_major,
        _ => false,
    }
}

/// What: Extract the leading numeric component from a version string.
///
/// Inputs:
/// - `version`: Version string to parse.
///
/// Output:
/// - `Some(u64)` for the first numeric segment.
/// - `None` when parsing fails.
///
/// Details:
/// - Splits on `.` and `-`, treating the first token as the major component.
fn extract_major_component(version: &str) -> Option<u64> {
    let token = version.split(['.', '-']).next()?;
    token.parse::<u64>().ok()
}
