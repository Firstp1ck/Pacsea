// Windows-only module - conditionally compiled in mod.rs
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tokio::task;

/// Windows-only helpers to fetch Arch mirror data into the repository folder and
/// to build the official package index by querying the public Arch Packages API.
///
/// This module does not depend on `pacman` (which is typically unavailable on
/// Windows). Instead, it calls out to `curl` to download JSON/text resources.
/// Windows 10+ systems usually ship with a `curl` binary; if it's not present,
/// the functions will return an error.
///
/// Public entrypoints:
/// - `fetch_mirrors_to_repo_dir(repo_dir)`
/// - `refresh_official_index_from_arch_api(persist_path, net_err_tx, notify_tx)`
/// - `refresh_windows_mirrors_and_index(persist_path, repo_dir, net_err_tx, notify_tx)`
use super::{OfficialPkg, idx, save_to_disk};
use crate::util::curl;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Download Arch mirror metadata and render a concise `mirrorlist.txt`.
///
/// Inputs:
/// - `repo_dir`: Target directory used to persist mirrors.json and mirrorlist.txt.
///
/// Output:
/// - `Ok(PathBuf)` pointing to the generated mirror list file; boxed error otherwise.
///
/// Details:
/// - Persists the raw JSON for reference and keeps up to 40 active HTTPS mirrors in the list.
pub async fn fetch_mirrors_to_repo_dir(repo_dir: &Path) -> Result<PathBuf> {
    let repo_dir = repo_dir.to_path_buf();
    task::spawn_blocking(move || {
        fs::create_dir_all(&repo_dir)?;
        let status_url = "https://archlinux.org/mirrors/status/json/";
        let json = curl::curl_json(status_url)?;

        // Persist the raw JSON for debugging/inspection
        let mirrors_json_path = repo_dir.join("mirrors.json");
        fs::write(&mirrors_json_path, serde_json::to_vec_pretty(&json)?)?;

        // Extract a handful of currently active HTTPS mirrors
        // JSON shape reference: { "urls": [ { "url": "...", "protocols": ["https", ...], "active": true, ... }, ... ] }
        let mut https_urls: Vec<String> = Vec::new();
        if let Some(arr) = json.get("urls").and_then(|v| v.as_array()) {
            for u in arr {
                let active = u.get("active").and_then(|v| v.as_bool()).unwrap_or(false);
                let url = u.get("url").and_then(|v| v.as_str()).unwrap_or_default();
                let protocols = u
                    .get("protocols")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let has_https = protocols.iter().any(|p| {
                    p.as_str()
                        .map(|s| s.eq_ignore_ascii_case("https"))
                        .unwrap_or(false)
                });
                if active && has_https && !url.is_empty() {
                    https_urls.push(url.to_string());
                }
            }
        }
        // Keep only a modest number to avoid noise; sort for determinism
        https_urls.sort();
        https_urls.dedup();
        if https_urls.len() > 40 {
            https_urls.truncate(40);
        }

        // Generate a pacman-like mirrorlist template
        // Note: This is for reference/offline usage; Pacsea does not execute pacman on Windows.
        let mut mirrorlist: String = String::new();
        mirrorlist.push_str("# Generated from Arch mirror status (Windows)\n");
        mirrorlist.push_str("# Only HTTPS and active mirrors are listed.\n");
        for base in &https_urls {
            let base = base.trim_end_matches('/');
            mirrorlist.push_str(&format!("Server = {base}/$repo/os/$arch\n"));
        }
        let mirrorlist_path = repo_dir.join("mirrorlist.txt");
        fs::write(&mirrorlist_path, mirrorlist.as_bytes())?;
        Ok::<PathBuf, Box<dyn std::error::Error + Send + Sync>>(mirrorlist_path)
    })
    .await?
}

/// What: Build the official index via the Arch Packages JSON API and persist it.
///
/// Inputs:
/// - `persist_path`: Destination file for the serialized index.
/// - `net_err_tx`: Channel receiving errors encountered during network fetches.
/// - `notify_tx`: Channel notified after successful persistence.
///
/// Output:
/// - No direct return value; communicates success/failure through channels and shared state.
///
/// Details:
/// - Pages through `core`, `extra`, and `multilib` results, dedupes by `(repo,name)`, and updates
///   the in-memory index before persisting.
pub async fn refresh_official_index_from_arch_api(
    persist_path: PathBuf,
    net_err_tx: tokio::sync::mpsc::UnboundedSender<String>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    let repos = vec!["core", "extra", "multilib"];
    let arch = "x86_64";

    let res = task::spawn_blocking(move || -> Result<Vec<OfficialPkg>> {
        let mut pkgs: Vec<OfficialPkg> = Vec::new();
        for repo in repos {
            tracing::info!(repo = repo, "Fetching packages from repository");
            let mut page: usize = 1;
            let limit: usize = 250;
            let mut repo_pkg_count = 0;
            loop {
                // Try URL without q parameter first (original format)
                let url = format!("https://archlinux.org/packages/search/json/?repo={repo}&arch={arch}&limit={limit}&page={page}");
                tracing::debug!(repo = repo, page = page, url = %url, "Fetching package page from API");
                let mut v = match curl::curl_json(&url) {
                    Ok(v) => v,
                    Err(e) => {
                        // If a page fails, bubble the error up; no partial repo result
                        tracing::error!(repo = repo, page = page, error = %e, "Failed to fetch package page");
                        return Err(format!("Failed to fetch package list for {repo} (page {page}): {e}").into());
                    }
                };
                // Check if the API response is valid
                // Note: Even if valid=false, we may still get results, so check results first
                let mut results = v
                    .get("results")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .unwrap_or_default();

                if let Some(valid) = v.get("valid").and_then(|x| x.as_bool()) {
                    if !valid && results.is_empty() {
                        // Only fail if valid=false AND no results
                        // Log the full response for debugging
                        let response_str = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "Failed to serialize response".to_string());
                tracing::warn!(
                    repo = repo,
                    page = page,
                    url = %url,
                    response = %response_str,
                    "API query returned valid=false with no results, trying with q parameter"
                );
                        // Try multiple alternative query formats
                        let alternatives = vec![
                            ("q=*", format!("https://archlinux.org/packages/search/json/?q=*&repo={repo}&arch={arch}&limit={limit}&page={page}")),
                            ("q=%2A", format!("https://archlinux.org/packages/search/json/?q=%2A&repo={repo}&arch={arch}&limit={limit}&page={page}")),
                            ("q=a", format!("https://archlinux.org/packages/search/json/?q=a&repo={repo}&arch={arch}&limit={limit}&page={page}")),
                            ("q=", format!("https://archlinux.org/packages/search/json/?q=&repo={repo}&arch={arch}&limit={limit}&page={page}")),
                        ];

                        let mut found_working = false;
                        for (format_name, alt_url) in alternatives {
                            tracing::debug!(repo = repo, page = page, format = format_name, url = %alt_url, "Trying alternative API URL format");
                            match curl::curl_json(&alt_url) {
                                Ok(alt_v) => {
                                    let alt_results = alt_v
                                        .get("results")
                                        .and_then(|x| x.as_array())
                                        .cloned()
                                        .unwrap_or_default();
                                    let alt_valid = alt_v.get("valid").and_then(|x| x.as_bool()).unwrap_or(true);
                                    if alt_valid && !alt_results.is_empty() {
                                        // This format worked!
                                        v = alt_v;
                                        results = alt_results;
                                        tracing::info!(repo = repo, page = page, format = format_name, "Alternative URL format worked");
                                        found_working = true;
                                        break;
                                    } else if !alt_results.is_empty() {
                                        // Got results even if valid=false, use them
                                        v = alt_v;
                                        results = alt_results;
                                        tracing::warn!(repo = repo, page = page, format = format_name, "Alternative URL returned results despite valid=false");
                                        found_working = true;
                                        break;
                                    }
                                    tracing::debug!(repo = repo, page = page, format = format_name, valid = alt_valid, result_count = alt_results.len(), "Alternative format returned no results");
                                }
                                Err(alt_e) => {
                                    tracing::debug!(repo = repo, page = page, format = format_name, error = %alt_e, "Alternative URL format failed");
                                }
                            }
                        }

                        if !found_working {
                            let error_msg = format!(
                                "Arch Linux Packages API returned invalid query response for {repo} (page {page}). All URL formats failed with valid=false and no results. The API may have changed or requires different parameters."
                            );
                            return Err(error_msg.into());
                        }
                    } else if !valid && !results.is_empty() {
                        // valid=false but we have results - log warning but continue
                        tracing::warn!(
                            repo = repo,
                            page = page,
                            result_count = results.len(),
                            "API returned valid=false but has results, processing anyway"
                        );
                    }
                }
                // Log the response structure for debugging
                if page == 1 {
                    tracing::debug!(
                        repo = repo,
                        response_keys = ?v.as_object().map(|o| o.keys().collect::<Vec<_>>()),
                        "API response structure"
                    );
                }
                if results.is_empty() {
                    tracing::debug!(repo = repo, page = page, "No more results for repository");
                    // On first page with empty results, log more details for debugging
                    if page == 1 {
                        // Log the full response structure to understand what the API returned
                        let response_str = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "Failed to serialize response".to_string());
                        let response_preview = if response_str.len() > 500 {
                            format!("{}...", &response_str[..500])
                        } else {
                            response_str.clone()
                        };
                        tracing::warn!(
                            repo = repo,
                            url = %url,
                            response_preview = %response_preview,
                            "First page returned empty results - checking API response structure"
                        );
                        // Check if the response has a different structure
                        if let Some(count) = v.get("count").and_then(|x| x.as_u64()) {
                            tracing::warn!(
                                repo = repo,
                                total_count = count,
                                "API reports total count but results array is empty"
                            );
                        }
                        if let Some(limit_val) = v.get("limit").and_then(|x| x.as_u64()) {
                            tracing::debug!(repo = repo, api_limit = limit_val, "API limit value");
                        }
                    }
                    break;
                }
                tracing::debug!(repo = repo, page = page, count = results.len(), "Fetched package page");
                for obj in results {
                    let name = obj
                        .get("pkgname")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let version = obj
                        .get("pkgver")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let description = obj
                        .get("pkgdesc")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let arch_val = obj
                        .get("arch")
                        .and_then(|v| v.as_str())
                        .unwrap_or(arch)
                        .to_string();
                    let repo_val = obj
                        .get("repo")
                        .and_then(|v| v.as_str())
                        .unwrap_or(repo)
                        .to_string();

                    pkgs.push(OfficialPkg {
                        name,
                        repo: repo_val,
                        arch: arch_val,
                        version,
                        description,
                    });
                    repo_pkg_count += 1;
                }
                page += 1;
            }
            tracing::info!(repo = repo, package_count = repo_pkg_count, "Completed fetching repository");
        }
        // Sort and dedup by (repo, name)
        pkgs.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
        let before_dedup = pkgs.len();
        pkgs.dedup_by(|a, b| a.repo == b.repo && a.name == b.name);
        let after_dedup = pkgs.len();
        if before_dedup != after_dedup {
            tracing::debug!(
                before = before_dedup,
                after = after_dedup,
                removed = before_dedup - after_dedup,
                "Deduplicated packages"
            );
        }
        tracing::info!(total_packages = pkgs.len(), "Completed fetching all repositories");
        Ok(pkgs)
    })
    .await;

    match res {
        Ok(Ok(new_list)) => {
            tracing::info!(
                package_count = new_list.len(),
                path = %persist_path.display(),
                "Successfully fetched official package index"
            );
            // Replace in-memory index and persist to disk
            if let Ok(mut guard) = idx().write() {
                guard.pkgs = new_list.clone();
                tracing::debug!("Updated in-memory index");
            } else {
                tracing::warn!("Failed to acquire write lock for index update");
            }
            save_to_disk(&persist_path);
            tracing::info!(path = %persist_path.display(), "Persisted index to disk");
            let _ = notify_tx.send(());
        }
        Ok(Err(e)) => {
            let msg = format!("Failed to fetch official index via API: {e}");
            let _ = net_err_tx.send(msg.clone());
            tracing::error!(error = %e, "Failed to fetch official index");
        }
        Err(join_err) => {
            let msg = format!("Task join error during index fetch: {join_err}");
            let _ = net_err_tx.send(msg.clone());
            tracing::error!(error = %join_err, "Task join error");
        }
    }
}

/// What: Check if curl is available and working.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Ok(())` if curl is available and working; `Err` with error message otherwise.
///
/// Details:
/// - Attempts to run `curl --version` to verify curl is in PATH and executable.
pub fn check_curl_availability() -> Result<()> {
    let output = std::process::Command::new("curl")
        .arg("--version")
        .output()
        .map_err(|e| format!("curl not found in PATH: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "curl command failed with exit code: {:?}",
            output.status.code()
        )
        .into());
    }
    Ok(())
}

/// What: Verify the index file exists and contains packages.
///
/// Inputs:
/// - `index_path`: Path to the index JSON file.
///
/// Output:
/// - `Ok((count, size))` with package count and file size in bytes; `Err` with error message otherwise.
///
/// Details:
/// - Checks file existence, reads and parses JSON, and returns package count and file size.
pub fn verify_index_file(index_path: &Path) -> Result<(usize, u64)> {
    if !index_path.exists() {
        return Err(format!("Index file does not exist: {}", index_path.display()).into());
    }
    let metadata =
        fs::metadata(index_path).map_err(|e| format!("Failed to read index file metadata: {e}"))?;
    let size = metadata.len();
    if size == 0 {
        return Err("Index file is empty".into());
    }
    let content =
        fs::read_to_string(index_path).map_err(|e| format!("Failed to read index file: {e}"))?;
    let index: super::OfficialIndex =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse index JSON: {e}"))?;
    let count = index.pkgs.len();
    if count == 0 {
        return Err("Index file contains no packages".into());
    }
    Ok((count, size))
}

/// What: Refresh both the Windows mirror metadata and official package index via the API.
///
/// Inputs:
/// - `persist_path`: Destination for the serialized index JSON.
/// - `repo_dir`: Directory in which mirror assets are stored.
/// - `net_err_tx`: Channel for surfacing network errors to the caller.
/// - `notify_tx`: Channel notified on successful mirror fetch or index refresh.
///
/// Output:
/// - No direct return value; uses the supplied channels for status updates.
///
/// Details:
/// - Attempts mirrors first (best-effort) and then always runs the API-based index refresh.
/// - Checks curl availability before attempting network operations and logs diagnostic information.
pub async fn refresh_windows_mirrors_and_index(
    persist_path: PathBuf,
    repo_dir: PathBuf,
    net_err_tx: tokio::sync::mpsc::UnboundedSender<String>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    // Check curl availability first
    match check_curl_availability() {
        Ok(()) => {
            tracing::info!("curl is available for Windows index refresh");
        }
        Err(e) => {
            let msg = format!(
                "curl is not available: {e}. Windows index refresh requires curl to be installed and in PATH."
            );
            let _ = net_err_tx.send(msg.clone());
            tracing::error!(error = %e, "curl availability check failed");
            return;
        }
    }

    // Check existing index file status
    if persist_path.exists() {
        match verify_index_file(&persist_path) {
            Ok((count, size)) => {
                tracing::info!(
                    path = %persist_path.display(),
                    package_count = count,
                    file_size_bytes = size,
                    "Existing index file found and verified"
                );
            }
            Err(e) => {
                tracing::warn!(
                    path = %persist_path.display(),
                    error = %e,
                    "Existing index file is invalid or empty, will refresh"
                );
            }
        }
    } else {
        tracing::info!(
            path = %persist_path.display(),
            "Index file does not exist, will create new index"
        );
    }

    // 1) Fetch mirrors into repository directory (best-effort)
    match fetch_mirrors_to_repo_dir(&repo_dir).await {
        Ok(path) => {
            let _ = notify_tx.send(());
            tracing::info!(mirrorlist = %path.display(), "Saved mirror list for reference");
        }
        Err(e) => {
            let _ = net_err_tx.send(format!("Failed to fetch mirrors: {e}"));
            tracing::warn!(error = %e, "Failed to fetch mirrors");
        }
    }

    // 2) Build the official package index from the Arch Packages API
    tracing::info!("Starting official package index refresh from Arch API");
    refresh_official_index_from_arch_api(
        persist_path.clone(),
        net_err_tx.clone(),
        notify_tx.clone(),
    )
    .await;

    // Verify the index was successfully created/updated
    match verify_index_file(&persist_path) {
        Ok((count, size)) => {
            tracing::info!(
                path = %persist_path.display(),
                package_count = count,
                file_size_bytes = size,
                "Index refresh completed successfully"
            );
            let _ = notify_tx.send(());
        }
        Err(e) => {
            let msg = format!("Index refresh completed but verification failed: {e}");
            let _ = net_err_tx.send(msg.clone());
            tracing::error!(
                path = %persist_path.display(),
                error = %e,
                "Index verification failed after refresh"
            );
        }
    }
}

#[cfg(test)]
#[cfg(not(target_os = "windows"))]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::sync::mpsc;
    use tokio::time;

    #[tokio::test]
    /// What: Ensure mirror fetching persists raw JSON and filtered HTTPS-only mirror list.
    async fn fetch_mirrors_to_repo_dir_writes_json_and_filtered_mirrorlist() {
        let mut repo_dir = std::env::temp_dir();
        repo_dir.push(format!(
            "pacsea_test_mirrors_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&repo_dir).unwrap();

        let old_path = std::env::var("PATH").unwrap_or_default();
        struct PathGuard {
            original: String,
        }
        impl Drop for PathGuard {
            fn drop(&mut self) {
                unsafe {
                    std::env::set_var("PATH", &self.original);
                }
            }
        }
        let _path_guard = PathGuard {
            original: old_path.clone(),
        };

        let mut shim_root = std::env::temp_dir();
        shim_root.push(format!(
            "pacsea_fake_curl_mirrors_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&shim_root).unwrap();
        let mut bin = shim_root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut script = bin.clone();
        script.push("curl");
        let body = r#"#!/usr/bin/env bash
set -e
if [[ "$1" == "-sSLf" ]]; then
  cat <<'EOF'
{"urls":[{"url":"https://fast.example/", "active":true, "protocols":["https"]},{"url":"http://slow.example/", "active":true, "protocols":["http"]},{"url":"https://inactive.example/", "active":false, "protocols":["https"]}]}
EOF
  exit 0
fi
exit 1
"#;
        std::fs::write(&script, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        let mirrorlist_path = super::fetch_mirrors_to_repo_dir(&repo_dir).await.unwrap();
        let raw_json_path = repo_dir.join("mirrors.json");
        assert!(raw_json_path.exists());
        assert!(mirrorlist_path.exists());

        let mirrorlist_body = std::fs::read_to_string(&mirrorlist_path).unwrap();
        assert!(mirrorlist_body.contains("https://fast.example/$repo/os/$arch"));
        assert!(!mirrorlist_body.contains("slow.example"));
        assert!(!mirrorlist_body.contains("inactive.example"));

        let _ = std::fs::remove_dir_all(&repo_dir);
        let _ = std::fs::remove_dir_all(&shim_root);
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    /// What: Ensure Windows index refresh consumes API responses, persists, and notifies without errors.
    async fn refresh_official_index_from_arch_api_consumes_api_results_and_persists() {
        let _guard = crate::index::test_mutex().lock().unwrap();

        if let Ok(mut g) = super::idx().write() {
            g.pkgs.clear();
        }

        let mut persist_path = std::env::temp_dir();
        persist_path.push(format!(
            "pacsea_mirrors_index_refresh_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let (net_err_tx, mut net_err_rx) = mpsc::unbounded_channel::<String>();
        let (notify_tx, mut notify_rx) = mpsc::unbounded_channel::<()>();

        let old_path = std::env::var("PATH").unwrap_or_default();
        struct PathGuard {
            original: String,
        }
        impl Drop for PathGuard {
            fn drop(&mut self) {
                unsafe {
                    std::env::set_var("PATH", &self.original);
                }
            }
        }
        let _path_guard = PathGuard {
            original: old_path.clone(),
        };

        let mut shim_root = std::env::temp_dir();
        shim_root.push(format!(
            "pacsea_fake_curl_index_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&shim_root).unwrap();
        let mut bin = shim_root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut script = bin.clone();
        script.push("curl");
        let body = r#"#!/usr/bin/env bash
set -e
if [[ "$1" == "-sSLf" ]]; then
  url="$2"
  if [[ "$url" == *"page=1"* ]]; then
    if [[ "$url" == *"repo=core"* ]]; then
      cat <<'EOF'
{"results":[{"pkgname":"core-pkg","pkgver":"1.0","pkgdesc":"Core package","arch":"x86_64","repo":"core"}]}
EOF
    elif [[ "$url" == *"repo=extra"* ]]; then
      cat <<'EOF'
{"results":[{"pkgname":"extra-pkg","pkgver":"2.0","pkgdesc":"Extra package","arch":"x86_64","repo":"extra"}]}
EOF
    else
      cat <<'EOF'
{"results":[]}
EOF
    fi
  else
    cat <<'EOF'
{"results":[]}
EOF
  fi
  exit 0
fi
exit 1
"#;
        std::fs::write(&script, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
        }

        super::refresh_official_index_from_arch_api(persist_path.clone(), net_err_tx, notify_tx)
            .await;

        let notified = time::timeout(Duration::from_millis(200), notify_rx.recv())
            .await
            .ok()
            .flatten()
            .is_some();
        assert!(notified);
        let err = time::timeout(Duration::from_millis(200), net_err_rx.recv())
            .await
            .ok()
            .flatten();
        assert!(err.is_none());

        let mut names: Vec<String> = crate::index::all_official()
            .into_iter()
            .map(|p| p.name)
            .collect();
        names.sort();
        assert_eq!(names, vec!["core-pkg".to_string(), "extra-pkg".to_string()]);

        let body = std::fs::read_to_string(&persist_path).unwrap();
        assert!(body.contains("\"core-pkg\""));
        assert!(body.contains("\"extra-pkg\""));

        if let Ok(mut g) = super::idx().write() {
            g.pkgs.clear();
        }

        let _ = std::fs::remove_file(&persist_path);
        let _ = std::fs::remove_dir_all(&shim_root);
    }
}

/// What: Download a repository sync database to disk for offline inspection.
///
/// Inputs:
/// - `repo_dir`: Directory to store the downloaded database file.
/// - `repo`: Repository name (e.g., `core`).
/// - `arch`: Architecture component (e.g., `x86_64`).
///
/// Output:
/// - `Ok(PathBuf)` to the downloaded file when successful; boxed error otherwise.
///
/// Details:
/// - Fetches via HTTPS, writes the raw payload without decompressing, and ensures directories
///   exist before saving.
#[allow(dead_code)]
pub async fn download_sync_db(repo_dir: &Path, repo: &str, arch: &str) -> Result<PathBuf> {
    let base = "https://geo.mirror.pkgbuild.com";
    let url = format!("{base}/{repo}/os/{arch}/{repo}.db");
    let out_path = repo_dir.join(format!("{repo}-{arch}.db"));
    let out_path_clone = out_path.clone();
    let body = task::spawn_blocking(move || curl::curl_text(&url)).await??;
    task::spawn_blocking(move || -> Result<()> {
        fs::create_dir_all(out_path_clone.parent().unwrap_or_else(|| Path::new(".")))?;
        let mut f = fs::File::create(&out_path_clone)?;
        f.write_all(body.as_bytes())?;
        Ok(())
    })
    .await??;
    Ok(out_path)
}
