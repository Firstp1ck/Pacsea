#[cfg(not(target_os = "windows"))]
use super::fetch::fetch_official_pkg_names;
#[cfg(not(target_os = "windows"))]
use super::{OfficialPkg, idx, save_to_disk};

/// What: Spawn a background task to refresh the official index and notify on changes.
///
/// Inputs:
/// - `persist_path`: File path to persist the updated index JSON
/// - `net_err_tx`: Channel to send human-readable errors on failure
/// - `notify_tx`: Channel to notify the UI when the set of names changes
///
/// Output:
/// - Launches a task that updates the in-memory index and persists to disk when the set of names
///   changes; sends notifications/errors via the provided channels.
///
/// Details:
/// - Merges new names while preserving previously enriched fields (repo, arch, version, description)
///   for still-existing packages.
#[cfg(not(target_os = "windows"))]
pub async fn update_in_background(
    persist_path: std::path::PathBuf,
    net_err_tx: tokio::sync::mpsc::UnboundedSender<String>,
    notify_tx: tokio::sync::mpsc::UnboundedSender<()>,
) {
    tokio::spawn(async move {
        tracing::info!("refreshing official index in background");
        match fetch_official_pkg_names().await {
            Ok(new_pkgs) => {
                let new_count = new_pkgs.len();
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
                    tracing::info!(count = new_count, "official index updated (names changed)");
                } else {
                    tracing::debug!(
                        count = new_count,
                        "official index up-to-date (no name changes)"
                    );
                }
            }
            Err(e) => {
                let _ = net_err_tx.send(format!("Failed to refresh official index: {e}"));
                tracing::warn!(error = %e, "failed to refresh official index");
            }
        }
    });
}

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod tests {
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    /// What: Merge fetched names while preserving enriched fields and notify on change.
    ///
    /// Inputs:
    /// - Seed index with enriched entry and stub `pacman -Sl` to add new packages.
    ///
    /// Output:
    /// - Notification sent, no error emitted, and enriched data retained.
    ///
    /// Details:
    /// - Simulates pacman output via PATH override to exercise merge path.
    async fn update_merges_preserving_enriched_fields_and_notifies_on_name_changes() {
        let _guard = crate::index::test_mutex().lock().unwrap();

        // Seed current index with enriched fields
        if let Ok(mut g) = super::idx().write() {
            g.pkgs = vec![super::OfficialPkg {
                name: "foo".to_string(),
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
                version: "0.9".to_string(),
                description: "old".to_string(),
            }];
        }

        // Create a fake pacman on PATH that returns -Sl results for fetch
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_update_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let mut script = bin.clone();
        script.push("pacman");
        let body = r#"#!/usr/bin/env bash
set -e
if [[ "$1" == "-Sl" ]]; then
  repo="$2"
  case "$repo" in
    core)
      echo "core foo 1.0"
      ;;
    extra)
      echo "extra bar 2.0"
      ;;
  esac
  exit 0
fi
exit 0
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
        unsafe { std::env::set_var("PATH", &new_path) };

        // Setup channels
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<()>();

        let mut tmp = std::env::temp_dir();
        tmp.push("pacsea_update_merge.json");
        super::update_in_background(tmp.clone(), err_tx, notify_tx).await;

        // Expect notify within timeout and no error
        let notified =
            tokio::time::timeout(std::time::Duration::from_millis(500), notify_rx.recv())
                .await
                .ok()
                .flatten()
                .is_some();
        assert!(notified);
        let none = tokio::time::timeout(std::time::Duration::from_millis(200), err_rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());

        // Check merge kept enriched fields for existing name "foo"
        let items = crate::index::all_official();
        let foo = items.iter().find(|p| p.name == "foo").unwrap();
        match &foo.source {
            crate::state::Source::Official { repo, arch } => {
                assert_eq!(repo, "core");
                assert_eq!(arch, "x86_64");
            }
            _ => panic!("expected official"),
        }
        assert_eq!(foo.version, "0.9"); // preserved from enriched

        // Teardown
        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_file(&tmp);
        let _ = std::fs::remove_dir_all(&root);
    }
}
