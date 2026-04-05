//! Detect AUR-selected installs whose `pkgname` also appears as an official/sync row in a catalog.

use std::collections::HashSet;

use crate::state::types::{PackageItem, Source};

/// What: Names of AUR-sourced install items that also have an official/sync row in `catalog`.
///
/// Inputs:
/// - `install`: Packages queued for install.
/// - `catalog`: Result rows to scan (typically `all_results` chained with `results`).
///
/// Output:
/// - Sorted unique names for stable UI.
///
/// Details:
/// - Uses exact `pkgname` equality; does not compare `pkgbase` or providers.
#[must_use]
pub fn aur_pkgnames_also_in_official_catalog(
    install: &[PackageItem],
    catalog: &[PackageItem],
) -> Vec<String> {
    let official: HashSet<&str> = catalog
        .iter()
        .filter_map(|p| matches!(p.source, Source::Official { .. }).then_some(p.name.as_str()))
        .collect();
    let mut names: Vec<String> = install
        .iter()
        .filter(|p| matches!(p.source, Source::Aur) && official.contains(p.name.as_str()))
        .map(|p| p.name.clone())
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn off(name: &str) -> PackageItem {
        PackageItem {
            name: name.into(),
            version: String::new(),
            description: String::new(),
            source: Source::Official {
                repo: "extra".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }

    fn aur(name: &str) -> PackageItem {
        PackageItem {
            name: name.into(),
            version: String::new(),
            description: String::new(),
            source: Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        }
    }

    #[test]
    fn detects_aur_when_official_same_name_in_catalog() {
        let catalog = vec![off("foo"), aur("bar")];
        let install = vec![aur("foo")];
        let d = aur_pkgnames_also_in_official_catalog(&install, &catalog);
        assert_eq!(d, vec!["foo".to_string()]);
    }

    #[test]
    fn empty_when_no_official_sibling() {
        let catalog = vec![aur("foo")];
        let install = vec![aur("foo")];
        assert!(aur_pkgnames_also_in_official_catalog(&install, &catalog).is_empty());
    }
}
