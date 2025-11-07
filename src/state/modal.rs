//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, OptionalDepRow, PackageItem};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightAction {
    Install,
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightTab {
    Summary,
    Deps,
    Files,
    Services,
    Sandbox,
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
    },
    /// Preflight execution screen with log and sticky sidebar.
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
        };
    }
}
