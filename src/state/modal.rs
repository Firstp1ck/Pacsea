//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, PackageItem};

#[derive(Debug, Clone, Copy)]
pub enum PreflightAction {
    Install,
    Remove,
}

#[derive(Debug, Clone, Copy)]
pub enum PreflightTab {
    Summary,
    Deps,
    Files,
    Services,
    Sandbox,
}

#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert { message: String },
    /// Confirmation dialog for installing the given items.
    ConfirmInstall { items: Vec<PackageItem> },
    /// Preflight summary before executing any action.
    Preflight { items: Vec<PackageItem>, action: PreflightAction, tab: PreflightTab },
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
}
