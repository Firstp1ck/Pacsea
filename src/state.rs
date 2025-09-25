use ratatui::widgets::ListState;
use std::{collections::HashMap, path::PathBuf, time::Instant};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Source {
    Official { repo: String, arch: String },
    Aur,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageItem {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: Source,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PackageDetails {
    pub repository: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub architecture: String,
    pub url: String,
    pub licenses: Vec<String>,
    pub groups: Vec<String>,
    pub provides: Vec<String>,
    pub depends: Vec<String>,
    pub opt_depends: Vec<String>,
    pub required_by: Vec<String>,
    pub optional_for: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    pub download_size: Option<u64>,
    pub install_size: Option<u64>,
    pub owner: String, // packager/maintainer
    pub build_date: String,
}

#[derive(Clone, Debug)]
pub struct QueryInput {
    pub id: u64,
    pub text: String,
}
#[derive(Clone, Debug)]
pub struct SearchResults {
    pub id: u64,
    pub items: Vec<PackageItem>,
}

#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    Alert { message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Search,
    Recent,
    Install,
}

#[derive(Debug)]
pub struct AppState {
    pub input: String,
    pub results: Vec<PackageItem>,
    pub selected: usize,
    pub details: PackageDetails,
    pub list_state: ListState,
    pub modal: Modal,
    pub dry_run: bool,
    // Recent searches
    pub recent: Vec<String>,
    pub history_state: ListState,
    pub focus: Focus,
    pub last_input_change: Instant,
    pub last_saved_value: Option<String>,
    // Persisted recent searches
    pub recent_path: PathBuf,
    pub recent_dirty: bool,

    // Search coordination
    pub latest_query_id: u64,
    pub next_query_id: u64,
    // Details cache
    pub details_cache: HashMap<String, PackageDetails>,
    pub cache_path: PathBuf,
    pub cache_dirty: bool,

    // Install list pane
    pub install_list: Vec<PackageItem>,
    pub install_state: ListState,
    // Persisted install list
    pub install_path: PathBuf,
    pub install_dirty: bool,

    // In-pane search (for Recent/Install panes)
    pub pane_find: Option<String>,
}

impl Default for AppState {
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
        }
    }
}
