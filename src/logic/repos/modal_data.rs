//! Build data for the read-only Repositories modal.

use std::path::Path;
use std::process::Command;

use crate::state::types::{RepositoryKeyTrust, RepositoryModalRow, RepositoryPacmanStatus};
use crate::theme::resolve_repos_config_path;

use super::config::load_resolve_repos_from_str;
use super::pacman_conf::{PacmanRepoPresence, scan_pacman_conf_path};

/// What: Merge `repos.conf`, pacman scan, and optional keyring snapshot into modal rows.
///
/// Inputs:
/// - `repos_path`: Resolved `repos.conf` path, if any.
/// - `pacman_conf_path`: Usually `/etc/pacman.conf`.
///
/// Output:
/// - `(rows, repos_conf_error, pacman_warnings)` for the Repositories modal UI.
///
/// Details:
/// - When `repos_path` is `None`, rows stay empty (user has no config file yet).
/// - Uses one batched `pacman-key --list-keys` when possible for fingerprint checks.
pub fn build_repositories_modal_fields(
    repos_path: Option<&Path>,
    pacman_conf_path: &Path,
) -> (Vec<RepositoryModalRow>, Option<String>, Vec<String>) {
    let scan = scan_pacman_conf_path(pacman_conf_path);
    let pacman_warnings = scan.warnings.clone();
    let key_blob = normalized_trusted_key_blob();

    let mut repos_conf_error: Option<String> = None;
    let mut rows: Vec<RepositoryModalRow> = Vec::new();

    let content_opt: Option<String> = repos_path.and_then(|p| match std::fs::read_to_string(p) {
        Ok(c) => Some(c),
        Err(e) => {
            repos_conf_error = Some(format!("Could not read repos.conf ({}): {e}", p.display()));
            None
        }
    });

    if let Some(content) = content_opt {
        match load_resolve_repos_from_str(&content) {
            Ok((repo_rows, _map)) => {
                for r in repo_rows {
                    let name = r.name.as_deref().unwrap_or("").trim().to_string();
                    let rf = r.results_filter.as_deref().unwrap_or("").trim().to_string();
                    let presence = scan.presence_of(&name);
                    let (pacman_status, source_hint) = map_presence(presence);
                    let key_trust = r
                        .key_id
                        .as_deref()
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map_or(RepositoryKeyTrust::NotApplicable, |kid| {
                            classify_key(kid, key_blob.as_deref())
                        });
                    rows.push(RepositoryModalRow {
                        pacman_section_name: name,
                        results_filter_display: rf,
                        pacman_status,
                        source_hint,
                        key_trust,
                    });
                }
            }
            Err(e) => {
                repos_conf_error = Some(e);
            }
        }
    }

    (rows, repos_conf_error, pacman_warnings)
}

/// What: Convenience wrapper using resolved Pacsea paths and system `pacman.conf`.
///
/// Inputs:
/// - None (reads [`resolve_repos_config_path`] and `/etc/pacman.conf`).
///
/// Output:
/// - Same tuple as [`build_repositories_modal_fields`].
///
/// Details:
/// - Call from UI handlers when opening the modal.
#[must_use]
pub fn build_repositories_modal_fields_default()
-> (Vec<RepositoryModalRow>, Option<String>, Vec<String>) {
    build_repositories_modal_fields(
        resolve_repos_config_path().as_deref(),
        Path::new("/etc/pacman.conf"),
    )
}

/// What: Map scanner presence to UI enums and a short source hint.
///
/// Inputs:
/// - `presence`: Merged [`PacmanRepoPresence`].
///
/// Output:
/// - Status + optional file name hint.
///
/// Details:
/// - Uses the file name from the scanner when available.
fn map_presence(presence: PacmanRepoPresence) -> (RepositoryPacmanStatus, Option<String>) {
    match presence {
        PacmanRepoPresence::Absent => (RepositoryPacmanStatus::Absent, None),
        PacmanRepoPresence::Active { source } => (
            RepositoryPacmanStatus::Active,
            source.as_deref().and_then(short_source_hint),
        ),
        PacmanRepoPresence::Commented { source } => (
            RepositoryPacmanStatus::Commented,
            source.as_deref().and_then(short_source_hint),
        ),
    }
}

/// What: Reduce a path to its file name for compact modal display.
///
/// Inputs:
/// - `p`: Filesystem path from the scanner.
///
/// Output:
/// - File name string, if any.
fn short_source_hint(p: &std::path::Path) -> Option<String> {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
}

/// What: Run `pacman-key --list-keys` once and normalize output for substring checks.
///
/// Inputs:
/// - None (uses `PATH` to find `pacman-key`).
///
/// Output:
/// - `None` when the tool is missing or the invocation fails.
///
/// Details:
/// - Strips non-hex characters so fingerprints match regardless of spacing/`0x` prefixes.
fn normalized_trusted_key_blob() -> Option<String> {
    if which::which("pacman-key").is_err() {
        return None;
    }
    let out = Command::new("pacman-key")
        .arg("--list-keys")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .chars()
            .filter(char::is_ascii_hexdigit)
            .collect::<String>()
            .to_uppercase(),
    )
}

/// What: Decide if a configured `key_id` appears in the pacman keyring listing.
///
/// Inputs:
/// - `key_id`: Value from `repos.conf`.
/// - `blob`: Output of [`normalized_trusted_key_blob`].
///
/// Output:
/// - [`RepositoryKeyTrust`] variant.
///
/// Details:
/// - Requires at least 8 hex digits after normalization to reduce false positives on short ids.
fn classify_key(key_id: &str, blob: Option<&str>) -> RepositoryKeyTrust {
    let needle: String = key_id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .to_uppercase();
    if needle.len() < 8 {
        return RepositoryKeyTrust::Unknown;
    }
    let Some(b) = blob else {
        return RepositoryKeyTrust::Unknown;
    };
    if b.contains(&needle) {
        RepositoryKeyTrust::Trusted
    } else {
        RepositoryKeyTrust::NotTrusted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn merge_row_with_active_pacman_section() {
        let tmp = tempdir().expect("td");
        let repo_file = tmp.path().join("repos.conf");
        std::fs::write(
            &repo_file,
            r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
key_id = "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"
"#,
        )
        .expect("write repos");
        let pac = tmp.path().join("pacman.conf");
        std::fs::write(&pac, "[myrepo]\nServer = https://x.test\n").expect("write pacman");
        let (rows, err, _) =
            build_repositories_modal_fields(Some(repo_file.as_path()), pac.as_path());
        assert!(err.is_none(), "{err:?}");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pacman_section_name, "myrepo");
        assert_eq!(rows[0].results_filter_display, "mine");
        assert_eq!(rows[0].pacman_status, RepositoryPacmanStatus::Active);
    }

    #[test]
    fn repos_parse_error_surfaces() {
        let tmp = tempdir().expect("td");
        let repo_file = tmp.path().join("repos.conf");
        std::fs::write(
            &repo_file,
            r#"
[[repo]]
preset = "unsupported"
"#,
        )
        .expect("bad");
        let pac = tmp.path().join("pacman.conf");
        std::fs::write(&pac, "\n").expect("write");
        let (rows, err, _) =
            build_repositories_modal_fields(Some(repo_file.as_path()), pac.as_path());
        assert!(err.is_some());
        assert!(rows.is_empty());
    }
}
