use crate::state::{AppState, SortMode, Source};

/// Apply the currently selected sorting mode to `app.results` in-place.
///
/// Preserves the selection by trying to keep the same package name selected
/// after sorting, falling back to index clamping.
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
