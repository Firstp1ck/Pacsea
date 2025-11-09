//! Distro-related logic helpers (filtering and labels).

/// What: Determine whether results from a repository should be visible under current toggles.
///
/// Inputs:
/// - `repo`: Name of the repository associated with a package result.
/// - `app`: Application state providing the filter toggles for official repos.
///
/// Output:
/// - `true` when the repository passes the active filters; otherwise `false`.
///
/// Details:
/// - Normalizes repository names and applies special-handling for EOS/CachyOS classification helpers.
/// - Unknown repositories are only allowed when every official filter is enabled simultaneously.
pub fn repo_toggle_for(repo: &str, app: &crate::state::AppState) -> bool {
    let r = repo.to_lowercase();
    if r == "core" {
        app.results_filter_show_core
    } else if r == "extra" {
        app.results_filter_show_extra
    } else if r == "multilib" {
        app.results_filter_show_multilib
    } else if crate::index::is_eos_repo(&r) {
        app.results_filter_show_eos
    } else if crate::index::is_cachyos_repo(&r) {
        app.results_filter_show_cachyos
    } else {
        // Unknown official repo: include only when all official filters are enabled
        app.results_filter_show_core
            && app.results_filter_show_extra
            && app.results_filter_show_multilib
            && app.results_filter_show_eos
            && app.results_filter_show_cachyos
    }
}

/// What: Produce a human-friendly label for an official package entry.
///
/// Inputs:
/// - `repo`: Repository reported by the package source.
/// - `name`: Package name used to detect Manjaro naming conventions.
/// - `owner`: Optional upstream owner string available from package metadata.
///
/// Output:
/// - Returns a display label describing the ecosystem the package belongs to.
///
/// Details:
/// - Distinguishes EndeavourOS and CachyOS repos, and detects Manjaro branding by name/owner heuristics.
/// - Falls back to the raw repository string when no special classification matches.
pub fn label_for_official(repo: &str, name: &str, owner: &str) -> String {
    let r = repo.to_lowercase();
    if crate::index::is_eos_repo(&r) {
        "EOS".to_string()
    } else if crate::index::is_cachyos_repo(&r) {
        "CachyOS".to_string()
    } else if crate::index::is_manjaro_name_or_owner(name, owner) {
        "Manjaro".to_string()
    } else {
        repo.to_string()
    }
}
