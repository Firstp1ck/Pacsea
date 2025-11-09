//! AUR-specific dependency resolution.

use super::parse::parse_dep_spec;
use super::source::{determine_dependency_source, is_system_package};
use super::status::determine_status;
use crate::state::modal::DependencyInfo;
use serde_json::Value;
use std::collections::HashSet;
use std::process::Command;

/// What: Retrieve dependency metadata for an AUR package via the RPC API.
///
/// Inputs:
/// - `name`: Target AUR package name.
/// - `installed`: Set of installed package names for status evaluation.
/// - `upgradable`: Set of upgradable packages used to detect pending upgrades.
///
/// Output:
/// - Returns a vector of `DependencyInfo` records or an error string when the request fails.
///
/// Details:
/// - Acts as a fallback path when helpers such as `paru` or `yay` are unavailable, parsing JSON directly.
pub(crate) fn fetch_aur_deps_from_api(
    name: &str,
    installed: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
    tracing::debug!("Fetching dependencies from AUR API for: {}", name);
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/info?arg={}",
        crate::util::percent_encode(name)
    );

    // Use curl_json similar to sources module
    let out = Command::new("curl")
        .args(["-sSLf", &url])
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !out.status.success() {
        return Err(format!("curl failed with status: {:?}", out.status.code()));
    }

    let body = String::from_utf8_lossy(&out.stdout);
    let v: Value =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "No 'results' array in AUR API response".to_string())?;

    let obj = arr
        .first()
        .ok_or_else(|| format!("No package found in AUR API for: {}", name))?;

    // Get dependencies from the API response
    let depends: Vec<String> = obj
        .get("Depends")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    tracing::debug!("Found {} dependencies from AUR API", depends.len());

    let mut deps = Vec::new();
    for dep_spec in depends {
        let (pkg_name, version_req) = parse_dep_spec(&dep_spec);

        // Filter out .so files (virtual packages) - they're not actual package dependencies
        // Patterns: "libgit2.so", "libedit.so=0-64", "libfoo.so.1"
        if pkg_name.ends_with(".so") || pkg_name.contains(".so.") || pkg_name.contains(".so=") {
            tracing::debug!("Filtering out virtual package: {}", pkg_name);
            continue;
        }

        let status = determine_status(&pkg_name, &version_req, installed, upgradable);

        // Determine source and repository
        let (source, is_core) = determine_dependency_source(&pkg_name, installed);
        let is_system = is_core || is_system_package(&pkg_name);

        deps.push(DependencyInfo {
            name: pkg_name,
            version: version_req,
            status,
            source,
            required_by: vec![name.to_string()],
            depends_on: Vec::new(),
            is_core,
            is_system,
        });
    }

    Ok(deps)
}
