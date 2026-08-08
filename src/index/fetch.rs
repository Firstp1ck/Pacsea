//! Official package index fetching through arch-toolkit.

#[cfg(not(windows))]
use super::OfficialPkg;

/// What: Fetch enabled official and configured repository packages through arch-toolkit.
///
/// Inputs:
/// - None; repository names come from pacman configuration and Pacsea `repos.conf` additions.
///
/// Output:
/// - Minimal official package rows with repository, name, and version fields.
///
/// Details:
/// - arch-toolkit performs locale-stable `pacman -Sl` queries, deduplication, and name-index
///   rebuilding. Pacsea retains distro/custom-repository policy and later enrichment.
#[cfg(not(windows))]
pub async fn fetch_official_pkg_names()
-> Result<Vec<OfficialPkg>, Box<dyn std::error::Error + Send + Sync>> {
    crate::integrations::arch_toolkit::index::fetch_official_packages()
        .await
        .map_err(Into::into)
}
