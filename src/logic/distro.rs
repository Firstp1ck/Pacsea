//! Distro-related logic helpers (filtering and labels).

/// Classify official repo toggles compactly.
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

/// Compute a human-friendly label for an official item, given repo and owner.
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
