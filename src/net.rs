use serde_json::Value;

use crate::state::{PackageDetails, PackageItem, Source};
use crate::util::{arrs, percent_encode, s, ss, u64_of};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn curl_json(url: &str) -> Result<Value> {
    // Use curl to fetch a URL and parse JSON
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed: {:?}", out.status).into());
    }
    let body = String::from_utf8(out.stdout)?;
    let v: Value = serde_json::from_str(&body)?;
    Ok(v)
}

// Fallback: query pacman -Si and parse human output
fn pacman_si(repo: &str, name: &str) -> Result<PackageDetails> {
    let spec = if repo.is_empty() {
        name.to_string()
    } else {
        format!("{repo}/{name}")
    };
    let out = std::process::Command::new("pacman")
        .args(["-Si", &spec])
        .output()?;
    if !out.status.success() {
        return Err(format!("pacman -Si failed: {:?}", out.status).into());
    }
    let text = String::from_utf8(out.stdout)?;

    // Parse Key : Value pairs, with continuation lines (leading spaces)
    let mut map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut last_key: Option<String> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(" : ") {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            map.insert(key.clone(), val);
            last_key = Some(key);
        } else if line.starts_with(' ')
            && let Some(k) = &last_key
        {
            let e = map.entry(k.clone()).or_default();
            if !e.ends_with(' ') {
                e.push(' ');
            }
            e.push_str(line.trim());
        }
    }

    let licenses = map
        .get("Licenses")
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let groups = map
        .get("Groups")
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let provides = map
        .get("Provides")
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let depends = map
        .get("Depends On")
        .map(|s| s.split_whitespace().map(|x| x.to_string()).collect())
        .unwrap_or_default();
    let opt_depends = map
        .get("Optional Deps")
        .map(|s| {
            s.split("  ")
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let required_by = map
        .get("Required By")
        .map(|s| {
            if s == "None" {
                Vec::new()
            } else {
                s.split_whitespace().map(|x| x.to_string()).collect()
            }
        })
        .unwrap_or_default();
    let conflicts = map
        .get("Conflicts With")
        .map(|s| {
            if s == "None" {
                Vec::new()
            } else {
                s.split_whitespace().map(|x| x.to_string()).collect()
            }
        })
        .unwrap_or_default();
    let replaces = map
        .get("Replaces")
        .map(|s| {
            if s == "None" {
                Vec::new()
            } else {
                s.split_whitespace().map(|x| x.to_string()).collect()
            }
        })
        .unwrap_or_default();

    let download_size = map.get("Download Size").and_then(|s| parse_size_bytes(s));
    let install_size = map.get("Installed Size").and_then(|s| parse_size_bytes(s));

    let pd = PackageDetails {
        repository: map
            .get("Repository")
            .cloned()
            .unwrap_or_else(|| repo.to_string()),
        name: map.get("Name").cloned().unwrap_or_else(|| name.to_string()),
        version: map.get("Version").cloned().unwrap_or_default(),
        description: map.get("Description").cloned().unwrap_or_default(),
        architecture: map.get("Architecture").cloned().unwrap_or_default(),
        url: map.get("URL").cloned().unwrap_or_default(),
        licenses,
        groups,
        provides,
        depends,
        opt_depends,
        required_by,
        optional_for: Vec::new(),
        conflicts,
        replaces,
        download_size,
        install_size,
        owner: map.get("Packager").cloned().unwrap_or_default(),
        build_date: map.get("Build Date").cloned().unwrap_or_default(),
    };
    Ok(pd)
}

fn parse_size_bytes(s: &str) -> Option<u64> {
    // Examples: "3.44 MiB", "123.0 KiB"
    let mut it = s.split_whitespace();
    let num = it.next()?.parse::<f64>().ok()?;
    let unit = it.next().unwrap_or("");
    let mult = match unit {
        "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((num * mult) as u64)
}

pub async fn fetch_details(item: PackageItem) -> Result<PackageDetails> {
    match item.source.clone() {
        Source::Official { repo, arch } => fetch_official_details(repo, arch, item).await,
        Source::Aur => fetch_aur_details(item).await,
    }
}

pub async fn fetch_aur_details(item: PackageItem) -> Result<PackageDetails> {
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/info?arg={}",
        percent_encode(&item.name)
    );
    let v = tokio::task::spawn_blocking(move || curl_json(&url)).await??;
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let obj = arr.first().cloned().unwrap_or(Value::Null);

    let version0 = s(&obj, "Version");
    let description0 = s(&obj, "Description");

    let d = PackageDetails {
        repository: "AUR".into(),
        name: item.name.clone(),
        version: if version0.is_empty() {
            item.version.clone()
        } else {
            version0
        },
        description: if description0.is_empty() {
            item.description.clone()
        } else {
            description0
        },
        architecture: "any".into(),
        url: s(&obj, "URL"),
        licenses: arrs(&obj, &["License", "Licenses"]),
        groups: arrs(&obj, &["Groups"]),
        provides: arrs(&obj, &["Provides"]),
        depends: arrs(&obj, &["Depends"]),
        opt_depends: arrs(&obj, &["OptDepends"]),
        required_by: vec![],
        optional_for: vec![],
        conflicts: arrs(&obj, &["Conflicts"]),
        replaces: arrs(&obj, &["Replaces"]),
        download_size: None,
        install_size: None,
        owner: s(&obj, "Maintainer"),
        build_date: crate::util::ts_to_date(obj.get("LastModified").and_then(|v| v.as_i64())),
    };
    Ok(d)
}

pub async fn fetch_official_details(
    repo: String,
    arch: String,
    item: PackageItem,
) -> Result<PackageDetails> {
    let arch_candidates = if arch.to_lowercase() == "any" {
        vec![arch.clone()]
    } else {
        vec![arch.clone(), "any".to_string()]
    };
    let mut v: Option<Value> = None;
    for a in arch_candidates {
        let url = format!(
            "https://archlinux.org/packages/{}/{}/{}/json/",
            repo.to_lowercase(),
            a,
            item.name
        );
        if let Ok(Ok(val)) = tokio::task::spawn_blocking(move || curl_json(&url)).await {
            v = Some(val);
            break;
        }
    }

    if let Some(v) = v {
        let obj = v.get("pkg").unwrap_or(&v);

        let d = PackageDetails {
            repository: repo,
            name: item.name.clone(),
            version: ss(obj, &["pkgver", "Version"]).unwrap_or(item.version),
            description: ss(obj, &["pkgdesc", "Description"]).unwrap_or(item.description),
            architecture: ss(obj, &["arch", "Architecture"]).unwrap_or(arch),
            url: ss(obj, &["url", "URL"]).unwrap_or_default(),
            licenses: arrs(obj, &["licenses", "Licenses"]),
            groups: arrs(obj, &["groups", "Groups"]),
            provides: arrs(obj, &["provides", "Provides"]),
            depends: arrs(obj, &["depends", "Depends"]),
            opt_depends: arrs(obj, &["optdepends", "OptDepends"]),
            required_by: arrs(obj, &["requiredby", "RequiredBy"]),
            optional_for: vec![],
            conflicts: arrs(obj, &["conflicts", "Conflicts"]),
            replaces: arrs(obj, &["replaces", "Replaces"]),
            download_size: u64_of(obj, &["compressed_size", "CompressedSize"]),
            install_size: u64_of(obj, &["installed_size", "InstalledSize"]),
            owner: ss(obj, &["packager", "Packager"]).unwrap_or_default(),
            build_date: ss(obj, &["build_date", "BuildDate"]).unwrap_or_default(),
        };
        return Ok(d);
    }

    // Fallback to pacman -Si if curl failed for all candidates
    match tokio::task::spawn_blocking({
        let repo = repo.clone();
        let name = item.name.clone();
        move || pacman_si(&repo, &name)
    })
    .await
    {
        Ok(Ok(pd)) => Ok(pd),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("fallback spawn failed: {e}").into()),
    }
}

pub async fn fetch_all_with_errors(query: String) -> (Vec<PackageItem>, Vec<String>) {
    let q = percent_encode(query.trim());
    let aur_url = format!("https://aur.archlinux.org/rpc/v5/search?by=name&arg={q}");

    // Official results are resolved by caller
    let mut items: Vec<PackageItem> = Vec::new();

    let ret = tokio::task::spawn_blocking(move || curl_json(&aur_url)).await;
    let mut errors = Vec::new();
    match ret {
        Ok(Ok(resp)) => {
            if let Some(arr) = resp.get("results").and_then(|v| v.as_array()) {
                for pkg in arr.iter().take(200) {
                    let name = s(pkg, "Name");
                    let version = s(pkg, "Version");
                    let description = s(pkg, "Description");
                    if name.is_empty() {
                        continue;
                    }
                    items.push(PackageItem {
                        name,
                        version,
                        description,
                        source: Source::Aur,
                    });
                }
            }
        }
        Ok(Err(e)) => errors.push(format!("AUR search unavailable: {e}")),
        Err(e) => errors.push(format!("AUR search failed: {e}")),
    }

    (items, errors)
}
