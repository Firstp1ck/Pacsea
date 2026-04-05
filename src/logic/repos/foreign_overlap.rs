//! Foreign (AUR) packages that also exist in a given sync repository.

use std::collections::HashSet;
use std::process::Command;

use crate::install::shell_single_quote;
use crate::logic::privilege::{PrivilegeTool, build_privilege_command};

/// What: One foreign package whose name exists in a sync database.
///
/// Inputs:
/// - Built by [`compute_foreign_repo_overlap`].
///
/// Output:
/// - Display and migration planning.
///
/// Details:
/// - `version` is the installed foreign version from `pacman -Qm` (not `-Qmq`: `-q` omits versions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignRepoOverlapEntry {
    /// Package name (`pkgname`).
    pub name: String,
    /// Installed version string (`ver-rel`).
    pub version: String,
}

/// What: Outcome of comparing foreign installs to a sync repository’s package names.
///
/// Inputs:
/// - Produced by [`analyze_foreign_repo_overlap`].
///
/// Output:
/// - `entries` for the overlap wizard; counts for user-facing follow-up toasts.
///
/// Details:
/// - `sync_pkg_name_count == 0` can mean an unknown repo (treated as empty), a failed list, or a repo
///   with no packages; see [`sync_repo_pkgnames_or_empty_if_repo_missing`].
#[derive(Debug, Clone)]
pub struct ForeignRepoOverlapAnalysis {
    /// Overlapping foreign packages (non-empty when the wizard should open).
    pub entries: Vec<ForeignRepoOverlapEntry>,
    /// Count of rows from `pacman -Qm` before intersection.
    pub foreign_pkg_count: usize,
    /// Distinct package names from `pacman -Sl <repo>` (0 when listing failed as “unknown repo” or empty).
    pub sync_pkg_name_count: usize,
}

/// What: Run `pacman -Qm` and parse foreign package names and versions.
///
/// Inputs:
/// - None (uses host `pacman`).
///
/// Output:
/// - Vector of `(pkgname, version)` or error message. `version` is the remainder of the line after the
///   package name (typically `pkgver-pkgrel` from `pacman -Qm`, but may be empty for name-only lines).
///
/// Details:
/// - Uses `-Qm` without `-q` so each line normally includes `pkgver-pkgrel`. `pacman -Qmq` only prints
///   names (see `pacman(8)` `--quiet`), which would make overlap detection see zero foreign packages.
/// - Returns empty vector when no foreign packages exist.
/// - Skips blank lines. Non-blank lines always yield an entry: the first whitespace-separated token is
///   the package name; any further tokens are joined with spaces into `version`, which is empty when the
///   line contains only a name (no version field to skip—overlap logic still keys on `pkgname`).
///
/// # Errors
///
/// - Returns an error when `pacman` cannot be executed or exits non-zero.
pub fn list_foreign_packages() -> Result<Vec<(String, String)>, String> {
    let out = Command::new("pacman")
        .args(["-Qm"])
        .output()
        .map_err(|e| format!("pacman -Qm failed to run: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "pacman -Qm failed (status {}): {stderr}",
            out.status
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut rows = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next().map(str::to_string) else {
            continue;
        };
        let version = parts.collect::<Vec<_>>().join(" ");
        rows.push((name, version));
    }
    Ok(rows)
}

/// What: Collect `pkgname` values from `pacman -Sl <repo>`.
///
/// Inputs:
/// - `repo`: Lowercase `[repo]` name (e.g. `chaotic-aur`).
///
/// Output:
/// - Set of package names or error.
///
/// Details:
/// - Ignores header lines and malformed rows.
///
/// # Errors
///
/// - Returns an error when `pacman` cannot be executed, exits non-zero, or the repo is unknown.
pub fn sync_repo_pkgnames(repo: &str) -> Result<HashSet<String>, String> {
    let repo = repo.trim();
    if repo.is_empty() {
        return Err("Repository name is empty.".to_string());
    }
    let out = Command::new("pacman")
        .args(["-Sl", repo])
        .output()
        .map_err(|e| format!("pacman -Sl failed to run: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "pacman -Sl {repo} failed (sync database missing or invalid?): {stderr}"
        ));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut set = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('[') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let _r = parts.next();
        let Some(pkg) = parts.next() else {
            continue;
        };
        set.insert(pkg.to_string());
    }
    Ok(set)
}

/// What: Detect pacman stderr/English/German messages meaning the repository is not configured.
///
/// Inputs:
/// - `combined`: Lowercased `pacman -Sl` error text (stdout+stderr merged is unnecessary; caller passes stderr body).
///
/// Output:
/// - `true` when the failure is consistent with an unknown or unavailable repo name.
///
/// Details:
/// - After a Pacsea apply disables the last managed repo, `pacman -Sl <name>` fails because the section
///   is gone; overlap detection should treat that as “no sync packages” instead of surfacing a connection alert.
fn sync_sl_failure_is_unknown_repository(combined: &str) -> bool {
    let lower = combined.to_lowercase();
    lower.contains("not found")
        || lower.contains("nicht gefunden")
        || lower.contains("wurde nicht gefunden")
        || lower.contains("could not find")
        || lower.contains("unable to find")
        || lower.contains("unknown repository")
        || lower.contains("repository not found")
        || lower.contains("no package database")
}

/// What: Like [`sync_repo_pkgnames`] but returns an empty set when the repo is unknown to pacman.
///
/// Inputs:
/// - `repo`: Lowercase `[repo]` name passed to `pacman -Sl`.
///
/// Output:
/// - Package name set, or empty when pacman reports the repository is missing.
///
/// Details:
/// - Propagates other `pacman -Sl` failures (permissions, corrupted DB, etc.).
fn sync_repo_pkgnames_or_empty_if_repo_missing(repo: &str) -> Result<HashSet<String>, String> {
    match sync_repo_pkgnames(repo) {
        Ok(s) => Ok(s),
        Err(msg) => {
            if sync_sl_failure_is_unknown_repository(&msg) {
                Ok(HashSet::new())
            } else {
                Err(msg)
            }
        }
    }
}

/// What: Build overlap analysis from a foreign list and a sync repo’s package-name set.
///
/// Inputs:
/// - `foreign`: `(pkgname, ver-rel)` rows (typically from `pacman -Qm`).
/// - `repo_names`: Names from `pacman -Sl <repo>`.
///
/// Output:
/// - [`ForeignRepoOverlapAnalysis`] with sorted overlap entries and counts.
///
/// Details:
/// - `foreign_pkg_count` is `foreign.len()`; `sync_pkg_name_count` is `repo_names.len()`.
fn overlap_analysis_from_foreign_and_repo_names(
    foreign: Vec<(String, String)>,
    repo_names: &HashSet<String>,
) -> ForeignRepoOverlapAnalysis {
    let foreign_pkg_count = foreign.len();
    let sync_pkg_name_count = repo_names.len();
    let mut entries: Vec<ForeignRepoOverlapEntry> = foreign
        .into_iter()
        .filter(|(n, _)| repo_names.contains(n))
        .map(|(name, version)| ForeignRepoOverlapEntry { name, version })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    ForeignRepoOverlapAnalysis {
        entries,
        foreign_pkg_count,
        sync_pkg_name_count,
    }
}

/// What: Compare foreign installs to `pacman -Sl <repo>` with counts for UI diagnostics.
///
/// Inputs:
/// - `repo`: Pacman repository name (same as `pacman -Sl` argument; usually lowercase).
///
/// Output:
/// - [`ForeignRepoOverlapAnalysis`] with sorted overlap entries and counts.
///
/// Details:
/// - Read-only; does not mutate the system.
///
/// # Errors
///
/// - Propagates failures from [`list_foreign_packages`] or [`sync_repo_pkgnames_or_empty_if_repo_missing`]
///   when `pacman -Sl` fails for reasons other than an unknown/missing repository.
pub fn analyze_foreign_repo_overlap(repo: &str) -> Result<ForeignRepoOverlapAnalysis, String> {
    analyze_foreign_repo_overlap_with_qm_snapshot(repo, None)
}

/// What: Compare foreign installs to `pacman -Sl <repo>`, using an optional pre-apply `pacman -Qm` snapshot.
///
/// Inputs:
/// - `repo`: Pacman repository name for `pacman -Sl`.
/// - `pre_apply_foreign_snapshot`: When `Some`, foreign rows captured when repo apply was **queued**
///   (before privileged commands). When `None`, calls [`list_foreign_packages`] at analysis time.
///
/// Output:
/// - Same as [`analyze_foreign_repo_overlap`].
///
/// Details:
/// - After a repo is enabled, pacman may reclassify installs so they no longer appear in `-Qm`; the
///   snapshot preserves the pre-enable foreign set without toggling repositories in `pacman.conf`.
///
/// # Errors
///
/// - Propagates failures from [`list_foreign_packages`] when `pre_apply_foreign_snapshot` is `None`,
///   or from [`sync_repo_pkgnames_or_empty_if_repo_missing`].
pub fn analyze_foreign_repo_overlap_with_qm_snapshot(
    repo: &str,
    pre_apply_foreign_snapshot: Option<&[(String, String)]>,
) -> Result<ForeignRepoOverlapAnalysis, String> {
    let foreign: Vec<(String, String)> = if let Some(rows) = pre_apply_foreign_snapshot {
        rows.to_vec()
    } else {
        list_foreign_packages()?
    };
    let repo_names = sync_repo_pkgnames_or_empty_if_repo_missing(repo)?;
    Ok(overlap_analysis_from_foreign_and_repo_names(
        foreign,
        &repo_names,
    ))
}

/// What: Foreign packages installed whose `pkgname` exists in `repo`'s sync DB.
///
/// Inputs:
/// - `repo`: Pacman repository name (same as `pacman -Sl` argument).
///
/// Output:
/// - Sorted overlap entries for stable UI.
///
/// Details:
/// - Read-only; does not mutate the system.
///
/// # Errors
///
/// - Propagates failures from [`list_foreign_packages`] or [`sync_repo_pkgnames_or_empty_if_repo_missing`].
pub fn compute_foreign_repo_overlap(repo: &str) -> Result<Vec<ForeignRepoOverlapEntry>, String> {
    Ok(analyze_foreign_repo_overlap(repo)?.entries)
}

/// What: Build privileged migrate commands: remove foreign packages then install from sync.
///
/// Inputs:
/// - `tool`: Active privilege backend.
/// - `dry_run`: When `true`, emit `echo DRY RUN: ...` only.
/// - `pkgs`: Package names to remove and reinstall (same `pkgname`).
///
/// Output:
/// - `(summary_lines, commands)` for [`crate::install::ExecutorRequest::Update`].
///
/// Details:
/// - Single transaction chain `pacman -Rns` then `pacman -S` for all names.
/// - `-Rns` may remove dependents; UI must warn users before calling.
///
/// # Errors
///
/// - Returns an error when `pkgs` is empty or privilege command construction fails.
pub fn build_foreign_to_sync_migrate_bundle(
    tool: PrivilegeTool,
    dry_run: bool,
    pkgs: &[String],
) -> Result<(Vec<String>, Vec<String>), String> {
    if pkgs.is_empty() {
        return Err("No packages selected for migration.".to_string());
    }
    let joined = pkgs.join(" ");
    let summary_lines = vec![
        format!("Remove foreign packages: {joined}"),
        format!("Install from sync repositories: {joined}"),
    ];
    let inner =
        format!("pacman -Rns --noconfirm {joined} && pacman -S --needed --noconfirm {joined}");
    let cmd = if dry_run {
        let quoted = shell_single_quote(&inner);
        format!("echo DRY RUN: {quoted}")
    } else {
        build_privilege_command(tool, &inner)
    };
    Ok((summary_lines, vec![cmd]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_qm_lines_include_name_only_rows() {
        let text = "foo 1.0-1\n\nbar\n";
        let mut rows = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let name = parts.next().expect("test line has name").to_string();
            let version = parts.collect::<Vec<_>>().join(" ");
            rows.push((name, version));
        }
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "foo");
        assert_eq!(rows[0].1, "1.0-1");
        assert_eq!(rows[1].0, "bar");
        assert!(rows[1].1.is_empty());
    }

    #[test]
    fn sl_parsing_collects_second_column() {
        let line = "chaotic-aur discord 0.0.45-1.1";
        let mut parts = line.split_whitespace();
        let _r = parts.next();
        let pkg = parts.next().expect("test sl line has pkg");
        assert_eq!(pkg, "discord");
    }

    #[test]
    fn unknown_repo_sl_errors_are_recognized_en_de() {
        assert!(super::sync_sl_failure_is_unknown_repository(
            "pacman -Sl chaotic-aur failed: error: repository 'chaotic-aur' was not found."
        ));
        assert!(super::sync_sl_failure_is_unknown_repository(
            "Fehler: Das Repositorium »chaotic-aur« wurde nicht gefunden."
        ));
        assert!(!super::sync_sl_failure_is_unknown_repository(
            "pacman -Sl failed to run: No such file or directory (os error 2)"
        ));
    }

    #[test]
    /// What: Verify pre-apply foreign rows intersect sync name set as used after repo apply.
    ///
    /// Inputs:
    /// - Synthetic foreign list and `HashSet` of repo pkgnames.
    ///
    /// Output:
    /// - Analysis counts and entries match intersection expectations.
    ///
    /// Details:
    /// - Mirrors [`super::overlap_analysis_from_foreign_and_repo_names`] without calling pacman.
    fn overlap_analysis_intersects_snapshot_foreign_with_repo_names() {
        use std::collections::HashSet;

        let foreign = vec![
            ("waypaper-git".to_string(), "2.7-1".to_string()),
            ("only-foreign".to_string(), "1-1".to_string()),
        ];
        let mut repo = HashSet::new();
        repo.insert("waypaper-git".to_string());
        let a = super::overlap_analysis_from_foreign_and_repo_names(foreign, &repo);
        assert_eq!(a.foreign_pkg_count, 2);
        assert_eq!(a.sync_pkg_name_count, 1);
        assert_eq!(a.entries.len(), 1);
        assert_eq!(a.entries[0].name, "waypaper-git");
        assert_eq!(a.entries[0].version, "2.7-1");
    }
}
