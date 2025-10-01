use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use crate::state::{PackageItem, Source};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct OfficialIndex {
    pub pkgs: Vec<OfficialPkg>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OfficialPkg {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String, // core or extra
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arch: String, // e.g., x86_64/any
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

static OFFICIAL_INDEX: OnceLock<RwLock<OfficialIndex>> = OnceLock::new();
static INSTALLED_SET: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

fn idx() -> &'static RwLock<OfficialIndex> {
    OFFICIAL_INDEX.get_or_init(|| RwLock::new(OfficialIndex { pkgs: Vec::new() }))
}

fn installed_lock() -> &'static RwLock<HashSet<String>> {
    INSTALLED_SET.get_or_init(|| RwLock::new(HashSet::new()))
}

pub fn load_from_disk(path: &Path) {
    if let Ok(s) = fs::read_to_string(path)
        && let Ok(new_idx) = serde_json::from_str::<OfficialIndex>(&s)
        && let Ok(mut guard) = idx().write()
    {
        *guard = new_idx;
    }
}

pub fn save_to_disk(path: &Path) {
    if let Ok(guard) = idx().read()
        && let Ok(s) = serde_json::to_string(&*guard)
    {
        let _ = fs::write(path, s);
    }
}

pub fn search_official(query: &str) -> Vec<PackageItem> {
    let ql = query.trim().to_lowercase();
    if ql.is_empty() {
        return Vec::new();
    }
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        for p in &g.pkgs {
            let nl = p.name.to_lowercase();
            if nl.contains(&ql) {
                items.push(PackageItem {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    description: p.description.clone(),
                    source: Source::Official {
                        repo: p.repo.clone(),
                        arch: p.arch.clone(),
                    },
                });
            }
        }
    }
    items
}

pub fn all_official() -> Vec<PackageItem> {
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        items.reserve(g.pkgs.len());
        for p in &g.pkgs {
            items.push(PackageItem {
                name: p.name.clone(),
                version: p.version.clone(),
                description: p.description.clone(),
                source: Source::Official {
                    repo: p.repo.clone(),
                    arch: p.arch.clone(),
                },
            });
        }
    }
    items
}

pub async fn all_official_or_fetch(persist_path: &Path) -> Vec<PackageItem> {
    // if empty, fetch using pacman and persist
    let is_empty = idx().read().map(|g| g.pkgs.is_empty()).unwrap_or(true);
    if is_empty && let Ok(new_pkgs) = fetch_official_pkg_names().await {
        if let Ok(mut g) = idx().write() {
            g.pkgs = new_pkgs;
        }
        save_to_disk(persist_path);
    }
    all_official()
}

pub async fn update_in_background(
    persist_path: std::path::PathBuf,
    net_err_tx: tokio::sync::mpsc::UnboundedSender<String>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    tokio::spawn(async move {
        match fetch_official_pkg_names().await {
            Ok(new_pkgs) => {
                let (different, merged): (bool, Vec<OfficialPkg>) = {
                    let guard = idx().read().ok();
                    if let Some(g) = guard {
                        use std::collections::{HashMap, HashSet};
                        let old_names: HashSet<String> = g.pkgs.iter().map(|p| p.name.clone()).collect();
                        let new_names: HashSet<String> = new_pkgs.iter().map(|p| p.name.clone()).collect();
                        let different = old_names != new_names;
                        // Merge: prefer old/enriched fields when same name exists
                        let mut old_map: HashMap<String, &OfficialPkg> = HashMap::new();
                        for p in &g.pkgs { old_map.insert(p.name.clone(), p); }
                        let mut merged = Vec::with_capacity(new_pkgs.len());
                        for mut p in new_pkgs.into_iter() {
                            if let Some(old) = old_map.get(&p.name) {
                                // keep enriched data
                                p.repo = old.repo.clone();
                                p.arch = old.arch.clone();
                                p.version = old.version.clone();
                                p.description = old.description.clone();
                            }
                            merged.push(p);
                        }
                        (different, merged)
                    } else {
                        (true, new_pkgs)
                    }
                };
                if different {
                    if let Ok(mut g) = idx().write() {
                        g.pkgs = merged;
                    }
                    save_to_disk(&persist_path);
                    let _ = notify_tx.send(());
                }
            }
            Err(e) => {
                let _ = net_err_tx.send(format!("Failed to refresh official index: {e}"));
            }
        }
    });
}

pub async fn fetch_official_pkg_names()
-> Result<Vec<OfficialPkg>, Box<dyn std::error::Error + Send + Sync>> {
    fn run_pacman(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman").args(args).output()?;
        if !out.status.success() {
            return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    // 1) Get repo/name/version quickly via -Sl
    let core = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "core"]))
        .await
        .map_err(|e| format!("spawn failed: {e}"))??;
    let extra = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "extra"]))
        .await
        .map_err(|e| format!("spawn failed: {e}"))??;
    let mut pkgs: Vec<OfficialPkg> = Vec::new();
    for (repo, text) in [("core", core), ("extra", extra)] {
        for line in text.lines() {
            // Format: "repo pkgname version [installed]"
            let mut it = line.split_whitespace();
            let r = it.next();
            let n = it.next();
            let v = it.next();
            if r.is_none() || n.is_none() {
                continue;
            }
            let r = r.unwrap();
            let n = n.unwrap();
            if r != repo {
                continue;
            }
            // Keep name, repo, version; leave arch/description empty for speed
            pkgs.push(OfficialPkg {
                name: n.to_string(),
                repo: r.to_string(),
                arch: String::new(),
                version: v.unwrap_or("").to_string(),
                description: String::new(),
            });
        }
    }
    // de-dup by (repo,name)
    pkgs.sort_by(|a, b| a.repo.cmp(&b.repo).then(a.name.cmp(&b.name)));
    pkgs.dedup_by(|a, b| a.repo == b.repo && a.name == b.name);

    // Do not enrich here; keep only fast fields for the initial on-disk index.
    Ok(pkgs)
}

pub fn request_enrich_for(
    persist_path: std::path::PathBuf,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
    names: Vec<String>,
) {
    tokio::spawn(async move {
        // Deduplicate names
        use std::collections::HashSet;
        let set: HashSet<String> = names.into_iter().collect();
        if set.is_empty() {
            return;
        }
        // Helper to run pacman
        fn run_pacman(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
            let out = std::process::Command::new("pacman").args(args).output()?;
            if !out.status.success() {
                return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
            }
            Ok(String::from_utf8(out.stdout)?)
        }
        // Batch -Si queries
        let mut desc_map: std::collections::HashMap<String, (String, String, String, String)> =
            std::collections::HashMap::new(); // name -> (desc, arch, repo, version)
        const BATCH: usize = 100;
        let all: Vec<String> = set.into_iter().collect();
        for chunk in all.chunks(BATCH) {
            let args_owned: Vec<String> = std::iter::once("-Si".to_string())
                .chain(chunk.iter().cloned())
                .collect();
            let block = tokio::task::spawn_blocking(move || {
                let args_ref: Vec<&str> = args_owned.iter().map(|s| s.as_str()).collect();
                run_pacman(&args_ref)
            })
            .await;
            let Ok(Ok(out)) = block else { continue };
            // Parse blocks
            let mut cur_name: Option<String> = None;
            let mut cur_desc: Option<String> = None;
            let mut cur_arch: Option<String> = None;
            let mut cur_repo: Option<String> = None;
            let mut cur_ver: Option<String> = None;
            for line in out.lines().chain([""] .iter().copied()) {
                let line = line.trim_end();
                if line.is_empty() {
                    if let Some(n) = cur_name.take() {
                        let d = cur_desc.take().unwrap_or_default();
                        let a = cur_arch.take().unwrap_or_default();
                        let r = cur_repo.take().unwrap_or_default();
                        let v = cur_ver.take().unwrap_or_default();
                        desc_map.insert(n, (d, a, r, v));
                    }
                    continue;
                }
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim();
                    let val = v.trim();
                    match key {
                        "Name" => cur_name = Some(val.to_string()),
                        "Description" => cur_desc = Some(val.to_string()),
                        "Architecture" => cur_arch = Some(val.to_string()),
                        "Repository" => cur_repo = Some(val.to_string()),
                        "Version" => cur_ver = Some(val.to_string()),
                        _ => {}
                    }
                }
            }
        }
        if desc_map.is_empty() {
            return;
        }
        // Update index entries
        if let Ok(mut g) = idx().write() {
            for p in &mut g.pkgs {
                if let Some((d, a, r, v)) = desc_map.get(&p.name) {
                    if p.description.is_empty() { p.description = d.clone(); }
                    if !a.is_empty() { p.arch = a.clone(); }
                    if !r.is_empty() { p.repo = r.clone(); }
                    if !v.is_empty() { p.version = v.clone(); }
                }
            }
        }
        save_to_disk(&persist_path);
        let _ = notify_tx.send(());
    });
}

pub async fn refresh_installed_cache() {
    // pacman -Qq to list installed names
    fn run_pacman_q() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qq"])
            .output()?;
        if !out.status.success() {
            return Err(format!("pacman -Qq exited with {:?}", out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(run_pacman_q).await {
        let set: HashSet<String> = body.lines().map(|s| s.trim().to_string()).collect();
        if let Ok(mut g) = installed_lock().write() {
            *g = set;
        }
    }
}

pub fn is_installed(name: &str) -> bool {
    installed_lock()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(false)
}
