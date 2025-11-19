use crate::logic::files::get_pkgbuild_from_cache;
use crate::state::{PackageItem, Source};
use crate::util::percent_encode;
use std::sync::Mutex;
use std::time::{Duration, Instant};

type Result<T> = super::Result<T>;

// Rate limiter for PKGBUILD requests to avoid overwhelming AUR servers
static PKGBUILD_RATE_LIMITER: Mutex<Option<Instant>> = Mutex::new(None);
const PKGBUILD_MIN_INTERVAL_MS: u64 = 200; // Minimum 200ms between requests (reduced from 500ms for faster preview)

/// What: Fetch PKGBUILD content for a package from AUR or official Git packaging repos.
///
/// Inputs:
/// - `item`: Package whose PKGBUILD should be retrieved.
///
/// Output:
/// - `Ok(String)` with PKGBUILD text when available; `Err` on network or lookup failure.
///
/// Details:
/// - First tries offline methods (yay/paru cache) for fast loading.
/// - Then tries network with rate limiting and timeout (10s).
/// - Uses curl with timeout to prevent hanging on slow servers.
pub async fn fetch_pkgbuild_fast(item: &PackageItem) -> Result<String> {
    let name = item.name.clone();

    // 1. Try offline methods first (yay/paru cache) - this is fast!
    if let Some(cached) = tokio::task::spawn_blocking({
        let name = name.clone();
        move || get_pkgbuild_from_cache(&name)
    })
    .await?
    {
        tracing::debug!("Using cached PKGBUILD for {} (offline)", name);
        return Ok(cached);
    }

    // 2. Rate limiting: ensure minimum interval between requests
    let delay = {
        let mut last_request = PKGBUILD_RATE_LIMITER.lock().unwrap();
        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            if elapsed < Duration::from_millis(PKGBUILD_MIN_INTERVAL_MS) {
                let delay = Duration::from_millis(PKGBUILD_MIN_INTERVAL_MS) - elapsed;
                tracing::debug!(
                    "Rate limiting PKGBUILD request for {}: waiting {:?}",
                    name,
                    delay
                );
                // Drop the guard before await
                *last_request = Some(Instant::now());
                Some(delay)
            } else {
                *last_request = Some(Instant::now());
                None
            }
        } else {
            *last_request = Some(Instant::now());
            None
        }
    };
    if let Some(delay) = delay {
        tokio::time::sleep(delay).await;
    }

    // 3. Fetch from network with timeout
    match &item.source {
        Source::Aur => {
            let url = format!(
                "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
                percent_encode(&name)
            );
            // Use curl with timeout to prevent hanging
            let res = tokio::task::spawn_blocking({
                let url = url.clone();
                move || super::curl_text_with_args(&url, &["--max-time", "10"])
            })
            .await??;
            Ok(res)
        }
        Source::Official { .. } => {
            let url_main = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
                percent_encode(&name)
            );
            if let Ok(Ok(txt)) = tokio::task::spawn_blocking({
                let u = url_main.clone();
                move || super::curl_text_with_args(&u, &["--max-time", "10"])
            })
            .await
            {
                return Ok(txt);
            }
            let url_master = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
                percent_encode(&name)
            );
            let txt = tokio::task::spawn_blocking({
                let u = url_master;
                move || super::curl_text_with_args(&u, &["--max-time", "10"])
            })
            .await??;
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

    #[test]
    #[allow(clippy::await_holding_lock)]
    fn pkgbuild_fetches_official_main_then_master() {
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
        // curl_args creates: ["-sSLf", "--max-time", "10", "url"]
        // In sh/bash, $1="-sSLf", $2="--max-time", $3="10", $4=url
        // Get the last argument using eval
        let script = "#!/bin/sh\neval \"url=\\$$#\"\nif echo \"$url\" | grep -q '/-/raw/main/'; then exit 22; fi\necho 'pkgrel=2'\n";
        std::fs::write(&curl, script.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm).unwrap();
        }

        // Create fake paru and yay that fail (to prevent get_pkgbuild_from_cache from fetching real data)
        let mut paru = bin.clone();
        paru.push("paru");
        std::fs::write(&paru, b"#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&paru).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&paru, perm).unwrap();
        }

        let mut yay = bin.clone();
        yay.push("yay");
        std::fs::write(&yay, b"#!/bin/sh\nexit 1\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&yay).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&yay, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe { std::env::set_var("PATH", &new_path) };

        // Set HOME to empty directory to avoid finding cached PKGBUILDs
        let old_home = std::env::var("HOME").unwrap_or_default();
        unsafe { std::env::set_var("HOME", root.to_string_lossy().as_ref()) };

        // Create a new tokio runtime AFTER setting PATH and HOME so worker threads inherit them
        let rt = tokio::runtime::Runtime::new().unwrap();
        let txt = rt.block_on(async {
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
            super::fetch_pkgbuild_fast(&item).await.unwrap()
        });

        assert!(txt.contains("pkgrel=2"));

        unsafe { std::env::set_var("PATH", &old_path) };
        unsafe { std::env::set_var("HOME", &old_home) };
        let _ = std::fs::remove_dir_all(&root);
    }
}
