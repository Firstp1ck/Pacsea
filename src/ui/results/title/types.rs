/// What: Pre-computed i18n strings for title rendering.
///
/// Inputs: Individual i18n strings from `AppState`.
///
/// Output: Struct containing all i18n strings needed for title rendering.
///
/// Details: Reduces data flow complexity by pre-computing all i18n strings upfront.
pub(super) struct TitleI18nStrings {
    /// Translated "Results" title text.
    pub(super) results_title: String,
    /// Translated sort button text.
    pub(super) sort_button: String,
    /// Translated options button text.
    pub(super) options_button: String,
    /// Translated panels button text.
    pub(super) panels_button: String,
    /// Translated config button text.
    pub(super) config_button: String,
    /// Translated menu button text.
    pub(super) menu_button: String,
    /// Translated AUR filter text.
    pub(super) filter_aur: String,
    /// Translated core repository filter text.
    pub(super) filter_core: String,
    /// Translated extra repository filter text.
    pub(super) filter_extra: String,
    /// Translated multilib repository filter text.
    pub(super) filter_multilib: String,
    /// Translated `EndeavourOS` repository filter text.
    pub(super) filter_eos: String,
    /// Translated `CachyOS` repository filter text.
    pub(super) filter_cachyos: String,
    /// Translated Artix repository filter text.
    pub(super) filter_artix: String,
    /// Translated Artix Omniverse repository filter text.
    pub(super) filter_artix_omniverse: String,
    /// Translated Artix Universe repository filter text.
    pub(super) filter_artix_universe: String,
    /// Translated Artix Lib32 repository filter text.
    pub(super) filter_artix_lib32: String,
    /// Translated Artix Galaxy repository filter text.
    pub(super) filter_artix_galaxy: String,
    /// Translated Artix World repository filter text.
    pub(super) filter_artix_world: String,
    /// Translated Artix System repository filter text.
    pub(super) filter_artix_system: String,
    /// Translated Manjaro repository filter text.
    pub(super) filter_manjaro: String,
}

/// What: Represents pre-formatted label strings for optional repos.
///
/// Inputs: Individual label strings.
///
/// Output: Struct containing all label strings.
///
/// Details: Used to pass multiple label strings as a single parameter.
pub(super) struct OptionalReposLabels {
    /// `EndeavourOS` repository label.
    pub(super) eos: String,
    /// `CachyOS` repository label.
    pub(super) cachyos: String,
    /// Artix repository label.
    pub(super) artix: String,
    /// Artix Omniverse repository label.
    pub(super) artix_omniverse: String,
    /// Artix Universe repository label.
    pub(super) artix_universe: String,
    /// Artix Lib32 repository label.
    pub(super) artix_lib32: String,
    /// Artix Galaxy repository label.
    pub(super) artix_galaxy: String,
    /// Artix World repository label.
    pub(super) artix_world: String,
    /// Artix System repository label.
    pub(super) artix_system: String,
    /// Manjaro repository label.
    pub(super) manjaro: String,
}

/// What: Represents labels for core filters.
///
/// Inputs: Individual label strings.
///
/// Output: Struct containing core filter labels.
///
/// Details: Used to pass core filter labels as a single parameter.
pub(super) struct CoreFilterLabels {
    /// AUR filter label.
    pub(super) aur: String,
    /// Core repository filter label.
    pub(super) core: String,
    /// Extra repository filter label.
    pub(super) extra: String,
    /// Multilib repository filter label.
    pub(super) multilib: String,
}

/// What: Shared layout calculation information for title bar.
///
/// Inputs: Calculated values from title text, button labels, and area dimensions.
///
/// Output: Struct containing all layout calculation results.
///
/// Details: Used to share layout calculations between rendering and rect recording functions.
pub(super) struct TitleLayoutInfo {
    /// Title text with result count.
    pub(super) results_title_text: String,
    /// Sort button label.
    pub(super) sort_button_label: String,
    /// Options button label.
    pub(super) options_button_label: String,
    /// Panels button label.
    pub(super) panels_button_label: String,
    /// Config/Lists button label.
    pub(super) config_button_label: String,
    /// Menu button label.
    pub(super) menu_button_label: String,
    /// Core filter labels (AUR/core/extra/multilib).
    pub(super) core_labels: CoreFilterLabels,
    /// Optional repository filter labels.
    pub(super) optional_labels: OptionalReposLabels,
    /// Available inner width for rendering.
    pub(super) inner_width: u16,
    /// Whether to show Artix-specific repositories.
    pub(super) show_artix_specific_repos: bool,
    /// Padding between elements.
    pub(super) pad: u16,
    /// Whether collapsed menu is used.
    pub(super) use_collapsed_menu: bool,
    /// Padding reserved for menu button area.
    pub(super) menu_pad: u16,
}

/// What: Layout state tracker for recording rectangles.
///
/// Inputs: Initial x position and y position.
///
/// Output: Struct that tracks current x cursor position and y position.
///
/// Details: Encapsulates layout state to avoid manual `x_cursor` tracking.
pub(super) struct LayoutState {
    /// Current x cursor position.
    pub(super) x: u16,
    /// Y position for all elements.
    pub(super) y: u16,
}

impl LayoutState {
    /// What: Create a new layout state.
    ///
    /// Inputs:
    /// - `x`: Initial x position
    /// - `y`: Y position (constant)
    ///
    /// Output: New `LayoutState` instance.
    ///
    /// Details: Initializes layout state with starting position.
    pub(super) const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// What: Advance x cursor by label width plus spacing.
    ///
    /// Inputs:
    /// - `label_width`: Width of the label in characters
    /// - `spacing`: Number of spaces after the label (default 1)
    ///
    /// Output: Updated x position.
    ///
    /// Details: Moves x cursor forward by label width plus spacing.
    #[allow(clippy::missing_const_for_fn)]
    pub(super) fn advance(&mut self, label_width: u16, spacing: u16) -> u16 {
        self.x = self.x.saturating_add(label_width).saturating_add(spacing);
        self.x
    }

    /// What: Record a rectangle at current position.
    ///
    /// Inputs:
    /// - `label`: Label text to measure
    ///
    /// Output: Rectangle tuple (x, y, width, height).
    ///
    /// Details: Creates rectangle at current x position with label width.
    /// Uses Unicode display width, not byte length, to handle wide characters.
    pub(super) fn record_rect(&self, label: &str) -> (u16, u16, u16, u16) {
        use unicode_width::UnicodeWidthStr;
        (
            self.x,
            self.y,
            u16::try_from(label.width()).unwrap_or(u16::MAX),
            1,
        )
    }
}

/// What: Context for adjusting Artix visibility calculations.
///
/// Inputs: Grouped parameters for visibility calculations.
///
/// Output: Struct containing calculation parameters.
///
/// Details: Reduces function argument count by grouping related parameters.
pub(super) struct ArtixVisibilityContext {
    /// Left space consumed so far.
    pub(super) consumed_left: u16,
    /// Final left space consumed.
    pub(super) final_consumed_left: u16,
    /// Inner width available.
    pub(super) inner_width: u16,
    /// Menu width.
    pub(super) menu_w: u16,
    /// Base consumed space.
    pub(super) base_consumed: u16,
}
