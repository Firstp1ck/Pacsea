//! Official package index management, persistence, and enrichment.
//!
//! Split into submodules for maintainability. Public API is re-exported
//! to remain compatible with previous `crate::index` consumers.

use std::collections::HashSet;
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
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct OfficialIndex {
    /// All known official packages in the process-wide index.
    pub pkgs: Vec<OfficialPkg>,
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
/// - Non-name fields may be empty initially; enrichment routines fill them lazily.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OfficialPkg {
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String, // core or extra
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub arch: String, // e.g., x86_64/any
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
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
    OFFICIAL_INDEX.get_or_init(|| RwLock::new(OfficialIndex { pkgs: Vec::new() }))
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

mod enrich;
mod explicit;
mod fetch;
mod installed;
mod persist;
mod query;

#[cfg(windows)]
mod mirrors;
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
