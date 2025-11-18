use crate::state::{PackageItem, Source};
use crate::util::percent_encode;

type Result<T> = super::Result<T>;

/// What: Fetch PKGBUILD content for a package from AUR or official Git packaging repos.
///
/// Inputs:
/// - `item`: Package whose PKGBUILD should be retrieved.
///
/// Output:
/// - `Ok(String)` with PKGBUILD text when available; `Err` on network or lookup failure.
pub async fn fetch_pkgbuild_fast(item: &PackageItem) -> Result<String> {
    match &item.source {
        Source::Aur => {
            let url = format!(
                "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
                percent_encode(&item.name)
            );
            let res = tokio::task::spawn_blocking(move || super::curl_text(&url)).await??;
            Ok(res)
        }
        Source::Official { .. } => {
            let name = item.name.clone();
            let url_main = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
                percent_encode(&name)
            );
            if let Ok(Ok(txt)) = tokio::task::spawn_blocking({
                let u = url_main.clone();
                move || super::curl_text(&u)
            })
            .await
            {
                return Ok(txt);
            }
            let url_master = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
                percent_encode(&name)
            );
            let txt = tokio::task::spawn_blocking(move || super::curl_text(&url_master)).await??;
            Ok(txt)
        }
    }
}

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn pkgbuild_fetches_aur_via_curl_text() {
        let _guard = crate::sources::test_mutex().lock().unwrap();
        // Shim PATH with fake curl
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_curl_pkgbuild_{}_{}",
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
        let mut curl = bin.clone();
        curl.push("curl");
        let script = "#!/bin/sh\necho 'pkgver=1'\n";
        std::fs::write(&curl, script.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe { std::env::set_var("PATH", &new_path) };

        let item = PackageItem {
            name: "yay-bin".into(),
            version: String::new(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
        };
        let txt = super::fetch_pkgbuild_fast(&item).await.unwrap();
        assert!(txt.contains("pkgver=1"));

        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn pkgbuild_fetches_official_main_then_master() {
        let _guard = crate::global_test_mutex_lock();
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_curl_pkgbuild_official_{}_{}",
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
        let mut curl = bin.clone();
        curl.push("curl");
        // Fail when URL contains '/-/raw/main/' and succeed when '/-/raw/master/'
        let script = "#!/usr/bin/env bash\nset -e\nargs=(); for a in \"$@\"; do args+=(\"$a\"); done; url=${args[3]}; if echo \"$url\" | grep -q '/-/raw/main/'; then exit 22; else echo 'pkgrel=2'; fi\n";
        std::fs::write(&curl, script.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe { std::env::set_var("PATH", &new_path) };
        // Ensure PATH is set before executing commands
        std::thread::sleep(std::time::Duration::from_millis(10));

        let item = PackageItem {
            name: "ripgrep".into(),
            version: String::new(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        };
        let txt = super::fetch_pkgbuild_fast(&item).await.unwrap();
        assert!(txt.contains("pkgrel=2"));

        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_dir_all(&root);
    }
}
