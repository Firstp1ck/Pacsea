//! AUR sandbox preflight checks for build dependencies.

use crate::state::types::PackageItem;
use crate::util::percent_encode;
use std::collections::HashSet;
use std::process::{Command, Stdio};

/// What: Information about a dependency's status in the host environment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DependencyDelta {
    /// Package name (may include version requirements)
    pub name: String,
    /// Whether this dependency is installed on the host
    pub is_installed: bool,
    /// Installed version (if available)
    pub installed_version: Option<String>,
    /// Whether the installed version satisfies the requirement
    pub version_satisfied: bool,
}

/// What: Sandbox analysis result for an AUR package.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SandboxInfo {
    /// Package name
    pub package_name: String,
    /// Runtime dependencies (depends)
    pub depends: Vec<DependencyDelta>,
    /// Build-time dependencies (makedepends)
    pub makedepends: Vec<DependencyDelta>,
    /// Test dependencies (checkdepends)
    pub checkdepends: Vec<DependencyDelta>,
    /// Optional dependencies (optdepends)
    pub optdepends: Vec<DependencyDelta>,
}

/// What: Resolve sandbox information for AUR packages.
///
/// Inputs:
/// - `items`: AUR packages to analyze.
///
/// Output:
/// - Vector of `SandboxInfo` entries, one per AUR package.
///
/// Details:
/// - Fetches `.SRCINFO` for each AUR package.
/// - Parses dependencies and compares against host environment.
/// - Returns empty vector if no AUR packages are present.
pub fn resolve_sandbox_info(items: &[PackageItem]) -> Vec<SandboxInfo> {
    let mut results = Vec::new();
    let installed = get_installed_packages();

    for item in items {
        if !matches!(item.source, crate::state::Source::Aur) {
            continue;
        }

        match fetch_and_analyze_package(&item.name, &installed) {
            Ok(info) => results.push(info),
            Err(e) => {
                tracing::warn!("Failed to analyze sandbox info for {}: {}", item.name, e);
            }
        }
    }

    results
}

/// What: Fetch and analyze a single AUR package's build dependencies.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `installed`: Set of installed package names.
///
/// Output:
/// - `SandboxInfo` with dependency deltas, or an error if fetch/parse fails.
fn fetch_and_analyze_package(
    package_name: &str,
    installed: &HashSet<String>,
) -> Result<SandboxInfo, String> {
    // Try to fetch .SRCINFO first (more reliable)
    let srcinfo_text = match fetch_srcinfo(package_name) {
        Ok(text) => text,
        Err(_) => {
            // Fallback to PKGBUILD if .SRCINFO fails
            tracing::debug!(
                "Failed to fetch .SRCINFO for {}, trying PKGBUILD",
                package_name
            );
            crate::logic::files::fetch_pkgbuild_sync(package_name)?
        }
    };

    // Parse dependencies from .SRCINFO or PKGBUILD
    let (depends, makedepends, checkdepends, optdepends) =
        if srcinfo_text.contains("pkgbase =") || srcinfo_text.contains("pkgname =") {
            // Looks like .SRCINFO format
            parse_srcinfo_deps(&srcinfo_text)
        } else {
            // Assume PKGBUILD format - extract dependencies
            parse_pkgbuild_deps(&srcinfo_text)
        };

    // Analyze each dependency against host environment
    let depends_delta = analyze_dependencies(&depends, installed);
    let makedepends_delta = analyze_dependencies(&makedepends, installed);
    let checkdepends_delta = analyze_dependencies(&checkdepends, installed);
    let optdepends_delta = analyze_dependencies(&optdepends, installed);

    Ok(SandboxInfo {
        package_name: package_name.to_string(),
        depends: depends_delta,
        makedepends: makedepends_delta,
        checkdepends: checkdepends_delta,
        optdepends: optdepends_delta,
    })
}

/// What: Fetch .SRCINFO content for an AUR package.
///
/// Inputs:
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
fn fetch_srcinfo(name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let output = Command::new("curl")
        .args(["-sSLf", &url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status: {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    Ok(text)
}

/// What: Parse dependencies from .SRCINFO content.
///
/// Inputs:
/// - `srcinfo`: Raw .SRCINFO file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
fn parse_srcinfo_deps(srcinfo: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    for line in srcinfo.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // .SRCINFO format: key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Filter out virtual packages (.so files)
            if value.ends_with(".so") || value.contains(".so.") || value.contains(".so=") {
                continue;
            }

            match key {
                "depends" => depends.push(value.to_string()),
                "makedepends" => makedepends.push(value.to_string()),
                "checkdepends" => checkdepends.push(value.to_string()),
                "optdepends" => optdepends.push(value.to_string()),
                _ => {}
            }
        }
    }

    (depends, makedepends, checkdepends, optdepends)
}

/// What: Parse dependencies from PKGBUILD content.
///
/// Inputs:
/// - `pkgbuild`: Raw PKGBUILD file content.
///
/// Output:
/// - Returns a tuple of (depends, makedepends, checkdepends, optdepends) vectors.
///
/// Details:
/// - Parses bash array syntax: `depends=('foo' 'bar>=1.2')`
fn parse_pkgbuild_deps(pkgbuild: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    for line in pkgbuild.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse array declarations: depends=('foo' 'bar')
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Extract array content (handle both single-line and multi-line)
            if value.starts_with('(') {
                let array_content = if value.ends_with(')') {
                    // Single-line array
                    &value[1..value.len() - 1]
                } else {
                    // Multi-line array - would need more complex parsing
                    // For now, just extract from single line
                    continue;
                };

                // Parse quoted strings from array
                let deps = parse_array_content(array_content);
                match key {
                    "depends" => depends.extend(deps),
                    "makedepends" => makedepends.extend(deps),
                    "checkdepends" => checkdepends.extend(deps),
                    "optdepends" => optdepends.extend(deps),
                    _ => {}
                }
            }
        }
    }

    (depends, makedepends, checkdepends, optdepends)
}

/// What: Parse quoted strings from bash array content.
///
/// Inputs:
/// - `content`: Array content string (e.g., "'foo' 'bar>=1.2'").
///
/// Output:
/// - Vector of dependency strings.
fn parse_array_content(content: &str) -> Vec<String> {
    let mut deps = Vec::new();
    let mut in_quotes = false;
    let mut quote_char = '\0';
    let mut current = String::new();

    for ch in content.chars() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    if !current.is_empty() {
                        deps.push(current.clone());
                        current.clear();
                    }
                    in_quotes = false;
                    quote_char = '\0';
                } else {
                    current.push(ch);
                }
            }
            _ if in_quotes => {
                current.push(ch);
            }
            _ => {
                // Skip whitespace outside quotes
            }
        }
    }

    // Handle unclosed quote
    if !current.is_empty() && in_quotes {
        deps.push(current);
    }

    deps
}

/// What: Analyze dependencies against the host environment.
///
/// Inputs:
/// - `deps`: Vector of dependency specifications.
/// - `installed`: Set of installed package names.
///
/// Output:
/// - Vector of `DependencyDelta` entries showing status of each dependency.
///
/// Details:
/// - Skips local packages entirely.
fn analyze_dependencies(deps: &[String], installed: &HashSet<String>) -> Vec<DependencyDelta> {
    deps.iter()
        .filter_map(|dep_spec| {
            // Extract package name (may include version requirements)
            let pkg_name = extract_package_name(dep_spec);
            let is_installed = installed.contains(&pkg_name);

            // Skip local packages - they're not relevant for sandbox analysis
            if is_installed && is_local_package(&pkg_name) {
                return None;
            }

            // Try to get installed version
            let installed_version = if is_installed {
                crate::logic::deps::get_installed_version(&pkg_name).ok()
            } else {
                None
            };

            // Check if version requirement is satisfied
            let version_satisfied = if let Some(ref version) = installed_version {
                crate::logic::deps::version_satisfies(version, dep_spec)
            } else {
                false
            };

            Some(DependencyDelta {
                name: dep_spec.clone(),
                is_installed,
                installed_version,
                version_satisfied,
            })
        })
        .collect()
}

/// What: Extract package name from a dependency specification.
///
/// Inputs:
/// - `dep_spec`: Dependency specification (e.g., "foo>=1.2", "bar", "baz: description").
///
/// Output:
/// - Package name without version requirements or description.
pub fn extract_package_name(dep_spec: &str) -> String {
    // Handle optdepends format: "package: description"
    let name = if let Some(colon_pos) = dep_spec.find(':') {
        &dep_spec[..colon_pos]
    } else {
        dep_spec
    };

    // Remove version operators: >=, <=, ==, >, <
    name.trim()
        .split(">=")
        .next()
        .unwrap_or(name)
        .split("<=")
        .next()
        .unwrap_or(name)
        .split("==")
        .next()
        .unwrap_or(name)
        .split('>')
        .next()
        .unwrap_or(name)
        .split('<')
        .next()
        .unwrap_or(name)
        .trim()
        .to_string()
}

/// What: Check if a package is a local package.
///
/// Inputs:
/// - `name`: Package name to check.
///
/// Output:
/// - `true` if the package is local, `false` otherwise.
fn is_local_package(name: &str) -> bool {
    let output = Command::new("pacman")
        .args(["-Qi", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            // Look for "Repository" field in pacman -Qi output
            for line in text.lines() {
                if line.starts_with("Repository")
                    && let Some(colon_pos) = line.find(':')
                {
                    let repo = line[colon_pos + 1..].trim().to_lowercase();
                    return repo == "local" || repo.is_empty();
                }
            }
        }
        _ => {
            // If we can't determine, assume it's not local
            return false;
        }
    }

    false
}

/// What: Get the set of installed packages.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Set of installed package names.
fn get_installed_packages() -> HashSet<String> {
    crate::logic::deps::get_installed_packages()
}
