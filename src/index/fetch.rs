#[cfg(not(windows))]
use std::collections::HashSet;
#[cfg(not(windows))]
use std::path::Path;

#[cfg(not(windows))]
use super::OfficialPkg;
#[cfg(not(windows))]
use super::distro::{artix_repo_names, blackarch_repo_names, cachyos_repo_names, eos_repo_names};

/// What: Keep only repository names that have an active section in `pacman.conf`.
///
/// Inputs:
/// - `candidates`: Built-in distro repo names to probe with `pacman -Sl`.
/// - `active_lower`: Lowercase names from [`crate::logic::repos::PacmanConfScan::active_repo_names_lower`].
///
/// Output:
/// - Names from `candidates` present in `active_lower`, preserving candidate order.
///
/// Details:
/// - Empty `active_lower` yields an empty list so callers skip optional probes when the config
///   scan produced no active repositories (e.g. unreadable `pacman.conf`).
#[cfg(not(windows))]
fn optional_repos_configured_for_sl(
    candidates: &[&'static str],
    active_lower: &HashSet<String>,
) -> Vec<&'static str> {
    if active_lower.is_empty() {
        return Vec::new();
    }
    candidates
        .iter()
        .copied()
        .filter(|repo| active_lower.contains(&repo.to_lowercase()))
        .collect()
}

/// What: Run `pacman -Sl` for one repository and return captured stdout.
///
/// Inputs:
/// - `repo`: Sync database name (must live for the `'static` lifetime of literals from distro tables).
///
/// Output:
/// - Decoded stdout, or an empty string when spawn or pacman fails (same as prior inline callers).
///
/// Details:
/// - Runs inside `spawn_blocking` because `Command::output` blocks the async runtime.
#[cfg(not(windows))]
async fn pacman_sl_stdout(repo: &'static str) -> String {
    tokio::task::spawn_blocking(move || crate::util::pacman::run_pacman(&["-Sl", repo]))
        .await
        .ok()
        .and_then(std::result::Result::ok)
        .unwrap_or_default()
}

/// What: Run `pacman -Sl` for each configured optional repository name.
///
/// Inputs:
/// - `repos`: Repository names already filtered against `pacman.conf`.
///
/// Output:
/// - Pairs of `(repo, stdout)` in the same order as `repos`.
///
/// Details:
/// - Each entry uses its own blocking task to mirror the prior sequential `spawn_blocking` layout.
#[cfg(not(windows))]
async fn pacman_sl_pairs_from_static_repos(repos: &[&'static str]) -> Vec<(&'static str, String)> {
    let mut out = Vec::with_capacity(repos.len());
    for &repo in repos {
        let body = pacman_sl_stdout(repo).await;
        out.push((repo, body));
    }
    out
}

/// What: Fetch a minimal list of official packages using `pacman -Sl`.
///
/// Inputs:
/// - None (calls `pacman -Sl` for known repositories in the background)
///
/// Output:
/// - `Ok(Vec<OfficialPkg>)` where `name`, `repo`, and `version` are set; `arch` and `description`
///   are empty for speed. The result is deduplicated by `(repo, name)`.
///
/// Details:
/// - Combines results from core, extra, multilib, `EndeavourOS`, `CachyOS`, `Artix Linux`, and
///   `BlackArch` repositories before sorting and deduplicating entries.
/// - Runs `pacman -Sl` for `EndeavourOS` / `CachyOS` / Artix / `BlackArch` names only when an active
///   matching `[repo]` exists in `/etc/pacman.conf` (includes), so missing databases do not spawn
///   pacman or emit warnings.
/// - Adds `pacman -Sl <name>` for each `[[repo]]` `name` in `repos.conf` that is not already in the
///   builtin list (e.g. Chaotic-AUR) so third-party databases appear in search.
#[cfg(not(windows))]
pub async fn fetch_official_pkg_names()
-> Result<Vec<OfficialPkg>, Box<dyn std::error::Error + Send + Sync>> {
    let active_lower = crate::logic::repos::scan_pacman_conf_path(Path::new(
        crate::logic::repos::DEFAULT_MAIN_PACMAN_PATH,
    ))
    .active_repo_names_lower();

    let eos_for_sl = optional_repos_configured_for_sl(eos_repo_names(), &active_lower);
    let cachyos_for_sl = optional_repos_configured_for_sl(cachyos_repo_names(), &active_lower);
    let artix_for_sl = optional_repos_configured_for_sl(artix_repo_names(), &active_lower);
    let blackarch_for_sl = optional_repos_configured_for_sl(blackarch_repo_names(), &active_lower);

    // 1) Get repo/name/version quickly via -Sl
    let core = pacman_sl_stdout("core").await;
    let extra = pacman_sl_stdout("extra").await;
    let multilib = pacman_sl_stdout("multilib").await;
    let eos_pairs = pacman_sl_pairs_from_static_repos(&eos_for_sl).await;
    let cach_pairs = pacman_sl_pairs_from_static_repos(&cachyos_for_sl).await;
    let artix_pairs = pacman_sl_pairs_from_static_repos(&artix_for_sl).await;
    let blackarch_pairs = pacman_sl_pairs_from_static_repos(&blackarch_for_sl).await;

    let mut sl_lower_done = HashSet::<String>::new();
    for x in ["core", "extra", "multilib"] {
        sl_lower_done.insert(x.to_string());
    }
    for &r in &eos_for_sl {
        sl_lower_done.insert(r.to_lowercase());
    }
    for &r in &cachyos_for_sl {
        sl_lower_done.insert(r.to_lowercase());
    }
    for &r in &artix_for_sl {
        sl_lower_done.insert(r.to_lowercase());
    }
    for &r in &blackarch_for_sl {
        sl_lower_done.insert(r.to_lowercase());
    }
    let extra_repo_names = crate::logic::repos::repos_conf_repo_names_for_index_sl(&sl_lower_done);
    let mut repos_conf_pairs: Vec<(String, String)> = Vec::with_capacity(extra_repo_names.len());
    for name in extra_repo_names {
        let nm = name.clone();
        let body =
            tokio::task::spawn_blocking(move || crate::util::pacman::run_pacman(&["-Sl", &nm]))
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_default();
        repos_conf_pairs.push((name, body));
    }

    let mut pkgs: Vec<OfficialPkg> = Vec::new();
    for (repo, text) in [("core", core), ("extra", extra), ("multilib", multilib)]
        .into_iter()
        .chain(eos_pairs)
        .chain(cach_pairs)
        .chain(artix_pairs)
        .chain(blackarch_pairs)
    {
        for line in text.lines() {
            // Format: "repo pkgname version [installed]"
            let mut it = line.split_whitespace();
            let r = it.next();
            let n = it.next();
            let v = it.next();
            let Some(r) = r else {
                continue;
            };
            let Some(n) = n else {
                continue;
            };
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
    for (repo, text) in repos_conf_pairs {
        for line in text.lines() {
            let mut it = line.split_whitespace();
            let r = it.next();
            let n = it.next();
            let v = it.next();
            let Some(r) = r else {
                continue;
            };
            let Some(n) = n else {
                continue;
            };
            if r != repo.as_str() {
                continue;
            }
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

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    /// What: Ensure `-Sl` output is parsed and deduplicated by `(repo, name)`.
    ///
    /// Inputs:
    /// - Fake `pacman` binary returning scripted `-Sl` responses for repos.
    ///
    /// Output:
    /// - `fetch_official_pkg_names` yields distinct package tuples in sorted order.
    ///
    /// Details:
    /// - Validates that cross-repo lines are filtered and duplicates removed before returning.
    async fn fetch_parses_sl_and_dedups_by_repo_and_name() {
        let _guard = crate::global_test_mutex_lock();

        // Create a fake pacman on PATH
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_pacman_sl_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("Failed to create test root directory");
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).expect("Failed to create test bin directory");
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
        std::fs::write(&script, body).expect("Failed to write test pacman script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&script)
                .expect("Failed to read test pacman script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&script, perm)
                .expect("Failed to set test pacman script permissions");
        }
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe { std::env::set_var("PATH", &new_path) };

        let pkgs = super::fetch_official_pkg_names()
            .await
            .expect("Failed to fetch official package names in test");

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
