//! Sandbox dependency and static-analysis adapters.

use std::collections::HashSet;

use futures::stream::{FuturesUnordered, StreamExt};

use crate::logic::sandbox::{DependencyDelta, SandboxInfo};
use crate::state::{PackageItem, Source};

use super::ToolkitContext;

/// What: Resolve sandbox dependency information for AUR packages.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `items`: Pacsea package rows.
///
/// Output:
/// - One Pacsea sandbox record per AUR package.
///
/// Details:
/// - `.SRCINFO` is fetched with toolkit's streamed 10 MiB bound; Pacsea retains cached/network
///   PKGBUILD fallback and never executes either document.
pub async fn resolve(context: &ToolkitContext, items: &[PackageItem]) -> Vec<SandboxInfo> {
    let installed = crate::integrations::arch_toolkit::deps::installed_packages();
    let provided = crate::integrations::arch_toolkit::deps::provided_packages(&installed);
    let mut futures = FuturesUnordered::new();
    for item in items
        .iter()
        .filter(|item| matches!(item.source, Source::Aur))
    {
        futures.push(analyze_package(
            context.clone(),
            item.name.clone(),
            installed.clone(),
            provided.clone(),
        ));
    }

    let mut results = Vec::new();
    while let Some(result) = futures.next().await {
        results.push(result);
    }
    results.sort_by(|left, right| left.package_name.cmp(&right.package_name));
    results
}

/// What: Analyze one AUR package with bounded metadata and fallback text.
///
/// Inputs:
/// - `context`: Shared configured toolkit clients.
/// - `package_name`: AUR package name.
/// - `installed`: Installed package names.
/// - `provided`: Supplied virtual provider names.
///
/// Output:
/// - Pacsea sandbox information, empty only when both metadata paths fail.
///
/// Details:
/// - PKGBUILD content is passed only to deterministic parsers and is never sourced or executed.
async fn analyze_package(
    context: ToolkitContext,
    package_name: String,
    installed: HashSet<String>,
    provided: HashSet<String>,
) -> SandboxInfo {
    match arch_toolkit::deps::fetch_srcinfo(context.http_client(), &package_name).await {
        Ok(srcinfo) => sandbox_info(arch_toolkit::sandbox::analyze_srcinfo(
            &package_name,
            &srcinfo,
            &installed,
            &provided,
        )),
        Err(srcinfo_error) => {
            tracing::debug!(
                package = %package_name,
                error = %srcinfo_error,
                "bounded .SRCINFO fetch failed; trying Pacsea PKGBUILD fallback"
            );
            analyze_pkgbuild_fallback(package_name, installed, provided).await
        }
    }
}

/// What: Analyze Pacsea's PKGBUILD fallback through arch-toolkit.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `installed`: Installed package names.
/// - `provided`: Supplied virtual provider names.
///
/// Output:
/// - Converted sandbox analysis or an empty named record.
///
/// Details:
/// - Fetching remains in Pacsea so helper cache behavior is unchanged.
async fn analyze_pkgbuild_fallback(
    package_name: String,
    installed: HashSet<String>,
    provided: HashSet<String>,
) -> SandboxInfo {
    let fetch_name = package_name.clone();
    match tokio::task::spawn_blocking(move || crate::logic::files::fetch_pkgbuild_sync(&fetch_name))
        .await
    {
        Ok(Ok(pkgbuild)) => sandbox_info(arch_toolkit::sandbox::analyze_pkgbuild(
            &package_name,
            &pkgbuild,
            &installed,
            &provided,
        )),
        Ok(Err(error)) => {
            tracing::warn!(
                package = %package_name,
                error = %error,
                "PKGBUILD fallback unavailable; install paru or yay and retry"
            );
            empty_info(package_name)
        }
        Err(error) => {
            tracing::warn!(
                package = %package_name,
                error = %error,
                "PKGBUILD fallback worker failed; retry the sandbox analysis"
            );
            empty_info(package_name)
        }
    }
}

/// What: Analyze PKGBUILD dependencies through arch-toolkit.
///
/// Inputs:
/// - `package_name`: Package name for the report.
/// - `text`: PKGBUILD text that is never executed here.
/// - `installed`: Installed package names.
/// - `provided`: Virtual provider names.
///
/// Output:
/// - Pacsea sandbox information.
///
/// Details:
/// - Exposed for deterministic parity tests and cache-driven callers.
#[cfg(test)]
pub fn analyze_pkgbuild(
    package_name: &str,
    text: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> SandboxInfo {
    sandbox_info(arch_toolkit::sandbox::analyze_pkgbuild(
        package_name,
        text,
        installed,
        provided,
    ))
}

/// What: Convert toolkit sandbox output into Pacsea persisted state.
///
/// Inputs:
/// - `value`: Toolkit sandbox information.
///
/// Output:
/// - Equivalent Pacsea sandbox information.
///
/// Details:
/// - The persisted field names and dependency-spec values remain unchanged.
fn sandbox_info(value: arch_toolkit::SandboxInfo) -> SandboxInfo {
    SandboxInfo {
        package_name: value.package_name,
        depends: value.depends.into_iter().map(dependency_delta).collect(),
        makedepends: value
            .makedepends
            .into_iter()
            .map(dependency_delta)
            .collect(),
        checkdepends: value
            .checkdepends
            .into_iter()
            .map(dependency_delta)
            .collect(),
        optdepends: value.optdepends.into_iter().map(dependency_delta).collect(),
    }
}

/// What: Convert one toolkit dependency delta into Pacsea persisted state.
///
/// Inputs:
/// - `value`: Toolkit delta.
///
/// Output:
/// - Equivalent Pacsea delta.
///
/// Details:
/// - Version satisfaction is preserved and now uses toolkit's Arch version comparator.
fn dependency_delta(value: arch_toolkit::DependencyDelta) -> DependencyDelta {
    DependencyDelta {
        name: value.name,
        is_installed: value.is_installed,
        installed_version: value.installed_version,
        version_satisfied: value.version_satisfied,
    }
}

/// What: Create a named empty fallback record.
///
/// Inputs:
/// - `package_name`: Package whose metadata could not be fetched.
///
/// Output:
/// - Empty Pacsea sandbox information.
///
/// Details:
/// - Ensures every requested AUR package remains visible in preflight results.
const fn empty_info(package_name: String) -> SandboxInfo {
    SandboxInfo {
        package_name,
        depends: Vec::new(),
        makedepends: Vec::new(),
        checkdepends: Vec::new(),
        optdepends: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    /// What: Verify PKGBUILD analysis preserves categories and provider membership.
    ///
    /// Inputs:
    /// - Deterministic PKGBUILD with runtime and build dependencies.
    /// - Caller-supplied installed and virtual provider sets.
    ///
    /// Output:
    /// - Converted deltas with expected installation state.
    ///
    /// Details:
    /// - Performs no network or package execution.
    #[test]
    fn pkgbuild_analysis_uses_toolkit_and_preserves_providers() {
        let installed = HashSet::from(["glibc".to_string()]);
        let provided = HashSet::from(["rust".to_string()]);
        let info = super::analyze_pkgbuild(
            "demo",
            "depends=('glibc')\nmakedepends=('rust')",
            &installed,
            &provided,
        );
        assert_eq!(info.package_name, "demo");
        assert!(info.depends[0].is_installed);
        assert!(info.makedepends[0].is_installed);
    }
}
