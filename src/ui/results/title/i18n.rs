use crate::i18n;
use crate::state::AppState;

use super::types::TitleI18nStrings;

/// What: Build `TitleI18nStrings` from `AppState`.
///
/// Inputs:
/// - `app`: Application state for i18n
///
/// Output: `TitleI18nStrings` containing all pre-computed i18n strings.
///
/// Details: Extracts all i18n strings needed for title rendering in one place.
pub(super) fn build_title_i18n_strings(app: &AppState) -> TitleI18nStrings {
    TitleI18nStrings {
        results_title: i18n::t(app, "app.results.title"),
        sort_button: i18n::t(app, "app.results.buttons.sort"),
        options_button: i18n::t(app, "app.results.buttons.options"),
        panels_button: i18n::t(app, "app.results.buttons.panels"),
        config_button: i18n::t(app, "app.results.buttons.config_lists"),
        menu_button: i18n::t(app, "app.results.buttons.menu"),
        filter_aur: i18n::t(app, "app.results.filters.aur"),
        filter_core: i18n::t(app, "app.results.filters.core"),
        filter_extra: i18n::t(app, "app.results.filters.extra"),
        filter_multilib: i18n::t(app, "app.results.filters.multilib"),
        filter_eos: i18n::t(app, "app.results.filters.eos"),
        filter_cachyos: i18n::t(app, "app.results.filters.cachyos"),
        filter_artix: i18n::t(app, "app.results.filters.artix"),
        filter_artix_omniverse: i18n::t(app, "app.results.filters.artix_omniverse"),
        filter_artix_universe: i18n::t(app, "app.results.filters.artix_universe"),
        filter_artix_lib32: i18n::t(app, "app.results.filters.artix_lib32"),
        filter_artix_galaxy: i18n::t(app, "app.results.filters.artix_galaxy"),
        filter_artix_world: i18n::t(app, "app.results.filters.artix_world"),
        filter_artix_system: i18n::t(app, "app.results.filters.artix_system"),
        filter_manjaro: i18n::t(app, "app.results.filters.manjaro"),
    }
}
