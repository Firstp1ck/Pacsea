use serde_json::Value;
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

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Fetch a JSON payload via `curl -sSLf` and deserialize it.
///
/// Inputs:
/// - `url`: HTTP(S) endpoint expected to return JSON.
///
/// Output:
/// - `Ok(serde_json::Value)` containing the parsed document; boxed error on failure.
///
/// Details:
/// - Treats non-success exit codes and JSON/UTF-8 parsing failures as errors to propagate.
fn curl_json(url: &str) -> Result<Value> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed for {url}: {:?}", out.status).into());
    }
    let body = String::from_utf8(out.stdout)?;
    let v: Value = serde_json::from_str(&body)?;
    Ok(v)
}

/// What: Fetch a text payload via `curl -sSLf`.
///
/// Inputs:
/// - `url`: HTTP(S) endpoint expected to return text data.
///
/// Output:
/// - `Ok(String)` containing UTF-8 text on success; boxed error otherwise.
///
/// Details:
/// - Treats non-success exit codes and UTF-8 decoding failures as errors to propagate.
#[allow(dead_code)]
fn curl_text(url: &str) -> Result<String> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed for {url}: {:?}", out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

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
        let json = curl_json(status_url)?;

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
            let mut page: usize = 1;
            let limit: usize = 250;
            loop {
                let url = format!("https://archlinux.org/packages/search/json/?repo={repo}&arch={arch}&limit={limit}&page={page}");
                let v = match curl_json(&url) {
                    Ok(v) => v,
                    Err(e) => {
                        // If a page fails, bubble the error up; no partial repo result
                        return Err(format!("Failed to fetch package list for {repo}: {e}").into());
                    }
                };
                let results = v
                    .get("results")
                    .and_then(|x| x.as_array())
                    .cloned()
                    .unwrap_or_default();
                if results.is_empty() {
                    break;
                }
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
                }
                page += 1;
            }
        }
        // Sort and dedup by (repo, name)
        pkgs.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
        pkgs.dedup_by(|a, b| a.repo == b.repo && a.name == b.name);
        Ok(pkgs)
    })
    .await;

    match res {
        Ok(Ok(new_list)) => {
            // Replace in-memory index and persist to disk
            if let Ok(mut guard) = idx().write() {
                guard.pkgs = new_list;
            }
            save_to_disk(&persist_path);
            let _ = notify_tx.send(());
        }
        Ok(Err(e)) => {
            let _ = net_err_tx.send(format!("Failed to fetch official index via API: {e}"));
        }
        Err(join_err) => {
            let _ = net_err_tx.send(format!("Task join error: {join_err}"));
        }
    }
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
pub async fn refresh_windows_mirrors_and_index(
    persist_path: PathBuf,
    repo_dir: PathBuf,
    net_err_tx: tokio::sync::mpsc::UnboundedSender<String>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
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
    refresh_official_index_from_arch_api(persist_path, net_err_tx, notify_tx).await;
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
    let body = task::spawn_blocking(move || curl_text(&url)).await??;
    task::spawn_blocking(move || -> Result<()> {
        fs::create_dir_all(out_path_clone.parent().unwrap_or_else(|| Path::new(".")))?;
        let mut f = fs::File::create(&out_path_clone)?;
        f.write_all(body.as_bytes())?;
        Ok(())
    })
    .await??;
    Ok(out_path)
}
