use unicode_width::UnicodeWidthStr;

use super::super::OptionalRepos;
use super::types::{CoreFilterLabels, OptionalReposLabels};

/// What: Calculate consumed horizontal space for optional repos.
///
/// Inputs:
/// - `repos`: Optional repository flags
/// - `labels`: Pre-formatted label strings for each repo
///
/// Output: Total consumed width in characters.
///
/// Details: Sums up the width of all available optional repos plus spacing.
/// Uses Unicode display width, not byte length, to handle wide characters.
pub(super) fn calculate_optional_repos_width(
    repos: &OptionalRepos,
    labels: &OptionalReposLabels,
) -> u16 {
    let mut width = 0u16;
    if repos.has_eos {
        width = width.saturating_add(1 + u16::try_from(labels.eos.width()).unwrap_or(u16::MAX));
    }
    if repos.has_cachyos {
        width = width.saturating_add(1 + u16::try_from(labels.cachyos.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix {
        width = width.saturating_add(1 + u16::try_from(labels.artix.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_omniverse {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_omniverse.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_universe {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_universe.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_lib32 {
        width =
            width.saturating_add(1 + u16::try_from(labels.artix_lib32.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_galaxy {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_galaxy.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_world {
        width =
            width.saturating_add(1 + u16::try_from(labels.artix_world.width()).unwrap_or(u16::MAX));
    }
    if repos.has_artix_system {
        width = width
            .saturating_add(1 + u16::try_from(labels.artix_system.width()).unwrap_or(u16::MAX));
    }
    if repos.has_manjaro {
        width = width.saturating_add(1 + u16::try_from(labels.manjaro.width()).unwrap_or(u16::MAX));
    }
    width
}

/// What: Calculate base consumed space (title, sort button, core filters).
///
/// Inputs:
/// - `results_title_text`: Title text with count
/// - `sort_button_label`: Sort button label
/// - `core_labels`: Labels for core filters (AUR, core, extra, multilib)
///
/// Output: Base consumed width in display columns.
///
/// Details: Calculates space for fixed elements that are always present.
/// Uses Unicode display width, not byte length, to handle wide characters.
pub(super) fn calculate_base_consumed_space(
    results_title_text: &str,
    sort_button_label: &str,
    core_labels: &CoreFilterLabels,
) -> u16 {
    u16::try_from(
        results_title_text.width()
            + 2 // spaces before Sort
            + sort_button_label.width()
            + 2 // spaces after Sort
            + core_labels.aur.width()
            + 1 // space
            + core_labels.core.width()
            + 1 // space
            + core_labels.extra.width()
            + 1 // space
            + core_labels.multilib.width(),
    )
    .unwrap_or(u16::MAX)
}

/// What: Create `OptionalRepos` without Artix-specific repos.
///
/// Inputs:
/// - `optional_repos`: Original optional repos
///
/// Output: `OptionalRepos` with all Artix-specific repos set to false.
///
/// Details: Helper to create a copy without Artix-specific repos for space calculations.
#[allow(clippy::missing_const_for_fn)] // Cannot be const due to reference parameter
pub(super) fn create_repos_without_specific(optional_repos: &OptionalRepos) -> OptionalRepos {
    OptionalRepos {
        has_eos: optional_repos.has_eos,
        has_cachyos: optional_repos.has_cachyos,
        has_artix: optional_repos.has_artix,
        has_artix_omniverse: false,
        has_artix_universe: false,
        has_artix_lib32: false,
        has_artix_galaxy: false,
        has_artix_world: false,
        has_artix_system: false,
        has_manjaro: optional_repos.has_manjaro,
    }
}

/// What: Calculate consumed space without Artix-specific repos.
///
/// Inputs:
/// - `base_consumed`: Base consumed space
/// - `repos_without_specific`: Optional repos without Artix-specific repos
/// - `optional_labels`: Labels for optional repos
/// - `has_artix`: Whether Artix filter is present (for dropdown indicator)
///
/// Output: Total consumed space without Artix-specific repos.
///
/// Details: Calculates consumed space and adds 3 chars for dropdown indicator if Artix is present.
pub(super) fn calculate_consumed_without_specific(
    base_consumed: u16,
    repos_without_specific: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
    has_artix: bool,
) -> u16 {
    let mut consumed = base_consumed.saturating_add(calculate_optional_repos_width(
        repos_without_specific,
        optional_labels,
    ));
    if has_artix {
        consumed = consumed.saturating_add(3); // " v" dropdown indicator
    }
    consumed
}
