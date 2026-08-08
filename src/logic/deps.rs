//! Dependency resolution and analysis through the arch-toolkit integration boundary.

use std::collections::HashSet;
use std::hash::BuildHasher;

use crate::state::modal::{DependencyInfo, ReverseRootSummary};
use crate::state::types::PackageItem;

/// What: Aggregate reverse-dependency data for Pacsea preflight views.
///
/// Inputs:
/// - Produced by [`resolve_reverse_dependencies`].
///
/// Output:
/// - Dependency rows and per-root summary counts.
///
/// Details:
/// - Preserves Pacsea's public modal/cache contract while arch-toolkit owns traversal.
#[derive(Clone, Debug, Default)]
pub struct ReverseDependencyReport {
    /// Flattened dependency rows reused by the preflight modal.
    pub dependencies: Vec<DependencyInfo>,
    /// Per-root direct and transitive dependency counts.
    pub summaries: Vec<ReverseRootSummary>,
}

/// What: Resolve direct dependencies for selected packages through arch-toolkit.
///
/// Inputs:
/// - `items`: Packages selected for install or update preflight.
///
/// Output:
/// - Pacsea dependency rows sorted by urgency.
///
/// Details:
/// - Host-tool failures remain nonfatal and are logged with actionable guidance.
#[must_use]
pub fn resolve_dependencies(items: &[PackageItem]) -> Vec<DependencyInfo> {
    crate::integrations::arch_toolkit::deps::resolve_dependencies(items)
}

/// What: Resolve reverse dependencies for selected removal targets through arch-toolkit.
///
/// Inputs:
/// - `items`: Packages selected for removal.
///
/// Output:
/// - Pacsea-compatible reverse dependency report.
///
/// Details:
/// - Toolkit traversal results are converted before reaching UI state.
#[must_use]
pub fn resolve_reverse_dependencies(items: &[PackageItem]) -> ReverseDependencyReport {
    crate::integrations::arch_toolkit::deps::resolve_reverse_dependencies(items)
}

/// What: Query installed package names through arch-toolkit.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Installed package names or an empty set when pacman is unavailable.
///
/// Details:
/// - The query runs with a stable C locale.
#[must_use]
pub fn get_installed_packages() -> HashSet<String> {
    crate::integrations::arch_toolkit::deps::installed_packages()
}

/// What: Query packages with available upgrades through arch-toolkit.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Upgradable package names or an empty set when pacman is unavailable.
///
/// Details:
/// - Nonzero pacman status remains a nonfatal empty result.
#[must_use]
pub fn get_upgradable_packages() -> HashSet<String> {
    crate::integrations::arch_toolkit::deps::upgradable_packages()
}

/// What: Return caller-compatible virtual provider state through arch-toolkit.
///
/// Inputs:
/// - `installed`: Installed package names.
///
/// Output:
/// - Provided package names known by the toolkit query policy.
///
/// Details:
/// - Lazy host lookup still occurs in [`is_package_installed_or_provided`].
#[must_use]
pub fn get_provided_packages<S: BuildHasher + Default>(
    installed: &HashSet<String, S>,
) -> HashSet<String> {
    crate::integrations::arch_toolkit::deps::provided_packages(installed)
}

/// What: Check installed or virtual-provider membership through arch-toolkit.
///
/// Inputs:
/// - `name`: Package or virtual dependency name.
/// - `installed`: Caller-supplied installed package names.
/// - `provided`: Caller-supplied virtual provider names.
///
/// Output:
/// - `true` when the dependency is satisfied.
///
/// Details:
/// - Supplied sets are honored before lazy host fallback.
#[must_use]
pub fn is_package_installed_or_provided<S: BuildHasher>(
    name: &str,
    installed: &HashSet<String, S>,
    provided: &HashSet<String, S>,
) -> bool {
    crate::integrations::arch_toolkit::deps::is_installed_or_provided(name, installed, provided)
}

/// What: Query one installed package version through arch-toolkit.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Installed version or a user-facing string error.
///
/// # Errors
///
/// Returns an error when pacman is unavailable, the package is missing, or output cannot be parsed.
///
/// Details:
/// - Preserves the pre-migration result shape used by preflight call sites.
pub fn get_installed_version(name: &str) -> Result<String, String> {
    crate::integrations::arch_toolkit::deps::installed_version(name)
}

/// What: Evaluate a pacman-style version requirement through arch-toolkit.
///
/// Inputs:
/// - `version`: Installed version.
/// - `requirement`: Comparison expression.
///
/// Output:
/// - Whether the requirement is satisfied.
///
/// Details:
/// - Uses epoch/pkgver/pkgrel-aware ordering.
#[must_use]
pub fn version_satisfies(version: &str, requirement: &str) -> bool {
    crate::integrations::arch_toolkit::deps::version_satisfies(version, requirement)
}

/// What: Check whether an installed package has installed reverse dependencies.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Whether at least one installed package requires it.
///
/// Details:
/// - Query failure degrades to `false`.
#[must_use]
pub fn has_installed_required_by(name: &str) -> bool {
    crate::integrations::arch_toolkit::deps::has_installed_required_by(name)
}

/// What: List installed packages that require another package.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Installed reverse dependency names.
///
/// Details:
/// - Query failure degrades to an empty vector.
#[must_use]
pub fn get_installed_required_by(name: &str) -> Vec<String> {
    crate::integrations::arch_toolkit::deps::installed_required_by(name)
}

#[cfg(test)]
mod tests {
    /// What: Verify empty dependency input remains a deterministic no-op.
    ///
    /// Inputs:
    /// - Empty package slice.
    ///
    /// Output:
    /// - Empty dependency rows without host queries.
    ///
    /// Details:
    /// - Protects preflight initialization and cache-reset paths.
    #[test]
    fn empty_dependency_resolution_is_noop() {
        assert!(super::resolve_dependencies(&[]).is_empty());
    }

    /// What: Verify empty reverse dependency input remains a deterministic no-op.
    ///
    /// Inputs:
    /// - Empty package slice.
    ///
    /// Output:
    /// - Empty dependency and summary collections.
    ///
    /// Details:
    /// - Protects removal-preflight initialization.
    #[test]
    fn empty_reverse_resolution_is_noop() {
        let report = super::resolve_reverse_dependencies(&[]);
        assert!(report.dependencies.is_empty());
        assert!(report.summaries.is_empty());
    }
}
