//! Dependency parsing, query, and resolution adapters.

use std::collections::HashSet;
use std::hash::BuildHasher;

use crate::logic::deps::ReverseDependencyReport;
use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus, ReverseRootSummary};
use crate::state::types::{PackageItem, Source};

/// What: Resolve direct package dependencies through arch-toolkit.
///
/// Inputs:
/// - `items`: Pacsea package rows selected for dependency analysis.
///
/// Output:
/// - Pacsea dependency rows sorted by toolkit priority.
///
/// Details:
/// - Host-tool failures degrade to an empty result after an actionable warning, matching existing
///   nonfatal preflight behavior. UI models remain inside Pacsea.
pub fn resolve_dependencies(items: &[PackageItem]) -> Vec<DependencyInfo> {
    let packages: Vec<arch_toolkit::PackageRef> = items.iter().map(package_ref).collect();
    let resolver = arch_toolkit::DependencyResolver::new();
    match resolver.resolve(&packages) {
        Ok(resolution) => resolution
            .dependencies
            .into_iter()
            .map(dependency)
            .collect(),
        Err(error) => {
            tracing::warn!(
                error = %error,
                "arch-toolkit dependency resolution failed; install pacman or an AUR helper and retry"
            );
            Vec::new()
        }
    }
}

/// What: Analyze reverse dependencies through arch-toolkit.
///
/// Inputs:
/// - `items`: Pacsea package rows selected for removal.
///
/// Output:
/// - Flattened dependency rows and per-root summaries.
///
/// Details:
/// - Pacsea retains its report type for modal and cache compatibility.
pub fn resolve_reverse_dependencies(items: &[PackageItem]) -> ReverseDependencyReport {
    let packages: Vec<arch_toolkit::PackageRef> = items.iter().map(package_ref).collect();
    match arch_toolkit::ReverseDependencyAnalyzer::new().analyze(&packages) {
        Ok(report) => ReverseDependencyReport {
            dependencies: report.dependents.into_iter().map(dependency).collect(),
            summaries: report.summaries.into_iter().map(reverse_summary).collect(),
        },
        Err(error) => {
            tracing::warn!(
                error = %error,
                "arch-toolkit reverse dependency analysis failed; verify pacman is available"
            );
            ReverseDependencyReport::default()
        }
    }
}

/// What: Query installed packages through arch-toolkit with graceful degradation.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Installed package names, or an empty set when pacman is unavailable.
///
/// Details:
/// - arch-toolkit enforces the C locale for parsing.
pub fn installed_packages() -> HashSet<String> {
    arch_toolkit::deps::get_installed_packages().unwrap_or_default()
}

/// What: Query upgradable packages through arch-toolkit with graceful degradation.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Upgradable package names, or an empty set when pacman is unavailable.
///
/// Details:
/// - Nonzero pacman status remains nonfatal for preflight.
pub fn upgradable_packages() -> HashSet<String> {
    arch_toolkit::deps::get_upgradable_packages().unwrap_or_default()
}

/// What: Return caller-compatible provided-package state through arch-toolkit.
///
/// Inputs:
/// - `installed`: Installed package set.
///
/// Output:
/// - Toolkit-provided package set.
///
/// Details:
/// - The current toolkit implementation keeps this set empty and performs lazy host fallback.
pub fn provided_packages<S: BuildHasher + Default>(
    installed: &HashSet<String, S>,
) -> HashSet<String> {
    arch_toolkit::deps::get_provided_packages(installed)
}

/// What: Test installed or virtual-provider membership through arch-toolkit.
///
/// Inputs:
/// - `name`: Package or virtual dependency name.
/// - `installed`: Caller-supplied installed package names.
/// - `provided`: Caller-supplied virtual provider names.
///
/// Output:
/// - `true` when supplied sets or lazy pacman lookup satisfy the dependency.
///
/// Details:
/// - Caller-provided sets are checked before host lookup for deterministic fixtures.
pub fn is_installed_or_provided<S: BuildHasher>(
    name: &str,
    installed: &HashSet<String, S>,
    provided: &HashSet<String, S>,
) -> bool {
    arch_toolkit::deps::is_package_installed_or_provided(name, installed, provided)
}

/// What: Query one installed package version through arch-toolkit.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Installed version or actionable string error.
///
/// Details:
/// - Preserves the former Pacsea `Result<String, String>` surface.
pub fn installed_version(name: &str) -> Result<String, String> {
    arch_toolkit::deps::get_installed_version(name).map_err(|error| error.to_string())
}

/// What: Evaluate an Arch package version requirement through arch-toolkit.
///
/// Inputs:
/// - `version`: Installed version.
/// - `requirement`: Pacman-style comparison expression.
///
/// Output:
/// - Whether the version satisfies the requirement.
///
/// Details:
/// - Uses toolkit epoch/pkgver/pkgrel ordering instead of lexical string comparison.
pub fn version_satisfies(version: &str, requirement: &str) -> bool {
    arch_toolkit::deps::version_satisfies(version, requirement)
}

/// What: Test whether an installed package has installed reverse dependencies.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Whether at least one installed package requires it.
///
/// Details:
/// - Host-query failures degrade to `false` in arch-toolkit.
pub fn has_installed_required_by(name: &str) -> bool {
    arch_toolkit::deps::has_installed_required_by(name)
}

/// What: List installed reverse dependencies through arch-toolkit.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Installed package names requiring the input.
///
/// Details:
/// - Host-query failures degrade to an empty vector.
pub fn installed_required_by(name: &str) -> Vec<String> {
    arch_toolkit::deps::get_installed_required_by(name)
}

/// What: Convert a Pacsea package row into toolkit input.
///
/// Inputs:
/// - `item`: Pacsea package row.
///
/// Output:
/// - Equivalent toolkit package reference.
///
/// Details:
/// - Local Pacsea rows retain their official source marker because toolkit package inputs have no
///   separate local variant; resolver behavior recognizes repository `local`.
fn package_ref(item: &PackageItem) -> arch_toolkit::PackageRef {
    match &item.source {
        Source::Official { repo, arch } => arch_toolkit::PackageRef::official(
            item.name.clone(),
            item.version.clone(),
            repo.clone(),
            arch.clone(),
        ),
        Source::Aur => arch_toolkit::PackageRef::aur(item.name.clone(), item.version.clone()),
    }
}

/// What: Convert one toolkit dependency into Pacsea modal state.
///
/// Inputs:
/// - `value`: Toolkit dependency value.
///
/// Output:
/// - Equivalent Pacsea dependency row.
///
/// Details:
/// - All toolkit enums are converted so they cannot leak into UI/event surfaces.
fn dependency(value: arch_toolkit::Dependency) -> DependencyInfo {
    DependencyInfo {
        name: value.name,
        version: value.version_req,
        status: dependency_status(value.status),
        source: dependency_source(value.source),
        required_by: value.required_by,
        depends_on: value.depends_on,
        is_core: value.is_core,
        is_system: value.is_system,
    }
}

/// What: Convert toolkit dependency status into Pacsea modal status.
///
/// Inputs:
/// - `value`: Toolkit status.
///
/// Output:
/// - Equivalent Pacsea status.
///
/// Details:
/// - Variant payloads are preserved exactly.
fn dependency_status(value: arch_toolkit::DependencyStatus) -> DependencyStatus {
    match value {
        arch_toolkit::DependencyStatus::Installed { version } => {
            DependencyStatus::Installed { version }
        }
        arch_toolkit::DependencyStatus::ToInstall => DependencyStatus::ToInstall,
        arch_toolkit::DependencyStatus::ToUpgrade { current, required } => {
            DependencyStatus::ToUpgrade { current, required }
        }
        arch_toolkit::DependencyStatus::Conflict { reason } => {
            DependencyStatus::Conflict { reason }
        }
        arch_toolkit::DependencyStatus::Missing => DependencyStatus::Missing,
    }
}

/// What: Convert toolkit dependency provenance into Pacsea modal provenance.
///
/// Inputs:
/// - `value`: Toolkit source.
///
/// Output:
/// - Equivalent Pacsea source.
///
/// Details:
/// - Repository names remain unchanged for deterministic UI ordering.
fn dependency_source(value: arch_toolkit::DependencySource) -> DependencySource {
    match value {
        arch_toolkit::DependencySource::Official { repo } => DependencySource::Official { repo },
        arch_toolkit::DependencySource::Aur => DependencySource::Aur,
        arch_toolkit::DependencySource::Local => DependencySource::Local,
    }
}

/// What: Convert one toolkit reverse-dependency summary into Pacsea state.
///
/// Inputs:
/// - `value`: Toolkit summary.
///
/// Output:
/// - Equivalent Pacsea summary.
///
/// Details:
/// - Count semantics remain direct depth one versus transitive depth two or greater.
fn reverse_summary(value: arch_toolkit::ReverseDependencySummary) -> ReverseRootSummary {
    ReverseRootSummary {
        package: value.package,
        direct_dependents: value.direct_dependents,
        transitive_dependents: value.transitive_dependents,
        total_dependents: value.total_dependents,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    /// What: Verify supplied virtual providers are honored without host state.
    ///
    /// Inputs:
    /// - Empty installed set and a supplied `rust` provider.
    ///
    /// Output:
    /// - Provider membership resolves as installed.
    ///
    /// Details:
    /// - Regression coverage for the v0.3.0 prerequisite contract.
    #[test]
    fn supplied_provider_membership_is_deterministic() {
        let installed = HashSet::new();
        let provided = HashSet::from(["rust".to_string()]);
        assert!(super::is_installed_or_provided(
            "rust", &installed, &provided
        ));
    }

    /// What: Verify epoch-aware version comparisons are delegated to arch-toolkit.
    ///
    /// Inputs:
    /// - Epoch-bearing installed version and a lower non-epoch requirement.
    ///
    /// Output:
    /// - Requirement is satisfied.
    ///
    /// Details:
    /// - Prevents regression to the former lexical comparison.
    #[test]
    fn version_comparison_handles_epochs() {
        assert!(super::version_satisfies("2:1.0-1", ">=9.9"));
    }

    /// What: Verify the public comparison adapter preserves relational requirement semantics.
    ///
    /// Inputs:
    /// - Greater-than, less-than, equal, unconstrained, and unknown-operator requirements.
    ///
    /// Output:
    /// - The same truth table exposed by arch-toolkit v0.3.0.
    ///
    /// Details:
    /// - Restores broad coverage removed with Pacsea's local version parser.
    #[test]
    fn version_satisfies_relational_matrix() {
        let cases = [
            ("2.0", ">=1.5", true),
            ("1.6", "<1.5", false),
            ("1.5", "=1.5", true),
            ("2.0", "", true),
            ("2.0", "~1.5", true),
        ];

        for (version, requirement, expected) in cases {
            assert_eq!(
                super::version_satisfies(version, requirement),
                expected,
                "unexpected result for {version} against {requirement}"
            );
        }
    }

    /// What: Verify toolkit dependency statuses map one-to-one into Pacsea modal state.
    ///
    /// Inputs:
    /// - Every toolkit status variant, including payload-bearing upgrade and conflict states.
    ///
    /// Output:
    /// - Equivalent Pacsea status variants with unchanged payloads.
    ///
    /// Details:
    /// - Protects the preflight UI contract at the migration conversion boundary.
    #[test]
    fn dependency_status_variants_map_one_to_one() {
        use crate::state::modal::DependencyStatus;

        let cases = [
            (
                arch_toolkit::DependencyStatus::Installed {
                    version: "1.0-1".to_string(),
                },
                DependencyStatus::Installed {
                    version: "1.0-1".to_string(),
                },
            ),
            (
                arch_toolkit::DependencyStatus::ToInstall,
                DependencyStatus::ToInstall,
            ),
            (
                arch_toolkit::DependencyStatus::ToUpgrade {
                    current: "1.0-1".to_string(),
                    required: "2.0-1".to_string(),
                },
                DependencyStatus::ToUpgrade {
                    current: "1.0-1".to_string(),
                    required: "2.0-1".to_string(),
                },
            ),
            (
                arch_toolkit::DependencyStatus::Conflict {
                    reason: "declared conflict".to_string(),
                },
                DependencyStatus::Conflict {
                    reason: "declared conflict".to_string(),
                },
            ),
            (
                arch_toolkit::DependencyStatus::Missing,
                DependencyStatus::Missing,
            ),
        ];

        for (toolkit, expected) in cases {
            assert_eq!(super::dependency_status(toolkit), expected);
        }
    }
}
