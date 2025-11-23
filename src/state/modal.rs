//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, OptionalDepRow, PackageItem, Source};
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

/// Removal cascade strategy for `pacman` operations.
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
    /// Return the `pacman` flag sequence corresponding to this `CascadeMode`.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Basic => "-R",
            Self::Cascade => "-Rs",
            Self::CascadeWithConfigs => "-Rns",
        }
    }

    /// Short text describing the effect of this `CascadeMode`.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Basic => "targets only",
            Self::Cascade => "remove dependents",
            Self::CascadeWithConfigs => "dependents + configs",
        }
    }

    /// Whether this `CascadeMode` allows removal when dependents exist.
    #[must_use]
    pub const fn allows_dependents(self) -> bool {
        !matches!(self, Self::Basic)
    }

    /// Cycle to the next `CascadeMode`.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Basic => Self::Cascade,
            Self::Cascade => Self::CascadeWithConfigs,
            Self::CascadeWithConfigs => Self::Basic,
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

/// What: Restart preference applied to an impacted `systemd` service.
///
/// Inputs:
/// - Assigned automatically from heuristics or by user toggles within the Services tab.
///
/// Output:
/// - Guides post-transaction actions responsible for restarting (or deferring) service units.
///
/// Details:
/// - Provides a simplified binary choice: restart immediately or defer for later manual handling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ServiceRestartDecision {
    /// Explicitly restart the unit after the transaction.
    Restart,
    /// Defer restarting the unit.
    Defer,
}

/// What: Aggregated information about a `systemd` unit affected by the pending operation.
///
/// Inputs:
/// - Populated by the service impact resolver which correlates package file lists and
///   `systemctl` state.
///
/// Output:
/// - Supplies UI rendering with package provenance, restart status, and the current user choice.
///
/// Details:
/// - `providers` lists packages that ship the unit. `is_active` flags if the unit currently runs.
///   `needs_restart` indicates detected impact. `recommended_decision` records the resolver default,
///   and `restart_decision` reflects any user override applied in the UI.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ServiceImpact {
    /// Fully-qualified unit name (e.g., `sshd.service`).
    pub unit_name: String,
    /// Packages contributing this unit.
    pub providers: Vec<String>,
    /// Whether the unit is active (`systemctl is-active == active`).
    pub is_active: bool,
    /// Whether a restart is recommended because files/configs will change.
    pub needs_restart: bool,
    /// Resolver-suggested action prior to user adjustments.
    pub recommended_decision: ServiceRestartDecision,
    /// Restart decision currently applied to the unit (may differ from recommendation).
    pub restart_decision: ServiceRestartDecision,
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

/// What: Risk severity buckets used by the preflight summary header and messaging.
///
/// Inputs:
/// - Assigned by the summary resolver based on aggregate risk score thresholds.
///
/// Output:
/// - Guides color selection and descriptive labels for risk indicators across the UI.
///
/// Details:
/// - Defaults to `Low` so callers without computed risk can render a safe baseline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl Default for RiskLevel {
    /// What: Provide a baseline risk level when no assessment has been computed yet.
    ///
    /// Inputs: None.
    ///
    /// Output: Always returns `RiskLevel::Low`.
    ///
    /// Details:
    /// - Keeps `Default` implementations for composite structs simple while biasing towards safety.
    fn default() -> Self {
        Self::Low
    }
}

/// What: Aggregated chip metrics displayed in the Preflight header, execution sidebar, and post-summary.
///
/// Inputs:
/// - Populated by the summary planner once package metadata and risk scores are available.
///
/// Output:
/// - Supplies counts and byte deltas for UI components needing condensed statistics.
///
/// Details:
/// - Stores signed install deltas so removals show negative values without additional conversion.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreflightHeaderChips {
    pub package_count: usize,
    pub download_bytes: u64,
    pub install_delta_bytes: i64,
    pub aur_count: usize,
    pub risk_score: u8,
    pub risk_level: RiskLevel,
}

impl Default for PreflightHeaderChips {
    /// What: Provide neutral header chip values prior to summary computation.
    ///
    /// Inputs: None.
    ///
    /// Output: Returns a struct with zeroed counters and low risk classification.
    ///
    /// Details:
    /// - Facilitates cheap initialization for modals created before async planners finish.
    fn default() -> Self {
        Self {
            package_count: 0,
            download_bytes: 0,
            install_delta_bytes: 0,
            aur_count: 0,
            risk_score: 0,
            risk_level: RiskLevel::Low,
        }
    }
}

/// What: Version comparison details for a single package in the preflight summary.
///
/// Inputs:
/// - Filled with installed and target versions, plus classification flags.
///
/// Output:
/// - Enables the UI to display per-package version deltas, major bumps, and downgrade warnings.
///
/// Details:
/// - Notes array allows the planner to surface auxiliary hints (e.g., pacnew prediction or service impacts).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreflightPackageSummary {
    pub name: String,
    pub source: Source,
    pub installed_version: Option<String>,
    pub target_version: String,
    pub is_downgrade: bool,
    pub is_major_bump: bool,
    /// Download size contribution for this package when available.
    pub download_bytes: Option<u64>,
    /// Net installed size delta contributed by this package (signed).
    pub install_delta_bytes: Option<i64>,
    pub notes: Vec<String>,
}

/// What: Comprehensive dataset backing the Preflight Summary tab.
///
/// Inputs:
/// - Populated by summary resolution logic once package metadata, sizes, and risk heuristics are computed.
///
/// Output:
/// - Delivers structured information for tab body rendering, risk callouts, and contextual notes.
///
/// Details:
/// - `summary_notes` aggregates high-impact bullet points (e.g., kernel updates, pacnew predictions).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PreflightSummaryData {
    pub packages: Vec<PreflightPackageSummary>,
    /// Total number of packages represented in `packages`.
    pub package_count: usize,
    /// Number of AUR-sourced packages participating in the plan.
    pub aur_count: usize,
    pub download_bytes: u64,
    pub install_delta_bytes: i64,
    pub risk_score: u8,
    pub risk_level: RiskLevel,
    pub risk_reasons: Vec<String>,
    /// Packages classified as major version bumps (e.g., 1.x -> 2.0).
    pub major_bump_packages: Vec<String>,
    /// Core/system packages flagged as high impact (kernel, glibc, etc.).
    pub core_system_updates: Vec<String>,
    /// Total predicted .pacnew files across all packages.
    pub pacnew_candidates: usize,
    /// Total predicted .pacsave files across all packages.
    pub pacsave_candidates: usize,
    /// Packages with configuration merge warnings (.pacnew expected).
    pub config_warning_packages: Vec<String>,
    /// Services likely requiring restart after the transaction.
    pub service_restart_units: Vec<String>,
    /// Free-form warnings assembled by the summary planner to highlight notable risks.
    pub summary_warnings: Vec<String>,
    pub summary_notes: Vec<String>,
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
#[allow(clippy::large_enum_variant)]
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
        /// Aggregated summary information for versions, sizes, and risk cues.
        summary: Option<Box<PreflightSummaryData>>,
        /// Scroll offset (lines) for the Summary tab content (mouse scrolling only).
        summary_scroll: u16,
        /// Header chip data shared across summary, execution, and post-summary screens.
        header_chips: PreflightHeaderChips,
        /// Resolved dependency information (populated when Deps tab is accessed).
        dependency_info: Vec<DependencyInfo>,
        /// Selected index in the dependency list (for navigation).
        dep_selected: usize,
        /// Set of dependency names with expanded tree nodes (for tree view).
        dep_tree_expanded: HashSet<String>,
        /// Error message from dependency resolution, if any.
        deps_error: Option<String>,
        /// File information (populated when Files tab is accessed).
        file_info: Vec<PackageFileInfo>,
        /// Selected index in the file list (for navigation).
        file_selected: usize,
        /// Set of package names with expanded file lists (for Files tab tree view).
        file_tree_expanded: HashSet<String>,
        /// Error message from file resolution, if any.
        files_error: Option<String>,
        /// Service impact information (populated when Services tab is accessed).
        service_info: Vec<ServiceImpact>,
        /// Selected index in the service impact list (for navigation).
        service_selected: usize,
        /// Whether service impacts have been resolved for the current session.
        services_loaded: bool,
        /// Error message from service resolution, if any.
        services_error: Option<String>,
        /// Sandbox information for AUR packages (populated when Sandbox tab is accessed).
        sandbox_info: Vec<crate::logic::sandbox::SandboxInfo>,
        /// Selected index in the sandbox display list (for navigation - can be package or dependency).
        sandbox_selected: usize,
        /// Set of package names with expanded dependency lists (for Sandbox tab tree view).
        sandbox_tree_expanded: HashSet<String>,
        /// Whether sandbox info has been resolved for the current session.
        sandbox_loaded: bool,
        /// Error message from sandbox resolution, if any.
        sandbox_error: Option<String>,
        /// Selected optional dependencies to install with their packages.
        /// Maps package name -> set of selected optional dependency names.
        selected_optdepends: std::collections::HashMap<String, std::collections::HashSet<String>>,
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
        /// Header chip metrics displayed in the sidebar.
        header_chips: PreflightHeaderChips,
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
    /// Available package updates: list of update entries with scroll support.
    Updates {
        /// Update entries with package name, old version, and new version.
        entries: Vec<(String, String, String)>, // (name, old_version, new_version)
        /// Scroll offset (lines) for the updates list.
        scroll: u16,
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
        /// Whether to run `ClamAV` (clamscan).
        do_clamav: bool,
        /// Whether to run Trivy filesystem scan.
        do_trivy: bool,
        /// Whether to run Semgrep static analysis.
        do_semgrep: bool,
        /// Whether to run `ShellCheck` on `PKGBUILD`/.install.
        do_shellcheck: bool,
        /// Whether to run `VirusTotal` hash lookups.
        do_virustotal: bool,
        /// Whether to run custom suspicious-pattern scan (PKGBUILD/.install).
        do_custom: bool,
        /// Whether to run aur-sleuth (LLM audit).
        do_sleuth: bool,
        /// Cursor row in the dialog.
        cursor: usize,
    },
    /// Prompt to install `GNOME Terminal` at startup on GNOME when not present.
    GnomeTerminalPrompt,
    /// Setup dialog for `VirusTotal` API key.
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
        let m = super::Modal::default();
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
            summary: None,
            summary_scroll: 0,
            header_chips: super::PreflightHeaderChips::default(),
            dependency_info: Vec::new(),
            dep_selected: 0,
            dep_tree_expanded: std::collections::HashSet::new(),
            deps_error: None,
            file_info: Vec::new(),
            file_selected: 0,
            file_tree_expanded: std::collections::HashSet::new(),
            files_error: None,
            service_info: Vec::new(),
            service_selected: 0,
            services_loaded: false,
            services_error: None,
            sandbox_info: Vec::new(),
            sandbox_selected: 0,
            sandbox_tree_expanded: std::collections::HashSet::new(),
            sandbox_loaded: false,
            sandbox_error: None,
            selected_optdepends: std::collections::HashMap::new(),
            cascade_mode: super::CascadeMode::Basic,
        };
    }
}
