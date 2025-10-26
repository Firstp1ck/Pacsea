use super::OfficialPkg;

/// What: Fetch a minimal list of official packages using `pacman -Sl`.
///
/// Inputs:
/// - None (calls `pacman -Sl` for known repositories in the background)
///
/// Output:
/// - `Ok(Vec<OfficialPkg>)` where `name`, `repo`, and `version` are set; `arch` and `description`
///   are empty for speed. The result is deduplicated by `(repo, name)`.
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
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let extra = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "extra"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let multilib = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "multilib"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    // EOS/EndeavourOS: attempt both known names
    let mut eos_pairs: Vec<(&str, String)> = Vec::new();
    for &repo in crate::index::eos_repo_names().iter() {
        let body = tokio::task::spawn_blocking(move || run_pacman(&["-Sl", repo]))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default();
        eos_pairs.push((repo, body));
    }
    // CachyOS: attempt multiple potential repo names; missing ones yield empty output
    let mut cach_pairs: Vec<(&str, String)> = Vec::new();
    for &repo in crate::index::cachyos_repo_names().iter() {
        let body = tokio::task::spawn_blocking(move || run_pacman(&["-Sl", repo]))
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or_default();
        cach_pairs.push((repo, body));
    }
    let mut pkgs: Vec<OfficialPkg> = Vec::new();
    for (repo, text) in [
        ("core", core),
        ("extra", extra),
        ("multilib", multilib),
    ]
    .into_iter()
    .chain(eos_pairs.into_iter())
    .chain(cach_pairs.into_iter())
    {
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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn fetch_parses_sl_and_dedups_by_repo_and_name() {
        let _guard = crate::index::test_mutex().lock().unwrap();

        // Create a fake pacman on PATH
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_sl_{}_{}",
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
      echo "core foo 1.0"  # duplicate
      echo "extra should_not_be_kept 9.9" # different repo, filtered out
      ;;
    extra)
      echo "extra foo 1.1"
      echo "extra baz 3.0"
      ;;
    *) ;;
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

        let pkgs = super::fetch_official_pkg_names().await.unwrap();

        // Cleanup PATH and temp files early
        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_dir_all(&root);

        // Expect: (core,foo 1.0), (extra,foo 1.1), (extra,baz 3.0)
        assert_eq!(pkgs.len(), 3);
        let mut tuples: Vec<(String, String, String)> = pkgs
            .into_iter()
            .map(|p| (p.repo, p.name, p.version))
            .collect();
        tuples.sort();
        assert_eq!(
            tuples,
            vec![
                ("core".to_string(), "foo".to_string(), "1.0".to_string()),
                ("extra".to_string(), "baz".to_string(), "3.0".to_string()),
                ("extra".to_string(), "foo".to_string(), "1.1".to_string()),
            ]
        );
    }
}
