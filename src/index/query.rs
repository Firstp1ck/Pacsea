use crate::state::{PackageItem, Source};

use super::idx;

/// What: Search the official index for packages whose names contain `query`.
///
/// Inputs:
/// - `query`: Raw query string
///
/// Output:
/// - Vector of `PackageItem`s populated from the index; enrichment is not performed here.
///   An empty or whitespace-only query returns an empty list.
///
/// Details:
/// - Performs a case-insensitive substring match on package names and clones matching entries.
#[must_use]
pub fn search_official(query: &str) -> Vec<PackageItem> {
    let ql = query.trim().to_lowercase();
    if ql.is_empty() {
        return Vec::new();
    }
    let mut items = Vec::new();
    if let Some(g) = idx().read().ok() {
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

/// What: Return the entire official index as a list of `PackageItem`s.
///
/// Inputs:
/// - None
///
/// Output:
/// - Vector of all official items mapped to `PackageItem`.
///
/// Details:
/// - Clones data from the shared index under a read lock and omits popularity data.
#[must_use]
pub fn all_official() -> Vec<PackageItem> {
    let mut items = Vec::new();
    if let Some(g) = idx().read().ok() {
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

/// What: Return the entire official list; if empty, try to populate from disk and return it.
///
/// Inputs:
/// - `path`: Path to on-disk JSON index to load as a fallback
///
/// Output:
/// - Vector of `PackageItem`s representing the current in-memory (or loaded) index.
///
/// Details:
/// - Loads from disk only when the in-memory list is empty to avoid redundant IO.
pub fn all_official_or_fetch(path: &std::path::Path) -> Vec<PackageItem> {
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
    /// What: Return empty vector when the query is blank.
    ///
    /// Inputs:
    /// - Seed index with an entry and call `search_official` using whitespace-only query.
    ///
    /// Output:
    /// - Empty result set.
    ///
    /// Details:
    /// - Confirms whitespace trimming logic works.
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
    /// What: Perform case-insensitive matching and field mapping.
    ///
    /// Inputs:
    /// - Seed index with uppercase/lowercase packages and query with lowercase substring.
    ///
    /// Output:
    /// - Single result matching expected fields.
    ///
    /// Details:
    /// - Verifies `Source::Official` metadata is preserved in mapped items.
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
            crate::state::Source::Aur => panic!("expected Source::Official"),
        }
    }

    #[test]
    /// What: Populate all official packages regardless of query.
    ///
    /// Inputs:
    /// - Seed index with two packages and call `all_official`.
    ///
    /// Output:
    /// - Vector containing both packages.
    ///
    /// Details:
    /// - Checks ordering is not enforced but the returned names set matches expectation.
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
    /// What: Load packages from disk when the in-memory index is empty.
    ///
    /// Inputs:
    /// - Clear the index and provide a temp JSON file with one package.
    ///
    /// Output:
    /// - Vector containing the package from disk.
    ///
    /// Details:
    /// - Ensures fallback to `persist::load_from_disk` is exercised.
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
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let idx_json = serde_json::json!({
            "pkgs": [
                {"name": "foo", "repo": "core", "arch": "x86_64", "version": "1", "description": ""}
            ]
        });
        std::fs::write(
            &path,
            serde_json::to_string(&idx_json).expect("failed to serialize index JSON"),
        )
        .expect("failed to write index JSON file");
        let items = super::all_official_or_fetch(&path);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "foo");
        let _ = std::fs::remove_file(&path);
    }
}
