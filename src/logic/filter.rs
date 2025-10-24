use crate::state::{AppState, PackageItem, Source};

/// What: Apply current repo/AUR filters to `app.all_results`, write into `app.results`, then sort.
///
/// Inputs:
/// - `app`: Mutable application state containing `all_results`, filter toggles, and selection
///
/// Output:
/// - Updates `app.results`, applies sorting, and preserves selection when possible.
///
/// Details:
/// - Unknown official repos are included only when all official filters are enabled.
/// - Selection is restored by name when present; otherwise clamped or cleared if list is empty.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item_official(name: &str, repo: &str) -> PackageItem {
        PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: format!("{name} desc"),
            source: Source::Official {
                repo: repo.to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        }
    }

    #[test]
    /// What: Apply repo/AUR filters and preserve selection
    ///
    /// - Input: Mixed all_results; core enabled, others disabled
    /// - Output: Only core repo remains in results
    fn apply_filters_and_preserve_selection() {
        let mut app = AppState {
            ..Default::default()
        };
        app.all_results = vec![
            PackageItem {
                name: "aur1".into(),
                version: "1".into(),
                description: String::new(),
                source: Source::Aur,
                popularity: Some(1.0),
            },
            item_official("core1", "core"),
            item_official("extra1", "extra"),
            item_official("other1", "community"),
        ];
        app.results_filter_show_aur = false;
        app.results_filter_show_core = true;
        app.results_filter_show_extra = false;
        app.results_filter_show_multilib = false;
        apply_filters_and_sort_preserve_selection(&mut app);
        assert!(app.results.iter().all(
            |p| matches!(&p.source, Source::Official{repo, ..} if repo.eq_ignore_ascii_case("core"))
        ));
    }

    #[test]
    /// What: Interaction of CachyOS and EOS toggles
    ///
    /// - Input: cachyos-* and endeavouros entries; EOS off, CachyOS on
    /// - Output: CachyOS entries included; EOS excluded
    fn apply_filters_cachyos_and_eos_interaction() {
        let mut app = AppState {
            ..Default::default()
        };
        app.all_results = vec![
            PackageItem {
                name: "cx".into(),
                version: "1".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "cachyos-core".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
            PackageItem {
                name: "ey".into(),
                version: "1".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "endeavouros".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
            item_official("core1", "core"),
        ];
        app.results_filter_show_core = true;
        app.results_filter_show_extra = true;
        app.results_filter_show_multilib = true;
        app.results_filter_show_eos = false;
        app.results_filter_show_cachyos = true;
        apply_filters_and_sort_preserve_selection(&mut app);
        assert!(app.results.iter().any(|p| match &p.source {
            Source::Official { repo, .. } => repo.to_lowercase().starts_with("cachyos"),
            _ => false,
        }));
        assert!(app.results.iter().all(|p| match &p.source {
            Source::Official { repo, .. } => !repo.eq_ignore_ascii_case("endeavouros"),
            _ => true,
        }));
    }

    #[test]
    /// What: Unknown official repo inclusion policy
    ///
    /// - Input: Unknown repo with at least one official toggle disabled, then all enabled
    /// - Output: Unknown excluded first; included when all official toggles on
    fn logic_filter_unknown_official_inclusion_policy() {
        let mut app = AppState {
            ..Default::default()
        };
        app.all_results = vec![
            PackageItem {
                name: "x1".into(),
                version: "1".into(),
                description: String::new(),
                source: Source::Official {
                    repo: "weirdrepo".into(),
                    arch: "x86_64".into(),
                },
                popularity: None,
            },
            item_official("core1", "core"),
        ];
        app.results_filter_show_aur = true;
        app.results_filter_show_core = true;
        app.results_filter_show_extra = true;
        app.results_filter_show_multilib = false;
        app.results_filter_show_eos = true;
        app.results_filter_show_cachyos = true;
        apply_filters_and_sort_preserve_selection(&mut app);
        assert!(app.results.iter().all(|p| match &p.source {
            Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
            _ => false,
        }));

        app.results_filter_show_multilib = true;
        apply_filters_and_sort_preserve_selection(&mut app);
        assert!(app.results.iter().any(|p| match &p.source {
            Source::Official { repo, .. } => repo.eq_ignore_ascii_case("weirdrepo"),
            _ => false,
        }));
    }
}
