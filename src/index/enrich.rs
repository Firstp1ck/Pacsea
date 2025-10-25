use super::{idx, save_to_disk};

/// Request enrichment (`pacman -Si`) for a set of package `names` in the
/// background, merge fields into the index, persist, and notify.
///
/// Only non-empty results are applied. Field updates prefer non-empty values
/// from `-Si` output and leave existing values untouched when `-Si` omits them.
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
