#![cfg(windows)]

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

/// Invoke `curl -sSLf` and parse JSON.
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

/// Invoke `curl -sSLf` and return plain text.
fn curl_text(url: &str) -> Result<String> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed for {url}: {:?}", out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// Fetch the mirror status JSON and generate a simple `mirrorlist.txt`, then save
/// both files under the provided repository directory.
///
/// - Writes:
///   - mirrors.json: the raw JSON from https://archlinux.org/mirrors/status/json/
///   - mirrorlist.txt: lines like `Server = <https_url>/$repo/os/$arch`
///
/// Returns the path to the written `mirrorlist.txt` on success.
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

/// Query the official Arch Packages API to build a minimal index of all packages
/// in the given repos for the `x86_64` architecture. Saves the resulting index
/// to `persist_path` and updates the in-memory index.
///
/// It constructs `OfficialPkg` entries with:
/// - name: pkgname
/// - repo: repo
/// - arch: arch
/// - version: pkgver
/// - description: pkgdesc
///
/// On success, it sends a `notify_tx` signal; on failure, a human-readable error via `net_err_tx`.
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

/// Convenience helper to fetch mirrors into the repository directory and then
/// refresh the official package index using the Arch API. Designed for Windows.
///
/// This does not run automatically; call it from your startup path (Windows only)
/// before or in parallel with the normal `update_in_background` if you want an
/// index without `pacman`.
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

/// Optional helper to download a specific repo sync database to the repository dir for
/// offline inspection (not required for the app to show packages).
///
/// Example URLs:
/// - https://geo.mirror.pkgbuild.com/core/os/x86_64/core.db
/// - https://geo.mirror.pkgbuild.com/extra/os/x86_64/extra.db
/// - https://geo.mirror.pkgbuild.com/multilib/os/x86_64/multilib.db
///
/// This function writes the raw file (likely zstd-compressed tar) without parsing.
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
