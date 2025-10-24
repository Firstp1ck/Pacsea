use crate::state::{PackageItem, Source};

use super::idx;

/// Search the official index for packages whose names contain `query`.
///
/// Returns `PackageItem`s with fields populated from the index; enrichment is
/// not performed here. An empty or whitespace-only query returns an empty list.
pub fn search_official(query: &str) -> Vec<PackageItem> {
    let ql = query.trim().to_lowercase();
    if ql.is_empty() {
        return Vec::new();
    }
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        for p in &g.pkgs {
            let nl = p.name.to_lowercase();
            if nl.contains(&ql) {
                items.push(PackageItem {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    description: p.description.clone(),
                    source: Source::Official {
                        repo: p.repo.clone(),
                        arch: p.arch.clone(),
                    },
                    popularity: None,
                });
            }
        }
    }
    items
}

/// Return the entire official index as a list of `PackageItem`s.
pub fn all_official() -> Vec<PackageItem> {
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        items.reserve(g.pkgs.len());
        for p in &g.pkgs {
            items.push(PackageItem {
                name: p.name.clone(),
                version: p.version.clone(),
                description: p.description.clone(),
                source: Source::Official {
                    repo: p.repo.clone(),
                    arch: p.arch.clone(),
                },
                popularity: None,
            });
        }
    }
    items
}

/// Return the entire official index as a list of `PackageItem`s; if empty, try
/// to populate from disk and return the now-current view.
pub async fn all_official_or_fetch(path: &std::path::Path) -> Vec<PackageItem> {
    let items = all_official();
    if !items.is_empty() {
        return items;
    }
    super::persist::load_from_disk(path);
    all_official()
}

#[cfg(test)]
mod tests {
    #[test]
    fn search_official_empty_query_returns_empty() {
        if let Ok(mut g) = super::idx().write() {
            g.pkgs = vec![crate::index::OfficialPkg {
                name: "example".to_string(),
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
                version: "1.0".to_string(),
                description: "desc".to_string(),
            }];
        }
        let res = super::search_official("   ");
        assert!(res.is_empty());
    }

    #[test]
    fn search_official_is_case_insensitive_and_maps_fields() {
        if let Ok(mut g) = super::idx().write() {
            g.pkgs = vec![
                crate::index::OfficialPkg {
                    name: "PacSea".to_string(),
                    repo: "core".to_string(),
                    arch: "x86_64".to_string(),
                    version: "1.2.3".to_string(),
                    description: "awesome".to_string(),
                },
                crate::index::OfficialPkg {
                    name: "other".to_string(),
                    repo: "extra".to_string(),
                    arch: "any".to_string(),
                    version: "0.1".to_string(),
                    description: "meh".to_string(),
                },
            ];
        }
        let res = super::search_official("pac");
        assert_eq!(res.len(), 1);
        let item = &res[0];
        assert_eq!(item.name, "PacSea");
        assert_eq!(item.version, "1.2.3");
        assert_eq!(item.description, "awesome");
        match &item.source {
            crate::state::Source::Official { repo, arch } => {
                assert_eq!(repo, "core");
                assert_eq!(arch, "x86_64");
            }
            _ => panic!("expected Source::Official"),
        }
    }

    #[test]
    fn all_official_returns_all_items() {
        if let Ok(mut g) = super::idx().write() {
            g.pkgs = vec![
                crate::index::OfficialPkg {
                    name: "aa".to_string(),
                    repo: "core".to_string(),
                    arch: "x86_64".to_string(),
                    version: "1".to_string(),
                    description: "A".to_string(),
                },
                crate::index::OfficialPkg {
                    name: "zz".to_string(),
                    repo: "extra".to_string(),
                    arch: "any".to_string(),
                    version: "2".to_string(),
                    description: "Z".to_string(),
                },
            ];
        }
        let items = super::all_official();
        assert_eq!(items.len(), 2);
        let mut names: Vec<String> = items.into_iter().map(|p| p.name).collect();
        names.sort();
        assert_eq!(names, vec!["aa", "zz"]);
    }

    #[tokio::test]
    async fn all_official_or_fetch_reads_from_disk_when_empty() {
        use std::path::PathBuf;
        if let Ok(mut g) = super::idx().write() {
            g.pkgs.clear();
        }
        let mut path: PathBuf = std::env::temp_dir();
        path.push(format!(
            "pacsea_idx_query_fetch_{}_{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let idx_json = serde_json::json!({
            "pkgs": [
                {"name": "foo", "repo": "core", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(&path, serde_json::to_string(&idx_json).unwrap()).unwrap();
        let items = super::all_official_or_fetch(&path).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "foo");
        let _ = std::fs::remove_file(&path);
    }
}
