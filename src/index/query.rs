use crate::state::{PackageItem, Source};

use super::idx;

/// Search the official index for packages whose names contain `query`.
///
/// Returns `PackageItem`s with fields populated from the index; enrichment is
/// not performed here. An empty or whitespace-only query returns an empty list.
pub fn search_official(query: &str) -> Vec<PackageItem> {
    let ql = query.trim().to_lowercase();
    if ql.is_empty() {
        return Vec::new();
    }
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        for p in &g.pkgs {
            let nl = p.name.to_lowercase();
            if nl.contains(&ql) {
                items.push(PackageItem {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    description: p.description.clone(),
                    source: Source::Official {
                        repo: p.repo.clone(),
                        arch: p.arch.clone(),
                    },
                    popularity: None,
                });
            }
        }
    }
    items
}

/// Return the entire official index as a list of `PackageItem`s.
pub fn all_official() -> Vec<PackageItem> {
    let guard = idx().read().ok();
    let mut items = Vec::new();
    if let Some(g) = guard {
        items.reserve(g.pkgs.len());
        for p in &g.pkgs {
            items.push(PackageItem {
                name: p.name.clone(),
                version: p.version.clone(),
                description: p.description.clone(),
                source: Source::Official {
                    repo: p.repo.clone(),
                    arch: p.arch.clone(),
                },
                popularity: None,
            });
        }
    }
    items
}

/// Return the entire official index as a list of `PackageItem`s; if empty, try
/// to populate from disk and return the now-current view.
pub async fn all_official_or_fetch(path: &std::path::Path) -> Vec<PackageItem> {
    let items = all_official();
    if !items.is_empty() {
        return items;
    }
    super::persist::load_from_disk(path);
    all_official()
}
