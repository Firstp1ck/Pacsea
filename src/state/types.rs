//! Core value types used by Pacsea state.

/// Minimal news entry for Arch news modal.
#[derive(Clone, Debug)]
pub struct NewsItem {
    /// Publication date (short, e.g., 2025-10-11)
    pub date: String,
    /// Title text
    pub title: String,
    /// Link URL
    pub url: String,
}

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
    /// AUR popularity score when available (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f64>,
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
    /// AUR popularity score when available (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popularity: Option<f64>,
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

/// Sorting mode for the Results list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    /// Default: Pacman (core/extra/other official) first, then AUR; name tiebreak.
    RepoThenName,
    /// AUR first (by highest popularity), then official repos; name tiebreak.
    AurPopularityThenOfficial,
    /// Best matches: Relevance by name to current query, then repo order, then name.
    BestMatches,
}

impl SortMode {
    /// Return the string key used in settings files for this sort mode.
    ///
    /// Inputs: none
    ///
    /// Output: Static config key string.
    pub fn as_config_key(&self) -> &'static str {
        match self {
            SortMode::RepoThenName => "alphabetical",
            SortMode::AurPopularityThenOfficial => "aur_popularity",
            SortMode::BestMatches => "best_matches",
        }
    }
    /// Parse a sort mode from its settings key or legacy aliases.
    ///
    /// Inputs: `s` config string (case-insensitive).
    ///
    /// Output: `Some(SortMode)` on recognized value; `None` otherwise.
    pub fn from_config_key(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "alphabetical" | "repo_then_name" | "pacman" => Some(SortMode::RepoThenName),
            "aur_popularity" | "popularity" => Some(SortMode::AurPopularityThenOfficial),
            "best_matches" | "relevance" => Some(SortMode::BestMatches),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SortMode;

    #[test]
    /// What: SortMode config key mapping roundtrip and alias handling
    ///
    /// - Input: Known keys and aliases; unknown key
    /// - Output: Correct mapping to enum variants; None for unknown
    fn state_sortmode_config_roundtrip_and_aliases() {
        assert_eq!(SortMode::RepoThenName.as_config_key(), "alphabetical");
        assert_eq!(
            SortMode::from_config_key("alphabetical"),
            Some(SortMode::RepoThenName)
        );
        assert_eq!(
            SortMode::from_config_key("repo_then_name"),
            Some(SortMode::RepoThenName)
        );
        assert_eq!(
            SortMode::from_config_key("pacman"),
            Some(SortMode::RepoThenName)
        );
        assert_eq!(
            SortMode::from_config_key("aur_popularity"),
            Some(SortMode::AurPopularityThenOfficial)
        );
        assert_eq!(
            SortMode::from_config_key("popularity"),
            Some(SortMode::AurPopularityThenOfficial)
        );
        assert_eq!(
            SortMode::from_config_key("best_matches"),
            Some(SortMode::BestMatches)
        );
        assert_eq!(
            SortMode::from_config_key("relevance"),
            Some(SortMode::BestMatches)
        );
        assert_eq!(SortMode::from_config_key("unknown"), None);
    }
}

/// Visual indicator for Arch status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchStatusColor {
    /// No color known yet.
    None,
    /// Everything operational (green).
    Operational,
    /// Relevant incident today (yellow).
    IncidentToday,
    /// Severe incident today (red).
    IncidentSevereToday,
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

/// Which sub-pane within the right column is currently focused when applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPaneFocus {
    /// Normal mode: single Install list occupies the right column.
    Install,
    /// Installed-only mode: left subpane for planned downgrades.
    Downgrade,
    /// Installed-only mode: right subpane for removals.
    Remove,
}

/// Row model for the "TUI Optional Deps" modal/list.
/// Each row represents a concrete package candidate such as an editor,
/// terminal, clipboard tool, mirror updater, or AUR helper.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct OptionalDepRow {
    /// Human-friendly label to display in the UI (e.g., "Editor: nvim", "Terminal: kitty").
    pub label: String,
    /// The concrete package name to check/install (e.g., "nvim", "kitty", "wl-clipboard",
    /// "reflector", "pacman-mirrors", "paru", "yay").
    pub package: String,
    /// Whether this dependency is currently installed on the system.
    #[serde(default)]
    pub installed: bool,
    /// Whether the user can select this row for installation (only when not installed).
    #[serde(default)]
    pub selectable: bool,
    /// Optional note for environment/distro constraints (e.g., "Wayland", "X11", "Manjaro only").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
