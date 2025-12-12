//! Official package index management, persistence, and enrichment.
//!
//! Split into submodules for maintainability. Public API is re-exported
//! to remain compatible with previous `crate::index` consumers.

use std::collections::{HashMap, HashSet};
use std::sync::{OnceLock, RwLock};

/// What: Represent the full collection of official packages maintained in memory.
///
/// Inputs:
/// - Populated by fetch and enrichment routines before being persisted or queried.
///
/// Output:
/// - Exposed through API helpers that clone or iterate the package list.
///
/// Details:
/// - Serializable via Serde to allow saving and restoring across sessions.
/// - The `name_to_idx` field is derived from `pkgs` and skipped during serialization.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct OfficialIndex {
    /// All known official packages in the process-wide index.
    pub pkgs: Vec<OfficialPkg>,
    /// Index mapping lowercase package names to their position in `pkgs` for O(1) lookups.
    /// Skipped during serialization; rebuilt after deserialization via `rebuild_name_index()`.
    #[serde(skip)]
    pub name_to_idx: HashMap<String, usize>,
}

impl OfficialIndex {
    /// What: Rebuild the `name_to_idx` `HashMap` from the current `pkgs` Vec.
    ///
    /// Inputs:
    /// - None (operates on `self.pkgs`)
    ///
    /// Output:
    /// - Populates `self.name_to_idx` with lowercase package names mapped to indices.
    ///
    /// Details:
    /// - Should be called after deserialization or when `pkgs` is modified.
    /// - Uses lowercase names for case-insensitive lookups.
    pub fn rebuild_name_index(&mut self) {
        self.name_to_idx.clear();
        self.name_to_idx.reserve(self.pkgs.len());
        for (i, pkg) in self.pkgs.iter().enumerate() {
            self.name_to_idx.insert(pkg.name.to_lowercase(), i);
        }
    }
}

/// What: Capture the minimal metadata about an official package entry.
///
/// Inputs:
/// - Populated primarily from `pacman -Sl`/API responses with optional enrichment.
///
/// Output:
/// - Serves as the source of truth for UI-facing `PackageItem` conversions.
///
/// Details:
/// - Represents a package from official Arch Linux repositories.
/// - Non-name fields may be empty initially; enrichment routines fill them lazily.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OfficialPkg {
    /// Package name.
    pub name: String,
    /// Repository name (e.g., "core", "extra", "community").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String,
    /// Target architecture (e.g., `x86_64`, `any`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arch: String,
    /// Package version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Package description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Process-wide holder for the official index state.
static OFFICIAL_INDEX: OnceLock<RwLock<OfficialIndex>> = OnceLock::new();
/// Process-wide set of installed package names.
static INSTALLED_SET: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
/// Process-wide set of explicitly-installed package names (dependency-free set).
static EXPLICIT_SET: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();

mod distro;
pub use distro::{
    is_artix_galaxy, is_artix_lib32, is_artix_omniverse, is_artix_repo, is_artix_system,
    is_artix_universe, is_artix_world, is_cachyos_repo, is_eos_name, is_eos_repo,
    is_manjaro_name_or_owner, is_name_manjaro,
};

/// What: Access the process-wide `OfficialIndex` lock for mutation or reads.
///
/// Inputs:
/// - None (initializes the underlying `OnceLock` on first use)
///
/// Output:
/// - `&'static RwLock<OfficialIndex>` guard used to manipulate the shared index state.
///
/// Details:
/// - Lazily seeds the index with an empty package list the first time it is accessed.
fn idx() -> &'static RwLock<OfficialIndex> {
    OFFICIAL_INDEX.get_or_init(|| {
        RwLock::new(OfficialIndex {
            pkgs: Vec::new(),
            name_to_idx: HashMap::new(),
        })
    })
}

/// What: Access the process-wide lock protecting the installed-package name cache.
///
/// Inputs:
/// - None (initializes the `OnceLock` on-demand)
///
/// Output:
/// - `&'static RwLock<HashSet<String>>` with the cached installed-package names.
///
/// Details:
/// - Lazily creates the shared `HashSet` the first time it is requested; subsequent calls reuse it.
fn installed_lock() -> &'static RwLock<HashSet<String>> {
    INSTALLED_SET.get_or_init(|| RwLock::new(HashSet::new()))
}

/// What: Access the process-wide lock protecting the explicit-package name cache.
///
/// Inputs:
/// - None (initializes the `OnceLock` on-demand)
///
/// Output:
/// - `&'static RwLock<HashSet<String>>` for explicitly installed package names.
///
/// Details:
/// - Lazily creates the shared set the first time it is requested; subsequent calls reuse it.
fn explicit_lock() -> &'static RwLock<HashSet<String>> {
    EXPLICIT_SET.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Package index enrichment utilities.
mod enrich;
/// Explicit package tracking.
mod explicit;
/// Package index fetching.
mod fetch;
/// Installed package utilities.
mod installed;
/// Package index persistence.
mod persist;
/// Package query utilities.
mod query;

#[cfg(windows)]
/// Mirror configuration for Windows.
mod mirrors;
/// Package index update utilities.
mod update;

pub use enrich::*;
pub use explicit::*;
pub use installed::*;
#[cfg(windows)]
pub use mirrors::*;
pub use persist::*;
pub use query::*;
#[cfg(not(windows))]
pub use update::update_in_background;

/// What: Find a package by name in the official index and return it as a `PackageItem`.
///
/// Inputs:
/// - `name`: Package name to search for
///
/// Output:
/// - `Some(PackageItem)` if the package is found in the official index, `None` otherwise.
///
/// Details:
/// - Uses the `name_to_idx` `HashMap` for O(1) lookup by lowercase name.
/// - Falls back to linear scan if `HashMap` is empty (e.g., before rebuild).
#[must_use]
pub fn find_package_by_name(name: &str) -> Option<crate::state::PackageItem> {
    use crate::state::{PackageItem, Source};

    if let Ok(g) = idx().read() {
        // Try O(1) HashMap lookup first
        let name_lower = name.to_lowercase();
        if let Some(&idx) = g.name_to_idx.get(&name_lower)
            && let Some(p) = g.pkgs.get(idx)
        {
            return Some(PackageItem {
                name: p.name.clone(),
                version: p.version.clone(),
                description: p.description.clone(),
                source: Source::Official {
                    repo: p.repo.clone(),
                    arch: p.arch.clone(),
                },
                popularity: None,
                out_of_date: None,
                orphaned: false,
            });
        }
        // Fallback to linear scan if HashMap is empty or index mismatch
        for p in &g.pkgs {
            if p.name.eq_ignore_ascii_case(name) {
                return Some(PackageItem {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    description: p.description.clone(),
                    source: Source::Official {
                        repo: p.repo.clone(),
                        arch: p.arch.clone(),
                    },
                    popularity: None,
                    out_of_date: None,
                    orphaned: false,
                });
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Verify `rebuild_name_index` populates `HashMap` correctly.
    ///
    /// Inputs:
    /// - `OfficialIndex` with two packages.
    ///
    /// Output:
    /// - `HashMap` contains lowercase names mapped to correct indices.
    ///
    /// Details:
    /// - Tests that the `HashMap` is built correctly and supports case-insensitive lookups.
    fn rebuild_name_index_populates_hashmap() {
        let mut index = OfficialIndex {
            pkgs: vec![
                OfficialPkg {
                    name: "PackageA".to_string(),
                    repo: "core".to_string(),
                    arch: "x86_64".to_string(),
                    version: "1.0".to_string(),
                    description: "Desc A".to_string(),
                },
                OfficialPkg {
                    name: "PackageB".to_string(),
                    repo: "extra".to_string(),
                    arch: "any".to_string(),
                    version: "2.0".to_string(),
                    description: "Desc B".to_string(),
                },
            ],
            name_to_idx: HashMap::new(),
        };

        index.rebuild_name_index();

        assert_eq!(index.name_to_idx.len(), 2);
        assert_eq!(index.name_to_idx.get("packagea"), Some(&0));
        assert_eq!(index.name_to_idx.get("packageb"), Some(&1));
        // Original case should not be found
        assert_eq!(index.name_to_idx.get("PackageA"), None);
    }

    #[test]
    /// What: Verify `find_package_by_name` uses `HashMap` for O(1) lookup.
    ///
    /// Inputs:
    /// - Seed index with packages and rebuilt `HashMap`.
    ///
    /// Output:
    /// - Package found via case-insensitive name lookup.
    ///
    /// Details:
    /// - Tests that find works with different case variations.
    fn find_package_by_name_uses_hashmap() {
        let _guard = crate::global_test_mutex_lock();

        if let Ok(mut g) = idx().write() {
            g.pkgs = vec![
                OfficialPkg {
                    name: "ripgrep".to_string(),
                    repo: "extra".to_string(),
                    arch: "x86_64".to_string(),
                    version: "14.0.0".to_string(),
                    description: "Fast grep".to_string(),
                },
                OfficialPkg {
                    name: "vim".to_string(),
                    repo: "extra".to_string(),
                    arch: "x86_64".to_string(),
                    version: "9.0".to_string(),
                    description: "Text editor".to_string(),
                },
            ];
            g.rebuild_name_index();
        }

        // Test exact case
        let result = find_package_by_name("ripgrep");
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|p| p.name.as_str()), Some("ripgrep"));

        // Test different case (HashMap uses lowercase)
        let result_upper = find_package_by_name("RIPGREP");
        assert!(result_upper.is_some());
        assert_eq!(
            result_upper.as_ref().map(|p| p.name.as_str()),
            Some("ripgrep")
        );

        // Test non-existent package
        let not_found = find_package_by_name("nonexistent");
        assert!(not_found.is_none());
    }
}
