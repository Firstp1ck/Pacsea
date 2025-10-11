use super::{OfficialPkg, fetch_official_pkg_names, idx, save_to_disk};

/// Spawn a background task to refresh the official index and notify on changes.
///
/// On success, merges new names while preserving previously enriched fields
/// (repo, arch, version, description) for packages that still exist. Persists
/// the updated index and sends a unit message on `notify_tx` when the set of
/// names changes. On failure, sends a human-readable error on `net_err_tx`.
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
                        let old_names: HashSet<String> =
                            g.pkgs.iter().map(|p| p.name.clone()).collect();
                        let new_names: HashSet<String> =
                            new_pkgs.iter().map(|p| p.name.clone()).collect();
                        let different = old_names != new_names;
                        // Merge: prefer old/enriched fields when same name exists
                        let mut old_map: HashMap<String, &OfficialPkg> = HashMap::new();
                        for p in &g.pkgs {
                            old_map.insert(p.name.clone(), p);
                        }
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
