//! Core value types used by Pacsea state.

use zeroize::Zeroize;

/// What: Zeroizing wrapper for sensitive in-memory string data such as passwords.
///
/// Inputs:
/// - Constructed from owned string data via [`From<String>`], [`From<&str>`], or [`SecureString::new`].
///
/// Output:
/// - Provides read-only string access while ensuring secret bytes are wiped on drop.
///
/// Details:
/// - The inner buffer is zeroized before deallocation to reduce residual secret exposure.
/// - `Debug` output is intentionally redacted and never reveals the secret value.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SecureString(String);

impl SecureString {
    /// What: Create a new zeroizing string wrapper from owned string data.
    ///
    /// Inputs:
    /// - `value`: Secret string to store.
    ///
    /// Output:
    /// - New [`SecureString`] containing `value`.
    ///
    /// Details:
    /// - Ownership is moved into the wrapper so drop-time zeroization covers this allocation.
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self(value)
    }

    /// What: Borrow the wrapped secret as an immutable string slice.
    ///
    /// Inputs:
    /// - `self`: Borrowed secure string instance.
    ///
    /// Output:
    /// - `&str` view of the wrapped value.
    ///
    /// Details:
    /// - Intended for short-lived read usage (validation and command construction).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// What: Return the current number of bytes in the wrapped secret.
    ///
    /// Inputs:
    /// - `self`: Borrowed secure string instance.
    ///
    /// Output:
    /// - Byte length of the underlying UTF-8 buffer.
    ///
    /// Details:
    /// - Mirrors `String::len` and is used by cursor movement logic in password input handling.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// What: Check whether the wrapped secret is empty.
    ///
    /// Inputs:
    /// - `self`: Borrowed secure string instance.
    ///
    /// Output:
    /// - `true` when no bytes are present, otherwise `false`.
    ///
    /// Details:
    /// - Mirrors `String::is_empty`.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// What: Insert a character into the wrapped secret at a byte index.
    ///
    /// Inputs:
    /// - `idx`: Byte position where `ch` is inserted.
    /// - `ch`: Character to insert.
    ///
    /// Output:
    /// - Mutates the wrapped secret in place.
    ///
    /// Details:
    /// - Panics if `idx` is not on a valid UTF-8 boundary, matching `String::insert`.
    pub fn insert(&mut self, idx: usize, ch: char) {
        self.0.insert(idx, ch);
    }

    /// What: Remove and return a character from the wrapped secret at a byte index.
    ///
    /// Inputs:
    /// - `idx`: Byte position of the character to remove.
    ///
    /// Output:
    /// - Removed `char` value.
    ///
    /// Details:
    /// - Panics if `idx` is not on a valid UTF-8 boundary, matching `String::remove`.
    pub fn remove(&mut self, idx: usize) -> char {
        self.0.remove(idx)
    }

    /// What: Append a character to the wrapped secret.
    ///
    /// Inputs:
    /// - `ch`: Character to append.
    ///
    /// Output:
    /// - Mutates the wrapped secret in place.
    ///
    /// Details:
    /// - Mirrors `String::push` while keeping ownership in the secure wrapper.
    pub fn push(&mut self, ch: char) {
        self.0.push(ch);
    }

    /// What: Clear all bytes from the wrapped secret.
    ///
    /// Inputs:
    /// - `self`: Mutable secure string instance.
    ///
    /// Output:
    /// - Empties the wrapped string.
    ///
    /// Details:
    /// - Mirrors `String::clear` for controlled reset paths.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl std::ops::Deref for SecureString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::fmt::Debug for SecureString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecureString([REDACTED])")
    }
}

impl From<String> for SecureString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecureString {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

impl Drop for SecureString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

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

/// What: High-level application mode.
///
/// Inputs: None (enum variants)
///
/// Output: Represents whether the UI is in package management or news view.
///
/// Details:
/// - `Package` preserves the existing package management experience.
/// - `News` switches panes to the news feed experience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    /// Package management/search mode (existing UI).
    Package,
    /// News feed mode (new UI).
    News,
}

/// What: News/advisory source type.
///
/// Inputs: None (enum variants)
///
/// Output: Identifies where a news feed item originates.
///
/// Details:
/// - Distinguishes Arch news RSS posts from security advisories.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum NewsFeedSource {
    /// Official Arch Linux news RSS item.
    ArchNews,
    /// security.archlinux.org advisory.
    SecurityAdvisory,
    /// Installed official package received a version update.
    InstalledPackageUpdate,
    /// Installed AUR package received a version update.
    AurPackageUpdate,
    /// New AUR comment on an installed package.
    AurComment,
}

/// What: Severity levels for security advisories.
///
/// Inputs: None (enum variants)
///
/// Output: Normalized advisory severity.
///
/// Details:
/// - Ordered from lowest to highest severity for sorting.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum AdvisorySeverity {
    /// Unknown or not provided.
    Unknown,
    /// Low severity.
    Low,
    /// Medium severity.
    Medium,
    /// High severity.
    High,
    /// Critical severity.
    Critical,
}

/// What: Map advisory severity to a numeric rank for sorting (higher is worse).
///
/// Inputs:
/// - `severity`: Optional advisory severity value.
///
/// Output:
/// - Numeric rank where larger numbers indicate higher severity (Critical highest).
///
/// Details:
/// - Returns `0` when severity is missing to ensure advisories without severity fall last.
/// - Keeps ordering stable across both news feed sorting and advisory-specific listings.
#[must_use]
pub const fn severity_rank(severity: Option<AdvisorySeverity>) -> u8 {
    match severity {
        Some(AdvisorySeverity::Critical) => 5,
        Some(AdvisorySeverity::High) => 4,
        Some(AdvisorySeverity::Medium) => 3,
        Some(AdvisorySeverity::Low) => 2,
        Some(AdvisorySeverity::Unknown) => 1,
        None => 0,
    }
}

/// What: Sort options for news feed results.
///
/// Inputs: None (enum variants)
///
/// Output: Selected sort mode for news items.
///
/// Details:
/// - `DateDesc` is newest-first default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewsSortMode {
    /// Newest first by date.
    DateDesc,
    /// Oldest first by date.
    DateAsc,
    /// Alphabetical by title.
    Title,
    /// Group by source then title.
    SourceThenTitle,
    /// Severity first (Critical..Unknown), then date (newest first).
    SeverityThenDate,
    /// Unread items first, then date (newest first).
    UnreadThenDate,
}

/// What: Read filter applied to news feed items.
///
/// Inputs: None (enum variants)
///
/// Output:
/// - Indicates whether to show all, only read, or only unread items.
///
/// Details:
/// - Used by the News Feed list and toolbar filter chip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewsReadFilter {
    /// Show all items regardless of read status.
    All,
    /// Show only items marked as read.
    Read,
    /// Show only items not marked as read.
    Unread,
}

/// What: Unified news/advisory feed item for the news view.
///
/// Inputs:
/// - Fields describing the item (title, summary, url, source, severity, packages, date)
///
/// Output:
/// - Data ready for list and details rendering in news mode.
///
/// Details:
/// - `id` is a stable identifier (URL for news, advisory ID for security).
/// - `packages` holds affected package names for advisories.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NewsFeedItem {
    /// Stable identifier (URL or advisory ID).
    pub id: String,
    /// Publication or update date (YYYY-MM-DD).
    pub date: String,
    /// Human-readable title/headline.
    pub title: String,
    /// Optional summary/description.
    pub summary: Option<String>,
    /// Optional link URL for details.
    pub url: Option<String>,
    /// Source type (Arch news vs security advisory).
    pub source: NewsFeedSource,
    /// Optional advisory severity.
    pub severity: Option<AdvisorySeverity>,
    /// Affected packages (advisories only).
    pub packages: Vec<String>,
}

/// What: Bundle of news feed items and associated last-seen state updates.
///
/// Inputs:
/// - `items`: Aggregated news feed entries ready for rendering.
/// - `seen_pkg_versions`: Updated map of installed package names to last-seen versions.
/// - `seen_aur_comments`: Updated map of AUR packages to last-seen comment identifiers.
///
/// Output:
/// - Carries feed payload plus dedupe state for persistence.
///
/// Details:
/// - Used as the payload between background fetchers and UI to keep last-seen maps in sync.
#[derive(Clone, Debug)]
pub struct NewsFeedPayload {
    /// Aggregated and sorted feed items.
    pub items: Vec<NewsFeedItem>,
    /// Last-seen versions for installed packages.
    pub seen_pkg_versions: std::collections::HashMap<String, String>,
    /// Last-seen comment identifiers for installed AUR packages.
    pub seen_aur_comments: std::collections::HashMap<String, String>,
}

/// What: Persisted bookmark entry for news items, including cached content and optional local HTML path.
///
/// Inputs:
/// - `item`: The news feed item metadata.
/// - `content`: Parsed article content stored locally for offline display.
/// - `html_path`: Optional filesystem path to the saved HTML file (if downloaded).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NewsBookmark {
    /// News feed metadata for the bookmark.
    pub item: NewsFeedItem,
    /// Parsed content cached locally.
    pub content: Option<String>,
    /// Path to the saved HTML file on disk (if downloaded).
    pub html_path: Option<String>,
}

/// Package source origin.
///
/// Indicates whether a package originates from the official repositories or
/// the Arch User Repository.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum Source {
    /// Official repository package and its associated repository and target
    /// architecture.
    Official {
        /// Repository name (e.g., "core", "extra", "community").
        repo: String,
        /// Target architecture (e.g., `x86_64`).
        arch: String,
    },
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
    /// Timestamp when package was flagged out-of-date (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_of_date: Option<u64>,
    /// Whether package is orphaned (no active maintainer) (AUR only).
    #[serde(default, skip_serializing_if = "is_false")]
    pub orphaned: bool,
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
    /// Timestamp when package was flagged out-of-date (AUR only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out_of_date: Option<u64>,
    /// Whether package is orphaned (no active maintainer) (AUR only).
    #[serde(default, skip_serializing_if = "is_false")]
    pub orphaned: bool,
}

/// Search query sent to the background search worker.
#[derive(Clone, Debug)]
pub struct QueryInput {
    /// Monotonic identifier used to correlate responses.
    pub id: u64,
    /// Raw query text entered by the user.
    pub text: String,
    /// Whether fuzzy search mode is enabled.
    pub fuzzy: bool,
}

/// Results corresponding to a prior [`QueryInput`].
#[derive(Clone, Debug)]
pub struct SearchResults {
    /// Echoed identifier from the originating query.
    pub id: u64,
    /// Matching packages in rank order.
    pub items: Vec<PackageItem>,
}

/// What: Request payload to run PKGBUILD static checks.
#[derive(Clone, Debug)]
pub struct PkgbuildCheckRequest {
    /// Selected package name.
    pub package_name: String,
    /// Current PKGBUILD text shown in preview.
    pub pkgbuild_text: String,
    /// Global dry-run flag.
    pub dry_run: bool,
}

/// What: Response payload for PKGBUILD static checks.
#[derive(Clone, Debug)]
pub struct PkgbuildCheckResponse {
    /// Package name tied to this run.
    pub package_name: String,
    /// Parsed findings for list rendering.
    pub findings: Vec<crate::state::app_state::PkgbuildCheckFinding>,
    /// Raw per-tool outputs from latest PKGBUILD check run.
    pub raw_results: Vec<crate::state::app_state::PkgbuildToolRawResult>,
    /// User-facing missing tool hints.
    pub missing_tools: Vec<String>,
    /// Optional high-level execution error.
    pub last_error: Option<String>,
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
    /// What: Map the enum variant to its persisted configuration key.
    /// - Input: None; uses the receiver variant.
    /// - Output: Static string representing the serialized value.
    /// - Details: Keeps `settings.conf` forward/backward compatible by
    ///   standardizing the keys stored on disk.
    #[must_use]
    pub const fn as_config_key(&self) -> &'static str {
        match self {
            Self::RepoThenName => "alphabetical",
            Self::AurPopularityThenOfficial => "aur_popularity",
            Self::BestMatches => "best_matches",
        }
    }
    /// Parse a sort mode from its settings key or legacy aliases.
    ///
    /// What: Convert persisted config values back into `SortMode` variants.
    /// - Input: `s` string slice containing the stored key (case-insensitive).
    /// - Output: `Some(SortMode)` when a known variant matches; `None` for
    ///   unrecognized keys.
    /// - Details: Accepts historical aliases to maintain compatibility with
    ///   earlier Pacsea releases.
    #[must_use]
    pub fn from_config_key(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "alphabetical" | "repo_then_name" | "pacman" => Some(Self::RepoThenName),
            "aur_popularity" | "popularity" => Some(Self::AurPopularityThenOfficial),
            "best_matches" | "relevance" => Some(Self::BestMatches),
            _ => None,
        }
    }
}

/// Filter mode for installed packages in the "Installed" toggle.
///
/// What: Controls which packages are shown when viewing installed packages.
/// - `LeafOnly`: Show only explicitly installed packages with no dependents (pacman -Qetq).
/// - `AllExplicit`: Show all explicitly installed packages (pacman -Qeq).
///
/// Details:
/// - `LeafOnly` is the default, showing packages safe to remove.
/// - `AllExplicit` includes packages that other packages depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InstalledPackagesMode {
    /// Show only leaf packages (explicitly installed, nothing depends on them).
    #[default]
    LeafOnly,
    /// Show all explicitly installed packages.
    AllExplicit,
}

impl InstalledPackagesMode {
    /// Return the string key used in settings files for this mode.
    ///
    /// What: Map the enum variant to its persisted configuration key.
    /// - Input: None; uses the receiver variant.
    /// - Output: Static string representing the serialized value.
    #[must_use]
    pub const fn as_config_key(&self) -> &'static str {
        match self {
            Self::LeafOnly => "leaf",
            Self::AllExplicit => "all",
        }
    }

    /// Parse an installed packages mode from its settings key.
    ///
    /// What: Convert persisted config values back into `InstalledPackagesMode` variants.
    /// - Input: `s` string slice containing the stored key (case-insensitive).
    /// - Output: `Some(InstalledPackagesMode)` when a known variant matches; `None` otherwise.
    #[must_use]
    pub fn from_config_key(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "leaf" | "leaf_only" => Some(Self::LeafOnly),
            "all" | "all_explicit" => Some(Self::AllExplicit),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstalledPackagesMode, SortMode};

    #[test]
    /// What: Validate `SortMode` converts to and from configuration keys, including legacy aliases.
    ///
    /// Inputs:
    /// - Known config keys, historical aliases, and a deliberately unknown key.
    ///
    /// Output:
    /// - Returns the expected enum variants for recognised keys and `None` for the unknown entry.
    ///
    /// Details:
    /// - Guards against accidental regressions when tweaking the accepted key list or canonical names.
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

    #[test]
    /// What: Validate `InstalledPackagesMode` converts to and from configuration keys, including aliases.
    ///
    /// Inputs:
    /// - Known config keys, aliases, case variations, whitespace, and a deliberately unknown key.
    ///
    /// Output:
    /// - Returns the expected enum variants for recognised keys and `None` for the unknown entry.
    ///
    /// Details:
    /// - Guards against accidental regressions when tweaking the accepted key list or canonical names.
    /// - Verifies roundtrip conversions and case-insensitive parsing.
    fn state_installedpackagesmode_config_roundtrip_and_aliases() {
        // Test as_config_key for both variants
        assert_eq!(InstalledPackagesMode::LeafOnly.as_config_key(), "leaf");
        assert_eq!(InstalledPackagesMode::AllExplicit.as_config_key(), "all");

        // Test from_config_key with canonical keys
        assert_eq!(
            InstalledPackagesMode::from_config_key("leaf"),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("all"),
            Some(InstalledPackagesMode::AllExplicit)
        );

        // Test from_config_key with aliases
        assert_eq!(
            InstalledPackagesMode::from_config_key("leaf_only"),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("all_explicit"),
            Some(InstalledPackagesMode::AllExplicit)
        );

        // Test roundtrip conversions
        assert_eq!(
            InstalledPackagesMode::from_config_key(InstalledPackagesMode::LeafOnly.as_config_key()),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key(
                InstalledPackagesMode::AllExplicit.as_config_key()
            ),
            Some(InstalledPackagesMode::AllExplicit)
        );

        // Test case insensitivity
        assert_eq!(
            InstalledPackagesMode::from_config_key("LEAF"),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("Leaf"),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("LEAF_ONLY"),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("All"),
            Some(InstalledPackagesMode::AllExplicit)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("ALL_EXPLICIT"),
            Some(InstalledPackagesMode::AllExplicit)
        );

        // Test whitespace trimming
        assert_eq!(
            InstalledPackagesMode::from_config_key("  leaf  "),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("  all  "),
            Some(InstalledPackagesMode::AllExplicit)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("  leaf_only  "),
            Some(InstalledPackagesMode::LeafOnly)
        );
        assert_eq!(
            InstalledPackagesMode::from_config_key("  all_explicit  "),
            Some(InstalledPackagesMode::AllExplicit)
        );

        // Test unknown key
        assert_eq!(InstalledPackagesMode::from_config_key("unknown"), None);
        assert_eq!(InstalledPackagesMode::from_config_key(""), None);
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

/// What: Pacman `[repo]` presence as shown in the read-only Repositories modal.
///
/// Inputs:
/// - Set when merging `repos.conf` rows with a live `pacman.conf` scan.
///
/// Output:
/// - Drives result column labels in the UI.
///
/// Details:
/// - Distinct from Pacsea results-filter toggles; this reflects `/etc/pacman.conf` only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryPacmanStatus {
    /// No matching section header found.
    Absent,
    /// Active `[name]` header exists.
    Active,
    /// Only `# [name]` (commented) headers exist.
    Commented,
}

/// What: Signing key trust hint for a `[[repo]]` row that declares `key_id`.
///
/// Inputs:
/// - Derived from a batched `pacman-key --list-keys` check.
///
/// Output:
/// - Column text in the Repositories modal.
///
/// Details:
/// - `Unknown` covers missing `pacman-key`, failed runs, or fingerprints too short to match safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepositoryKeyTrust {
    /// Row has no `key_id`; nothing to verify.
    NotApplicable,
    /// Fingerprint (normalized) appears in the key listing.
    Trusted,
    /// Listing succeeded but fingerprint not found.
    NotTrusted,
    /// Could not determine (tool missing, error, or invalid id).
    Unknown,
}

/// What: One row in the read-only Repositories modal (merged `repos.conf` + live pacman scan).
///
/// Inputs:
/// - Built when opening the Repositories modal from `logic::repos`.
///
/// Output:
/// - Rendered as a list line with status chips.
///
/// Details:
/// - Read-only in Phase 2; apply flows will extend behavior later.
#[derive(Clone, Debug)]
pub struct RepositoryModalRow {
    /// Pacman section `name` from `repos.conf`.
    pub pacman_section_name: String,
    /// Raw `results_filter` label for display.
    pub results_filter_display: String,
    /// Whether `/etc/pacman.conf` (includes) contains this repo section.
    pub pacman_status: RepositoryPacmanStatus,
    /// Optional short source file hint (e.g. include file name).
    pub source_hint: Option<String>,
    /// Keyring trust classification when `key_id` is set.
    pub key_trust: RepositoryKeyTrust,
}

/// AUR package comment data structure.
///
/// What: Represents a single comment from an AUR package page.
///
/// Inputs: None (data structure).
///
/// Output: None (data structure).
///
/// Details:
/// - Contains author, date, and content of a comment.
/// - Includes optional timestamp for reliable chronological sorting.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AurComment {
    /// Stable comment identifier parsed from DOM when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Comment author username.
    pub author: String,
    /// Human-readable date string.
    pub date: String,
    /// Unix timestamp for sorting (None if parsing failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_timestamp: Option<i64>,
    /// URL from the date link (None if not available).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_url: Option<String>,
    /// Comment content text.
    pub content: String,
    /// Whether this comment is pinned (shown at the top).
    #[serde(default)]
    pub pinned: bool,
}

/// Helper function for serde to skip serializing false boolean values.
#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(b: &bool) -> bool {
    !(*b)
}
