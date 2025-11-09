//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, OptionalDepRow, PackageItem};
use std::collections::HashSet;

/// What: Enumerates the high-level operations represented in the preflight
/// workflow.
///
/// - Input: Selected by callers when presenting confirmation or preflight
///   dialogs.
/// - Output: Indicates whether the UI should prepare for an install or remove
///   transaction.
/// - Details: Drives copy, button labels, and logging in the preflight and
///   execution flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightAction {
    Install,
    Remove,
}

/// What: Identifies which tab within the preflight modal is active.
///
/// - Input: Set by UI event handlers responding to user navigation.
/// - Output: Informs the renderer which data set to display (summary, deps,
///   files, etc.).
/// - Details: Enables multi-step review of package operations without losing
///   context between tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightTab {
    Summary,
    Deps,
    Files,
    Services,
    Sandbox,
}

/// Removal cascade strategy for pacman operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeMode {
    /// `pacman -R` – remove targets only.
    Basic,
    /// `pacman -Rs` – remove targets and orphaned dependencies.
    Cascade,
    /// `pacman -Rns` – cascade removal and prune configuration files.
    CascadeWithConfigs,
}

impl CascadeMode {
    /// Return the pacman flag sequence corresponding to this cascade mode.
    pub const fn flag(self) -> &'static str {
        match self {
            CascadeMode::Basic => "-R",
            CascadeMode::Cascade => "-Rs",
            CascadeMode::CascadeWithConfigs => "-Rns",
        }
    }

    /// Short text describing the effect of this mode.
    pub const fn description(self) -> &'static str {
        match self {
            CascadeMode::Basic => "targets only",
            CascadeMode::Cascade => "remove dependents",
            CascadeMode::CascadeWithConfigs => "dependents + configs",
        }
    }

    /// Whether this mode allows removal when dependents exist.
    pub const fn allows_dependents(self) -> bool {
        !matches!(self, CascadeMode::Basic)
    }

    /// Cycle to the next cascade mode.
    pub const fn next(self) -> Self {
        match self {
            CascadeMode::Basic => CascadeMode::Cascade,
            CascadeMode::Cascade => CascadeMode::CascadeWithConfigs,
            CascadeMode::CascadeWithConfigs => CascadeMode::Basic,
        }
    }
}

/// Dependency information for a package in the preflight dependency view.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DependencyInfo {
    /// Package name.
    pub name: String,
    /// Required version constraint (e.g., ">=1.2.3" or "1.2.3").
    pub version: String,
    /// Current status of this dependency.
    pub status: DependencyStatus,
    /// Source repository or origin.
    pub source: DependencySource,
    /// Packages that require this dependency.
    pub required_by: Vec<String>,
    /// Packages that this dependency depends on (transitive deps).
    pub depends_on: Vec<String>,
    /// Whether this is a core repository package.
    pub is_core: bool,
    /// Whether this is a critical system package.
    pub is_system: bool,
}

/// Summary statistics for reverse dependency analysis of removal targets.
#[derive(Clone, Debug, Default)]
pub struct ReverseRootSummary {
    /// Package slated for removal.
    pub package: String,
    /// Number of packages that directly depend on this package (depth 1).
    pub direct_dependents: usize,
    /// Number of packages that depend on this package through other packages (depth ≥ 2).
    pub transitive_dependents: usize,
    /// Total number of dependents (direct + transitive).
    pub total_dependents: usize,
}

/// Status of a dependency relative to the current system state.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DependencyStatus {
    /// Already installed and version matches requirement.
    Installed { version: String },
    /// Not installed, needs to be installed.
    ToInstall,
    /// Installed but outdated, needs upgrade.
    ToUpgrade { current: String, required: String },
    /// Conflicts with existing packages.
    Conflict { reason: String },
    /// Cannot be found in configured repositories or AUR.
    Missing,
}

/// Source of a dependency package.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DependencySource {
    /// Official repository package.
    Official { repo: String },
    /// AUR package.
    Aur,
    /// Local package (not in repos).
    Local,
}

/// Type of file change in a package operation.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FileChangeType {
    /// File will be newly installed (not currently on system).
    New,
    /// File exists but will be replaced/updated.
    Changed,
    /// File will be removed (for Remove operations).
    Removed,
}

/// Information about a file change in a package operation.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FileChange {
    /// Full path of the file.
    pub path: String,
    /// Type of change (new/changed/removed).
    pub change_type: FileChangeType,
    /// Package that owns this file.
    pub package: String,
    /// Whether this is a configuration file (under /etc or marked as backup).
    pub is_config: bool,
    /// Whether this file is predicted to create a .pacnew file (config conflict).
    pub predicted_pacnew: bool,
    /// Whether this file is predicted to create a .pacsave file (config removal).
    pub predicted_pacsave: bool,
}

/// File information for a package in the preflight file view.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PackageFileInfo {
    /// Package name.
    pub name: String,
    /// List of file changes for this package.
    pub files: Vec<FileChange>,
    /// Total number of files (including directories).
    pub total_count: usize,
    /// Number of new files.
    pub new_count: usize,
    /// Number of changed files.
    pub changed_count: usize,
    /// Number of removed files.
    pub removed_count: usize,
    /// Number of configuration files.
    pub config_count: usize,
    /// Number of files predicted to create .pacnew files.
    pub pacnew_candidates: usize,
    /// Number of files predicted to create .pacsave files.
    pub pacsave_candidates: usize,
}

/// What: Captures all dialog state for the various modal overlays presented in
/// the Pacsea TUI.
///
/// - Input: Mutated by event handlers in response to user actions or
///   background updates.
/// - Output: Drives conditional rendering and behavior of each modal type.
/// - Details: Acts as a tagged union so only one modal can be active at a time
///   while carrying the precise data needed for that modal's UI.
#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert { message: String },
    /// Confirmation dialog for installing the given items.
    #[allow(dead_code)]
    ConfirmInstall { items: Vec<PackageItem> },
    /// Preflight summary before executing any action.
    Preflight {
        items: Vec<PackageItem>,
        action: PreflightAction,
        tab: PreflightTab,
        /// Resolved dependency information (populated when Deps tab is accessed).
        dependency_info: Vec<DependencyInfo>,
        /// Selected index in the dependency list (for navigation).
        dep_selected: usize,
        /// Set of dependency names with expanded tree nodes (for tree view).
        dep_tree_expanded: HashSet<String>,
        /// File information (populated when Files tab is accessed).
        file_info: Vec<PackageFileInfo>,
        /// Selected index in the file list (for navigation).
        file_selected: usize,
        /// Set of package names with expanded file lists (for Files tab tree view).
        file_tree_expanded: HashSet<String>,
        /// Current cascade removal strategy for this session.
        cascade_mode: CascadeMode,
    },
    /// Preflight execution screen with log and sticky sidebar.
    #[allow(dead_code)]
    PreflightExec {
        items: Vec<PackageItem>,
        action: PreflightAction,
        tab: PreflightTab,
        verbose: bool,
        log_lines: Vec<String>,
        abortable: bool,
    },
    /// Post-transaction summary with results and follow-ups.
    PostSummary {
        success: bool,
        changed_files: usize,
        pacnew_count: usize,
        pacsave_count: usize,
        services_pending: Vec<String>,
        snapshot_label: Option<String>,
    },
    /// Help overlay with keybindings. Non-interactive; dismissed with Esc/Enter.
    Help,
    /// Confirmation dialog for removing the given items.
    #[allow(dead_code)]
    ConfirmRemove { items: Vec<PackageItem> },
    /// System update dialog with multi-select options and optional country.
    SystemUpdate {
        /// Whether to update Arch mirrors using reflector.
        do_mirrors: bool,
        /// Whether to update system packages via pacman.
        do_pacman: bool,
        /// Whether to update AUR packages via paru/yay.
        do_aur: bool,
        /// Whether to remove caches (pacman and AUR helper).
        do_cache: bool,
        /// Index into `countries` for the reflector `--country` argument.
        country_idx: usize,
        /// Available countries to choose from for reflector.
        countries: Vec<String>,
        /// Requested mirror count to fetch/rank.
        mirror_count: u16,
        /// Cursor row in the dialog (0..=4)
        cursor: usize,
    },
    /// Arch Linux News: list of recent items with selection.
    News {
        /// Latest news items (date, title, link).
        items: Vec<NewsItem>,
        /// Selected row index.
        selected: usize,
    },
    /// TUI Optional Dependencies chooser: selectable rows with install status.
    OptionalDeps {
        /// Rows to display (pre-filtered by environment/distro).
        rows: Vec<OptionalDepRow>,
        /// Selected row index.
        selected: usize,
    },
    /// Select which scans to run before executing the AUR scan.
    ScanConfig {
        /// Whether to run ClamAV (clamscan).
        do_clamav: bool,
        /// Whether to run Trivy filesystem scan.
        do_trivy: bool,
        /// Whether to run Semgrep static analysis.
        do_semgrep: bool,
        /// Whether to run ShellCheck on PKGBUILD/.install.
        do_shellcheck: bool,
        /// Whether to run VirusTotal hash lookups.
        do_virustotal: bool,
        /// Whether to run custom suspicious-pattern scan (PKGBUILD/.install).
        do_custom: bool,
        /// Whether to run aur-sleuth (LLM audit).
        do_sleuth: bool,
        /// Cursor row in the dialog.
        cursor: usize,
    },
    /// Prompt to install GNOME Terminal at startup on GNOME when not present.
    GnomeTerminalPrompt,
    /// Setup dialog for VirusTotal API key.
    VirusTotalSetup {
        /// User-entered API key buffer.
        input: String,
        /// Cursor position within the input buffer.
        cursor: usize,
    },
    /// Information dialog explaining the Import file format.
    ImportHelp,
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Confirm each `Modal` variant can be constructed and the `Default` implementation returns `Modal::None`.
    ///
    /// Inputs:
    /// - No external inputs; instantiates representative variants directly inside the test.
    ///
    /// Output:
    /// - Ensures `Default::default()` yields `Modal::None` and variant constructors remain stable.
    ///
    /// Details:
    /// - Acts as a regression guard when fields or defaults change, catching compile-time or panicking construction paths.
    fn modal_default_and_variants_construct() {
        let m: super::Modal = Default::default();
        matches!(m, super::Modal::None);
        let _ = super::Modal::Alert {
            message: "hi".into(),
        };
        let _ = super::Modal::ConfirmInstall { items: Vec::new() };
        let _ = super::Modal::Help;
        let _ = super::Modal::ConfirmRemove { items: Vec::new() };
        let _ = super::Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: true,
            do_aur: true,
            do_cache: false,
            country_idx: 0,
            countries: vec!["US".into()],
            mirror_count: 20,
            cursor: 0,
        };
        let _ = super::Modal::News {
            items: Vec::new(),
            selected: 0,
        };
        let _ = super::Modal::OptionalDeps {
            rows: Vec::new(),
            selected: 0,
        };
        let _ = super::Modal::GnomeTerminalPrompt;
        let _ = super::Modal::VirusTotalSetup {
            input: String::new(),
            cursor: 0,
        };
        let _ = super::Modal::ImportHelp;
        let _ = super::Modal::Preflight {
            items: Vec::new(),
            action: super::PreflightAction::Install,
            tab: super::PreflightTab::Summary,
            dependency_info: Vec::new(),
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: std::collections::HashSet::new(),
            cascade_mode: super::CascadeMode::Basic,
        };
    }
}
