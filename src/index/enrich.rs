use super::{idx, save_to_disk};

/// What: Request enrichment (`pacman -Si`) for a set of package `names` in the background,
/// merge fields into the index, persist, and notify.
///
/// Inputs:
/// - `persist_path`: Path to write the updated index JSON
/// - `notify_tx`: Channel to notify the UI after enrichment/persist
/// - `names`: Package names to enrich
///
/// Output:
/// - Spawns a task that enriches and persists the index; sends a unit notification on completion.
///
/// Details:
/// - Only non-empty results are applied; fields prefer non-empty values from `-Si` output and leave
///   existing values untouched when omitted.
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
            let mut cur_packager: Option<String> = None;
            for line in out.lines().chain([""].iter().copied()) {
                let line = line.trim_end();
                if line.is_empty() {
                    if let Some(n) = cur_name.take() {
                        let d = cur_desc.take().unwrap_or_default();
                        let a = cur_arch.take().unwrap_or_default();
                        let mut r = cur_repo.take().unwrap_or_default();
                        let v = cur_ver.take().unwrap_or_default();
                        let packager = cur_packager.take().unwrap_or_default();

                        // Detect Manjaro packages: name starts with "manjaro-" or packager contains "manjaro"
                        if n.starts_with("manjaro-") || packager.to_lowercase().contains("manjaro") {
                            r = "manjaro".to_string();
                        }

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
                        "Packager" => cur_packager = Some(val.to_string()),
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
                    if p.description.is_empty() {
                        p.description = d.clone();
                    }
                    if !a.is_empty() {
                        p.arch = a.clone();
                    }
                    if !r.is_empty() {
                        p.repo = r.clone();
                    }
                    if !v.is_empty() {
                        p.version = v.clone();
                    }
                }
            }
        }
        save_to_disk(&persist_path);
        let _ = notify_tx.send(());
    });
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    /// What: No enrichment work triggered when names list is empty
    ///
    /// - Input: Empty names vector passed to request_enrich_for
    /// - Output: No notification sent on channel within timeout
    async fn index_enrich_noop_on_empty_names() {
        use std::path::PathBuf;
        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!(
            "pacsea_idx_empty_enrich_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let idx_json = serde_json::json!({ "pkgs": [] });
        std::fs::write(&path, serde_json::to_string(&idx_json).unwrap()).unwrap();
        crate::index::load_from_disk(&path);

        let (notify_tx, mut notify_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        super::request_enrich_for(path.clone(), notify_tx, Vec::new());
        let none = tokio::time::timeout(std::time::Duration::from_millis(200), notify_rx.recv())
            .await
            .ok()
            .flatten();
        assert!(none.is_none());
        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    /// What: Enrichment updates fields from pacman -Si and notifies
    ///
    /// - Input: Seed index with empty fields; fake pacman -Si output for name
    /// - Output: Index fields updated (version/desc/repo/arch) and notify sent
    async fn enrich_updates_fields_and_notifies() {
        let _guard = crate::index::test_mutex().lock().unwrap();
        // Seed index with minimal entries
        if let Ok(mut g) = crate::index::idx().write() {
            g.pkgs = vec![crate::index::OfficialPkg {
                name: "foo".to_string(),
                repo: String::new(),
                arch: String::new(),
                version: String::new(),
                description: String::new(),
            }];
        }
        // Fake pacman -Si output via PATH shim
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_si_{}_{}",
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
if [[ "$1" == "-Si" ]]; then
  # Print two blocks, one for foo, one unrelated
  cat <<EOF
Name            : foo
Version         : 1.2.3
Architecture    : x86_64
Repository      : core
Description     : hello

Name            : other
Version         : 9.9.9
Architecture    : any
Repository      : extra
Description     : nope
EOF
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

        // Temp file for persistence
        let mut path: std::path::PathBuf = std::env::temp_dir();
        path.push("pacsea_enrich_test.json");
        crate::index::save_to_disk(&path);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        super::request_enrich_for(path.clone(), tx, vec!["foo".into(), "foo".into()]);
        // Wait for notify
        let notified = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await
            .ok()
            .flatten()
            .is_some();
        assert!(notified);

        // Check that fields got updated for foo
        let all = crate::index::all_official();
        let pkg = all.iter().find(|p| p.name == "foo").unwrap();
        assert_eq!(pkg.version, "1.2.3");
        assert_eq!(pkg.description, "hello");
        match &pkg.source {
            crate::state::Source::Official { repo, arch } => {
                assert_eq!(repo, "core");
                assert_eq!(arch, "x86_64");
            }
            _ => panic!("expected official"),
        }

        // Cleanup
        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&root);
    }
}
