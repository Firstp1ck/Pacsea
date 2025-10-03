//! Core application state types for Pacsea's TUI.
//!
//! This module defines the serializable data structures used across the
//! application: package descriptors, search coordination types, UI focus and
//! modals, and the central [`AppState`] container mutated by the event and UI
//! layers. Many of these types are persisted between runs.
use ratatui::widgets::ListState;
use std::{collections::HashMap, path::PathBuf, time::Instant};

/// Package source origin.
///
/// Indicates whether a package originates from the official repositories or
/// the Arch User Repository.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Source {
    /// Official repository package and its associated repository and target
    /// architecture.
    Official { repo: String, arch: String },
    /// AUR package.
    Aur,
}

/// Minimal package summary used in lists and search results.
///
/// This is compact enough to render in lists and panes. For a richer, detailed
/// view, see [`PackageDetails`].
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageItem {
    /// Canonical package name.
    pub name: String,
    /// Version string as reported by the source.
    pub version: String,
    /// One-line description suitable for list display.
    pub description: String,
    /// Origin of the package (official repo or AUR).
    pub source: Source,
}

/// Full set of details for a package, suitable for a dedicated information
/// pane.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageDetails {
    /// Repository name (e.g., "extra").
    pub repository: String,
    /// Package name.
    pub name: String,
    /// Full version string.
    pub version: String,
    /// Long description.
    pub description: String,
    /// Target architecture.
    pub architecture: String,
    /// Upstream project URL (may be empty if unknown).
    pub url: String,
    /// SPDX or human-readable license identifiers.
    pub licenses: Vec<String>,
    /// Group memberships.
    pub groups: Vec<String>,
    /// Virtual provisions supplied by this package.
    pub provides: Vec<String>,
    /// Required dependencies.
    pub depends: Vec<String>,
    /// Optional dependencies with annotations.
    pub opt_depends: Vec<String>,
    /// Packages that require this package.
    pub required_by: Vec<String>,
    /// Packages for which this package is optional.
    pub optional_for: Vec<String>,
    /// Conflicting packages.
    pub conflicts: Vec<String>,
    /// Packages that this package replaces.
    pub replaces: Vec<String>,
    /// Download size in bytes, if available.
    pub download_size: Option<u64>,
    /// Installed size in bytes, if available.
    pub install_size: Option<u64>,
    /// Packager or maintainer name.
    pub owner: String, // packager/maintainer
    /// Build or packaging date (string-formatted for display).
    pub build_date: String,
}

/// Search query sent to the background search worker.
#[derive(Clone, Debug)]
pub struct QueryInput {
    /// Monotonic identifier used to correlate responses.
    pub id: u64,
    /// Raw query text entered by the user.
    pub text: String,
}
/// Results corresponding to a prior [`QueryInput`].
#[derive(Clone, Debug)]
pub struct SearchResults {
    /// Echoed identifier from the originating query.
    pub id: u64,
    /// Matching packages in rank order.
    pub items: Vec<PackageItem>,
}

/// Modal dialog state for the UI.
#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert {
        message: String,
    },
    /// Confirmation dialog for installing the given items.
    ConfirmInstall {
        items: Vec<PackageItem>,
    },
}

/// Which UI pane currently has keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// Center pane: search input and results.
    Search,
    /// Left pane: recent queries list.
    Recent,
    /// Right pane: pending install list.
    Install,
}

/// Global application state shared by the event, networking, and UI layers.
///
/// This structure is mutated frequently in response to input and background
/// updates. Certain subsets are persisted to disk to preserve user context
/// across runs (e.g., recent searches, details cache, install list).
#[derive(Debug)]
pub struct AppState {
    /// Current search input text.
    pub input: String,
    /// Current search results, most relevant first.
    pub results: Vec<PackageItem>,
    /// Index into `results` that is currently highlighted.
    pub selected: usize,
    /// Details for the currently highlighted result.
    pub details: PackageDetails,
    /// List selection state for the search results list.
    pub list_state: ListState,
    /// Active modal dialog, if any.
    pub modal: Modal,
    /// If `true`, show install steps without executing side effects.
    pub dry_run: bool,
    // Recent searches
    /// Previously executed queries.
    pub recent: Vec<String>,
    /// List selection state for the Recent pane.
    pub history_state: ListState,
    /// Which pane is currently focused.
    pub focus: Focus,
    /// Timestamp of the last input edit, used for debouncing or throttling.
    pub last_input_change: Instant,
    /// Last value persisted for the input field, to avoid redundant writes.
    pub last_saved_value: Option<String>,
    // Persisted recent searches
    /// Path where recent searches are persisted as JSON.
    pub recent_path: PathBuf,
    /// Dirty flag indicating `recent` needs to be saved.
    pub recent_dirty: bool,

    // Search coordination
    /// Identifier of the latest query whose results are being displayed.
    pub latest_query_id: u64,
    /// Next query identifier to allocate.
    pub next_query_id: u64,
    // Details cache
    /// Cache of details keyed by package name.
    pub details_cache: HashMap<String, PackageDetails>,
    /// Path where the details cache is persisted as JSON.
    pub cache_path: PathBuf,
    /// Dirty flag indicating `details_cache` needs to be saved.
    pub cache_dirty: bool,

    // Install list pane
    /// Packages selected for installation.
    pub install_list: Vec<PackageItem>,
    /// List selection state for the Install pane.
    pub install_state: ListState,
    // Persisted install list
    /// Path where the install list is persisted as JSON.
    pub install_path: PathBuf,
    /// Dirty flag indicating `install_list` needs to be saved.
    pub install_dirty: bool,

    // In-pane search (for Recent/Install panes)
    /// Optional, transient find pattern used by pane-local search ("/").
    pub pane_find: Option<String>,

    // Official package index persistence
    /// Path to the persisted official package index used for fast offline lookups.
    pub official_index_path: PathBuf,

    // Loading indicator for official index generation
    /// Whether the application is currently generating the official index.
    pub loading_index: bool,

    // Track which package’s details the UI is focused on
    /// Name of the package whose details are being emphasized in the UI, if any.
    pub details_focus: Option<String>,

    // Ring prefetch debounce state
    /// Smooth scrolling accumulator for prefetch heuristics.
    pub scroll_moves: u32,
    /// Timestamp at which to resume ring prefetching, if paused.
    pub ring_resume_at: Option<Instant>,
    /// Whether a ring prefetch is needed soon.
    pub need_ring_prefetch: bool,

    // Clickable URL button rectangle (x, y, w, h) in terminal cells
    /// Rectangle of the clickable URL button in terminal cell coordinates.
    pub url_button_rect: Option<(u16, u16, u16, u16)>,
}

impl Default for AppState {
    /// Construct a default, empty [`AppState`], initializing paths, selection
    /// states, and timers with sensible defaults.
    fn default() -> Self {
        Self {
            input: String::new(),
            results: Vec::new(),
            selected: 0,
            details: PackageDetails::default(),
            list_state: ListState::default(),
            modal: Modal::None,
            dry_run: false,
            recent: Vec::new(),
            history_state: ListState::default(),
            focus: Focus::Search,
            last_input_change: Instant::now(),
            last_saved_value: None,
            // Persisted recent searches
            recent_path: PathBuf::from("recent_searches.json"),
            recent_dirty: false,

            latest_query_id: 0,
            next_query_id: 1,
            details_cache: HashMap::new(),
            cache_path: PathBuf::from("details_cache.json"),
            cache_dirty: false,

            install_list: Vec::new(),
            install_state: ListState::default(),
            install_path: PathBuf::from("install_list.json"),
            install_dirty: false,

            pane_find: None,

            official_index_path: PathBuf::from("official_index.json"),

            loading_index: false,

            details_focus: None,

            scroll_moves: 0,
            ring_resume_at: None,
            need_ring_prefetch: false,
            url_button_rect: None,
        }
    }
}
