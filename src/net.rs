//! Network and system data retrieval for package metadata.
//!
//! This module fetches package details from two sources:
//! - AUR RPC v5 endpoints for AUR packages
//! - Arch Linux official package JSON pages and local `pacman -Si` for
//!   official repository packages
//!
//! It provides helpers that normalize data into `PackageDetails` and utilities
//! to search the AUR. For robustness and offline support, official details are
//! attempted via `pacman -Si` first, then fall back to web JSON.
use serde_json::Value;

use crate::state::{PackageDetails, PackageItem, Source};
use crate::util::{arrs, percent_encode, s, ss, u64_of};

/// Convenient `Result` alias for network/detail fetching operations.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Execute `curl` with strict flags and parse the response as JSON.
///
/// Uses `curl -sSLf` to be quiet, follow redirects, and fail on HTTP errors.
/// Returns a parsed `serde_json::Value` on success.
///
/// Errors if `curl` exits non-zero or the body is not valid JSON.
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

/// Execute `curl` with strict flags and return body as text.
///
/// Uses `curl -sSLf` to be quiet, follow redirects, and fail on HTTP errors.
fn curl_text(url: &str) -> Result<String> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed: {:?}", out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// Fetch PKGBUILD quickly from the most likely upstream location.
///
/// For AUR packages, fetch from AUR cgit raw URL. For official packages, try the
/// Arch GitLab packaging repo (`main` then `master` branch). Returns the text.
pub async fn fetch_pkgbuild_fast(item: &PackageItem) -> Result<String> {
    match &item.source {
        Source::Aur => {
            let url = format!(
                "https://aur.archlinux.org/cgit/aur.git/plain/PKGBUILD?h={}",
                percent_encode(&item.name)
            );
            // run in blocking thread since curl is blocking
            let res = tokio::task::spawn_blocking(move || curl_text(&url)).await??;
            Ok(res)
        }
        Source::Official { .. } => {
            // Try GitLab packaging repo raw file
            let name = item.name.clone();
            let url_main = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/main/PKGBUILD",
                percent_encode(&name)
            );
            if let Ok(Ok(txt)) = tokio::task::spawn_blocking({
                let u = url_main.clone();
                move || curl_text(&u)
            })
            .await
            {
                return Ok(txt);
            }
            let url_master = format!(
                "https://gitlab.archlinux.org/archlinux/packaging/packages/{}/-/raw/master/PKGBUILD",
                percent_encode(&name)
            );
            let txt = tokio::task::spawn_blocking(move || curl_text(&url_master)).await??;
            Ok(txt)
        }
    }
}

// Fallback: query pacman -Si and parse human output
/// Run `pacman -Si` for an official package and parse human-readable output.
///
/// The `repo` may be empty; when provided, the `repo/name` form is used. The
/// parser handles multi-line fields and preserves line breaks for
/// "Optional Deps" while collapsing continuation lines for other keys.
///
/// If description or architecture are missing from `pacman -Si`, the function
/// attempts to fill them from the in-memory official index.
fn pacman_si(repo: &str, name: &str) -> Result<PackageDetails> {
    let spec = if repo.is_empty() {
        name.to_string()
    } else {
        format!("{repo}/{name}")
    };
    let out = std::process::Command::new("pacman")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .args(["-Si", &spec])
        .output()?;
    if !out.status.success() {
        return Err(format!("pacman -Si failed: {:?}", out.status).into());
    }
    let text = String::from_utf8(out.stdout)?;

    // Parse Key: Value pairs, with continuation lines; for Optional Deps, preserve line breaks
    let mut map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    let mut last_key: Option<String> = None;
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().to_string();
            map.insert(key.clone(), val);
            last_key = Some(key);
        } else if line.starts_with(' ')
            && let Some(k) = &last_key
        {
            let e = map.entry(k.clone()).or_default();
            if k == "Optional Deps" {
                e.push('\n');
                e.push_str(line.trim());
            } else {
                if !e.ends_with(' ') {
                    e.push(' ');
                }
                e.push_str(line.trim());
            }
        }
    }

    fn split_ws_or_none(s: Option<&String>) -> Vec<String> {
        match s {
            Some(v) if v != "None" => v.split_whitespace().map(|x| x.to_string()).collect(),
            _ => Vec::new(),
        }
    }

    let licenses = split_ws_or_none(map.get("Licenses").or_else(|| map.get("License")));
    let groups = split_ws_or_none(map.get("Groups"));
    let provides = split_ws_or_none(map.get("Provides"));
    let depends = split_ws_or_none(map.get("Depends On"));
    let opt_depends = map
        .get("Optional Deps")
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_once(':').map(|(pkg, _)| pkg.trim().to_string()))
                .filter(|x| !x.is_empty() && x != "None")
                .collect()
        })
        .unwrap_or_default();
    let required_by = split_ws_or_none(map.get("Required By"));
    let optional_for = split_ws_or_none(map.get("Optional For"));
    let conflicts = split_ws_or_none(map.get("Conflicts With"));
    let replaces = split_ws_or_none(map.get("Replaces"));

    let mut description = map.get("Description").cloned().unwrap_or_default();
    let mut architecture = map.get("Architecture").cloned().unwrap_or_default();

    // Fill from official index if missing
    if description.is_empty() || architecture.is_empty() {
        let mut from_idx = None;
        for it in crate::index::search_official(name) {
            if it.name.eq_ignore_ascii_case(name) {
                from_idx = Some(it);
                break;
            }
        }
        if let Some(it) = from_idx {
            if description.is_empty() {
                description = it.description;
            }
            if architecture.is_empty()
                && let Source::Official { arch, .. } = it.source
            {
                architecture = arch;
            }
        }
    }

    let download_size = map.get("Download Size").and_then(|s| parse_size_bytes(s));
    let install_size = map.get("Installed Size").and_then(|s| parse_size_bytes(s));

    let pd = PackageDetails {
        repository: map
            .get("Repository")
            .cloned()
            .unwrap_or_else(|| repo.to_string()),
        name: map.get("Name").cloned().unwrap_or_else(|| name.to_string()),
        version: map.get("Version").cloned().unwrap_or_default(),
        description,
        architecture,
        url: map.get("URL").cloned().unwrap_or_default(),
        licenses,
        groups,
        provides,
        depends,
        opt_depends,
        required_by,
        optional_for,
        conflicts,
        replaces,
        download_size,
        install_size,
        owner: map.get("Packager").cloned().unwrap_or_default(),
        build_date: map.get("Build Date").cloned().unwrap_or_default(),
    };
    Ok(pd)
}

/// Parse a human-readable size string like "3.44 MiB" into bytes.
///
/// Supports units: B, KiB, MiB, GiB, TiB. Returns `None` if parsing fails.
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

/// Fetch details for a `PackageItem` based on its `Source`.
///
/// Delegates to `fetch_official_details` or `fetch_aur_details` accordingly.
pub async fn fetch_details(item: PackageItem) -> Result<PackageDetails> {
    match item.source.clone() {
        Source::Official { repo, arch } => fetch_official_details(repo, arch, item).await,
        Source::Aur => fetch_aur_details(item).await,
    }
}

/// Fetch details for an AUR package using AUR RPC v5 `info` endpoint.
///
/// Populates fields from the JSON response, falling back to values already
/// present on the `PackageItem` for missing description/version.
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

/// Fetch details for an official package.
///
/// Strategy:
/// 1) Try `pacman -Si` locally (via `spawn_blocking`) for fast/offline data.
///    If it yields core fields (description, architecture, or licenses), return
///    its result.
/// 2) Otherwise, query `https://archlinux.org/packages/<repo>/<arch>/<name>/json/`.
///    If `repo` or `arch` are unknown, try sensible candidates (core/extra and
///    x86_64/any) until one succeeds.
pub async fn fetch_official_details(
    repo: String,
    arch: String,
    item: PackageItem,
) -> Result<PackageDetails> {
    // Prefer local pacman -Si (fast, offline)
    if let Ok(Ok(pd)) = tokio::task::spawn_blocking({
        let repo = repo.clone();
        let name = item.name.clone();
        move || pacman_si(&repo, &name)
    })
    .await
    {
        // If pacman provided basic fields, return; else try web
        let has_core =
            !(pd.description.is_empty() && pd.architecture.is_empty() && pd.licenses.is_empty());
        if has_core {
            return Ok(pd);
        }
    }

    // Fall back to web JSON
    let arch_candidates: Vec<String> = if arch.trim().is_empty() {
        vec!["x86_64".to_string(), "any".to_string()]
    } else if arch.to_lowercase() == "any" {
        vec!["any".to_string()]
    } else {
        vec![arch.clone(), "any".to_string()]
    };
    let repo_candidates: Vec<String> = if repo.trim().is_empty() {
        vec!["core".to_string(), "extra".to_string()]
    } else {
        vec![repo.clone()]
    };
    let mut v: Option<Value> = None;
    let mut repo_selected = repo.clone();
    let mut arch_selected = arch.clone();
    'outer: for r in &repo_candidates {
        for a in &arch_candidates {
            let url = format!(
                "https://archlinux.org/packages/{}/{}/{}/json/",
                r.to_lowercase(),
                a,
                item.name
            );
            if let Ok(Ok(val)) = tokio::task::spawn_blocking({
                let url = url.clone();
                move || curl_json(&url)
            })
            .await
            {
                v = Some(val);
                repo_selected = r.clone();
                arch_selected = a.clone();
                break 'outer;
            }
        }
    }

    if let Some(v) = v {
        let obj = v.get("pkg").unwrap_or(&v);
        let d = PackageDetails {
            repository: repo_selected,
            name: item.name.clone(),
            version: ss(obj, &["pkgver", "Version"]).unwrap_or(item.version),
            description: ss(obj, &["pkgdesc", "Description"]).unwrap_or(item.description),
            architecture: ss(obj, &["arch", "Architecture"]).unwrap_or(arch_selected),
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

    Err("official details unavailable".into())
}

/// Fetch AUR search results for a name query and report any errors.
///
/// Returns a tuple of `(items, errors)` where `items` are `PackageItem`s from
/// AUR whose names match the query and `errors` contains human-readable error
/// strings for failed network operations. Official results are not included and
/// are expected to be provided by the caller.
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
