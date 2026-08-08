//! Official repository index adapters.

use std::collections::HashSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::{OnceLock, RwLock};

use crate::index::{OfficialIndex, OfficialPkg};
use crate::state::{PackageItem, Source};

/// Cached toolkit index used by interactive non-fuzzy searches.
static SEARCH_INDEX_CACHE: OnceLock<RwLock<Option<CachedSearchIndex>>> = OnceLock::new();

/// Cached conversion paired with a content fingerprint of the Pacsea index.
struct CachedSearchIndex {
    /// Fingerprint of every package field represented in `index`.
    fingerprint: u64,
    /// Converted toolkit index reused across unchanged searches.
    index: arch_toolkit::OfficialIndex,
}

#[cfg(test)]
/// Number of Pacsea-to-toolkit index conversions performed in tests.
static SEARCH_INDEX_CONVERSIONS: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// What: Detect enabled repositories, merge Pacsea additions, and fetch through arch-toolkit.
///
/// Inputs:
/// - None; reads pacman and Pacsea repository configuration.
///
/// Output:
/// - Pacsea official package rows or an actionable error.
///
/// Details:
/// - Repository order is preserved and duplicates are removed case-insensitively before toolkit
///   runs locale-stable `pacman -Sl` queries.
pub async fn fetch_official_packages() -> Result<Vec<OfficialPkg>, String> {
    let mut repos = arch_toolkit::index::detect_enabled_repos();
    let mut seen: HashSet<String> = repos.iter().map(|repo| repo.to_lowercase()).collect();
    for repo in crate::logic::repos::repos_conf_repo_names_for_index_sl(&seen) {
        if seen.insert(repo.to_lowercase()) {
            repos.push(repo);
        }
    }
    arch_toolkit::index::fetch_official_index_for_repos_async(repos)
        .await
        .map(|index| index.pkgs.into_iter().map(official_package).collect())
        .map_err(|error| {
            format!(
                "official package index refresh failed: {error}; verify pacman repositories and retry"
            )
        })
}

/// What: Search Pacsea's in-memory official index through arch-toolkit.
///
/// Inputs:
/// - `index`: Pacsea index snapshot.
/// - `query`: Non-empty substring query.
///
/// Output:
/// - Pacsea package rows with toolkit query scores.
///
/// Details:
/// - Pacsea keeps fuzzy ranking separately because the toolkit fuzzy feature is intentionally disabled.
pub fn search(index: &OfficialIndex, query: &str) -> Vec<(PackageItem, Option<i64>)> {
    let fingerprint = index_fingerprint(index);
    let cache = SEARCH_INDEX_CACHE.get_or_init(|| RwLock::new(None));
    if let Ok(guard) = cache.read()
        && let Some(cached) = guard.as_ref()
        && cached.fingerprint == fingerprint
    {
        return search_toolkit_index(&cached.index, query);
    }

    let converted = toolkit_index(index);
    match cache.write() {
        Ok(mut guard) => {
            *guard = Some(CachedSearchIndex {
                fingerprint,
                index: converted,
            });
            guard.as_ref().map_or_else(Vec::new, |cached| {
                search_toolkit_index(&cached.index, query)
            })
        }
        Err(_) => search_toolkit_index(&converted, query),
    }
}

/// What: Search one already-converted toolkit index and map results into Pacsea state.
///
/// Inputs:
/// - `index`: Cached or temporary toolkit index.
/// - `query`: Non-empty substring query.
///
/// Output:
/// - Pacsea package rows with toolkit query scores.
///
/// Details:
/// - Keeps conversion/cache policy separate from result mapping.
fn search_toolkit_index(
    index: &arch_toolkit::OfficialIndex,
    query: &str,
) -> Vec<(PackageItem, Option<i64>)> {
    arch_toolkit::index::search_official(index, query, false)
        .into_iter()
        .map(|result| (package_item(result.package), result.fuzzy_score))
        .collect()
}

/// What: Fingerprint the Pacsea index content represented in the toolkit search cache.
///
/// Inputs:
/// - `index`: Pacsea official index snapshot.
///
/// Output:
/// - Process-local content fingerprint.
///
/// Details:
/// - Hashing remains O(n) but allocation-free, avoiding full package cloning and lookup-map
///   rebuilding on every keystroke while detecting metadata-only refreshes.
fn index_fingerprint(index: &OfficialIndex) -> u64 {
    let mut hasher = DefaultHasher::new();
    index.pkgs.len().hash(&mut hasher);
    for package in &index.pkgs {
        package.name.hash(&mut hasher);
        package.repo.hash(&mut hasher);
        package.arch.hash(&mut hasher);
        package.version.hash(&mut hasher);
        package.description.hash(&mut hasher);
    }
    hasher.finish()
}

/// What: Load a Pacsea-compatible official index through arch-toolkit persistence.
///
/// Inputs:
/// - `path`: Existing JSON index path.
///
/// Output:
/// - Converted Pacsea index or an actionable error.
///
/// Details:
/// - The derived lowercase name index is rebuilt on both toolkit and Pacsea sides.
pub fn load(path: &Path) -> Result<OfficialIndex, String> {
    arch_toolkit::index::load_from_disk(path)
        .map(pacsea_index)
        .map_err(|error| format!("failed to load official package index: {error}"))
}

/// What: Persist a Pacsea official index through arch-toolkit.
///
/// Inputs:
/// - `index`: Pacsea index snapshot.
/// - `path`: Destination path.
///
/// Output:
/// - Success or an actionable error.
///
/// Details:
/// - Serialization preserves the existing `pkgs` schema and skips the derived name map.
pub fn save(index: &OfficialIndex, path: &Path) -> Result<(), String> {
    arch_toolkit::index::save_to_disk(&toolkit_index(index), path)
        .map_err(|error| format!("failed to save official package index: {error}"))
}

/// What: Convert a Pacsea index snapshot into toolkit state.
///
/// Inputs:
/// - `index`: Pacsea index.
///
/// Output:
/// - Toolkit index with rebuilt name lookup.
///
/// Details:
/// - Package order and duplicate repository/name rows are preserved.
fn toolkit_index(index: &OfficialIndex) -> arch_toolkit::OfficialIndex {
    #[cfg(test)]
    SEARCH_INDEX_CONVERSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut converted = arch_toolkit::OfficialIndex {
        pkgs: index.pkgs.iter().cloned().map(toolkit_package).collect(),
        name_to_idx: std::collections::HashMap::new(),
    };
    converted.rebuild_name_index();
    converted
}

/// What: Convert toolkit index state into Pacsea state.
///
/// Inputs:
/// - `index`: Toolkit index.
///
/// Output:
/// - Pacsea index with rebuilt name lookup.
///
/// Details:
/// - Used at persistence boundaries so UI consumers remain unchanged.
fn pacsea_index(index: arch_toolkit::OfficialIndex) -> OfficialIndex {
    let mut converted = OfficialIndex {
        pkgs: index.pkgs.into_iter().map(official_package).collect(),
        name_to_idx: std::collections::HashMap::new(),
    };
    converted.rebuild_name_index();
    converted
}

/// What: Convert one Pacsea official package into toolkit state.
///
/// Inputs:
/// - `package`: Pacsea package row.
///
/// Output:
/// - Equivalent toolkit package.
///
/// Details:
/// - All persisted fields map one-to-one.
fn toolkit_package(package: OfficialPkg) -> arch_toolkit::OfficialPackage {
    arch_toolkit::OfficialPackage {
        name: package.name,
        repo: package.repo,
        arch: package.arch,
        version: package.version,
        description: package.description,
    }
}

/// What: Convert one toolkit official package into Pacsea state.
///
/// Inputs:
/// - `package`: Toolkit package row.
///
/// Output:
/// - Equivalent Pacsea package.
///
/// Details:
/// - All persisted fields map one-to-one.
fn official_package(package: arch_toolkit::OfficialPackage) -> OfficialPkg {
    OfficialPkg {
        name: package.name,
        repo: package.repo,
        arch: package.arch,
        version: package.version,
        description: package.description,
    }
}

/// What: Convert one toolkit official package into a Pacsea UI row.
///
/// Inputs:
/// - `package`: Toolkit package row.
///
/// Output:
/// - Pacsea official package item.
///
/// Details:
/// - AUR-only popularity and status fields remain unset.
fn package_item(package: arch_toolkit::OfficialPackage) -> PackageItem {
    PackageItem {
        name: package.name,
        version: package.version,
        description: package.description,
        source: Source::Official {
            repo: package.repo,
            arch: package.arch,
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

#[cfg(test)]
mod tests {
    use crate::index::{OfficialIndex, OfficialPkg};

    /// What: Verify toolkit persistence conversion preserves Pacsea's JSON schema.
    ///
    /// Inputs:
    /// - One Pacsea index row.
    ///
    /// Output:
    /// - Round-tripped index with rebuilt lowercase lookup.
    ///
    /// Details:
    /// - Uses only in-memory conversion and does not inspect host repositories.
    #[test]
    fn index_conversion_round_trip_preserves_schema() {
        let input = OfficialIndex {
            pkgs: vec![OfficialPkg {
                name: "ripgrep".to_string(),
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
                version: "14".to_string(),
                description: "search".to_string(),
            }],
            name_to_idx: std::collections::HashMap::new(),
        };
        let output = super::pacsea_index(super::toolkit_index(&input));
        assert_eq!(output.pkgs[0].name, "ripgrep");
        assert_eq!(output.name_to_idx.get("ripgrep"), Some(&0));
    }

    /// What: Verify repeated searches reuse the converted toolkit index until content changes.
    ///
    /// Inputs:
    /// - One Pacsea index searched twice, then searched after a metadata change.
    ///
    /// Output:
    /// - One conversion for unchanged searches and one additional conversion after mutation.
    ///
    /// Details:
    /// - Protects the TUI search loop from cloning the full index on every keystroke.
    #[test]
    fn search_cache_reuses_conversion_until_content_changes() {
        let _guard = crate::global_test_mutex_lock();
        if let Ok(mut cache) = super::SEARCH_INDEX_CACHE
            .get_or_init(|| std::sync::RwLock::new(None))
            .write()
        {
            *cache = None;
        }
        super::SEARCH_INDEX_CONVERSIONS.store(0, std::sync::atomic::Ordering::Relaxed);
        let mut index = OfficialIndex {
            pkgs: vec![OfficialPkg {
                name: "ripgrep".to_string(),
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
                version: "14".to_string(),
                description: "search".to_string(),
            }],
            name_to_idx: std::collections::HashMap::new(),
        };

        assert_eq!(super::search(&index, "rip").len(), 1);
        assert_eq!(super::search(&index, "grep").len(), 1);
        assert_eq!(
            super::SEARCH_INDEX_CONVERSIONS.load(std::sync::atomic::Ordering::Relaxed),
            1
        );

        index.pkgs[0].description = "updated search tool".to_string();
        assert_eq!(super::search(&index, "rip").len(), 1);
        assert_eq!(
            super::SEARCH_INDEX_CONVERSIONS.load(std::sync::atomic::Ordering::Relaxed),
            2
        );
    }
}
