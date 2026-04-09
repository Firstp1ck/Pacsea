//! Third-party repository definitions (`repos.conf`) and results-filter wiring.
//!
//! Phase 1 parses TOML (user-defined `[[repo]]` rows only) and exposes maps for [`crate::logic::distro::repo_toggle_for`].
//! Shipped repo recipes (disabled by default) live in `config/repos.conf` in the Pacsea tree.
//! Phase 3 adds privileged apply via [`apply_plan`] and the Repositories modal (see `events/modals/repositories.rs`).

mod apply_plan;
mod config;
mod foreign_overlap;
mod modal_data;
mod pacman_conf;

pub use apply_plan::{
    DEFAULT_DROPIN_PATH, DEFAULT_MAIN_PACMAN_PATH, MANAGED_DROPIN_FILE, PACMAN_MANAGED_BEGIN,
    PACMAN_MANAGED_END, RepoApplyBundle, build_repo_apply_bundle, build_repo_key_refresh_bundle,
    read_main_pacman_conf_text,
};
pub use config::{
    RepoRow, ReposConfFile, build_dynamic_visibility, build_repo_name_to_filter_map,
    canonical_results_filter_key, disable_repo_section_in_repos_conf_if_enabled,
    load_repo_name_map_from_path, load_resolve_repos_from_str, repo_row_declares_apply_sources,
    repos_conf_repo_names_for_index_sl, repos_conf_section_is_disabled_with_apply_sources,
    row_is_enabled_for_repos_conf, save_repos_conf_file, toggle_repo_enabled_for_section_in_file,
};
pub use foreign_overlap::{
    ForeignRepoOverlapAnalysis, ForeignRepoOverlapEntry, analyze_foreign_repo_overlap,
    analyze_foreign_repo_overlap_with_qm_snapshot, build_foreign_to_sync_migrate_bundle,
    compute_foreign_repo_overlap, list_foreign_packages, sync_repo_pkgnames,
};
pub use modal_data::{build_repositories_modal_fields, build_repositories_modal_fields_default};
pub use pacman_conf::{PacmanConfScan, PacmanRepoPresence, scan_pacman_conf_path};

use crate::state::AppState;
use crate::theme::Settings;

/// What: Populate `repo_results_filter_by_name` from the resolved config path, if any.
///
/// Inputs:
/// - `app`: Application state to update.
/// - `repos_path`: Optional path from [`crate::theme::resolve_repos_config_path`].
///
/// Output:
/// - None (mutates `app`).
///
/// Details:
/// - On missing path or parse errors, the map is cleared; failures are logged in the loader.
pub fn load_repos_config_into_app(app: &mut AppState, repos_path: Option<std::path::PathBuf>) {
    let Some(path) = repos_path else {
        app.repo_results_filter_by_name.clear();
        return;
    };
    app.repo_results_filter_by_name = load_repo_name_map_from_path(&path);
}

/// What: Recompute `results_filter_dynamic` from settings toggles and repo map.
///
/// Inputs:
/// - `app`: Application state.
/// - `prefs`: Loaded settings (includes `results_filter_toggles`).
///
/// Output:
/// - None (mutates `app`).
///
/// Details:
/// - Call after `apply_settings_to_app_state` field updates and after reloading `repos.conf`.
pub fn refresh_dynamic_filters_in_app(app: &mut AppState, prefs: &Settings) {
    app.results_filter_dynamic = build_dynamic_visibility(
        &prefs.results_filter_toggles,
        &app.repo_results_filter_by_name,
    );
}

/// What: Update one dynamic results filter, persist `settings.conf`, and re-run filtering.
///
/// Inputs:
/// - `app`: Application state.
/// - `canonical_id`: Canonical `results_filter` token from `repos.conf`.
/// - `new_visible`: Desired visibility in Results.
///
/// Output:
/// - None.
///
/// Details:
/// - Ignores unknown ids; uses [`crate::theme::save_results_filter_show_canonical`].
pub fn persist_dynamic_filter_toggle_and_refresh(
    app: &mut AppState,
    canonical_id: &str,
    new_visible: bool,
) {
    if !app.results_filter_dynamic.contains_key(canonical_id) {
        return;
    }
    if let Some(v) = app.results_filter_dynamic.get_mut(canonical_id) {
        *v = new_visible;
    }
    crate::theme::save_results_filter_show_canonical(canonical_id, new_visible);
    crate::logic::apply_filters_and_sort_preserve_selection(app);
}

/// What: Turn every dynamic repo filter on or off together, persist, and re-filter.
///
/// Inputs:
/// - `app`: Application state.
/// - `new_visible`: Target visibility for all dynamic ids.
///
/// Output:
/// - None.
///
/// Details:
/// - No-op when the dynamic map is empty.
pub fn persist_dynamic_filters_set_all(app: &mut AppState, new_visible: bool) {
    if app.results_filter_dynamic.is_empty() {
        return;
    }
    let ids: Vec<String> = app.results_filter_dynamic.keys().cloned().collect();
    for id in &ids {
        if let Some(v) = app.results_filter_dynamic.get_mut(id) {
            *v = new_visible;
        }
        crate::theme::save_results_filter_show_canonical(id, new_visible);
    }
    crate::logic::apply_filters_and_sort_preserve_selection(app);
}

/// What: Whether Arch-style repository tooling (pacman paths, privileged apply) is available.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `true` on Linux; `false` elsewhere.
///
/// Details:
/// - Used to gate Repositories modal actions that assume a pacman-managed system.
#[must_use]
pub const fn repositories_linux_actions_supported() -> bool {
    cfg!(target_os = "linux")
}
