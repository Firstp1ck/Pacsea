use crate::state::{PackageItem, Source};
use crate::util::{percent_encode, s};

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
