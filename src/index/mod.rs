//! Official package index management, persistence, and enrichment.
//!
//! Split into submodules for maintainability. Public API is re-exported
//! to remain compatible with previous `crate::index` consumers.

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

/// In-memory representation of the persisted official package index.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct OfficialIndex {
    /// All known official packages in the process-wide index.
    pub pkgs: Vec<OfficialPkg>,
}

/// Minimal, serializable record for a package in the official repositories.
///
/// Fields other than `name` may be empty for speed when the index is first
/// fetched. Additional details are filled in later by enrichment routines.
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
    is_cachyos_repo, is_eos_name, is_eos_repo, is_manjaro_name_or_owner, is_name_manjaro,
};

/// Get a reference to the global `OfficialIndex` lock, initializing it if needed.
fn idx() -> &'static RwLock<OfficialIndex> {
    OFFICIAL_INDEX.get_or_init(|| RwLock::new(OfficialIndex { pkgs: Vec::new() }))
}

/// Get a reference to the global installed-name set lock, initializing it if needed.
fn installed_lock() -> &'static RwLock<HashSet<String>> {
    INSTALLED_SET.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Get a reference to the global explicit-name set lock, initializing it if needed.
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
#[cfg(not(windows))]
pub use update::update_in_background;
pub use persist::*;
pub use query::*;

#[cfg(test)]
static TEST_MUTEX: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

#[cfg(test)]
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
