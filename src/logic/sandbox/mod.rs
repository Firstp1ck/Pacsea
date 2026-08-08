//! AUR sandbox preflight checks through the arch-toolkit integration boundary.

use crate::integrations::arch_toolkit::ToolkitContext;
use crate::state::types::PackageItem;

/// What: Information about one dependency relative to the host environment.
///
/// Inputs:
/// - Produced by arch-toolkit sandbox analysis.
///
/// Output:
/// - Persisted installation and version-satisfaction state.
///
/// Details:
/// - The shape remains compatible with existing Pacsea cache files.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependencyDelta {
    /// Dependency specification as declared.
    pub name: String,
    /// Whether the dependency is installed or provided.
    pub is_installed: bool,
    /// Installed version when available.
    pub installed_version: Option<String>,
    /// Whether the installed version satisfies the declared constraint.
    pub version_satisfied: bool,
}

/// What: Sandbox dependency analysis for one AUR package.
///
/// Inputs:
/// - Produced from bounded `.SRCINFO` or PKGBUILD text.
///
/// Output:
/// - Persisted dependency deltas grouped by package metadata category.
///
/// Details:
/// - Fetching, cache fallback, scanners, and UI remain Pacsea-owned.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxInfo {
    /// Package name.
    pub package_name: String,
    /// Runtime dependencies.
    pub depends: Vec<DependencyDelta>,
    /// Build-time dependencies.
    pub makedepends: Vec<DependencyDelta>,
    /// Test dependencies.
    pub checkdepends: Vec<DependencyDelta>,
    /// Optional dependencies.
    pub optdepends: Vec<DependencyDelta>,
}

/// What: Resolve sandbox information with a standalone toolkit context.
///
/// Inputs:
/// - `items`: Packages selected for preflight.
///
/// Output:
/// - One sandbox record per AUR package.
///
/// Details:
/// - Runtime workers use [`resolve_sandbox_info_with_context`] to share configured clients.
pub async fn resolve_sandbox_info_async(items: &[PackageItem]) -> Vec<SandboxInfo> {
    match ToolkitContext::new() {
        Ok(context) => resolve_sandbox_info_with_context(&context, items).await,
        Err(error) => {
            tracing::warn!(error = %error, "sandbox client initialization failed");
            Vec::new()
        }
    }
}

/// What: Resolve sandbox information with a caller-owned shared toolkit context.
///
/// Inputs:
/// - `context`: Runtime integration context.
/// - `items`: Packages selected for preflight.
///
/// Output:
/// - One sandbox record per AUR package.
///
/// Details:
/// - arch-toolkit owns parsing and host comparison; Pacsea owns fetch fallback and state.
pub(crate) async fn resolve_sandbox_info_with_context(
    context: &ToolkitContext,
    items: &[PackageItem],
) -> Vec<SandboxInfo> {
    crate::integrations::arch_toolkit::sandbox::resolve(context, items).await
}

/// What: Parse PKGBUILD dependency categories through arch-toolkit.
///
/// Inputs:
/// - `text`: PKGBUILD text that is never executed here.
///
/// Output:
/// - Runtime, build, check, and optional dependency specs.
///
/// Details:
/// - Pure parsing preserves the existing Pacsea helper surface.
#[must_use]
pub fn parse_pkgbuild_deps(text: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    arch_toolkit::deps::parse_pkgbuild_deps(text)
}

/// What: Parse PKGBUILD conflicts through arch-toolkit.
///
/// Inputs:
/// - `text`: PKGBUILD text that is never executed here.
///
/// Output:
/// - Conflicting package names.
///
/// Details:
/// - Version constraints are normalized by the toolkit parser.
#[must_use]
pub fn parse_pkgbuild_conflicts(text: &str) -> Vec<String> {
    arch_toolkit::deps::parse_pkgbuild_conflicts(text)
}

/// What: Extract a package name from a dependency specification through arch-toolkit.
///
/// Inputs:
/// - `dependency`: Versioned or annotated dependency specification.
///
/// Output:
/// - Bare package name.
///
/// Details:
/// - Handles optional-dependency descriptions before comparison operators.
#[must_use]
pub fn extract_package_name(dependency: &str) -> String {
    arch_toolkit::sandbox::extract_package_name(dependency)
}

#[cfg(test)]
mod tests {
    /// What: Verify Pacsea's public parser wrapper delegates all dependency categories.
    ///
    /// Inputs:
    /// - Deterministic PKGBUILD text.
    ///
    /// Output:
    /// - Expected runtime and build dependency lists.
    ///
    /// Details:
    /// - The PKGBUILD is parsed as text and never executed.
    #[test]
    fn pkgbuild_parser_delegates_to_toolkit() {
        let (depends, make, check, optional) =
            super::parse_pkgbuild_deps("depends=('glibc')\nmakedepends=('rust')");
        assert_eq!(depends, vec!["glibc"]);
        assert_eq!(make, vec!["rust"]);
        assert!(check.is_empty());
        assert!(optional.is_empty());
    }

    /// What: Verify dependency-name extraction handles annotations and versions.
    ///
    /// Inputs:
    /// - Optional dependency with a version and description.
    ///
    /// Output:
    /// - Bare package name.
    ///
    /// Details:
    /// - Protects UI optional-dependency matching.
    #[test]
    fn dependency_name_extraction_delegates_to_toolkit() {
        assert_eq!(
            super::extract_package_name("python>=3.12: scripting"),
            "python"
        );
    }
}
