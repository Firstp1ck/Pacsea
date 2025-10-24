use crate::state::{PackageItem, Source};
use crate::util::{percent_encode, s};

/// Fetch search results from AUR and return items along with any error messages.
///
/// Inputs:
/// - `query`: Raw query string to search.
///
/// Output:
/// - Tuple `(items, errors)` where `items` are `PackageItem`s found and `errors` are human-readable
///   messages for partial failures.
pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    let q = percent_encode(query.trim());
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search?by=name&arg={q}");

    let mut items: Vec<PackageItem> = Vec::new();

    let ret = tokio::task::spawn_blocking(move || super::curl_json(&aur_url)).await;
    let mut errors = Vec::new();
    match ret {
        Ok(Ok(resp)) => {
            if let Some(arr) = resp.get("results").and_then(|v| v.as_array()) {
                for pkg in arr.iter().take(200) {
                    let name = s(pkg, "Name");
                    let version = s(pkg, "Version");
                    let description = s(pkg, "Description");
                    let popularity = pkg.get("Popularity").and_then(|v| v.as_f64());
                    if name.is_empty() {
                        continue;
                    }
                    items.push(PackageItem {
                        name,
                        version,
                        description,
                        source: Source::Aur,
                        popularity,
                    });
                }
            }
        }
        Ok(Err(e)) => errors.push(format!("AUR search unavailable: {e}")),
        Err(e) => errors.push(format!("AUR search failed: {e}")),
    }

    (items, errors)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn search_returns_items_on_success_and_error_on_failure() {
        let _guard = crate::sources::test_mutex().lock().unwrap();
        // Shim PATH curl to return a small JSON for success call, then fail on a second invocation
        let old_path = std::env::var("PATH").unwrap_or_default();
        let mut root = std::env::temp_dir();
        root.push(format!(
            "pacsea_fake_curl_search_{}_{}",
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
        let script = r##"#!/usr/bin/env bash
set -e
state_dir="${PACSEA_FAKE_STATE_DIR:-.}"
if [[ ! -f "$state_dir/pacsea_search_called" ]]; then
  : > "$state_dir/pacsea_search_called"
  echo '{"results":[{"Name":"yay","Version":"12","Description":"AUR helper","Popularity":3.14}]}'
else
  exit 22
fi
"##;
        std::fs::write(&curl, script.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perm = std::fs::metadata(&curl).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&curl, perm).unwrap();
        }
        let new_path = format!("{}:{}", bin.to_string_lossy(), old_path);
        unsafe {
            std::env::set_var("PATH", &new_path);
            std::env::set_var("PACSEA_FAKE_STATE_DIR", bin.to_string_lossy().to_string());
        }

        let (items, errs) = super::fetch_all_with_errors("yay".into()).await;
        assert_eq!(items.len(), 1);
        assert!(errs.is_empty());

        // Call again to exercise error path
        let (_items2, errs2) = super::fetch_all_with_errors("yay".into()).await;
        assert!(!errs2.is_empty());

        unsafe { std::env::set_var("PATH", &old_path) };
        let _ = std::fs::remove_dir_all(&root);
    }
}
