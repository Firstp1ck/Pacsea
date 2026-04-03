//! Build data for the read-only Repositories modal.

use std::collections::HashSet;
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
    let key_index = pacman_trusted_key_index();

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
                            classify_key(kid, key_index.as_ref())
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

/// What: Index of `OpenPGP` v4-style fingerprints from `pacman-key --list-keys` output.
///
/// Inputs:
/// - Built only via [`TrustedKeyIndex::from_pacman_list_keys_stdout`].
///
/// Output:
/// - N/A (data container).
///
/// Details:
/// - Stores full 40-hex fingerprints plus `gpg`-style long (16) and short (8) key ids as **suffixes**
///   of those fingerprints so lookups are token-bounded instead of substring checks on merged hex.
struct TrustedKeyIndex {
    /// Uppercase 40-hex fingerprints parsed from listing lines.
    full_fingerprints: HashSet<String>,
    /// Last 16 hex digits of each fingerprint (`gpg` long key id).
    long_key_ids: HashSet<String>,
    /// Last 8 hex digits of each fingerprint (`gpg` short key id).
    short_key_ids: HashSet<String>,
}

impl TrustedKeyIndex {
    /// What: Parse human-readable `pacman-key --list-keys` stdout into fingerprint/id sets.
    ///
    /// Inputs:
    /// - `stdout`: Raw listing text (`pacman-key` / gpg human format).
    ///
    /// Output:
    /// - Populated index (possibly empty when no 40-hex runs are present).
    ///
    /// Details:
    /// - Treats each **maximal** contiguous `ASCII` hex run on a line as a candidate; only runs of
    ///   length **40** are fingerprints (matching `gpg` human listings of primary key material).
    /// - Shorter/longer runs are ignored so arbitrary in-fingerprint substrings cannot register as
    ///   trusted key ids.
    fn from_pacman_list_keys_stdout(stdout: &str) -> Self {
        let mut full_fingerprints = HashSet::new();
        let mut long_key_ids = HashSet::new();
        let mut short_key_ids = HashSet::new();
        for line in stdout.lines() {
            for run in hex_digit_runs(line) {
                if run.len() == 40 {
                    let fp = run.to_uppercase();
                    long_key_ids.insert(fp[24..40].to_string());
                    short_key_ids.insert(fp[32..40].to_string());
                    full_fingerprints.insert(fp);
                }
            }
        }
        Self {
            full_fingerprints,
            long_key_ids,
            short_key_ids,
        }
    }
}

/// What: Extract maximal contiguous ASCII hexadecimal runs from one text line.
///
/// Inputs:
/// - `line`: Single line from gpg-style key listing output.
///
/// Output:
/// - Borrowed substrings of `line` that are hex-only runs.
///
/// Details:
/// - Keeps fingerprint tokens line-bounded; callers filter by run length.
fn hex_digit_runs(line: &str) -> Vec<&str> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in line.char_indices() {
        if c.is_ascii_hexdigit() {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(st) = start.take() {
            runs.push(&line[st..i]);
        }
    }
    if let Some(st) = start {
        runs.push(&line[st..]);
    }
    runs
}

/// What: Run `pacman-key --list-keys` once and parse fingerprints for trust checks.
///
/// Inputs:
/// - None (uses `PATH` to find `pacman-key`).
///
/// Output:
/// - `None` when the tool is missing or the invocation fails.
///
/// Details:
/// - Parses discrete 40-hex fingerprint tokens per line so matches cannot span key boundaries or
///   arbitrary positions inside a fingerprint.
fn pacman_trusted_key_index() -> Option<TrustedKeyIndex> {
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
    Some(TrustedKeyIndex::from_pacman_list_keys_stdout(
        &String::from_utf8_lossy(&out.stdout),
    ))
}

/// What: Decide if a configured `key_id` appears in the pacman keyring listing.
///
/// Inputs:
/// - `key_id`: Value from `repos.conf`.
/// - `index`: Output of [`pacman_trusted_key_index`].
///
/// Output:
/// - [`RepositoryKeyTrust`] variant.
///
/// Details:
/// - Requires at least 8 hex digits after normalization (same floor as repo key refresh).
/// - **40** hex chars: exact fingerprint match only.
/// - **16** hex chars: `gpg` long key id (last 16 of a listed fingerprint) only.
/// - **8** hex chars: `gpg` short key id (last 8 of a listed fingerprint) only.
/// - Other lengths are **Unknown** so partial in-fingerprint substrings cannot mark a key trusted.
fn classify_key(key_id: &str, index: Option<&TrustedKeyIndex>) -> RepositoryKeyTrust {
    let needle: String = key_id
        .chars()
        .filter(char::is_ascii_hexdigit)
        .collect::<String>()
        .to_uppercase();
    if needle.len() < 8 {
        return RepositoryKeyTrust::Unknown;
    }
    let Some(idx) = index else {
        return RepositoryKeyTrust::Unknown;
    };
    let trusted = match needle.len() {
        40 => idx.full_fingerprints.contains(&needle),
        16 => idx.long_key_ids.contains(&needle),
        8 => idx.short_key_ids.contains(&needle),
        _ => return RepositoryKeyTrust::Unknown,
    };
    if trusted {
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
    fn classify_key_rejects_short_id_not_matching_any_listed_fingerprint_suffix() {
        // Two valid 40-hex fingerprints whose short ids are all-1 / all-2; "5678ABCD" must not match
        // via substring tricks across separate keys.
        let stdout = concat!(
            "1111111111111111111111111111111111111111\n",
            "2222222222222222222222222222222222222222\n",
        );
        let index = TrustedKeyIndex::from_pacman_list_keys_stdout(stdout);
        assert_eq!(
            classify_key("5678ABCD", Some(&index)),
            RepositoryKeyTrust::NotTrusted
        );
    }

    #[test]
    fn classify_key_rejects_short_id_that_exists_only_across_merged_hex_runs() {
        // Regression: a single hex-only blob of the whole listing would contain "5678ABCD" across
        // the join between fp1’s tail (...12345678) and fp2’s head (ABCDEF01...).
        let stdout = concat!(
            "pub\n      0000000000000000000000000000000012345678\n",
            "uid           a <a@test>\n",
            "\n",
            "pub\n      ABCDEF0123456789ABCDEF0123456789012345678\n",
        );
        let index = TrustedKeyIndex::from_pacman_list_keys_stdout(stdout);
        assert_eq!(
            classify_key("5678ABCD", Some(&index)),
            RepositoryKeyTrust::NotTrusted
        );
    }

    #[test]
    fn classify_key_trusted_when_short_id_matches_listed_fingerprint_suffix() {
        let stdout = "0000000000000000000000000000000012345678\n";
        let index = TrustedKeyIndex::from_pacman_list_keys_stdout(stdout);
        assert_eq!(
            classify_key("12345678", Some(&index)),
            RepositoryKeyTrust::Trusted
        );
    }

    #[test]
    fn classify_key_unknown_when_normalized_id_shorter_than_eight_hex() {
        let stdout = "0000000000000000000000000000000012345678\n";
        let index = TrustedKeyIndex::from_pacman_list_keys_stdout(stdout);
        assert_eq!(
            classify_key("1234567", Some(&index)),
            RepositoryKeyTrust::Unknown
        );
    }

    #[test]
    fn classify_key_unknown_when_keyring_index_unavailable() {
        assert_eq!(classify_key("12345678", None), RepositoryKeyTrust::Unknown);
    }

    #[test]
    fn classify_accepts_full_long_and_short_key_forms() {
        let fp = "0123456789ABCDEF0123456789ABCDEF01234567";
        let listing = format!("pub rsa4096 2020-01-01 [SC]\n      {fp}\n");
        let idx = TrustedKeyIndex::from_pacman_list_keys_stdout(&listing);
        assert_eq!(classify_key(fp, Some(&idx)), RepositoryKeyTrust::Trusted);
        assert_eq!(
            classify_key("89ABCDEF01234567", Some(&idx)),
            RepositoryKeyTrust::Trusted
        );
        assert_eq!(
            classify_key("01234567", Some(&idx)),
            RepositoryKeyTrust::Trusted
        );
    }

    #[test]
    fn classify_rejects_short_id_that_is_only_inside_fingerprint() {
        let fp = "0123456789ABCDEF0123456789ABCDEF01234567";
        let listing = format!("pub\n      {fp}\n");
        let idx = TrustedKeyIndex::from_pacman_list_keys_stdout(&listing);
        assert_eq!(
            classify_key("89ABCDEF", Some(&idx)),
            RepositoryKeyTrust::NotTrusted
        );
    }

    #[test]
    fn classify_rejects_sixteen_hex_that_is_not_long_key_id_suffix() {
        let fp = "0123456789ABCDEF0123456789ABCDEF01234567";
        let listing = format!("pub\n      {fp}\n");
        let idx = TrustedKeyIndex::from_pacman_list_keys_stdout(&listing);
        assert_eq!(
            classify_key("456789ABCDEF0123", Some(&idx)),
            RepositoryKeyTrust::NotTrusted
        );
    }

    #[test]
    fn classify_unknown_for_nonstandard_hex_lengths() {
        let fp = "0123456789ABCDEF0123456789ABCDEF01234567";
        let listing = format!("pub\n      {fp}\n");
        let idx = TrustedKeyIndex::from_pacman_list_keys_stdout(&listing);
        assert_eq!(
            classify_key("0123456789ABC", Some(&idx)),
            RepositoryKeyTrust::Unknown
        );
    }

    #[test]
    fn hex_digit_runs_splits_non_hex_separators() {
        assert_eq!(
            hex_digit_runs("      ABCD0123EFABCD0123EFABCD0123EFABCD0123"),
            vec!["ABCD0123EFABCD0123EFABCD0123EFABCD0123"]
        );
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
