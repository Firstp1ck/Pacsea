//! Official repository index adapters.

use std::collections::HashSet;
use std::path::Path;

use crate::index::{OfficialIndex, OfficialPkg};
use crate::state::{PackageItem, Source};

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
    let toolkit = toolkit_index(index);
    arch_toolkit::index::search_official(&toolkit, query, false)
        .into_iter()
        .map(|result| (package_item(result.package), result.fuzzy_score))
        .collect()
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
}
