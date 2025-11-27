//! AUR search query execution and result parsing.

use crate::state::{PackageItem, Source};
use crate::util::{percent_encode, s};

/// What: Fetch search results from AUR and return items along with any error messages.
///
/// Input:
/// - `query` raw query string to search
///
/// Output:
/// - Tuple `(items, errors)` where `items` are `PackageItem`s found and `errors` are human-readable messages for partial failures
///
/// Details:
/// - Percent-encodes the query and calls the AUR RPC v5 search endpoint in a blocking task, maps up to 200 results into `PackageItem`s, and collects any network/parse failures as error strings.
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    let q = percent_encode(query.trim());
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search?by=name&arg={q}");

    let mut items: Vec<PackageItem> = Vec::new();

    let ret = tokio::task::spawn_blocking(move || crate::util::curl::curl_json(&aur_url)).await;
    let mut errors = Vec::new();
    match ret {
        Ok(Ok(resp)) => {
            if let Some(arr) = resp.get("results").and_then(|v| v.as_array()) {
                for pkg in arr.iter().take(200) {
                    let name = s(pkg, "Name");
                    let version = s(pkg, "Version");
                    let description = s(pkg, "Description");
                    let popularity = pkg.get("Popularity").and_then(serde_json::Value::as_f64);
                    if name.is_empty() {
                        continue;
                    }
                    // Extract OutOfDate timestamp (i64 or null)
                    let out_of_date = pkg
                        .get("OutOfDate")
                        .and_then(serde_json::Value::as_i64)
                        .and_then(|ts| u64::try_from(ts).ok())
                        .filter(|&ts| ts > 0);
                    // Extract Maintainer and determine if orphaned (empty or null means orphaned)
                    let maintainer = s(pkg, "Maintainer");
                    let orphaned = maintainer.is_empty();
                    items.push(PackageItem {
                        name,
                        version,
                        description,
                        source: Source::Aur,
                        popularity,
                        out_of_date,
                        orphaned,
                    });
                }
            }
        }
        Ok(Err(e)) => errors.push(format!("AUR search unavailable: {e}")),
        Err(e) => errors.push(format!("AUR search failed: {e}")),
    }

    (items, errors)
}

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
mod tests {
    #[tokio::test]
    #[allow(clippy::await_holding_lock, clippy::all)] // Shell variable syntax ${VAR:-default} in raw strings - false positive
    async fn search_returns_items_on_success_and_error_on_failure() {
        let _guard = crate::global_test_mutex_lock();
        // Shim PATH curl to return a small JSON for success call, then fail on a second invocation
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_curl_search_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("failed to create test root directory");
        let mut bin = root.clone();
        bin.push("bin");
        std::fs::create_dir_all(&bin).expect("failed to create test bin directory");
        let mut curl = bin.clone();
        curl.push("curl");
        // Shell variable syntax ${VAR:-default} - not a Rust format string
        #[allow(clippy::all, clippy::literal_string_with_formatting_args)]
        let script = r#"#!/bin/sh
set -e
state_dir="${PACSEA_FAKE_STATE_DIR:-.}"
if [ ! -f "$state_dir/pacsea_search_called" ]; then
  : > "$state_dir/pacsea_search_called"
  echo '{"results":[{"Name":"yay","Version":"12","Description":"AUR helper","Popularity":3.14,"OutOfDate":null,"Maintainer":"someuser"}]}'
else
  exit 22
fi
"#;
        std::fs::write(&curl, script.as_bytes()).expect("failed to write test curl script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl)
                .expect("failed to read test curl script metadata")
                .permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm)
                .expect("failed to set test curl script permissions");
        }
        let new_path = format!("{}:{old_path}", bin.to_string_lossy());
        unsafe {
            std::env::set_var("PATH", &new_path);
            std::env::set_var("PACSEA_FAKE_STATE_DIR", bin.to_string_lossy().to_string());
            // Enable curl PATH lookup override so our fake curl is used instead of /usr/bin/curl
            std::env::set_var("PACSEA_CURL_PATH", "1");
        }
        // Ensure PATH is set before executing commands
        std::thread::sleep(std::time::Duration::from_millis(10));

        let (items, errs) = super::fetch_all_with_errors("yay".into()).await;
        assert_eq!(
            items.len(),
            1,
            "Expected 1 item, got {} items. Errors: {:?}",
            items.len(),
            errs
        );
        assert!(errs.is_empty());
        // Verify status fields are parsed correctly
        assert_eq!(items[0].out_of_date, None);
        assert!(!items[0].orphaned);

        // Call again to exercise error path
        let (_items2, errs2) = super::fetch_all_with_errors("yay".into()).await;
        assert!(!errs2.is_empty());

        unsafe {
            std::env::set_var("PATH", &old_path);
            std::env::remove_var("PACSEA_CURL_PATH");
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
