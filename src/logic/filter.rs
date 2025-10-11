use crate::state::{AppState, PackageItem, Source};

/// Apply current repo/AUR filters to `app.all_results`, write into `app.results`, then sort.
///
/// Attempts to preserve the selection by name; falls back to clamping.
pub fn apply_filters_and_sort_preserve_selection(app: &mut AppState) {
    // Capture previous selected name to preserve when possible
    let prev_name = app.results.get(app.selected).map(|p| p.name.clone());

    // Filter from all_results into results based on toggles
    let mut filtered: Vec<PackageItem> = Vec::with_capacity(app.all_results.len());
    for it in app.all_results.iter().cloned() {
        let include = match &it.source {
            Source::Aur => app.results_filter_show_aur,
            Source::Official { repo, .. } => {
                let r = repo.to_lowercase();
                if r == "core" {
                    app.results_filter_show_core
                } else if r == "extra" {
                    app.results_filter_show_extra
                } else if r == "multilib" {
                    app.results_filter_show_multilib
                } else if r == "eos" || r == "endeavouros" {
                    app.results_filter_show_eos
                } else if r.starts_with("cachyos") {
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
        };
        if include {
            filtered.push(it);
        }
    }
    app.results = filtered;
    // Apply existing sort policy and preserve selection
    crate::logic::sort_results_preserve_selection(app);
    // Restore by name if possible
    if let Some(name) = prev_name {
        if let Some(pos) = app.results.iter().position(|p| p.name == name) {
            app.selected = pos;
            app.list_state.select(Some(pos));
        } else if !app.results.is_empty() {
            app.selected = app.selected.min(app.results.len() - 1);
            app.list_state.select(Some(app.selected));
        } else {
            app.selected = 0;
            app.list_state.select(None);
        }
    } else if app.results.is_empty() {
        app.selected = 0;
        app.list_state.select(None);
    } else {
        app.selected = app.selected.min(app.results.len() - 1);
        app.list_state.select(Some(app.selected));
    }
}
