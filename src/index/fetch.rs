use super::OfficialPkg;

/// Fetch a minimal list of official packages using `pacman -Sl`.
///
/// Returns `OfficialPkg` entries with `name`, `repo`, and `version` set when
/// available. The `arch` and `description` fields are left empty for speed. The
/// result is deduplicated by `(repo, name)`.
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
    let eos = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "eos"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let endeavouros = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "endeavouros"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    // CachyOS: attempt multiple potential repo names; missing ones yield empty output
    let cachyos = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_core = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-core"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_extra = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-extra"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_v3 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-v3"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_core_v3 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-core-v3"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_extra_v3 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-extra-v3"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_v4 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-v4"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_core_v4 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-core-v4"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let cachyos_extra_v4 = tokio::task::spawn_blocking(|| run_pacman(&["-Sl", "cachyos-extra-v4"]))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let mut pkgs: Vec<OfficialPkg> = Vec::new();
    for (repo, text) in [
        ("core", core),
        ("extra", extra),
        ("multilib", multilib),
        ("eos", eos),
        ("endeavouros", endeavouros),
        ("cachyos", cachyos),
        ("cachyos-core", cachyos_core),
        ("cachyos-extra", cachyos_extra),
        ("cachyos-v3", cachyos_v3),
        ("cachyos-core-v3", cachyos_core_v3),
        ("cachyos-extra-v3", cachyos_extra_v3),
        ("cachyos-v4", cachyos_v4),
        ("cachyos-core-v4", cachyos_core_v4),
        ("cachyos-extra-v4", cachyos_extra_v4),
    ] {
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
