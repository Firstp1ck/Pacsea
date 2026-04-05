//! Warn when install targets include AUR rows whose `pkgname` also appears as official in results.

use crate::logic::aur_pkgnames_also_in_official_catalog;
use crate::state::modal::PreflightHeaderChips;
use crate::state::{AppState, PackageItem};

/// What: Build the catalog used to detect AUR vs official duplicates in the current search UI.
///
/// Inputs:
/// - `app`: Live results buffers.
///
/// Output:
/// - Concatenation of `all_results` then `results` (order does not affect detection).
///
/// Details:
/// - Duplicate rows are harmless; [`aur_pkgnames_also_in_official_catalog`] uses a set of official names.
#[must_use]
pub fn catalog_for_install_dup_check(app: &AppState) -> Vec<PackageItem> {
    app.all_results
        .iter()
        .chain(app.results.iter())
        .cloned()
        .collect()
}

/// What: Open [`crate::state::Modal::WarnAurRepoDuplicate`] when AUR installs shadow official rows.
///
/// Inputs:
/// - `app`: UI state (may set modal).
/// - `packages`: Packages about to be installed.
/// - `header_chips`: Preflight metrics to restore after continue.
///
/// Output:
/// - `true` when the warning modal was shown and the caller must stop the install flow.
///
/// Details:
/// - Honors [`AppState::skip_aur_repo_dup_warning_once`] for the continue path.
#[must_use]
pub fn try_open_warn_aur_repo_duplicate_modal(
    app: &mut AppState,
    packages: &[PackageItem],
    header_chips: PreflightHeaderChips,
) -> bool {
    if app.skip_aur_repo_dup_warning_once {
        app.skip_aur_repo_dup_warning_once = false;
        return false;
    }
    let catalog = catalog_for_install_dup_check(app);
    let dups = aur_pkgnames_also_in_official_catalog(packages, &catalog);
    if dups.is_empty() {
        return false;
    }
    app.modal = crate::state::Modal::WarnAurRepoDuplicate {
        dup_names: dups,
        packages: packages.to_vec(),
        header_chips,
    };
    true
}
