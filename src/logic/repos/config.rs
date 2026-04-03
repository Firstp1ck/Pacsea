//! TOML parsing and validation for `repos.conf`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What: Root document for `repos.conf` (array of `[[repo]]` tables).
///
/// Inputs:
/// - N/A (Serde shape).
///
/// Output:
/// - Deserialized list under `repo`.
///
/// Details:
/// - TOML maps `[[repo]]` to `repo = [ ... ]`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReposConfFile {
    /// Repository entries from the file.
    #[serde(default)]
    pub repo: Vec<RepoRow>,
}

/// What: One `[[repo]]` row supplied by the user.
///
/// Inputs:
/// - N/A (Serde shape).
///
/// Output:
/// - Field bundle for validation and later apply phases.
///
/// Details:
/// - Required for the results map: `name`, `results_filter`. Other fields are reserved for Phase 3 (apply).
/// - The `preset` key is rejected: use full rows; see `config/examples/repos.conf.example` in the Pacsea repo.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct RepoRow {
    /// Stable Pacsea id (optional; for future UI / apply).
    pub id: Option<String>,
    /// Unsupported; if set, parsing fails with a pointer to the example file.
    pub preset: Option<String>,
    /// Desired enabled state for future apply flows (`None` treated as true).
    pub enabled: Option<bool>,
    /// Pacman `[repo]` section name.
    pub name: Option<String>,
    /// Logical bucket for results-list toggles (`settings.conf` / dynamic map).
    pub results_filter: Option<String>,
    /// `Server =` URL (Phase 3).
    pub server: Option<String>,
    /// `SigLevel` (Phase 3).
    pub sig_level: Option<String>,
    /// Signing key fingerprint/id (Phase 3).
    pub key_id: Option<String>,
    /// Keyserver hostname (Phase 3).
    pub key_server: Option<String>,
    /// Local mirrorlist path (Phase 3).
    pub mirrorlist: Option<String>,
    /// Remote mirrorlist URL (Phase 3).
    pub mirrorlist_url: Option<String>,
}

/// What: Normalize a `results_filter` label for map keys and `settings.conf` suffixes.
///
/// Inputs:
/// - `raw`: Value from `repos.conf` (may contain `-`, spaces, etc.).
///
/// Output:
/// - Lowercase string with non-alphanumeric runs folded to a single `_`.
///
/// Details:
/// - Matches `results_filter_show_<token>` in `settings.conf` where `token` uses the same rule.
#[must_use]
pub fn canonical_results_filter_key(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    let mut out = String::new();
    let mut prev_sep = true;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_sep = false;
        } else if !prev_sep && !out.is_empty() {
            out.push('_');
            prev_sep = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// What: Ensure a row has `name` and `results_filter`.
///
/// Inputs:
/// - `row`: Parsed `[[repo]]` entry.
///
/// Output:
/// - `Ok(())` or an error message for empty required fields.
///
/// Details:
/// - Used before the row enters the repo-name map.
fn validate_row(row: &RepoRow) -> Result<(), String> {
    let name_ok = row
        .name
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if !name_ok {
        return Err("repo `name` is missing or empty".to_string());
    }
    let rf_ok = row
        .results_filter
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if !rf_ok {
        return Err("repo `results_filter` is missing or empty".to_string());
    }
    Ok(())
}

/// What: Validate one deserialized row (no preset merge).
///
/// Inputs:
/// - `row`: Raw deserialized row.
///
/// Output:
/// - `Ok(RepoRow)` clone for resolved list, or `Err(message)`.
///
/// Details:
/// - Rejects non-empty `preset`; Pacsea does not ship an in-tree catalog.
fn finalize_row(row: &RepoRow) -> Result<RepoRow, String> {
    if row
        .preset
        .as_deref()
        .map(str::trim)
        .is_some_and(|s| !s.is_empty())
    {
        return Err(
            "repos.conf: `preset` is not supported; define a full [[repo]] block. \
             See config/examples/repos.conf.example in the Pacsea repository."
                .to_string(),
        );
    }
    validate_row(row)?;
    Ok(row.clone())
}

/// What: Build lowercase pacman repo name → canonical results-filter key.
///
/// Inputs:
/// - `rows`: Valid rows.
///
/// Output:
/// - Map for `repo_toggle_for` lookups.
///
/// # Errors
///
/// - Returns `Err` when two rows share the same case-insensitive `name`.
///
/// Details:
/// - Errors on duplicate `name` (case-insensitive).
pub fn build_repo_name_to_filter_map(rows: &[RepoRow]) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for row in rows {
        let name = row.name.as_deref().map_or("", str::trim).to_lowercase();
        if name.is_empty() {
            continue;
        }
        let rf_raw = row.results_filter.as_deref().unwrap_or("");
        let canon = canonical_results_filter_key(rf_raw);
        if map.insert(name.clone(), canon).is_some() {
            return Err(format!("duplicate repo `name` in repos.conf: {name}"));
        }
    }
    Ok(map)
}

/// What: Parse TOML, validate rows, and build the repo-name map.
///
/// Inputs:
/// - `content`: Full file contents.
///
/// Output:
/// - Resolved rows and name→filter map, or concatenated error string.
///
/// # Errors
///
/// - Invalid TOML, unsupported `preset`, validation failures, or duplicate `name` values.
///
/// Details:
/// - Empty or whitespace-only content yields empty results without error.
pub fn load_resolve_repos_from_str(
    content: &str,
) -> Result<(Vec<RepoRow>, HashMap<String, String>), String> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok((Vec::new(), HashMap::new()));
    }
    let file: ReposConfFile =
        toml::from_str(trimmed).map_err(|e| format!("repos.conf TOML: {e}"))?;
    let mut errors: Vec<String> = Vec::new();
    let mut resolved: Vec<RepoRow> = Vec::new();
    for row in file.repo {
        match finalize_row(&row) {
            Ok(r) => resolved.push(r),
            Err(e) => errors.push(e),
        }
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    let name_map = build_repo_name_to_filter_map(&resolved)?;
    Ok((resolved, name_map))
}

/// What: Load and resolve `repos.conf` from disk for app initialization.
///
/// Inputs:
/// - `path`: File path (should exist when called).
///
/// Output:
/// - Name→filter map, or empty map on read/parse failure (warnings logged).
///
/// Details:
/// - IO errors and parse errors are non-fatal for startup.
pub fn load_repo_name_map_from_path(path: &Path) -> HashMap<String, String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        tracing::warn!(path = %path.display(), "repos.conf: read failed");
        return HashMap::new();
    };
    match load_resolve_repos_from_str(&content) {
        Ok((_, m)) => m,
        Err(e) => {
            tracing::warn!(path = %path.display(), err = %e, "repos.conf: parse failed");
            HashMap::new()
        }
    }
}

/// What: Merge per-filter toggles from `settings.conf` with defaults for all ids from repos.
///
/// Inputs:
/// - `toggles`: Parsed `results_filter_show_*` entries (canonical key → bool).
/// - `repo_name_to_filter`: Lowercase pacman repo name → canonical filter key.
///
/// Output:
/// - Canonical filter key → visible in results.
///
/// Details:
/// - Default is `true` when a key is absent from `toggles`.
#[allow(clippy::implicit_hasher)]
#[must_use]
pub fn build_dynamic_visibility(
    toggles: &HashMap<String, bool>,
    repo_name_to_filter: &HashMap<String, String>,
) -> HashMap<String, bool> {
    let ids: HashSet<String> = repo_name_to_filter.values().cloned().collect();
    let mut out = HashMap::new();
    for id in ids {
        let v = toggles.get(&id).copied().unwrap_or(true);
        out.insert(id, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_collapses_separators() {
        assert_eq!(canonical_results_filter_key("vendor-aur"), "vendor_aur");
        assert_eq!(canonical_results_filter_key("  Foo..Bar  "), "foo_bar");
    }

    #[test]
    fn full_repo_row_builds_map() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "vendor_pkgs"
"#;
        let (_rows, map) = load_resolve_repos_from_str(toml).expect("parse");
        assert_eq!(map.get("myrepo").map(String::as_str), Some("vendor_pkgs"));
    }

    #[test]
    fn duplicate_name_errors() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "a"

[[repo]]
name = "myrepo"
results_filter = "b"
"#;
        let err = load_resolve_repos_from_str(toml).expect_err("dup");
        assert!(err.contains("duplicate"));
    }

    #[test]
    fn preset_key_is_rejected() {
        let toml = r#"
[[repo]]
preset = "anything"
"#;
        let err = load_resolve_repos_from_str(toml).expect_err("preset");
        assert!(err.contains("preset"));
    }

    #[test]
    fn dynamic_visibility_defaults_true() {
        let mut repo = HashMap::new();
        repo.insert("foo".to_string(), "bar".to_string());
        let toggles = HashMap::new();
        let v = build_dynamic_visibility(&toggles, &repo);
        assert_eq!(v.get("bar").copied(), Some(true));
    }

    #[test]
    fn dynamic_visibility_respects_settings() {
        let mut repo = HashMap::new();
        repo.insert("foo".to_string(), "bar".to_string());
        let mut toggles = HashMap::new();
        toggles.insert("bar".to_string(), false);
        let v = build_dynamic_visibility(&toggles, &repo);
        assert_eq!(v.get("bar").copied(), Some(false));
    }
}
