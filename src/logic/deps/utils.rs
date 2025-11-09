//! Utility functions for dependency resolution.

use crate::state::modal::DependencyStatus;

/// What: Provide a numeric priority used to order dependency statuses.
///
/// Inputs:
/// - `status`: Dependency status variant subject to sorting.
///
/// Output:
/// - Returns a numeric priority where lower numbers represent higher urgency.
///
/// Details:
/// - Aligns the ordering logic with UI expectations (conflicts first, installed last).
pub(crate) fn dependency_priority(status: &DependencyStatus) -> u8 {
    match status {
        DependencyStatus::Conflict { .. } => 0,
        DependencyStatus::Missing => 1,
        DependencyStatus::ToInstall => 2,
        DependencyStatus::ToUpgrade { .. } => 3,
        DependencyStatus::Installed { .. } => 4,
    }
}
