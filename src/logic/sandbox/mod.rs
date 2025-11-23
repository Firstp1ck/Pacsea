//! AUR sandbox preflight checks for build dependencies.

mod analyze;
mod fetch;
mod parse;
mod types;

#[cfg(test)]
mod tests;

pub use analyze::extract_package_name;
pub use parse::parse_pkgbuild_deps;
pub use types::{DependencyDelta, SandboxInfo};

use crate::logic::sandbox::analyze::{
    analyze_package_from_pkgbuild, analyze_package_from_srcinfo, get_installed_packages,
};
use crate::logic::sandbox::fetch::fetch_srcinfo_async;
use crate::state::types::PackageItem;
use futures::stream::{FuturesUnordered, StreamExt};

/// What: Create an empty `SandboxInfo` for a package when analysis fails.
///
/// Inputs:
/// - `name`: Package name.
///
/// Output:
/// - Empty `SandboxInfo` with the package name.
///
/// Details:
/// - Used as fallback when analysis fails to ensure package appears in results.
#[allow(clippy::missing_const_for_fn)]
fn create_empty_sandbox_info(name: String) -> SandboxInfo {
    SandboxInfo {
        package_name: name,
        depends: Vec::new(),
        makedepends: Vec::new(),
        checkdepends: Vec::new(),
        optdepends: Vec::new(),
    }
}

/// What: Handle .SRCINFO analysis for a package.
///
/// Inputs:
/// - `name`: Package name.
/// - `srcinfo_text`: .SRCINFO file content.
/// - `installed`: Installed packages set.
/// - `provided`: Provided packages set.
///
/// Output:
/// - `SandboxInfo` on success.
///
/// Details:
/// - Analyzes dependencies from .SRCINFO and creates `SandboxInfo`.
fn handle_srcinfo_analysis(
    name: &str,
    srcinfo_text: &str,
    installed: &std::collections::HashSet<String>,
    provided: &std::collections::HashSet<String>,
) -> SandboxInfo {
    analyze_package_from_srcinfo(name, srcinfo_text, installed, provided)
}

/// What: Handle PKGBUILD fallback analysis for a package.
///
/// Inputs:
/// - `name`: Package name.
/// - `pkgbuild_text`: PKGBUILD file content.
/// - `installed`: Installed packages set.
/// - `provided`: Provided packages set.
///
/// Output:
/// - `SandboxInfo` on success.
///
/// Details:
/// - Analyzes dependencies from PKGBUILD when .SRCINFO is unavailable.
fn handle_pkgbuild_analysis(
    name: &str,
    pkgbuild_text: &str,
    installed: &std::collections::HashSet<String>,
    provided: &std::collections::HashSet<String>,
) -> SandboxInfo {
    let info = analyze_package_from_pkgbuild(name, pkgbuild_text, installed, provided);
    let total_deps = info.depends.len()
        + info.makedepends.len()
        + info.checkdepends.len()
        + info.optdepends.len();
    tracing::info!(
        "Parsed PKGBUILD for {}: {} total dependencies (depends={}, makedepends={}, checkdepends={}, optdepends={})",
        name,
        total_deps,
        info.depends.len(),
        info.makedepends.len(),
        info.checkdepends.len(),
        info.optdepends.len()
    );
    info
}

/// What: Process a single AUR package to resolve sandbox information.
///
/// Inputs:
/// - `name`: Package name.
/// - `client`: HTTP client for fetching.
/// - `installed`: Installed packages set.
/// - `provided`: Provided packages set.
///
/// Output:
/// - `Some(SandboxInfo)` if resolved, `None` otherwise.
///
/// Details:
/// - Tries .SRCINFO first, falls back to PKGBUILD if needed.
async fn process_sandbox_package(
    name: String,
    client: reqwest::Client,
    installed: std::collections::HashSet<String>,
    provided: std::collections::HashSet<String>,
) -> Option<SandboxInfo> {
    match fetch_srcinfo_async(&client, &name).await {
        Ok(srcinfo_text) => Some(handle_srcinfo_analysis(
            &name,
            &srcinfo_text,
            &installed,
            &provided,
        )),
        Err(e) => {
            tracing::debug!(
                "Failed to fetch .SRCINFO for {}: {}, trying PKGBUILD",
                name,
                e
            );
            let name_for_fallback = name.clone();
            let installed_for_fallback = installed.clone();
            let provided_for_fallback = provided.clone();
            match tokio::task::spawn_blocking(move || {
                crate::logic::files::fetch_pkgbuild_sync(&name_for_fallback)
            })
            .await
            {
                Ok(Ok(pkgbuild_text)) => {
                    tracing::debug!(
                        "Successfully fetched PKGBUILD for {}, parsing dependencies",
                        name
                    );
                    Some(handle_pkgbuild_analysis(
                        &name,
                        &pkgbuild_text,
                        &installed_for_fallback,
                        &provided_for_fallback,
                    ))
                }
                Ok(Err(e)) => {
                    tracing::warn!("Failed to fetch PKGBUILD for {}: {}", name, e);
                    tracing::info!(
                        "Creating empty sandbox info for {} (both .SRCINFO and PKGBUILD fetch failed)",
                        name
                    );
                    Some(create_empty_sandbox_info(name))
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to spawn blocking task for PKGBUILD fetch for {}: {}",
                        name,
                        e
                    );
                    tracing::info!(
                        "Creating empty sandbox info for {} (spawn task failed)",
                        name
                    );
                    Some(create_empty_sandbox_info(name))
                }
            }
        }
    }
}

/// What: Resolve sandbox information for AUR packages using async HTTP.
///
/// Inputs:
/// - `items`: AUR packages to analyze.
///
/// Output:
/// - Vector of `SandboxInfo` entries, one per AUR package.
///
/// Details:
/// - Fetches `.SRCINFO` for each AUR package in parallel using async HTTP.
/// - Parses dependencies and compares against host environment.
/// - Returns empty vector if no AUR packages are present.
pub async fn resolve_sandbox_info_async(items: &[PackageItem]) -> Vec<SandboxInfo> {
    let aur_items: Vec<_> = items
        .iter()
        .filter(|i| matches!(i.source, crate::state::Source::Aur))
        .collect();
    let span = tracing::info_span!(
        "resolve_sandbox_info",
        stage = "sandbox",
        item_count = aur_items.len()
    );
    let _guard = span.enter();
    let start_time = std::time::Instant::now();

    let installed = get_installed_packages();
    let provided = crate::logic::deps::get_provided_packages(&installed);

    // Fetch all .SRCINFO files in parallel
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut fetch_futures = FuturesUnordered::new();
    for item in items {
        if matches!(item.source, crate::state::Source::Aur) {
            let name = item.name.clone();
            let installed_clone = installed.clone();
            let provided_clone = provided.clone();
            let client_clone = client.clone();
            fetch_futures.push(process_sandbox_package(
                name,
                client_clone,
                installed_clone,
                provided_clone,
            ));
        }
    }

    // Collect all results as they complete
    let mut results = Vec::new();
    while let Some(result) = fetch_futures.next().await {
        if let Some(info) = result {
            results.push(info);
        }
    }

    let elapsed = start_time.elapsed();
    let duration_ms = u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX);
    tracing::info!(
        stage = "sandbox",
        item_count = aur_items.len(),
        result_count = results.len(),
        duration_ms = duration_ms,
        "Sandbox resolution complete"
    );
    results
}

/// What: Resolve sandbox information for AUR packages (synchronous wrapper for async version).
///
/// Inputs:
/// - `items`: AUR packages to analyze.
///
/// Output:
/// - Vector of `SandboxInfo` entries, one per AUR package.
///
/// # Panics
/// - Panics if a tokio runtime cannot be created when no runtime handle is available
///
/// Details:
/// - Wraps the async version for use in blocking contexts.
#[must_use]
pub fn resolve_sandbox_info(items: &[PackageItem]) -> Vec<SandboxInfo> {
    // Use tokio runtime handle if available, otherwise create a new runtime
    tokio::runtime::Handle::try_current().map_or_else(
        |_| {
            // No runtime available, create a new one
            let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to create tokio runtime for sandbox resolution: {}",
                    e
                );
                panic!("Cannot resolve sandbox info without tokio runtime");
            });
            rt.block_on(resolve_sandbox_info_async(items))
        },
        |handle| handle.block_on(resolve_sandbox_info_async(items)),
    )
}
