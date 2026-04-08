//! Modal dialog state for the UI.

use crate::sources::VoteAction;
use crate::state::types::{OptionalDepRow, PackageItem, RepositoryModalRow, Source};
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
    /// Install packages action.
    Install,
    /// Remove packages action.
    Remove,
    /// Downgrade packages action.
    Downgrade,
}

/// What: Purpose for password prompt.
///
/// Inputs:
/// - Set when showing password prompt modal.
///
/// Output:
/// - Used to customize prompt message and context.
///
/// Details:
/// - Indicates which operation requires sudo authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordPurpose {
    /// Installing packages.
    Install,
    /// Removing packages.
    Remove,
    /// Updating system.
    Update,
    /// Downgrading packages.
    Downgrade,
    /// Syncing file database.
    FileSync,
    /// Applying custom repository configuration (`repos.conf` → managed drop-in, keys).
    RepoApply,
    /// Migrating foreign packages to sync after repository overlap detection.
    RepoForeignMigrate,
}

/// What: Remembers which repository section was fully applied so a follow-up overlap scan can run.
///
/// Inputs:
/// - Set in [`crate::events::modals::repositories::enter_repo_apply`] when a full apply is queued.
///
/// Output:
/// - Consumed after successful `PreflightExec` when the user dismisses the log (Enter).
///
/// Details:
/// - `repo_section` is normalized lowercase for `pacman -Sl`.
/// - `pre_apply_foreign_snapshot` is `pacman -Qm` captured when apply was **queued** (before sudo/sync).
///   After the repo is enabled, pacman may stop classifying matching installs as foreign; the snapshot
///   preserves the pre-apply set for overlap detection without disabling repositories globally.
#[derive(Debug, Clone)]
pub struct RepoOverlapApplyPending {
    /// Pacman repository name from the applied `[[repo]]` row.
    pub repo_section: String,
    /// Foreign packages from `pacman -Qm` before privileged apply commands ran.
    ///
    /// `None` if the snapshot failed (overlap analysis falls back to live `-Qm` at completion).
    pub pre_apply_foreign_snapshot: Option<Vec<(String, String)>>,
}

/// What: Restore the Repositories modal after a successful repo apply preflight flow ends.
///
/// Inputs:
/// - Set when queuing [`PasswordPurpose::RepoApply`] (full apply, not key-only refresh).
///
/// Output:
/// - Consumed when the UI returns to `Modal::None` and rescans pacman rows.
///
/// Details:
/// - `section_name` selects the same `[[repo]]` row after refresh; `scroll` is re-clamped to the new row count.
#[derive(Debug, Clone)]
pub struct RepositoriesModalResume {
    /// Pacman `[repo]` section name from the row that was focused when apply started.
    pub section_name: String,
    /// First visible list index to restore when possible.
    pub scroll: u16,
}

/// What: Step state for the post-apply foreign vs sync overlap workflow.
///
/// Inputs:
/// - Owned by [`Modal::ForeignRepoOverlap`].
///
/// Output:
/// - Drives which screen and scroll position the renderer shows.
///
/// Details:
/// - `WarnAck` uses two substeps before package selection; `Select` supports multi-toggle migration targets.
#[derive(Debug, Clone)]
pub enum ForeignRepoOverlapPhase {
    /// Red warning screens (`step` 0 then 1) before package selection.
    WarnAck {
        /// `0` = primary warning, `1` = secondary acknowledgment.
        step: u8,
        /// Vertical scroll for the overlap list on warning steps 0 and 1.
        list_scroll: u16,
    },
    /// Multi-select which overlapping packages to migrate (Space toggles).
    Select {
        /// Focused row index into `entries`.
        cursor: usize,
        /// Scroll for the selectable list.
        list_scroll: u16,
        /// Selected package names to migrate.
        selected: HashSet<String>,
    },
    /// Final confirmation before the password prompt (Esc returns to [`Self::Select`]).
    FinalConfirm {
        /// Cursor to restore when backing out.
        select_cursor: usize,
        /// Scroll to restore when backing out.
        select_scroll: u16,
        /// Packages slated for `pacman -Rns` / `pacman -S`.
        selected: HashSet<String>,
    },
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
    /// Summary tab showing overview of package operations.
    Summary,
    /// Dependencies tab showing dependency analysis.
    Deps,
    /// Files tab showing file change analysis.
    Files,
    /// Services tab showing service impact analysis.
    Services,
    /// Sandbox tab showing sandbox analysis.
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

/// What: Step identifier for the guided AUR SSH setup workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshSetupStep {
    /// Intro/instructions step before executing the setup flow.
    Intro,
    /// Confirmation step shown when an existing host block needs overwrite approval.
    ConfirmOverwrite,
    /// Result step containing final status lines.
    Result,
}

/// What: Selectable startup setup tasks presented in the first-run setup selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StartupSetupTask {
    /// Configure startup Arch/feeds news preferences.
    ArchNews,
    /// Configure SSH for AUR vote/unvote operations.
    SshAurSetup,
    /// Review and install missing optional dependencies.
    OptionalDepsMissing,
    /// Optional wizard: extend `sudo` credential cache for long installs/updates (`sudoers`).
    SudoTimestampSetup,
    /// Configure aur-sleuth integration.
    AurSleuthSetup,
    /// Configure `VirusTotal` API key.
    VirusTotalSetup,
}

/// What: User-selected `sudo` credential cache duration in the optional setup wizard.
///
/// Inputs:
/// - Chosen by the user in [`Modal::SudoTimestampSetup`].
///
/// Output:
/// - Maps to `timestamp_timeout` minutes in `sudoers`, or `-1` for no expiry in the session.
///
/// Details:
/// - See `sudoers(5)` `timestamp_timeout`. This only affects policy once applied on the system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoTimestampChoice {
    /// Cache sudo credentials for ten minutes after a successful password prompt.
    TenMinutes,
    /// Cache sudo credentials for thirty minutes after a successful password prompt.
    ThirtyMinutes,
    /// Use `timestamp_timeout=-1` (do not expire until `sudo -k` or reboot, per sudo policy).
    Infinity,
}

/// What: Active phase of the sudo timestamp setup wizard.
///
/// Inputs:
/// - Driven by key events in the sudo timestamp setup handler.
///
/// Output:
/// - Tells the renderer whether to show the option list or the instruction pane.
///
/// Details:
/// - The select phase uses [`SudoTimestampSetupModalState::select_cursor`]; instructions carry their own scroll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SudoTimestampSetupPhase {
    /// User is picking a recommended `timestamp_timeout` or skipping.
    Select,
    /// User is reading copy-paste / terminal instructions for the chosen option.
    Instructions {
        /// Selected duration mapping.
        choice: SudoTimestampChoice,
        /// Vertical scroll offset in lines for long instruction text.
        scroll: u16,
    },
}

/// What: Stateful fields for [`Modal::SudoTimestampSetup`].
///
/// Inputs:
/// - Constructed when opening the wizard from optional deps or startup setup.
///
/// Output:
/// - Updated by the sudo timestamp setup key handler and read by the renderer.
///
/// Details:
/// - `select_cursor` is kept when switching to instructions so Esc returns to the same row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SudoTimestampSetupModalState {
    /// Current wizard phase.
    pub phase: SudoTimestampSetupPhase,
    /// Row index in the select phase (`0..SUDO_TIMESTAMP_SELECT_ROWS`).
    pub select_cursor: usize,
}

/// Row count for [`SudoTimestampSetupModalState::select_cursor`] (10m, 30m, infinity, skip).
pub const SUDO_TIMESTAMP_SELECT_ROWS: usize = 4;

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
    Installed {
        /// Installed version of the package.
        version: String,
    },
    /// Not installed, needs to be installed.
    ToInstall,
    /// Installed but outdated, needs upgrade.
    ToUpgrade {
        /// Current installed version.
        current: String,
        /// Required version for upgrade.
        required: String,
    },
    /// Conflicts with existing packages.
    Conflict {
        /// Reason for the conflict.
        reason: String,
    },
    /// Cannot be found in configured repositories or AUR.
    Missing,
}

/// Source of a dependency package.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DependencySource {
    /// Official repository package.
    Official {
        /// Repository name (e.g., "core", "extra").
        repo: String,
    },
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
    /// Low risk level.
    Low,
    /// Medium risk level.
    Medium,
    /// High risk level.
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
    /// Number of packages in the operation.
    pub package_count: usize,
    /// Total download size in bytes.
    pub download_bytes: u64,
    /// Net change in installed size in bytes (positive for installs, negative for removals).
    pub install_delta_bytes: i64,
    /// Number of AUR packages in the operation.
    pub aur_count: usize,
    /// Risk score (0-255) computed from various risk factors.
    pub risk_score: u8,
    /// Risk level category (Low/Medium/High).
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
    /// Package name.
    pub name: String,
    /// Package source (official/AUR/local).
    pub source: Source,
    /// Installed version, if present.
    pub installed_version: Option<String>,
    /// Target version to be installed.
    pub target_version: String,
    /// Whether the operation downgrades the package.
    pub is_downgrade: bool,
    /// Whether the update is a major version bump.
    pub is_major_bump: bool,
    /// Download size contribution for this package when available.
    pub download_bytes: Option<u64>,
    /// Net installed size delta contributed by this package (signed).
    pub install_delta_bytes: Option<i64>,
    /// Notes or warnings specific to this package.
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
    /// Per-package summaries for the operation.
    pub packages: Vec<PreflightPackageSummary>,
    /// Total number of packages represented in `packages`.
    pub package_count: usize,
    /// Number of AUR-sourced packages participating in the plan.
    pub aur_count: usize,
    /// Total download size for the plan.
    pub download_bytes: u64,
    /// Net install size delta for the plan (signed).
    pub install_delta_bytes: i64,
    /// Aggregate risk score (0-255).
    pub risk_score: u8,
    /// Aggregate risk level (Low/Medium/High).
    pub risk_level: RiskLevel,
    /// Reasons contributing to the risk score.
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
    /// Notes summarizing key items in the plan.
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
    /// No modal is currently displayed.
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert {
        /// Alert message text.
        message: String,
    },
    /// Loading indicator shown during background computation.
    Loading {
        /// Loading message text.
        message: String,
    },
    /// Confirmation dialog for installing the given items.
    ConfirmInstall {
        /// Package items to install.
        items: Vec<PackageItem>,
    },
    /// Confirmation dialog for reinstalling already installed packages.
    ConfirmReinstall {
        /// Packages that are already installed (shown in the confirmation dialog).
        items: Vec<PackageItem>,
        /// All packages to install (including both installed and not installed).
        all_items: Vec<PackageItem>,
        /// Header chip metrics for the operation.
        header_chips: PreflightHeaderChips,
    },
    /// Confirmation dialog for batch updates that may cause dependency conflicts.
    ConfirmBatchUpdate {
        /// Package items to update.
        items: Vec<PackageItem>,
        /// Whether this is a dry-run operation.
        dry_run: bool,
    },
    /// Confirmation dialog for continuing AUR update after pacman failed.
    ConfirmAurUpdate {
        /// Message explaining the situation.
        message: String,
    },
    /// Warning: AUR install targets also appear as official/sync rows in current results.
    WarnAurRepoDuplicate {
        /// `pkgname` values that are both AUR-selected and present as official rows.
        dup_names: Vec<String>,
        /// Full install set to resume after continue.
        packages: Vec<PackageItem>,
        /// Preflight header chips to restore [`handle_proceed_install`] context.
        header_chips: PreflightHeaderChips,
    },
    /// Post full repo-apply: foreign packages that share a name with the new sync repository.
    ForeignRepoOverlap {
        /// Repository that was applied (for copy and `pacman -Sl`).
        repo_name: String,
        /// Overlapping `(pkgname, installed version)` rows sorted by name.
        entries: Vec<(String, String)>,
        /// Current wizard phase.
        phase: ForeignRepoOverlapPhase,
    },
    /// Confirmation dialog for AUR vote/unvote actions.
    ConfirmAurVote {
        /// AUR package base the action targets.
        pkgbase: String,
        /// Vote action to execute on confirmation.
        action: VoteAction,
        /// Confirmation message shown to the user.
        message: String,
    },
    /// Preflight summary before executing any action.
    Preflight {
        /// Packages selected for the operation.
        items: Vec<PackageItem>,
        /// Action to perform (install/remove/downgrade).
        action: PreflightAction,
        /// Currently active preflight tab.
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
        /// Cached reverse dependency report for Remove actions (populated during summary computation).
        /// This avoids redundant resolution when switching to the Deps tab.
        cached_reverse_deps_report: Option<crate::logic::deps::ReverseDependencyReport>,
    },
    /// Preflight execution screen with log and sticky sidebar.
    PreflightExec {
        /// Packages being processed.
        items: Vec<PackageItem>,
        /// Action being executed (install/remove/downgrade).
        action: PreflightAction,
        /// Tab to display while executing.
        tab: PreflightTab,
        /// Whether verbose logging is enabled.
        verbose: bool,
        /// Execution log lines.
        log_lines: Vec<String>,
        /// Whether the operation can be aborted.
        abortable: bool,
        /// Header chip metrics displayed in the sidebar.
        header_chips: PreflightHeaderChips,
        /// Execution result: `Some(true)` for success, `Some(false)` for failure, `None` if not yet completed.
        success: Option<bool>,
    },
    /// Post-transaction summary with results and follow-ups.
    PostSummary {
        /// Whether the operation succeeded.
        success: bool,
        /// Number of files changed.
        changed_files: usize,
        /// Number of .pacnew files created.
        pacnew_count: usize,
        /// Number of .pacsave files created.
        pacsave_count: usize,
        /// Services pending restart.
        services_pending: Vec<String>,
        /// Snapshot label if created.
        snapshot_label: Option<String>,
    },
    /// Help overlay with keybindings. Non-interactive; dismissed with Esc/Enter.
    Help,
    /// Confirmation dialog for removing the given items.
    ConfirmRemove {
        /// Package items to remove.
        items: Vec<PackageItem>,
    },
    /// System update dialog with multi-select options and optional country.
    SystemUpdate {
        /// Whether to update Arch mirrors using reflector.
        do_mirrors: bool,
        /// Whether to update system packages via pacman.
        do_pacman: bool,
        /// Whether to force sync databases (pacman -Syyu instead of -Syu).
        force_sync: bool,
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
        /// Latest news feed items (Arch news, advisories, updates, comments).
        items: Vec<crate::state::types::NewsFeedItem>,
        /// Selected row index.
        selected: usize,
        /// Scroll offset (lines) for the news list.
        scroll: u16,
    },
    /// Application announcement: markdown content displayed at startup.
    Announcement {
        /// Title to display in the modal header.
        title: String,
        /// Markdown content to display.
        content: String,
        /// Unique identifier for this announcement (version string or remote ID).
        id: String,
        /// Scroll offset (lines) for long content.
        scroll: u16,
    },
    /// Available package updates: list of update entries with scroll support.
    Updates {
        /// Update entries with package name, old version, and new version.
        entries: Vec<(String, String, String)>, // (name, old_version, new_version)
        /// Scroll offset (lines) for the updates list.
        scroll: u16,
        /// Selected row index.
        selected: usize,
        /// Whether slash-filter text mode is active.
        filter_active: bool,
        /// Current slash-filter query text.
        filter_query: String,
        /// Caret position (character index) within `filter_query`.
        filter_caret: usize,
        /// Last selected package identity used for restoration across filter changes.
        last_selected_pkg_name: Option<String>,
        /// Visible updates rows as original-entry indices after applying filter.
        filtered_indices: Vec<usize>,
        /// Selected package names for batch preflight actions.
        selected_pkg_names: HashSet<String>,
    },
    /// TUI Optional Dependencies chooser: selectable rows with install status.
    OptionalDeps {
        /// Rows to display (pre-filtered by environment/distro).
        rows: Vec<OptionalDepRow>,
        /// Selected row index.
        selected: usize,
    },
    /// Read-only Repositories viewer: `repos.conf` vs live `pacman.conf` / includes.
    Repositories {
        /// Merged rows for each configured `[[repo]]`.
        rows: Vec<RepositoryModalRow>,
        /// Selected row index.
        selected: usize,
        /// Scroll offset: index of first visible data row.
        scroll: u16,
        /// Error loading or parsing `repos.conf`, if any.
        repos_conf_error: Option<String>,
        /// Warnings from reading `pacman.conf` or includes.
        pacman_warnings: Vec<String>,
    },
    /// Guided SSH setup workflow for AUR voting.
    SshAurSetup {
        /// Active setup step in the wizard-like flow.
        step: SshSetupStep,
        /// Status and instruction lines shown in the modal body.
        status_lines: Vec<String>,
        /// Existing `Host aur.archlinux.org` block shown when overwrite confirmation is required.
        existing_host_block: Option<String>,
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
    /// Optional wizard: configure `sudo` `timestamp_timeout` via `sudoers` drop-in (instructions / terminal).
    SudoTimestampSetup {
        /// Wizard phase and cursor state.
        setup: SudoTimestampSetupModalState,
    },
    /// Information dialog explaining the Import file format.
    ImportHelp,
    /// Setup dialog for startup news popup configuration.
    NewsSetup {
        /// Whether to show Arch news.
        show_arch_news: bool,
        /// Whether to show security advisories.
        show_advisories: bool,
        /// Whether to show AUR updates.
        show_aur_updates: bool,
        /// Whether to show AUR comments.
        show_aur_comments: bool,
        /// Whether to show official package updates.
        show_pkg_updates: bool,
        /// Maximum age of news items in days (7, 30, or 90).
        max_age_days: Option<u32>,
        /// Current cursor position (0-5 for toggles, 6-8 for date buttons).
        cursor: usize,
    },
    /// First-startup selector for choosing which setup flows to run.
    StartupSetupSelector {
        /// Currently highlighted row index.
        cursor: usize,
        /// Selected startup setup tasks to execute.
        selected: std::collections::HashSet<StartupSetupTask>,
    },
    /// Password prompt for sudo authentication.
    PasswordPrompt {
        /// Purpose of the password prompt.
        purpose: PasswordPurpose,
        /// Packages involved in the operation.
        items: Vec<PackageItem>,
        /// User input buffer for password.
        input: String,
        /// Cursor position within the input buffer.
        cursor: usize,
        /// Error message if password was incorrect.
        error: Option<String>,
    },
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
        let _ = super::Modal::ConfirmReinstall {
            items: Vec::new(),
            all_items: Vec::new(),
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
        };
        let _ = super::Modal::Help;
        let _ = super::Modal::ConfirmRemove { items: Vec::new() };
        let _ = super::Modal::SystemUpdate {
            do_mirrors: true,
            do_pacman: true,
            force_sync: false,
            do_aur: true,
            do_cache: false,
            country_idx: 0,
            countries: vec!["US".into()],
            mirror_count: 20,
            cursor: 0,
        };
        let _ = super::Modal::ConfirmAurVote {
            pkgbase: "pacsea-bin".into(),
            action: crate::sources::VoteAction::Vote,
            message: "confirm".into(),
        };
        let _ = super::Modal::WarnAurRepoDuplicate {
            dup_names: vec!["foo".into()],
            packages: Vec::new(),
            header_chips: super::PreflightHeaderChips::default(),
        };
        let _ = super::Modal::ForeignRepoOverlap {
            repo_name: "extra".into(),
            entries: vec![("a".into(), "1-1".into())],
            phase: super::ForeignRepoOverlapPhase::FinalConfirm {
                select_cursor: 0,
                select_scroll: 0,
                selected: std::collections::HashSet::new(),
            },
        };
        let _ = super::Modal::News {
            items: Vec::new(),
            selected: 0,
            scroll: 0,
        };
        let _ = super::Modal::Updates {
            entries: vec![("pkg".into(), "1".into(), "2".into())],
            scroll: 0,
            selected: 0,
            filter_active: false,
            filter_query: String::new(),
            filter_caret: 0,
            last_selected_pkg_name: None,
            filtered_indices: vec![0],
            selected_pkg_names: std::collections::HashSet::new(),
        };
        let _ = super::Modal::OptionalDeps {
            rows: Vec::new(),
            selected: 0,
        };
        let _ = super::Modal::Repositories {
            rows: Vec::new(),
            selected: 0,
            scroll: 0,
            repos_conf_error: None,
            pacman_warnings: Vec::new(),
        };
        let _ = super::Modal::SshAurSetup {
            step: super::SshSetupStep::Intro,
            status_lines: Vec::new(),
            existing_host_block: None,
        };
        let _ = super::Modal::GnomeTerminalPrompt;
        let _ = super::Modal::VirusTotalSetup {
            input: String::new(),
            cursor: 0,
        };
        let _ = super::Modal::SudoTimestampSetup {
            setup: super::SudoTimestampSetupModalState {
                phase: super::SudoTimestampSetupPhase::Select,
                select_cursor: 0,
            },
        };
        let _ = super::Modal::ImportHelp;
        let _ = super::Modal::PasswordPrompt {
            purpose: super::PasswordPurpose::Install,
            items: Vec::new(),
            input: String::new(),
            cursor: 0,
            error: None,
        };
        let _ = super::Modal::PasswordPrompt {
            purpose: super::PasswordPurpose::RepoForeignMigrate,
            items: Vec::new(),
            input: String::new(),
            cursor: 0,
            error: None,
        };
        let _ = super::Modal::StartupSetupSelector {
            cursor: 0,
            selected: std::collections::HashSet::new(),
        };
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
            cached_reverse_deps_report: None,
        };
    }
}
