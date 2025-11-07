//! Dependency resolution and analysis for preflight checks.

use crate::state::modal::{DependencyInfo, DependencySource, DependencyStatus};
use crate::state::types::{PackageItem, Source};
use std::collections::HashSet;
use std::process::Command;
use serde_json::Value;

/// Resolve dependencies for a list of packages to install.
///
/// This function queries pacman/paru to determine all dependencies (direct and transitive)
/// and checks their installation status on the system.
///
/// Inputs:
/// - `items`: List of packages to install
///
/// Output:
/// - Vector of `DependencyInfo` with resolved status for each dependency
pub fn resolve_dependencies(items: &[PackageItem]) -> Vec<DependencyInfo> {
    tracing::info!("Starting dependency resolution for {} package(s)", items.len());
    
    if items.is_empty() {
        tracing::warn!("No packages provided for dependency resolution");
        return Vec::new();
    }

    let mut deps = Vec::new();
    let mut seen = HashSet::new();

    // Get installed packages set
    tracing::info!("Fetching list of installed packages...");
    let installed = get_installed_packages();
    tracing::info!("Found {} installed packages", installed.len());
    
    // Get list of upgradable packages to detect if dependencies need upgrades
    let upgradable = get_upgradable_packages();
    tracing::info!("Found {} upgradable packages", upgradable.len());

    // For each package, resolve its dependencies
    for (idx, item) in items.iter().enumerate() {
        tracing::info!("[{}/{}] Resolving dependencies for package: {} ({:?})", 
                      idx + 1, items.len(), item.name, item.source);
        
        match resolve_package_deps(&item.name, &item.source, &installed, &upgradable) {
            Ok(mut resolved) => {
                tracing::info!("  Found {} dependencies for {}", resolved.len(), item.name);
                for dep in resolved.drain(..) {
                    let dep_name = dep.name.clone();
                    if let Some(existing_idx) = deps.iter().position(|d: &DependencyInfo| d.name == dep_name) {
                        // Merge required_by lists for duplicate dependencies
                        let existing = &mut deps[existing_idx];
                        for req in &dep.required_by {
                            if !existing.required_by.contains(req) {
                                existing.required_by.push(req.clone());
                            }
                        }
                        // Keep the "worst" status when merging (Conflict > Missing > ToUpgrade > ToInstall > Installed)
                        let existing_priority = dependency_priority(&existing.status);
                        let new_priority = dependency_priority(&dep.status);
                        
                        // Update version requirement if needed and re-evaluate status
                        let version_changed = if !dep.version.is_empty() && dep.version != existing.version {
                            if existing.version.is_empty() {
                                // Existing has no version requirement, use the new one
                                existing.version = dep.version.clone();
                                true
                            } else {
                                // Both have version requirements - check which one is more restrictive
                                // by evaluating both and keeping the one that results in worse status
                                let existing_status = determine_status(&existing.name, &existing.version, &installed, &upgradable);
                                let new_status = determine_status(&existing.name, &dep.version, &installed, &upgradable);
                                let existing_req_priority = dependency_priority(&existing_status);
                                let new_req_priority = dependency_priority(&new_status);
                                
                                if new_req_priority < existing_req_priority {
                                    // New requirement is more restrictive (results in worse status)
                                    existing.version = dep.version.clone();
                                    true
                                } else {
                                    // Existing requirement is more restrictive, keep it
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        
                        // If version requirement changed, we need to re-evaluate the status
                        if version_changed {
                            // Re-evaluate status based on the updated version requirement
                            existing.status = determine_status(&existing.name, &existing.version, &installed, &upgradable);
                        } else if new_priority < existing_priority {
                            // New status is worse (lower priority number), so update it
                            existing.status = dep.status.clone();
                        }
                        tracing::debug!("    Merged dependency: {} (now required by: {:?}, status: {:?})", dep_name, existing.required_by, existing.status);
                    } else {
                        seen.insert(dep_name.clone());
                        tracing::debug!("    Added dependency: {} ({:?})", dep_name, dep.status);
                        deps.push(dep);
                    }
                }
            }
            Err(e) => {
                tracing::warn!("  Failed to resolve dependencies for {}: {}", item.name, e);
            }
        }
    }

    tracing::info!("Total unique dependencies found: {}", deps.len());

    // Sort dependencies: conflicts first, then missing, then to-install, then installed
    deps.sort_by(|a, b| {
        let priority_a = dependency_priority(&a.status);
        let priority_b = dependency_priority(&b.status);
        priority_a.cmp(&priority_b).then_with(|| a.name.cmp(&b.name))
    });

    tracing::info!("Dependency resolution complete. Returning {} dependencies", deps.len());
    deps
}

/// Get a set of upgradable package names.
fn get_upgradable_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qu");
    let output = Command::new("pacman")
        .args(["-Qu"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                // pacman -Qu outputs "name old-version -> new-version" or just "name" for AUR packages
                let packages: HashSet<String> = text
                    .lines()
                    .filter_map(|line| {
                        let line = line.trim();
                        if line.is_empty() {
                            return None;
                        }
                        // Extract package name (everything before space or "->")
                        if let Some(space_pos) = line.find(' ') {
                            Some(line[..space_pos].trim().to_string())
                        } else {
                            Some(line.to_string())
                        }
                    })
                    .collect();
                tracing::debug!("Successfully retrieved {} upgradable packages", packages.len());
                packages
            } else {
                // No upgradable packages or error - return empty set
                HashSet::new()
            }
        }
        Err(e) => {
            tracing::debug!("Failed to execute pacman -Qu: {} (assuming no upgrades)", e);
            HashSet::new()
        }
    }
}

/// Get a set of installed package names.
fn get_installed_packages() -> HashSet<String> {
    tracing::debug!("Running: pacman -Qq");
    let output = Command::new("pacman")
        .args(["-Qq"])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output();

    match output {
        Ok(output) => {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                let packages: HashSet<String> = text.lines().map(|s| s.trim().to_string()).collect();
                tracing::debug!("Successfully retrieved {} installed packages", packages.len());
                packages
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!("pacman -Qq failed with status {:?}: {}", output.status.code(), stderr);
                HashSet::new()
            }
        }
        Err(e) => {
            tracing::error!("Failed to execute pacman -Qq: {}", e);
            HashSet::new()
        }
    }
}

/// Fetch dependencies for an AUR package from the AUR RPC API.
///
/// This is a fallback when paru/yay is not available.
fn fetch_aur_deps_from_api(
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
    let v: Value = serde_json::from_str(&body)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;
    
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .ok_or_else(|| "No 'results' array in AUR API response".to_string())?;
    
    let obj = arr.first()
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

/// Resolve dependencies for a single package.
fn resolve_package_deps(
    name: &str,
    source: &Source,
    installed: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> Result<Vec<DependencyInfo>, String> {
    let mut deps = Vec::new();

    match source {
        Source::Official { repo, .. } => {
            // Use pacman -Si to get dependency list (shows all deps, not just ones to download)
            tracing::debug!("Running: pacman -Si {}", name);
            let spec = if repo.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", repo, name)
            };
            let output = Command::new("pacman")
                .args(["-Si", &spec])
                .env("LC_ALL", "C")
                .env("LANG", "C")
                .output()
                .map_err(|e| {
                    tracing::error!("Failed to execute pacman -Si {}: {}", spec, e);
                    format!("pacman -Si failed: {}", e)
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                tracing::error!("pacman -Si {} failed with status {:?}: {}", spec, output.status.code(), stderr);
                return Err(format!("pacman -Si failed for {}: {}", spec, stderr));
            }

            let text = String::from_utf8_lossy(&output.stdout);
            tracing::debug!("pacman -Si {} output ({} bytes)", spec, text.len());
            
            // Parse "Depends On" field from pacman -Si output
            let dep_names = parse_pacman_si_deps(&text);
            tracing::debug!("Parsed {} dependency names from pacman -Si output", dep_names.len());

            for dep_spec in dep_names {
                let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                // Skip if this dependency is the package itself (shouldn't happen, but be safe)
                if pkg_name == name {
                    tracing::debug!("Skipping self-reference: {} == {}", pkg_name, name);
                    continue;
                }
                // Filter out .so files (virtual packages) - safety check in case filtering in parse_pacman_si_deps missed something
                if pkg_name.ends_with(".so") || pkg_name.contains(".so.") || pkg_name.contains(".so=") {
                    tracing::debug!("Filtering out virtual package: {}", pkg_name);
                    continue;
                }

                let status = determine_status(&pkg_name, &version_req, installed, upgradable);
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
        }
        Source::Aur => {
            // For AUR packages, try to use paru/yay to resolve dependencies
            // Fallback: fetch from AUR API if paru/yay is not available
            tracing::debug!("Checking for paru/yay availability for AUR package: {}", name);
            
            // Check if paru exists
            let has_paru = Command::new("paru")
                .args(["--version"])
                .output()
                .is_ok();
            
            // Check if yay exists
            let has_yay = Command::new("yay")
                .args(["--version"])
                .output()
                .is_ok();
            
            // Try paru/yay first, but fall back to API if they fail
            // Use -Si to get all dependencies (similar to pacman -Si)
            let mut used_helper = false;
            
            if has_paru {
                tracing::debug!("Trying paru -Si {} for dependency resolution", name);
                match Command::new("paru")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let text = String::from_utf8_lossy(&output.stdout);
                            tracing::debug!("paru -Si {} output ({} bytes)", name, text.len());
                            let dep_names = parse_pacman_si_deps(&text);
                            if !dep_names.is_empty() {
                                tracing::info!("Using paru to resolve dependencies for {}", name);
                                used_helper = true;
                                for dep_spec in dep_names {
                                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                                    // Skip if this dependency is the package itself
                                    if pkg_name == name {
                                        tracing::debug!("Skipping self-reference: {} == {}", pkg_name, name);
                                        continue;
                                    }
                                    // Filter out .so files (virtual packages)
                                    if pkg_name.ends_with(".so") || pkg_name.contains(".so.") || pkg_name.contains(".so=") {
                                        tracing::debug!("Filtering out virtual package: {}", pkg_name);
                                        continue;
                                    }

                                    let status = determine_status(&pkg_name, &version_req, installed, upgradable);
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
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::debug!("paru -Si {} failed (will try yay or API): {}", name, stderr.trim());
                        }
                    }
                    Err(_) => {
                        // paru not available, continue to try yay or API
                    }
                }
            }
            
            if !used_helper && has_yay {
                tracing::debug!("Trying yay -Si {} for dependency resolution", name);
                match Command::new("yay")
                    .args(["-Si", name])
                    .env("LC_ALL", "C")
                    .env("LANG", "C")
                    .output()
                {
                    Ok(output) => {
                        if output.status.success() {
                            let text = String::from_utf8_lossy(&output.stdout);
                            tracing::debug!("yay -Si {} output ({} bytes)", name, text.len());
                            let dep_names = parse_pacman_si_deps(&text);
                            if !dep_names.is_empty() {
                                tracing::info!("Using yay to resolve dependencies for {}", name);
                                used_helper = true;
                                for dep_spec in dep_names {
                                    let (pkg_name, version_req) = parse_dep_spec(&dep_spec);
                                    // Skip if this dependency is the package itself
                                    if pkg_name == name {
                                        tracing::debug!("Skipping self-reference: {} == {}", pkg_name, name);
                                        continue;
                                    }
                                    // Filter out .so files (virtual packages)
                                    if pkg_name.ends_with(".so") || pkg_name.contains(".so.") || pkg_name.contains(".so=") {
                                        tracing::debug!("Filtering out virtual package: {}", pkg_name);
                                        continue;
                                    }

                                    let status = determine_status(&pkg_name, &version_req, installed, upgradable);
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
                            }
                        } else {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            tracing::debug!("yay -Si {} failed (will use API): {}", name, stderr.trim());
                        }
                    }
                    Err(_) => {
                        // yay not available, continue to API fallback
                    }
                }
            }
            
            // Always fall back to AUR API if helper didn't work or wasn't available
            // This ensures we get dependencies even if paru/yay fails or isn't installed
            if !used_helper {
                if has_paru || has_yay {
                    tracing::info!("Using AUR API to resolve dependencies for {} (paru/yay -Si failed or not available)", name);
                } else {
                    tracing::info!("Using AUR API to resolve dependencies for {} (paru/yay not available)", name);
                }
                match fetch_aur_deps_from_api(name, installed, upgradable) {
                    Ok(api_deps) => {
                        tracing::info!("Fetched {} dependencies from AUR API", api_deps.len());
                        deps.extend(api_deps);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch dependencies from AUR API: {}", e);
                    }
                }
            }
        }
    }

    tracing::debug!("Resolved {} dependencies for package {}", deps.len(), name);
    Ok(deps)
}

/// Parse "Depends On" field from pacman -Si output.
///
/// The "Depends On" field contains space-separated dependency specifications.
/// Example: "curl  expat  perl  perl-error  perl-mailtools  openssl  pcre2  grep  shadow  zlib-ng"
/// Filters out virtual packages (.so files) like "libedit.so=0-64"
fn parse_pacman_si_deps(text: &str) -> Vec<String> {
    for line in text.lines() {
        if line.starts_with("Depends On") {
            if let Some(colon_pos) = line.find(':') {
                let deps_str = line[colon_pos + 1..].trim();
                if deps_str.is_empty() || deps_str == "None" {
                    return Vec::new();
                }
                // Split by whitespace, filter out empty strings and .so files (virtual packages)
                return deps_str
                    .split_whitespace()
                    .map(|s| s.trim().to_string())
                    .filter(|s| {
                        if s.is_empty() {
                            return false;
                        }
                        // Filter out .so files (virtual packages)
                        // Patterns: "libedit.so=0-64", "libgit2.so", "libfoo.so.1"
                        // Check if it ends with .so or contains .so. or .so=
                        !(s.ends_with(".so") || s.contains(".so.") || s.contains(".so="))
                    })
                    .collect();
            }
        }
    }
    Vec::new()
}

/// Parse dependency output from pacman/paru -Sp.
///
/// The output format can be:
///   - Package names: "core/glibc", "extra/python>=3.12"
///   - Full URLs: "/mirror/archlinux/extra/os/x86_64/package-1.0-1-x86_64.pkg.tar.zst"
///   - Library files: "libgit2.so" (virtual packages/provides)
///
/// Returns cleaned package names, filtering out invalid entries.
fn parse_dependency_output(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            
            // Handle full URLs/paths (e.g., "/mirror/archlinux/extra/os/x86_64/package-1.0-1-x86_64.pkg.tar.zst")
            if line.contains(".pkg.tar.zst") {
                // Extract package name from path
                // Format: .../package-name-version-revision-arch.pkg.tar.zst
                if let Some(pkg_start) = line.rfind('/') {
                    let filename = &line[pkg_start + 1..];
                    if let Some(pkg_end) = filename.find(".pkg.tar.zst") {
                        let pkg_with_ver = &filename[..pkg_end];
                        // Extract package name (everything before the last version-like segment)
                        // e.g., "jujutsu-0.35.0-1-x86_64" -> "jujutsu"
                        if let Some(name_end) = pkg_with_ver.rfind('-') {
                            // Try to find where version starts (look for pattern like "-1-x86_64" or version numbers)
                            let potential_name = &pkg_with_ver[..name_end];
                            // Check if there's another dash (version-revision-arch pattern)
                            if let Some(ver_start) = potential_name.rfind('-') {
                                // Might be "package-version-revision-arch", extract just package
                                return Some(potential_name[..ver_start].to_string());
                            }
                            return Some(potential_name.to_string());
                        }
                        return Some(pkg_with_ver.to_string());
                    }
                }
                return None;
            }
            
            // Handle .so files (shared libraries) - these are virtual packages
            // Skip them as they're not actual package dependencies
            if line.ends_with(".so") || line.contains(".so.") {
                return None;
            }
            
            // Handle repo/package format (e.g., "core/glibc" -> "glibc")
            if let Some(slash_pos) = line.find('/') {
                let after_slash = &line[slash_pos + 1..];
                // Check if it's still a valid package name (not a path)
                if !after_slash.contains('/') && !after_slash.contains("http") {
                    return Some(after_slash.to_string());
                }
            }
            
            // Plain package name
            Some(line.to_string())
        })
        .collect()
}

/// Parse a dependency specification into (name, version_constraint).
///
/// Examples:
///   "glibc" -> ("glibc", "")
///   "python>=3.12" -> ("python", ">=3.12")
///   "firefox=121.0" -> ("firefox", "=121.0")
fn parse_dep_spec(spec: &str) -> (String, String) {
    for op in ["<=", ">=", "=", "<", ">"] {
        if let Some(pos) = spec.find(op) {
            let name = spec[..pos].trim().to_string();
            let version = spec[pos..].trim().to_string();
            return (name, version);
        }
    }
    (spec.trim().to_string(), String::new())
}

/// Determine the source repository for a dependency package.
///
/// Returns (source, is_core) tuple.
fn determine_dependency_source(
    name: &str,
    installed: &HashSet<String>,
) -> (DependencySource, bool) {
    if !installed.contains(name) {
        // Not installed - could be AUR or official, default to AUR
        return (DependencySource::Aur, false);
    }

    // Package is installed - check which repository it came from
    let output = Command::new("pacman")
        .args(["-Qi", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            // Look for "Repository" field in pacman -Qi output
            for line in text.lines() {
                if line.starts_with("Repository") {
                    if let Some(colon_pos) = line.find(':') {
                        let repo = line[colon_pos + 1..].trim().to_lowercase();
                        let is_core = repo == "core";
                        return (
                            DependencySource::Official {
                                repo: if repo.is_empty() { "unknown".to_string() } else { repo },
                            },
                            is_core,
                        );
                    }
                }
            }
        }
        _ => {
            // Fallback: try pacman -Q to see if it's installed
            // If we can't determine repo, assume it's from an official repo
            tracing::debug!("Could not determine repository for {}, assuming official", name);
        }
    }

    // Default: assume official repository (most installed packages are)
    let is_core = is_system_package(name);
    (
        DependencySource::Official {
            repo: if is_core { "core".to_string() } else { "extra".to_string() },
        },
        is_core,
    )
}

/// Determine the status of a dependency based on installation state.
fn determine_status(
    name: &str,
    version_req: &str,
    installed: &HashSet<String>,
    upgradable: &HashSet<String>,
) -> DependencyStatus {
    if !installed.contains(name) {
        return DependencyStatus::ToInstall;
    }

    // Check if package is upgradable (even without version requirement)
    let is_upgradable = upgradable.contains(name);

    // If version requirement is specified, check if it matches
    if !version_req.is_empty() {
        // Try to get installed version
        if let Ok(installed_version) = get_installed_version(name) {
            // Simple version comparison (basic implementation)
            if !version_satisfies(&installed_version, version_req) {
                return DependencyStatus::ToUpgrade {
                    current: installed_version,
                    required: version_req.to_string(),
                };
            }
            // Version requirement satisfied, but check if package is upgradable anyway
            if is_upgradable {
                // Get available version from pacman -Si if possible
                let available_version = get_available_version(name).unwrap_or_else(|| "newer".to_string());
                return DependencyStatus::ToUpgrade {
                    current: installed_version,
                    required: available_version,
                };
            }
            return DependencyStatus::Installed {
                version: installed_version,
            };
        }
    }

    // Installed but no version check needed - check if upgradable
    if is_upgradable {
        match get_installed_version(name) {
            Ok(current_version) => {
                let available_version = get_available_version(name).unwrap_or_else(|| "newer".to_string());
                return DependencyStatus::ToUpgrade {
                    current: current_version,
                    required: available_version,
                };
            }
            Err(_) => {
                return DependencyStatus::ToUpgrade {
                    current: "installed".to_string(),
                    required: "newer".to_string(),
                };
            }
        }
    }

    // Installed and up-to-date - get actual version
    match get_installed_version(name) {
        Ok(version) => DependencyStatus::Installed { version },
        Err(_) => DependencyStatus::Installed {
            version: "installed".to_string(),
        },
    }
}

/// Get the available version of a package from repositories.
fn get_available_version(name: &str) -> Option<String> {
    let output = Command::new("pacman")
        .args(["-Si", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if line.starts_with("Version") {
            if let Some(colon_pos) = line.find(':') {
                let version = line[colon_pos + 1..].trim();
                // Remove revision suffix if present
                let version = version.split('-').next().unwrap_or(version);
                return Some(version.to_string());
            }
        }
    }
    None
}

/// Get the installed version of a package.
fn get_installed_version(name: &str) -> Result<String, String> {
    let output = Command::new("pacman")
        .args(["-Q", name])
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .map_err(|e| format!("pacman -Q failed: {}", e))?;

    if !output.status.success() {
        return Err("Package not found".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    if let Some(line) = text.lines().next() {
        // Format: "name version" or "name version-revision"
        if let Some(space_pos) = line.find(' ') {
            let version = line[space_pos + 1..].trim();
            // Remove revision suffix if present (e.g., "1.2.3-1" -> "1.2.3")
            let version = version.split('-').next().unwrap_or(version);
            return Ok(version.to_string());
        }
    }

    Err("Could not parse version".to_string())
}

/// Check if a version satisfies a requirement (simplified).
fn version_satisfies(installed: &str, requirement: &str) -> bool {
    // This is a simplified version checker
    // For production, use a proper version comparison library
    if requirement.starts_with(">=") {
        let req_ver = &requirement[2..];
        installed >= req_ver
    } else if requirement.starts_with("<=") {
        let req_ver = &requirement[2..];
        installed <= req_ver
    } else if requirement.starts_with("=") {
        let req_ver = &requirement[1..];
        installed == req_ver
    } else if requirement.starts_with(">") {
        let req_ver = &requirement[1..];
        installed > req_ver
    } else if requirement.starts_with("<") {
        let req_ver = &requirement[1..];
        installed < req_ver
    } else {
        // No version requirement, assume satisfied
        true
    }
}

/// Check if a package is a critical system package.
fn is_system_package(name: &str) -> bool {
    // List of critical system packages
    let system_packages = [
        "glibc", "linux", "systemd", "pacman", "bash", "coreutils", "gcc",
        "binutils", "filesystem", "util-linux", "shadow", "sed", "grep",
    ];
    system_packages.contains(&name)
}

/// Priority for sorting dependencies (lower = higher priority).
fn dependency_priority(status: &DependencyStatus) -> u8 {
    match status {
        DependencyStatus::Conflict { .. } => 0,
        DependencyStatus::Missing => 1,
        DependencyStatus::ToInstall => 2,
        DependencyStatus::ToUpgrade { .. } => 3,
        DependencyStatus::Installed { .. } => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dep_spec_basic() {
        let (name, version) = parse_dep_spec("glibc");
        assert_eq!(name, "glibc");
        assert_eq!(version, "");
    }

    #[test]
    fn parse_dep_spec_with_version() {
        let (name, version) = parse_dep_spec("python>=3.12");
        assert_eq!(name, "python");
        assert_eq!(version, ">=3.12");
    }

    #[test]
    fn parse_dep_spec_equals() {
        let (name, version) = parse_dep_spec("firefox=121.0");
        assert_eq!(name, "firefox");
        assert_eq!(version, "=121.0");
    }

    #[test]
    fn is_system_package_detects_core() {
        assert!(is_system_package("glibc"));
        assert!(is_system_package("linux"));
        assert!(!is_system_package("firefox"));
    }
}

