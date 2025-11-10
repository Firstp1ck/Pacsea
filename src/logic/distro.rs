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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    /// What: Validate canonical repository toggles deny disabled repositories while permitting enabled ones.
    ///
    /// Inputs:
    /// - `app`: Application state with `core` enabled and other official toggles disabled.
    ///
    /// Output:
    /// - `repo_toggle_for` allows `core` entries but rejects `extra` and `multilib`.
    ///
    /// Details:
    /// - Ensures the per-repository gate respects the individual boolean flags.
    fn repo_toggle_respects_individual_flags() {
        let mut app = AppState {
            ..Default::default()
        };
        app.results_filter_show_core = true;
        app.results_filter_show_extra = false;
        app.results_filter_show_multilib = false;
        app.results_filter_show_eos = false;
        app.results_filter_show_cachyos = false;

        assert!(repo_toggle_for("core", &app));
        assert!(!repo_toggle_for("extra", &app));
        assert!(!repo_toggle_for("multilib", &app));
    }

    #[test]
    /// What: Ensure unknown official repositories require every official toggle to be enabled.
    ///
    /// Inputs:
    /// - `app`: Application state with all official flags on, then one flag disabled.
    ///
    /// Output:
    /// - Unknown repository accepted when fully enabled and rejected once any flag is turned off.
    ///
    /// Details:
    /// - Exercises the fallback clause guarding unfamiliar repositories.
    fn repo_toggle_unknown_only_with_full_whitelist() {
        let mut app = AppState {
            ..Default::default()
        };
        app.results_filter_show_core = true;
        app.results_filter_show_extra = true;
        app.results_filter_show_multilib = true;
        app.results_filter_show_eos = true;
        app.results_filter_show_cachyos = true;

        assert!(repo_toggle_for("unlisted", &app));

        app.results_filter_show_multilib = false;
        assert!(!repo_toggle_for("unlisted", &app));
    }

    #[test]
    /// What: Confirm label helper emits ecosystem-specific aliases for recognised repositories.
    ///
    /// Inputs:
    /// - Repository/name permutations covering EndeavourOS, CachyOS, Manjaro, and a generic repo.
    ///
    /// Output:
    /// - Labels reduce to `EOS`, `CachyOS`, `Manjaro`, and the original repo name respectively.
    ///
    /// Details:
    /// - Validates the Manjaro heuristic via package name and the repo classification helpers.
    fn label_for_official_prefers_special_cases() {
        assert_eq!(label_for_official("endeavouros", "pkg", ""), "EOS");
        assert_eq!(label_for_official("cachyos-extra", "pkg", ""), "CachyOS");
        assert_eq!(label_for_official("extra", "manjaro-kernel", ""), "Manjaro");
        assert_eq!(label_for_official("core", "glibc", ""), "core");
    }
}
