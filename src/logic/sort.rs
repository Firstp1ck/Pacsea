use crate::state::{AppState, SortMode, Source};

/// What: Apply the currently selected sorting mode to `app.results` in-place.
///
/// Inputs:
/// - `app`: Mutable application state (results, selected, input, sort_mode)
///
/// Output:
/// - Sorts `app.results` and preserves selection by name when possible; otherwise clamps index.
pub fn sort_results_preserve_selection(app: &mut AppState) {
    if app.results.is_empty() {
        return;
    }
    let prev_name = app.results.get(app.selected).map(|p| p.name.clone());
    match app.sort_mode {
        SortMode::RepoThenName => {
            app.results.sort_by(|a, b| {
                let oa = crate::util::repo_order(&a.source);
                let ob = crate::util::repo_order(&b.source);
                if oa != ob {
                    return oa.cmp(&ob);
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
        SortMode::AurPopularityThenOfficial => {
            app.results.sort_by(|a, b| {
                // AUR first
                let aur_a = matches!(a.source, Source::Aur);
                let aur_b = matches!(b.source, Source::Aur);
                if aur_a != aur_b {
                    return aur_b.cmp(&aur_a); // true before false
                }
                if aur_a && aur_b {
                    // Desc popularity for AUR
                    let pa = a.popularity.unwrap_or(0.0);
                    let pb = b.popularity.unwrap_or(0.0);
                    if (pa - pb).abs() > f64::EPSILON {
                        return pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal);
                    }
                } else {
                    // Both official: keep pacman order (repo_order), then name
                    let oa = crate::util::repo_order(&a.source);
                    let ob = crate::util::repo_order(&b.source);
                    if oa != ob {
                        return oa.cmp(&ob);
                    }
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
        SortMode::BestMatches => {
            // Compute simple match rank based on current input; lower is better
            let ql = app.input.trim().to_lowercase();
            app.results.sort_by(|a, b| {
                let ra = crate::util::match_rank(&a.name, &ql);
                let rb = crate::util::match_rank(&b.name, &ql);
                if ra != rb {
                    return ra.cmp(&rb);
                }
                // Tiebreak: keep pacman repo order first to keep layout familiar
                let oa = crate::util::repo_order(&a.source);
                let ob = crate::util::repo_order(&b.source);
                if oa != ob {
                    return oa.cmp(&ob);
                }
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            });
        }
    }
    if let Some(name) = prev_name {
        if let Some(pos) = app.results.iter().position(|p| p.name == name) {
            app.selected = pos;
            app.list_state.select(Some(pos));
        } else {
            app.selected = app.selected.min(app.results.len().saturating_sub(1));
            app.list_state.select(Some(app.selected));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_official(name: &str, repo: &str) -> crate::state::PackageItem {
        crate::state::PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: format!("{name} desc"),
            source: crate::state::Source::Official {
                repo: repo.to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        }
    }
    fn item_aur(name: &str, pop: Option<f64>) -> crate::state::PackageItem {
        crate::state::PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: format!("{name} desc"),
            source: crate::state::Source::Aur,
            popularity: pop,
        }
    }

    #[test]
    /// What: Sorting preserves selection and honors BestMatches relevance
    ///
    /// - Input: Mixed AUR/official results; change sort modes; set input "bb"
    /// - Output: Official first for RepoThenName; AUR first for popularity; relevant names early
    fn sort_preserve_selection_and_best_matches() {
        let mut app = AppState {
            ..Default::default()
        };
        app.results = vec![
            item_aur("zzz", Some(1.0)),
            item_official("aaa", "core"),
            item_official("bbb", "extra"),
            item_aur("ccc", Some(10.0)),
        ];
        app.selected = 2;
        app.list_state.select(Some(2));
        app.sort_mode = SortMode::RepoThenName;
        sort_results_preserve_selection(&mut app);
        assert_eq!(
            app.results
                .iter()
                .filter(|p| matches!(p.source, Source::Official { .. }))
                .count(),
            2
        );
        assert_eq!(app.results[app.selected].name, "bbb");

        app.sort_mode = SortMode::AurPopularityThenOfficial;
        sort_results_preserve_selection(&mut app);
        let aur_first = &app.results[0];
        assert!(matches!(aur_first.source, Source::Aur));

        app.input = "bb".into();
        app.sort_mode = SortMode::BestMatches;
        sort_results_preserve_selection(&mut app);
        assert!(
            app.results
                .iter()
                .position(|p| p.name.contains("bb"))
                .unwrap()
                <= 1
        );
    }

    #[test]
    /// What: Tiebreakers in BestMatches use repo order then name
    ///
    /// - Input: All names matching "alpha" with core/extra repos
    /// - Output: core wins before extra; extra sorted by name
    fn sort_bestmatches_tiebreak_repo_then_name() {
        let mut app = AppState {
            ..Default::default()
        };
        app.results = vec![
            item_official("alpha2", "extra"),
            item_official("alpha1", "extra"),
            item_official("alpha_core", "core"),
        ];
        app.input = "alpha".into();
        app.sort_mode = SortMode::BestMatches;
        sort_results_preserve_selection(&mut app);
        let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["alpha_core", "alpha1", "alpha2"]);
    }

    #[test]
    /// What: AUR popularity sort then official tiebreakers
    ///
    /// - Input: AUR items with equal popularity and official items core/extra
    /// - Output: AUR name-asc for ties; official core before extra, name tiebreak
    fn sort_aur_popularity_and_official_tiebreaks() {
        let mut app = AppState {
            ..Default::default()
        };
        app.results = vec![
            item_aur("aurB", Some(1.0)),
            item_aur("aurA", Some(1.0)),
            item_official("z_off", "core"),
            item_official("a_off", "extra"),
        ];
        app.sort_mode = SortMode::AurPopularityThenOfficial;
        sort_results_preserve_selection(&mut app);
        let names: Vec<String> = app.results.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["aurA", "aurB", "z_off", "a_off"]);
    }
}
