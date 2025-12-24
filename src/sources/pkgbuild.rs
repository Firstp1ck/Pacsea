//! PKGBUILD fetching with arch-toolkit integration and caching.

use crate::logic::files::get_pkgbuild_from_cache;
use crate::sources::get_arch_client;
use crate::state::{PackageItem, Source};
use crate::util::percent_encode;

/// Result type alias for PKGBUILD fetching operations.
type Result<T> = super::Result<T>;

/// What: Fetch PKGBUILD content for a package from AUR or official Git packaging repos.
///
/// Inputs:
/// - `item`: Package whose PKGBUILD should be retrieved.
///
/// Output:
/// - `Ok(String)` with PKGBUILD text when available; `Err` on network or lookup failure.
///
/// # Errors
/// - Returns `Err` when network request fails
/// - Returns `Err` when PKGBUILD cannot be fetched from AUR or official GitLab repositories
/// - Returns `Err` when task spawn fails
/// - Returns `Err` when `ArchClient` is not initialized (for AUR packages)
///
/// Details:
/// - First tries offline methods (yay/paru cache) for fast loading.
/// - For AUR packages: uses arch-toolkit with automatic rate limiting and retry logic.
/// - For official packages: uses curl with timeout to fetch from GitLab.
/// - Leverages automatic rate limiting, retry logic, and optional caching from arch-toolkit for AUR packages.
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

    // 2. Fetch from network
    match &item.source {
        Source::Aur => {
            // Use arch-toolkit for AUR packages
            let Some(client) = get_arch_client() else {
                return Err("AUR PKGBUILD unavailable: ArchClient not initialized".into());
            };

            match client.aur().pkgbuild(&name).await {
                Ok(text) if !text.trim().is_empty() && text.contains("pkgname") => Ok(text),
                Ok(_) => Err("AUR returned empty or invalid PKGBUILD".into()),
                Err(e) => Err(format!("AUR PKGBUILD fetch failed: {e}").into()),
            }
        }
        Source::Official { .. } => {
            let url_main = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
                percent_encode(&name)
            );
            let main_result = tokio::task::spawn_blocking({
                let u = url_main.clone();
                move || crate::util::curl::curl_text_with_args(&u, &["--max-time", "10"])
            })
            .await;
            if let Ok(Ok(txt)) = main_result {
                return Ok(txt);
            }
            let url_master = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
                percent_encode(&name)
            );
            let txt = tokio::task::spawn_blocking({
                let u = url_master;
                move || crate::util::curl::curl_text_with_args(&u, &["--max-time", "10"])
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
    #[ignore = "Requires network access and ArchClient initialization"]
    async fn pkgbuild_fetches_aur_via_arch_toolkit() {
        // Initialize ArchClient for testing
        crate::sources::init_arch_client().expect("Failed to initialize ArchClient for test");

        let item = PackageItem {
            name: "yay".into(),
            version: String::new(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        let result = super::fetch_pkgbuild_fast(&item).await;
        match result {
            Ok(txt) => {
                // Verify PKGBUILD structure
                assert!(txt.contains("pkgname") || txt.contains("pkgver"));
            }
            Err(e) => {
                // Network errors are acceptable in tests
                assert!(
                    e.to_string().contains("AUR PKGBUILD")
                        || e.to_string().contains("ArchClient")
                        || e.to_string().contains("Network"),
                    "Unexpected error: {e}"
                );
            }
        }
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
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("Failed to create test root directory");
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).expect("Failed to create test bin directory");
        let mut curl = bin.clone();
        curl.push("curl");
        // Fail when URL contains '/-/raw/main/' and succeed when '/-/raw/master/'
        // curl_args creates: ["-sSLf", "--connect-timeout", "30", "--max-time", "60", "-H", "User-Agent: ...", "--max-time", "10", "url"]
        // Get the last argument by looping through all arguments
        // Use printf instead of echo to avoid trailing newline that confuses the HTTP header parser
        let script = "#!/bin/sh\nfor arg; do :; done\nurl=\"$arg\"\nif echo \"$url\" | grep -q '/-/raw/main/'; then exit 22; fi\nprintf 'pkgrel=2'\n";
        std::fs::write(&curl, script.as_bytes()).expect("Failed to write test curl script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl)
                .expect("Failed to read test curl script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm)
                .expect("Failed to set test curl script permissions");
        }

        // Create fake paru and yay that fail (to prevent get_pkgbuild_from_cache from fetching real data)
        let mut paru = bin.clone();
        paru.push("paru");
        std::fs::write(&paru, b"#!/bin/sh\nexit 1\n").expect("Failed to write test paru script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&paru)
                .expect("Failed to read test paru script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&paru, perm)
                .expect("Failed to set test paru script permissions");
        }

        let mut yay = bin.clone();
        yay.push("yay");
        std::fs::write(&yay, b"#!/bin/sh\nexit 1\n").expect("Failed to write test yay script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&yay)
                .expect("Failed to read test yay script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&yay, perm)
                .expect("Failed to set test yay script permissions");
        }
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe { std::env::set_var("PATH", &new_path) };
        // Enable curl PATH lookup override so our fake curl is used instead of /usr/bin/curl
        unsafe { std::env::set_var("PACSEA_CURL_PATH", "1") };

        // Set HOME to empty directory to avoid finding cached PKGBUILDs
        let old_home = std::env::var("HOME").unwrap_or_default();
        unsafe { std::env::set_var("HOME", root.to_string_lossy().as_ref()) };

        // Create a new tokio runtime AFTER setting PATH and HOME so worker threads inherit them
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime for test");
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
                out_of_date: None,
                orphaned: false,
            };
            super::fetch_pkgbuild_fast(&item)
                .await
                .expect("Failed to fetch PKGBUILD in test")
        });

        assert!(txt.contains("pkgrel=2"));

        unsafe { std::env::set_var("PATH", &old_path) };
        unsafe { std::env::set_var("HOME", &old_home) };
        unsafe { std::env::remove_var("PACSEA_CURL_PATH") };
        let _ = std::fs::remove_dir_all(&root);
    }
}
