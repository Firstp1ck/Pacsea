//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, OptionalDepRow, PackageItem};

#[derive(Debug, Clone, Default)]
pub enum Modal {
    #[default]
    None,
    /// Informational alert with a non-interactive message.
    Alert { message: String },
    /// Confirmation dialog for installing the given items.
    ConfirmInstall { items: Vec<PackageItem> },
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
    }
}
