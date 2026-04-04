//! Read-only scan of `pacman.conf` repository sections with shallow `Include` expansion.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// What: Maximum depth when following `Include` directives.
///
/// Inputs:
/// - N/A (constant).
///
/// Output:
/// - Depth bound to avoid infinite recursion on cyclic includes.
///
/// Details:
/// - Matches common small stacks of `Include`d fragments under `/etc/pacman.d/`.
const MAX_INCLUDE_DEPTH: usize = 8;

/// What: One occurrence of a repository section header in a parsed file.
///
/// Inputs:
/// - N/A (internal struct).
///
/// Output:
/// - Used to merge active vs commented sections across files.
///
/// Details:
/// - The same repo name may appear multiple times; active headers take precedence over commented.
struct Occurrence {
    /// Whether the `[repo]` line was active (not prefixed with `#`).
    active: bool,
    /// File path where the header was found.
    path: PathBuf,
}

/// What: Presence of a pacman repository section after scanning config trees.
///
/// Inputs:
/// - Produced by [`scan_pacman_conf_path`].
///
/// Output:
/// - Classification for UI and merge logic.
///
/// Details:
/// - `[options]` is never stored here; only repository sections are tracked.
/// - If both active and commented headers exist anywhere, [`Self::Active`] wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PacmanRepoPresence {
    /// No `[name]` or `# [name]` header was found.
    Absent,
    /// At least one active `[name]` section exists.
    Active {
        /// File where an active header was first found (any matching occurrence).
        source: Option<PathBuf>,
    },
    /// Only commented `# [name]` headers exist.
    Commented {
        /// File path for reference.
        source: Option<PathBuf>,
    },
}

/// What: Result of scanning `/etc/pacman.conf` and included files.
///
/// Inputs:
/// - Returned by [`scan_pacman_conf_path`].
///
/// Output:
/// - Map keyed by lowercase repository section name.
///
/// Details:
/// - Warnings list I/O or include issues without failing the whole scan.
#[derive(Debug, Clone)]
pub struct PacmanConfScan {
    /// Repository section name (lowercase) mapped to merged [`PacmanRepoPresence`].
    pub repos: HashMap<String, PacmanRepoPresence>,
    /// Non-fatal parse, I/O, or include problems collected during the scan.
    pub warnings: Vec<String>,
}

impl PacmanConfScan {
    /// What: Look up merged presence for a repo name from `repos.conf`.
    ///
    /// Inputs:
    /// - `repo_name`: `[[repo]]` `name` value (case-insensitive).
    ///
    /// Output:
    /// - [`PacmanRepoPresence::Absent`] when unknown.
    #[must_use]
    pub fn presence_of(&self, repo_name: &str) -> PacmanRepoPresence {
        let key = repo_name.trim().to_lowercase();
        self.repos
            .get(&key)
            .cloned()
            .unwrap_or(PacmanRepoPresence::Absent)
    }
}

/// What: Scan the system pacman configuration for repository section headers.
///
/// Inputs:
/// - `root`: Typically `/etc/pacman.conf`.
///
/// Output:
/// - [`PacmanConfScan`] with merged repo keys and warnings.
///
/// Details:
/// - Follows `Include =` relative to the including file's directory.
/// - Skips duplicate canonical include targets with a warning.
/// - Missing files add warnings and continue.
#[must_use]
pub fn scan_pacman_conf_path(root: &Path) -> PacmanConfScan {
    let mut occurrences: HashMap<String, Vec<Occurrence>> = HashMap::new();
    let mut warnings = Vec::new();
    let mut visited = HashSet::new();
    scan_file_recursive(root, 0, &mut visited, &mut occurrences, &mut warnings);
    let repos = fold_occurrences_map(occurrences);
    PacmanConfScan { repos, warnings }
}

/// What: Parse a bracketed section header like `[core]` from a trimmed line.
///
/// Inputs:
/// - `line`: Line without leading `#` (caller strips comments).
///
/// Output:
/// - Inner section name, or `None` if not a header.
fn parse_bracket_header(line: &str) -> Option<&str> {
    let s = line.trim();
    let rest = s.strip_prefix('[')?;
    let inner = rest.strip_suffix(']')?;
    let name = inner.trim();
    if name.is_empty() {
        return None;
    }
    Some(name)
}

/// What: Parse `Include = path` (case-insensitive key).
///
/// Inputs:
/// - `line`: Non-comment trimmed line.
///
/// Output:
/// - Include path without surrounding quotes.
fn parse_include_line(line: &str) -> Option<&str> {
    let mut iter = line.splitn(2, '=');
    let key = iter.next()?.trim();
    if !key.eq_ignore_ascii_case("include") {
        return None;
    }
    let val = iter.next()?.trim();
    let trimmed = val.trim_matches(|c| c == '"' || c == '\'');
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed)
}

/// What: Resolve an include path against the current file's directory.
///
/// Inputs:
/// - `base_dir`: Parent directory of the file containing the `Include` line.
/// - `raw`: Path string from config.
///
/// Output:
/// - Absolute or joined path.
fn resolve_include_path(base_dir: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base_dir.join(p)
    }
}

/// What: Record section headers and queue includes from one file's contents.
///
/// Inputs:
/// - `content`: Full file text.
/// - `source_path`: Path of this file (for occurrences and include resolution).
/// - `occurrences`: Running map of section names.
/// - `pending_includes`: Output paths to recurse into.
///
/// Output:
/// - None (mutates maps).
///
/// Details:
/// - Lines starting with `#` may contain `# [repo]` which counts as commented.
fn collect_from_content(
    content: &str,
    source_path: &Path,
    occurrences: &mut HashMap<String, Vec<Occurrence>>,
    pending_includes: &mut Vec<PathBuf>,
) {
    let base_dir = source_path.parent().unwrap_or_else(|| Path::new("/"));
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            let inner = rest.trim();
            if let Some(sec) = parse_bracket_header(inner)
                && !sec.eq_ignore_ascii_case("options")
            {
                let name = sec.trim().to_lowercase();
                occurrences.entry(name).or_default().push(Occurrence {
                    active: false,
                    path: source_path.to_path_buf(),
                });
            }
            continue;
        }
        if let Some(sec) = parse_bracket_header(trimmed) {
            if !sec.eq_ignore_ascii_case("options") {
                let name = sec.trim().to_lowercase();
                occurrences.entry(name).or_default().push(Occurrence {
                    active: true,
                    path: source_path.to_path_buf(),
                });
            }
            continue;
        }
        if let Some(inc) = parse_include_line(trimmed) {
            pending_includes.push(resolve_include_path(base_dir, inc));
        }
    }
}

/// What: Fold per-repo occurrence lists into a single [`PacmanRepoPresence`].
///
/// Inputs:
/// - `occurrences`: Map built while scanning files.
///
/// Output:
/// - Map suitable for [`PacmanConfScan::repos`].
///
/// Details:
/// - Any active header forces [`PacmanRepoPresence::Active`].
fn fold_occurrences_map(
    occurrences: HashMap<String, Vec<Occurrence>>,
) -> HashMap<String, PacmanRepoPresence> {
    occurrences
        .into_iter()
        .map(|(k, v)| (k, fold_one_repo(&v)))
        .collect()
}

/// What: Merge occurrences for one repository name.
///
/// Inputs:
/// - `items`: Non-empty list of occurrences for that name.
///
/// Output:
/// - Merged [`PacmanRepoPresence`].
///
/// Details:
/// - Prefers active over commented; picks a representative source path.
fn fold_one_repo(items: &[Occurrence]) -> PacmanRepoPresence {
    if items.is_empty() {
        return PacmanRepoPresence::Absent;
    }
    if items.iter().any(|o| o.active) {
        PacmanRepoPresence::Active {
            source: items.iter().find(|o| o.active).map(|o| o.path.clone()),
        }
    } else {
        PacmanRepoPresence::Commented {
            source: items.first().map(|o| o.path.clone()),
        }
    }
}

/// What: Recursively read a pacman config file and follow includes.
///
/// Inputs:
/// - `path`: File to read.
/// - `depth`: Current include depth.
/// - `visited`: Canonical paths already parsed.
/// - `occurrences`: Aggregated section headers.
/// - `warnings`: Diagnostic messages.
///
/// Output:
/// - None.
fn scan_file_recursive(
    path: &Path,
    depth: usize,
    visited: &mut HashSet<PathBuf>,
    occurrences: &mut HashMap<String, Vec<Occurrence>>,
    warnings: &mut Vec<String>,
) {
    if depth > MAX_INCLUDE_DEPTH {
        warnings.push(format!(
            "pacman.conf: max Include depth ({MAX_INCLUDE_DEPTH}) reached at {}",
            path.display()
        ));
        return;
    }

    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if visited.contains(&canon) {
        warnings.push(format!(
            "pacman.conf: skipping duplicate Include {}",
            path.display()
        ));
        return;
    }

    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warnings.push(format!(
                "pacman.conf: could not read {}: {e}",
                path.display()
            ));
            return;
        }
    };

    visited.insert(canon);

    let mut pending_includes: Vec<PathBuf> = Vec::new();
    collect_from_content(&content, path, occurrences, &mut pending_includes);

    for inc in pending_includes {
        scan_file_recursive(&inc, depth + 1, visited, occurrences, warnings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn active_repo_recorded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("pacman.conf");
        std::fs::write(
            &main,
            "[options]\n[chaotic-aur]\nServer = https://example.invalid\n",
        )
        .expect("write");
        let scan = scan_pacman_conf_path(&main);
        assert!(matches!(
            scan.presence_of("chaotic-aur"),
            PacmanRepoPresence::Active { .. }
        ));
    }

    #[test]
    fn commented_repo_recorded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("pacman.conf");
        std::fs::write(&main, "# [endeavouros]\n").expect("write");
        let scan = scan_pacman_conf_path(&main);
        assert!(matches!(
            scan.presence_of("endeavouros"),
            PacmanRepoPresence::Commented { .. }
        ));
    }

    #[test]
    fn include_pulls_in_child_sections() {
        let dir = tempfile::tempdir().expect("tempdir");
        let child = dir.path().join("extra.conf");
        std::fs::write(&child, "[customrepo]\nServer = https://x.test\n").expect("write");
        let main = dir.path().join("pacman.conf");
        std::fs::write(
            &main,
            format!(
                "Include = {}\n",
                child.file_name().expect("name").to_str().expect("utf8")
            ),
        )
        .expect("write");
        let scan = scan_pacman_conf_path(&main);
        assert!(matches!(
            scan.presence_of("customrepo"),
            PacmanRepoPresence::Active { .. }
        ));
    }

    #[test]
    fn active_beats_commented_across_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let child = dir.path().join("b.conf");
        std::fs::write(&child, "[same]\n").expect("write");
        let main = dir.path().join("pacman.conf");
        let mut f = std::fs::File::create(&main).expect("create");
        writeln!(f, "# [same]").expect("write");
        writeln!(
            f,
            "Include = {}",
            child.file_name().expect("n").to_str().expect("utf8")
        )
        .expect("write");
        drop(f);
        let scan = scan_pacman_conf_path(&main);
        assert!(matches!(
            scan.presence_of("same"),
            PacmanRepoPresence::Active { .. }
        ));
    }

    #[test]
    fn options_section_not_a_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let main = dir.path().join("pacman.conf");
        std::fs::write(&main, "[options]\nHoldPkg = pacman glibc\n").expect("write");
        let scan = scan_pacman_conf_path(&main);
        assert!(matches!(
            scan.presence_of("options"),
            PacmanRepoPresence::Absent
        ));
    }
}
