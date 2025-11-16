//! AUR sandbox preflight checks for build dependencies.

use crate::state::types::PackageItem;
use crate::util::{curl_args, percent_encode};
use futures::stream::{FuturesUnordered, StreamExt};
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

            fetch_futures.push(async move {
                match fetch_srcinfo_async(&client_clone, &name).await {
                    Ok(srcinfo_text) => {
                        match analyze_package_from_srcinfo(
                            &name,
                            &srcinfo_text,
                            &installed_clone,
                            &provided_clone,
                        ) {
                            Ok(info) => Some(info),
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to analyze sandbox info for {}: {}",
                                    name,
                                    e
                                );
                                // Create empty SandboxInfo so package still appears in results
                                tracing::info!(
                                    "Creating empty sandbox info for {} (.SRCINFO analysis failed)",
                                    name
                                );
                                Some(SandboxInfo {
                                    package_name: name,
                                    depends: Vec::new(),
                                    makedepends: Vec::new(),
                                    checkdepends: Vec::new(),
                                    optdepends: Vec::new(),
                                })
                            }
                        }
                    }
                    Err(e) => {
                        // Fallback to PKGBUILD if .SRCINFO fails (use spawn_blocking for blocking call)
                        tracing::debug!(
                            "Failed to fetch .SRCINFO for {}: {}, trying PKGBUILD",
                            name,
                            e
                        );
                        let name_for_fallback = name.clone();
                        let installed_for_fallback = installed_clone.clone();
                        let provided_for_fallback = provided_clone.clone();
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
                                match analyze_package_from_pkgbuild(
                                    &name,
                                    &pkgbuild_text,
                                    &installed_for_fallback,
                                    &provided_for_fallback,
                                ) {
                                    Ok(info) => {
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
                                        Some(info)
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to analyze sandbox info from PKGBUILD for {}: {}",
                                            name,
                                            e
                                        );
                                        // Create empty SandboxInfo so package still appears in results
                                        tracing::info!(
                                            "Creating empty sandbox info for {} (PKGBUILD analysis failed)",
                                            name
                                        );
                                        Some(SandboxInfo {
                                            package_name: name,
                                            depends: Vec::new(),
                                            makedepends: Vec::new(),
                                            checkdepends: Vec::new(),
                                            optdepends: Vec::new(),
                                        })
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                tracing::warn!("Failed to fetch PKGBUILD for {}: {}", name, e);
                                // Create empty SandboxInfo so package still appears in results
                                // This allows UI to show that resolution failed for this package
                                tracing::info!(
                                    "Creating empty sandbox info for {} (both .SRCINFO and PKGBUILD fetch failed)",
                                    name
                                );
                                Some(SandboxInfo {
                                    package_name: name,
                                    depends: Vec::new(),
                                    makedepends: Vec::new(),
                                    checkdepends: Vec::new(),
                                    optdepends: Vec::new(),
                                })
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to spawn blocking task for PKGBUILD fetch for {}: {}",
                                    name,
                                    e
                                );
                                // Create empty SandboxInfo so package still appears in results
                                tracing::info!(
                                    "Creating empty sandbox info for {} (spawn task failed)",
                                    name
                                );
                                Some(SandboxInfo {
                                    package_name: name,
                                    depends: Vec::new(),
                                    makedepends: Vec::new(),
                                    checkdepends: Vec::new(),
                                    optdepends: Vec::new(),
                                })
                            }
                        }
                    }
                }
            });
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
    let duration_ms = elapsed.as_millis() as u64;
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
/// Details:
/// - Wraps the async version for use in blocking contexts.
pub fn resolve_sandbox_info(items: &[PackageItem]) -> Vec<SandboxInfo> {
    // Use tokio runtime handle if available, otherwise create a new runtime
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle.block_on(resolve_sandbox_info_async(items)),
        Err(_) => {
            // No runtime available, create a new one
            let rt = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
                tracing::error!(
                    "Failed to create tokio runtime for sandbox resolution: {}",
                    e
                );
                panic!("Cannot resolve sandbox info without tokio runtime");
            });
            rt.block_on(resolve_sandbox_info_async(items))
        }
    }
}

/// What: Fetch .SRCINFO content for an AUR package using async HTTP.
///
/// Inputs:
/// - `client`: Reqwest HTTP client.
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
async fn fetch_srcinfo_async(client: &reqwest::Client, name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP request failed with status: {}",
            response.status()
        ));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    // Check if we got an HTML error page instead of .SRCINFO content
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO".to_string());
    }

    Ok(text)
}

/// What: Analyze package dependencies from .SRCINFO content.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `srcinfo_text`: .SRCINFO content.
/// - `installed`: Set of installed package names.
/// - `provided`: Set of package names provided by installed packages.
///
/// Output:
/// - `SandboxInfo` with dependency deltas.
fn analyze_package_from_srcinfo(
    package_name: &str,
    srcinfo_text: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> Result<SandboxInfo, String> {
    let (depends, makedepends, checkdepends, optdepends) = parse_srcinfo_deps(srcinfo_text);

    // Analyze each dependency against host environment
    let depends_delta = analyze_dependencies(&depends, installed, provided);
    let makedepends_delta = analyze_dependencies(&makedepends, installed, provided);
    let checkdepends_delta = analyze_dependencies(&checkdepends, installed, provided);
    let optdepends_delta = analyze_dependencies(&optdepends, installed, provided);

    Ok(SandboxInfo {
        package_name: package_name.to_string(),
        depends: depends_delta,
        makedepends: makedepends_delta,
        checkdepends: checkdepends_delta,
        optdepends: optdepends_delta,
    })
}

/// What: Analyze package dependencies from PKGBUILD content.
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `pkgbuild_text`: PKGBUILD content.
/// - `installed`: Set of installed package names.
/// - `provided`: Set of package names provided by installed packages.
///
/// Output:
/// - `SandboxInfo` with dependency deltas.
fn analyze_package_from_pkgbuild(
    package_name: &str,
    pkgbuild_text: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> Result<SandboxInfo, String> {
    let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild_text);

    // Analyze each dependency against host environment
    let depends_delta = analyze_dependencies(&depends, installed, provided);
    let makedepends_delta = analyze_dependencies(&makedepends, installed, provided);
    let checkdepends_delta = analyze_dependencies(&checkdepends, installed, provided);
    let optdepends_delta = analyze_dependencies(&optdepends, installed, provided);

    Ok(SandboxInfo {
        package_name: package_name.to_string(),
        depends: depends_delta,
        makedepends: makedepends_delta,
        checkdepends: checkdepends_delta,
        optdepends: optdepends_delta,
    })
}

/// What: Fetch and analyze a single AUR package's build dependencies (legacy synchronous version).
///
/// Inputs:
/// - `package_name`: AUR package name.
/// - `installed`: Set of installed package names.
/// - `provided`: Set of package names provided by installed packages.
///
/// Output:
/// - `SandboxInfo` with dependency deltas, or an error if fetch/parse fails.
#[allow(dead_code)]
fn fetch_and_analyze_package(
    package_name: &str,
    installed: &HashSet<String>,
    provided: &HashSet<String>,
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
    if srcinfo_text.contains("pkgbase =") || srcinfo_text.contains("pkgname =") {
        // Looks like .SRCINFO format
        analyze_package_from_srcinfo(package_name, &srcinfo_text, installed, provided)
    } else {
        // Assume PKGBUILD format - extract dependencies
        analyze_package_from_pkgbuild(package_name, &srcinfo_text, installed, provided)
    }
}

/// What: Fetch .SRCINFO content for an AUR package (legacy synchronous version using curl).
///
/// Inputs:
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
#[allow(dead_code)]
fn fetch_srcinfo(name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let args = curl_args(&url, &[]);
    let output = Command::new("curl")
        .args(&args)
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

    // Check if we got an HTML error page instead of .SRCINFO content
    // AUR sometimes returns HTML error pages (e.g., 502 Bad Gateway) with curl exit code 0
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO content".to_string());
    }

    // Validate that it looks like .SRCINFO format (should have pkgbase or pkgname)
    if !text.contains("pkgbase =") && !text.contains("pkgname =") {
        return Err("Response does not appear to be valid .SRCINFO format".to_string());
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
/// - Parses bash array syntax: `depends=('foo' 'bar>=1.2')` (single-line)
/// - Also handles `depends+=` patterns used in functions like `package()`
/// - Handles both quoted and unquoted dependencies
/// - Also handles multi-line arrays:
///   ```
///   depends=(
///       'foo'
///       'bar>=1.2'
///   )
///   ```
fn parse_pkgbuild_deps(pkgbuild: &str) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    tracing::debug!(
        "parse_pkgbuild_deps: Starting parse, PKGBUILD length={}, first 500 chars: {:?}",
        pkgbuild.len(),
        pkgbuild.chars().take(500).collect::<String>()
    );
    let mut depends = Vec::new();
    let mut makedepends = Vec::new();
    let mut checkdepends = Vec::new();
    let mut optdepends = Vec::new();

    let lines: Vec<&str> = pkgbuild.lines().collect();
    tracing::debug!(
        "parse_pkgbuild_deps: Total lines in PKGBUILD: {}",
        lines.len()
    );
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();
        i += 1;

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse array declarations: depends=('foo' 'bar') or depends=( or depends+=('foo' 'bar')
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();

            // Handle both depends= and depends+= patterns
            let base_key = if let Some(stripped) = key.strip_suffix('+') {
                stripped
            } else {
                key
            };

            tracing::debug!(
                "parse_pkgbuild_deps: Found key-value pair: key='{}', base_key='{}', value='{}'",
                key,
                base_key,
                value.chars().take(100).collect::<String>()
            );

            // Check if this is an array declaration
            if value.starts_with('(') {
                tracing::debug!(
                    "parse_pkgbuild_deps: Detected array declaration for key='{}'",
                    key
                );
                let deps = if let Some(closing_paren_pos) = find_matching_closing_paren(value) {
                    // Single-line array (may have content after closing paren): depends=('foo' 'bar') or depends+=('foo' 'bar') other_code
                    let array_content = &value[1..closing_paren_pos];
                    tracing::debug!("Parsing single-line {} array: {}", key, array_content);
                    parse_array_content(array_content)
                } else {
                    // Multi-line array: depends=(
                    //     'foo'
                    //     'bar'
                    // )
                    tracing::debug!("Parsing multi-line {} array", key);
                    let mut array_lines = Vec::new();
                    // Collect lines until we find the closing parenthesis
                    while i < lines.len() {
                        let next_line = lines[i].trim();
                        i += 1;

                        // Skip empty lines and comments
                        if next_line.is_empty() || next_line.starts_with('#') {
                            continue;
                        }

                        // Check if this line closes the array
                        if next_line == ")" {
                            break;
                        }

                        // Add this line to the array content
                        array_lines.push(next_line);
                    }

                    // Parse all collected lines as array content
                    let array_content = array_lines.join(" ");
                    tracing::debug!(
                        "Collected {} lines for multi-line {} array: {}",
                        array_lines.len(),
                        key,
                        array_content
                    );
                    parse_array_content(&array_content)
                };

                // Add dependencies to the appropriate vector (using base_key to handle both = and +=)
                match base_key {
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

/// What: Find the position of the matching closing parenthesis in a string.
///
/// Inputs:
/// - `s`: String starting with an opening parenthesis.
///
/// Output:
/// - `Some(position)` if a matching closing parenthesis is found, `None` otherwise.
///
/// Details:
/// - Handles nested parentheses and quoted strings.
fn find_matching_closing_paren(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_quotes = false;
    let mut quote_char = '\0';

    for (pos, ch) in s.char_indices() {
        match ch {
            '\'' | '"' => {
                if !in_quotes {
                    in_quotes = true;
                    quote_char = ch;
                } else if ch == quote_char {
                    in_quotes = false;
                    quote_char = '\0';
                }
            }
            '(' if !in_quotes => {
                depth += 1;
            }
            ')' if !in_quotes => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
    }
    None
}

/// What: Parse quoted and unquoted strings from bash array content.
///
/// Inputs:
/// - `content`: Array content string (e.g., "'foo' 'bar>=1.2'" or "libcairo.so libdbus-1.so").
///
/// Output:
/// - Vector of dependency strings.
///
/// Details:
/// - Handles both quoted ('foo') and unquoted (foo) dependencies.
/// - Splits on whitespace for unquoted values.
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
            ch if ch.is_whitespace() => {
                // Whitespace outside quotes - end current unquoted value
                if !current.is_empty() {
                    deps.push(current.clone());
                    current.clear();
                }
            }
            _ => {
                // Non-whitespace character outside quotes - add to current value
                current.push(ch);
            }
        }
    }

    // Handle unclosed quote or trailing unquoted value
    if !current.is_empty() {
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
fn analyze_dependencies(
    deps: &[String],
    installed: &HashSet<String>,
    provided: &HashSet<String>,
) -> Vec<DependencyDelta> {
    deps.iter()
        .filter_map(|dep_spec| {
            // Extract package name (may include version requirements)
            let pkg_name = extract_package_name(dep_spec);
            // Check if package is installed or provided by an installed package
            let is_installed = crate::logic::deps::is_package_installed_or_provided(
                &pkg_name, installed, provided,
            );

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Test parsing dependencies from PKGBUILD with depends= syntax.
    ///
    /// Inputs:
    /// - PKGBUILD with standard depends= array.
    ///
    /// Output:
    /// - Correctly parsed dependencies.
    ///
    /// Details:
    /// - Validates basic dependency parsing works.
    fn test_parse_pkgbuild_deps_basic() {
        let pkgbuild = r#"
pkgname=test-package
pkgver=1.0.0
depends=('foo' 'bar>=1.2')
makedepends=('make' 'gcc')
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

        assert_eq!(depends.len(), 2);
        assert!(depends.contains(&"foo".to_string()));
        assert!(depends.contains(&"bar>=1.2".to_string()));

        assert_eq!(makedepends.len(), 2);
        assert!(makedepends.contains(&"make".to_string()));
        assert!(makedepends.contains(&"gcc".to_string()));

        assert_eq!(checkdepends.len(), 0);
        assert_eq!(optdepends.len(), 0);
    }

    #[test]
    /// What: Test parsing dependencies with depends+= syntax in package() function.
    ///
    /// Inputs:
    /// - PKGBUILD with depends+= inside package() function.
    ///
    /// Output:
    /// - Correctly parsed dependencies from depends+=.
    ///
    /// Details:
    /// - Validates that depends+= patterns are detected and parsed.
    fn test_parse_pkgbuild_deps_append() {
        let pkgbuild = r#"
pkgname=test-package
pkgver=1.0.0
package() {
    depends+=(libcairo.so libdbus-1.so)
    cd $_pkgname
    make DESTDIR="$pkgdir" PREFIX=/usr install
}
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

        assert_eq!(depends.len(), 2);
        assert!(depends.contains(&"libcairo.so".to_string()));
        assert!(depends.contains(&"libdbus-1.so".to_string()));

        assert_eq!(makedepends.len(), 0);
        assert_eq!(checkdepends.len(), 0);
        assert_eq!(optdepends.len(), 0);
    }

    #[test]
    /// What: Test parsing unquoted dependencies.
    ///
    /// Inputs:
    /// - PKGBUILD with unquoted dependencies.
    ///
    /// Output:
    /// - Correctly parsed unquoted dependencies.
    ///
    /// Details:
    /// - Validates that unquoted dependencies are parsed correctly.
    fn test_parse_pkgbuild_deps_unquoted() {
        let pkgbuild = r#"
pkgname=test-package
depends=(libcairo.so libdbus-1.so)
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

        assert_eq!(depends.len(), 2);
        assert!(depends.contains(&"libcairo.so".to_string()));
        assert!(depends.contains(&"libdbus-1.so".to_string()));

        assert_eq!(makedepends.len(), 0);
        assert_eq!(checkdepends.len(), 0);
        assert_eq!(optdepends.len(), 0);
    }

    #[test]
    /// What: Test parsing multi-line dependency arrays.
    ///
    /// Inputs:
    /// - PKGBUILD with multi-line depends array.
    ///
    /// Output:
    /// - Correctly parsed dependencies from multi-line array.
    ///
    /// Details:
    /// - Validates multi-line array parsing works correctly.
    fn test_parse_pkgbuild_deps_multiline() {
        let pkgbuild = r#"
pkgname=test-package
depends=(
    'foo'
    'bar>=1.2'
    'baz'
)
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

        assert_eq!(depends.len(), 3);
        assert!(depends.contains(&"foo".to_string()));
        assert!(depends.contains(&"bar>=1.2".to_string()));
        assert!(depends.contains(&"baz".to_string()));

        assert_eq!(makedepends.len(), 0);
        assert_eq!(checkdepends.len(), 0);
        assert_eq!(optdepends.len(), 0);
    }

    #[test]
    /// What: Test parsing makedepends+= syntax.
    ///
    /// Inputs:
    /// - PKGBUILD with makedepends+= pattern.
    ///
    /// Output:
    /// - Correctly parsed makedepends from += pattern.
    ///
    /// Details:
    /// - Validates that makedepends+= is also handled.
    fn test_parse_pkgbuild_deps_makedepends_append() {
        let pkgbuild = r#"
pkgname=test-package
build() {
    makedepends+=(cmake ninja)
    cmake -B build
}
"#;

        let (depends, makedepends, checkdepends, optdepends) = parse_pkgbuild_deps(pkgbuild);

        assert_eq!(makedepends.len(), 2);
        assert!(makedepends.contains(&"cmake".to_string()));
        assert!(makedepends.contains(&"ninja".to_string()));

        assert_eq!(depends.len(), 0);
        assert_eq!(checkdepends.len(), 0);
        assert_eq!(optdepends.len(), 0);
    }
}
