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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Validate dependency priorities rank conflict states ahead of installed cases.
    ///
    /// Inputs:
    /// - Constructs each `DependencyStatus` variant with lightweight sample payloads.
    ///
    /// Output:
    /// - Asserts the assigned numeric priorities ascend from conflict through installed statuses.
    ///
    /// Details:
    /// - Guards the ordering relied upon by sorting logic so that regression changes surface quickly.
    fn dependency_priority_orders_by_severity() {
        let conflict = dependency_priority(&DependencyStatus::Conflict {
            reason: String::new(),
        });
        let missing = dependency_priority(&DependencyStatus::Missing);
        let install = dependency_priority(&DependencyStatus::ToInstall);
        let upgrade = dependency_priority(&DependencyStatus::ToUpgrade {
            current: "1".into(),
            required: "2".into(),
        });
        let installed = dependency_priority(&DependencyStatus::Installed {
            version: "1".into(),
        });

        assert!(conflict < missing);
        assert!(missing < install);
        assert!(install < upgrade);
        assert!(upgrade < installed);
    }
}
