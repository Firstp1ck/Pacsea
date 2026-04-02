//! PKGBUILD static check state tied to the selected package.

use crate::state::AppState;
use crate::state::app_state::PkgbuildCheckStatus;

/// What: Drop PKGBUILD check results when they belong to a different package than the current selection.
///
/// Inputs:
/// - `app`: Application state holding check status and findings.
/// - `selected_package`: Name of the package now highlighted in the results list.
///
/// Output:
/// - Mutates `app` to idle check state with empty findings when the last check target does not match.
///
/// Details:
/// - Compares `selected_package` to [`AppState::pkgb_check_last_package_name`], which is set when a
///   run starts and when results arrive. Keeps results when the user is still on the same package.
pub fn clear_stale_pkgbuild_checks_for_selection(app: &mut AppState, selected_package: &str) {
    let still_valid = app
        .pkgb_check_last_package_name
        .as_deref()
        .is_some_and(|pkg| pkg == selected_package);
    if still_valid {
        return;
    }
    reset_pkgbuild_check_ui_state(app);
}

/// What: Reset all PKGBUILD check UI fields to an idle, empty state.
///
/// Inputs:
/// - `app`: Application state.
///
/// Output:
/// - Clears findings, raw output, errors, and scroll positions for the check panels.
///
/// Details:
/// - Used when switching packages or when discarding stale worker responses.
fn reset_pkgbuild_check_ui_state(app: &mut AppState) {
    app.pkgb_check_status = PkgbuildCheckStatus::Idle;
    app.pkgb_check_findings.clear();
    app.pkgb_check_raw_results.clear();
    app.pkgb_check_missing_tools.clear();
    app.pkgb_check_last_error = None;
    app.pkgb_check_last_package_name = None;
    app.pkgb_check_last_run_at = None;
    app.pkgb_check_scroll = 0;
    app.pkgb_check_raw_scroll = 0;
    app.pkgb_check_show_raw_output = false;
}

/// What: Whether a [`crate::state::PkgbuildCheckResponse`] should update the UI for the current row.
///
/// Inputs:
/// - `app`: Application state.
/// - `response_package`: Package name carried in the worker response.
///
/// Output:
/// - `true` if the response matches the focused or selected package (same rule as PKGBUILD fetch).
///
/// Details:
/// - Prevents late results from repopulating the panel after the user moved to another package.
#[must_use]
pub fn pkgbuild_check_response_matches_selection(app: &AppState, response_package: &str) -> bool {
    app.details_focus.as_deref() == Some(response_package)
        || app
            .results
            .get(app.selected)
            .is_some_and(|item| item.name == response_package)
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Test setup mutates a default `AppState` field by field.
mod tests {
    use super::*;
    use crate::state::app_state::{PkgbuildCheckFinding, PkgbuildCheckSeverity, PkgbuildCheckTool};
    use crate::state::{PackageItem, Source};

    fn aur_item(name: &str) -> PackageItem {
        PackageItem {
            name: name.to_string(),
            version: "1".to_string(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }

    #[test]
    /// What: Stale check results clear when the selection package name changes.
    ///
    /// Inputs:
    /// - `AppState` with completed checks recorded for `pkg-a`.
    /// - `selected_package` of `pkg-b`.
    ///
    /// Output:
    /// - Check status is idle and findings are empty.
    ///
    /// Details:
    /// - Mirrors navigating from one result row to another after running checks.
    fn clear_stale_drops_results_for_other_package() {
        let mut app = AppState::default();
        app.pkgb_check_status = PkgbuildCheckStatus::Complete;
        app.pkgb_check_last_package_name = Some("pkg-a".to_string());
        app.pkgb_check_findings.push(PkgbuildCheckFinding {
            tool: PkgbuildCheckTool::Shellcheck,
            severity: PkgbuildCheckSeverity::Warning,
            line: Some(1),
            message: "old".to_string(),
        });

        clear_stale_pkgbuild_checks_for_selection(&mut app, "pkg-b");

        assert_eq!(app.pkgb_check_status, PkgbuildCheckStatus::Idle);
        assert!(app.pkgb_check_findings.is_empty());
        assert!(app.pkgb_check_last_package_name.is_none());
    }

    #[test]
    /// What: Results are retained when the selection still matches the check target.
    ///
    /// Inputs:
    /// - `AppState` with checks for `pkg-a` and selection `pkg-a`.
    ///
    /// Output:
    /// - Findings and complete status unchanged.
    ///
    /// Details:
    /// - Ensures we do not clear on no-op navigation callbacks.
    fn clear_stale_keeps_results_for_same_package() {
        let mut app = AppState::default();
        app.pkgb_check_status = PkgbuildCheckStatus::Complete;
        app.pkgb_check_last_package_name = Some("pkg-a".to_string());
        app.pkgb_check_findings.push(PkgbuildCheckFinding {
            tool: PkgbuildCheckTool::Namcap,
            severity: PkgbuildCheckSeverity::Info,
            line: None,
            message: "keep".to_string(),
        });

        clear_stale_pkgbuild_checks_for_selection(&mut app, "pkg-a");

        assert_eq!(app.pkgb_check_status, PkgbuildCheckStatus::Complete);
        assert_eq!(app.pkgb_check_findings.len(), 1);
    }

    #[test]
    /// What: Response matches when the selected results row names the same package.
    ///
    /// Inputs:
    /// - `AppState` with `results` and `selected` pointing at `foo`.
    ///
    /// Output:
    /// - `pkgbuild_check_response_matches_selection` is true for `foo`.
    ///
    /// Details:
    /// - Covers the worker completion path when `details_focus` is unset but the row matches.
    fn response_matches_selected_row() {
        let mut app = AppState::default();
        app.results = vec![aur_item("foo")];
        app.selected = 0;

        assert!(pkgbuild_check_response_matches_selection(&app, "foo"));
        assert!(!pkgbuild_check_response_matches_selection(&app, "bar"));
    }

    #[test]
    /// What: Response matches when `details_focus` agrees even if row index were wrong.
    ///
    /// Inputs:
    /// - `AppState` with `details_focus` set to `bar`.
    ///
    /// Output:
    /// - Matcher returns true for `bar`.
    ///
    /// Details:
    /// - Aligns with `handle_pkgbuild_result` gating.
    fn response_matches_details_focus() {
        let mut app = AppState::default();
        app.details_focus = Some("bar".to_string());

        assert!(pkgbuild_check_response_matches_selection(&app, "bar"));
    }
}
