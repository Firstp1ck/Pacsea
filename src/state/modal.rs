//! Modal dialog state for the UI.

use crate::state::types::{NewsItem, PackageItem};

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
