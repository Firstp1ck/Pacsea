//! Utility functions for dependency resolution.

use crate::state::modal::DependencyStatus;

/// Priority for sorting dependencies (lower = higher priority).
pub(crate) fn dependency_priority(status: &DependencyStatus) -> u8 {
    match status {
        DependencyStatus::Conflict { .. } => 0,
        DependencyStatus::Missing => 1,
        DependencyStatus::ToInstall => 2,
        DependencyStatus::ToUpgrade { .. } => 3,
        DependencyStatus::Installed { .. } => 4,
    }
}
