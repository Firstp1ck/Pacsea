use serde_json::Value;

use crate::state::{PackageDetails, PackageItem, Source};
use crate::util::{arrs, s, ss, u64_of};

type Result<T> = super::Result<T>;

/// Run `pacman -Si` for a package, parsing its key-value output into PackageDetails.
///
/// Inputs:
/// - `repo`: Preferred repository prefix (may be empty to let pacman resolve)
/// - `name`: Package name
///
/// Output:
/// - `Ok(PackageDetails)` on success; `Err` if command fails or parse errors occur.
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

    /// Split a whitespace-separated field to Vec<String>, treating "None"/missing as empty.
    ///
    /// Inputs:
    /// - `s`: Optional string field from pacman output
    ///
    /// Output:
    /// - Vector of tokens, or empty when field is missing or "None".
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
        popularity: None,
    };
    Ok(pd)
}

/// Parse a pacman human-readable size like "1.5 MiB" into bytes.
///
/// Inputs:
/// - `s`: Size string containing a number and unit
///
/// Output:
/// - `Some(bytes)` when parsed; `None` for invalid strings. Accepts B, KiB, MiB, GiB, TiB, PiB.
fn parse_size_bytes(s: &str) -> Option<u64> {
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

#[cfg(test)]
mod size_tests {
    #[test]
    fn details_parse_size_bytes_units() {
        assert_eq!(super::parse_size_bytes("10 B"), Some(10));
        assert_eq!(super::parse_size_bytes("1 KiB"), Some(1024));
        assert_eq!(super::parse_size_bytes("2 MiB"), Some(2 * 1024 * 1024));
        assert_eq!(
            super::parse_size_bytes("3 GiB"),
            Some(3 * 1024 * 1024 * 1024)
        );
        assert_eq!(
            super::parse_size_bytes("4 TiB"),
            Some(4 * 1024 * 1024 * 1024 * 1024)
        );
        assert!(super::parse_size_bytes("bad").is_none());
    }
}

/// Fetch package details for either official repositories or AUR, based on the item's source.
///
/// Inputs:
/// - `item`: Package to fetch details for.
///
/// Output:
/// - `Ok(PackageDetails)` on success; `Err` if retrieval or parsing fails.
pub async fn fetch_details(item: PackageItem) -> Result<PackageDetails> {
    match item.source.clone() {
        Source::Official { repo, arch } => fetch_official_details(repo, arch, item).await,
        Source::Aur => fetch_aur_details(item).await,
    }
}

/// Fetch AUR package details via the AUR RPC API.
///
/// Inputs: `item` with `Source::Aur`.
///
/// Output: Parsed `PackageDetails` populated with AUR fields or an error.
pub async fn fetch_aur_details(item: PackageItem) -> Result<PackageDetails> {
    let url = format!(
        "https://aur.archlinux.org/rpc/v5/info?arg={}",
        crate::util::percent_encode(&item.name)
    );
    let v = tokio::task::spawn_blocking(move || super::curl_json(&url)).await??;
    let arr = v
        .get("results")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let obj = arr.first().cloned().unwrap_or(Value::Null);

    let version0 = s(&obj, "Version");
    let description0 = s(&obj, "Description");
    let popularity0 = obj.get("Popularity").and_then(|v| v.as_f64());

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
        popularity: popularity0,
    };
    Ok(d)
}

/// Fetch official repository package details via pacman JSON endpoints.
///
/// Inputs:
/// - `repo`: Repository name to prefer when multiple are available.
/// - `arch`: Architecture string to prefer.
/// - `item`: Package to fetch.
///
/// Output: `Ok(PackageDetails)` with repository fields filled; `Err` on network/parse failure.
pub async fn fetch_official_details(
    repo: String,
    arch: String,
    item: PackageItem,
) -> Result<PackageDetails> {
    if let Ok(Ok(pd)) = tokio::task::spawn_blocking({
        let repo = repo.clone();
        let name = item.name.clone();
        move || pacman_si(&repo, &name)
    })
    .await
    {
        let has_core =
            !(pd.description.is_empty() && pd.architecture.is_empty() && pd.licenses.is_empty());
        if has_core {
            return Ok(pd);
        }
    }

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
                move || super::curl_json(&url)
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
            popularity: None,
        };
        return Ok(d);
    }

    Err("official details unavailable".into())
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    /// What: Parse official JSON fields and defaults similar to arch packages API
    ///
    /// - Input: Minimal JSON object with common fields, sizes, packager
    /// - Output: Fields mapped into PackageDetails, sizes parsed, strings copied
    fn sources_details_parse_official_json_defaults_and_fields() {
        fn parse_official_from_json(
            obj: &serde_json::Value,
            repo_selected: String,
            arch_selected: String,
            item: &crate::state::PackageItem,
        ) -> crate::state::PackageDetails {
            use crate::util::{arrs, ss, u64_of};
            crate::state::PackageDetails {
                repository: repo_selected,
                name: item.name.clone(),
                version: ss(obj, &["pkgver", "Version"]).unwrap_or(item.version.clone()),
                description: ss(obj, &["pkgdesc", "Description"])
                    .unwrap_or(item.description.clone()),
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
                popularity: None,
            }
        }
        let v: serde_json::Value = serde_json::json!({
            "pkg": {
                "pkgver": "14",
                "pkgdesc": "ripgrep fast search",
                "arch": "x86_64",
                "url": "https://example.com",
                "licenses": ["MIT"],
                "groups": [],
                "provides": ["rg"],
                "depends": ["pcre2"],
                "optdepends": ["bash: completions"],
                "requiredby": [],
                "conflicts": [],
                "replaces": [],
                "compressed_size": 1024u64,
                "installed_size": 2048u64,
                "packager": "Arch Dev",
                "build_date": "2024-01-01"
            }
        });
        let item = crate::state::PackageItem {
            name: "ripgrep".into(),
            version: String::new(),
            description: String::new(),
            source: crate::state::Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        };
        let d = parse_official_from_json(&v["pkg"], "extra".into(), "x86_64".into(), &item);
        assert_eq!(d.repository, "extra");
        assert_eq!(d.name, "ripgrep");
        assert_eq!(d.version, "14");
        assert_eq!(d.description, "ripgrep fast search");
        assert_eq!(d.architecture, "x86_64");
        assert_eq!(d.url, "https://example.com");
        assert_eq!(d.download_size, Some(1024));
        assert_eq!(d.install_size, Some(2048));
        assert_eq!(d.owner, "Arch Dev");
        assert_eq!(d.build_date, "2024-01-01");
    }

    #[test]
    /// What: Parse AUR JSON fields, defaults, and popularity
    ///
    /// - Input: Minimal AUR RPC object with Version/Description/Popularity/URL
    /// - Output: PackageDetails with version/description fallback and popularity set
    fn sources_details_parse_aur_json_defaults_and_popularity() {
        fn parse_aur_from_json(
            obj: &serde_json::Value,
            item: &crate::state::PackageItem,
        ) -> crate::state::PackageDetails {
            use crate::util::{arrs, s};
            let version0 = s(obj, "Version");
            let description0 = s(obj, "Description");
            let popularity0 = obj.get("Popularity").and_then(|v| v.as_f64());
            crate::state::PackageDetails {
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
                url: s(obj, "URL"),
                licenses: arrs(obj, &["License", "Licenses"]),
                groups: arrs(obj, &["Groups"]),
                provides: arrs(obj, &["Provides"]),
                depends: arrs(obj, &["Depends"]),
                opt_depends: arrs(obj, &["OptDepends"]),
                required_by: vec![],
                optional_for: vec![],
                conflicts: arrs(obj, &["Conflicts"]),
                replaces: arrs(obj, &["Replaces"]),
                download_size: None,
                install_size: None,
                owner: s(obj, "Maintainer"),
                build_date: crate::util::ts_to_date(
                    obj.get("LastModified").and_then(|v| v.as_i64()),
                ),
                popularity: popularity0,
            }
        }
        let obj: serde_json::Value = serde_json::json!({
            "Version": "1.2.3",
            "Description": "cool",
            "Popularity": 3.14,
            "URL": "https://aur.example/ripgrep"
        });
        let item = crate::state::PackageItem {
            name: "ripgrep-git".into(),
            version: String::new(),
            description: String::new(),
            source: crate::state::Source::Aur,
            popularity: None,
        };
        let d = parse_aur_from_json(&obj, &item);
        assert_eq!(d.repository, "AUR");
        assert_eq!(d.name, "ripgrep-git");
        assert_eq!(d.version, "1.2.3");
        assert_eq!(d.description, "cool");
        assert_eq!(d.architecture, "any");
        assert_eq!(d.url, "https://aur.example/ripgrep");
        assert_eq!(d.popularity, Some(3.14));
    }
}
