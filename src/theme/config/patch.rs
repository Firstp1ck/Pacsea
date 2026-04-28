//! Result-returning, dry-run-aware config patch foundation.
//!
//! What: Provides `patch_key` and atomic-write helpers that preserve comments
//! and unknown keys when changing one value in a Pacsea `*.conf` file.
//!
//! Used as the substrate for the integrated config editor described in
//! `dev/IMPROVEMENTS/IMPLEMENTATION_PLAN_tui_integrated_config_editing.md`.
//! Existing fire-and-forget `save_*` helpers in [`crate::theme::config::settings_save`]
//! continue to live alongside this module; new editor code paths should prefer
//! [`patch_key`] so failures are surfaced to the UI and dry-run is honored.

use std::fs;
use std::path::{Path, PathBuf};

use crate::theme::config::skeletons::{
    KEYBINDS_SKELETON_CONTENT, REPOS_SKELETON_CONTENT, SETTINGS_SKELETON_CONTENT,
    THEME_SKELETON_CONTENT,
};
use crate::theme::paths::{
    config_dir, resolve_keybinds_config_path, resolve_repos_config_path,
    resolve_settings_config_path, resolve_theme_config_path,
};

/// Identifies which Pacsea config file a patch targets.
///
/// Used by [`patch_key`] and [`PatchRequest`] to resolve the on-disk location
/// and, when bootstrapping a missing/empty file, to seed it from the matching
/// skeleton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigFile {
    /// `settings.conf` — user preferences (toggles, layout, mirrors, scans, …).
    Settings,
    /// `keybinds.conf` — keychord-to-action mappings.
    Keybinds,
    /// `theme.conf` — theme color overrides.
    Theme,
    /// `repos.conf` — additional repository recipes.
    Repos,
}

impl ConfigFile {
    /// What: Skeleton content used when bootstrapping a missing file.
    ///
    /// Inputs:
    /// - `self`: target file enum value.
    ///
    /// Output:
    /// - Static skeleton string.
    ///
    /// Details:
    /// - Mirrors the seeding behavior of the existing `save_*` helpers and the
    ///   `ensure_*` migration paths, so a fresh patch on a missing file produces
    ///   the same starting content the rest of the app expects.
    pub(crate) const fn skeleton(self) -> &'static str {
        match self {
            Self::Settings => SETTINGS_SKELETON_CONTENT,
            Self::Keybinds => KEYBINDS_SKELETON_CONTENT,
            Self::Theme => THEME_SKELETON_CONTENT,
            Self::Repos => REPOS_SKELETON_CONTENT,
        }
    }

    /// What: Default filename inside `~/.config/pacsea/`.
    ///
    /// Inputs:
    /// - `self`: target file enum value.
    ///
    /// Output:
    /// - Filename like `settings.conf`.
    ///
    /// Details:
    /// - Used as the leaf name when falling back to `XDG_CONFIG_HOME`/`HOME` if
    ///   the file does not yet exist on disk.
    const fn default_filename(self) -> &'static str {
        match self {
            Self::Settings => "settings.conf",
            Self::Keybinds => "keybinds.conf",
            Self::Theme => "theme.conf",
            Self::Repos => "repos.conf",
        }
    }

    /// What: Try the existing path resolver for this config file.
    ///
    /// Inputs:
    /// - `self`: target file enum value.
    ///
    /// Output:
    /// - `Some(PathBuf)` when an existing file (or canonical fallback) was located.
    /// - `None` when no resolver candidate matched.
    fn try_resolve(self) -> Option<PathBuf> {
        match self {
            Self::Settings => resolve_settings_config_path(),
            Self::Keybinds => resolve_keybinds_config_path(),
            Self::Theme => resolve_theme_config_path(),
            Self::Repos => resolve_repos_config_path(),
        }
    }
}

/// Description of a single key/value patch against one config file.
///
/// `aliases` enables forward migration: when a deprecated key is encountered on
/// disk it is rewritten to `key` while keeping the surrounding comments intact.
#[derive(Debug, Clone)]
pub struct PatchRequest<'a> {
    /// Which config file to update.
    pub file: ConfigFile,
    /// Canonical (output) key name. Will be used as the left-hand side when
    /// the line is rewritten or appended.
    pub key: &'a str,
    /// Alternate keys (already-deprecated names) that should also match an
    /// existing line. Use the same casing convention as the file; matching is
    /// done after normalization (lowercase, `[. - <space>]` -> `_`).
    pub aliases: &'a [&'a str],
    /// Value to write on the right-hand side. Caller is responsible for
    /// formatting (e.g. `"true"`/`"false"`, hex colors, `sort_mode` keys, …).
    pub value: &'a str,
    /// When `true`, compute the diff and target path but do not modify disk.
    pub dry_run: bool,
}

/// Successful outcome of a [`patch_key`] call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchOutcome {
    /// Disk already held the requested value; no I/O performed.
    NoChange {
        /// Resolved absolute path that was inspected.
        path: PathBuf,
    },
    /// Value was written atomically.
    Written {
        /// Resolved absolute path that was updated.
        path: PathBuf,
    },
    /// Dry-run mode: this is the content that would have been written.
    DryRun {
        /// Resolved absolute path that would have been updated.
        path: PathBuf,
        /// Full content that would have replaced the file.
        proposed: String,
    },
}

/// Failure modes of [`patch_key`].
#[derive(Debug)]
pub enum ConfigWriteError {
    /// Filesystem I/O failure during read, temp write, or atomic rename.
    Io {
        /// Path involved in the failed I/O operation (target or temp file).
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// Validation rejected the request before any I/O. The string explains
    /// which constraint failed (e.g. "key must be non-empty").
    Invalid(String),
}

impl std::fmt::Display for ConfigWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "I/O error on {}: {source}", path.display())
            }
            Self::Invalid(msg) => write!(f, "invalid patch request: {msg}"),
        }
    }
}

impl std::error::Error for ConfigWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

/// What: Normalize a config key the same way the rest of Pacsea does.
///
/// Inputs:
/// - `key`: Raw key string from the config file or a request.
///
/// Output:
/// - Lowercased, underscore-normalized owned string.
///
/// Details:
/// - Mirrors the inline normalization used in `settings_save` and
///   `settings_ensure` so behavior stays identical across modules.
fn normalize_key(key: &str) -> String {
    key.trim().to_lowercase().replace(['.', '-', ' '], "_")
}

/// What: Resolve a target path, falling back to `XDG_CONFIG_HOME`/`HOME` when
/// no candidate file exists yet.
///
/// Inputs:
/// - `file`: The target config file kind.
///
/// Output:
/// - Resolved absolute path. Falls back to `config_dir()` (which itself
///   defaults to a sensible XDG path) when no resolver candidate matches.
///
/// Details:
/// - Matches the fallback logic used by existing fire-and-forget helpers so the
///   patch API and the `save_*` functions write to the same file.
fn resolve_path(file: ConfigFile) -> PathBuf {
    if let Some(p) = file.try_resolve() {
        return p;
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| Path::new(&h).join(".config"))
        });
    if let Some(base) = base {
        return base.join("pacsea").join(file.default_filename());
    }
    config_dir().join(file.default_filename())
}

/// What: Resolve the active path for a Pacsea config file.
///
/// Inputs:
/// - `file`: Target config file kind.
///
/// Output:
/// - Absolute path the patch layer will read/write for this file.
///
/// Details:
/// - Uses the same resolution logic as [`patch_key`], including existing-file
///   discovery (split files and legacy fallbacks) and default path fallback.
#[must_use]
pub fn resolved_config_path(file: ConfigFile) -> PathBuf {
    resolve_path(file)
}

/// What: Read the existing file as a list of lines, seeding from the skeleton
/// when the file is missing or empty.
///
/// Inputs:
/// - `path`: Resolved target path.
/// - `file`: Config file kind, used to pick the skeleton when seeding.
///
/// Output:
/// - `Ok(Vec<String>)` of lines (skeleton or existing content).
/// - `ConfigWriteError::Io` if the file exists but cannot be read.
fn read_or_seed_lines(path: &Path, file: ConfigFile) -> Result<Vec<String>, ConfigWriteError> {
    let meta = fs::metadata(path).ok();
    let exists = meta.is_some();
    let empty = meta.is_none_or(|m| m.len() == 0);
    if !exists || empty {
        return Ok(file.skeleton().lines().map(ToString::to_string).collect());
    }
    match fs::read_to_string(path) {
        Ok(content) => Ok(content.lines().map(ToString::to_string).collect()),
        Err(source) => Err(ConfigWriteError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// What: Replace an existing line for `primary` (or any alias) with the desired
/// `key = value` pair, in place.
///
/// Inputs:
/// - `lines`: Mutable line buffer, modified in place.
/// - `primary`: Canonical key name (used both for matching and for the rewritten line).
/// - `aliases`: Already-normalized alias list to also match.
/// - `value`: Value to serialize.
///
/// Output:
/// - `true` if at least one line was rewritten; `false` if the key was not found.
fn replace_in_place(lines: &mut [String], primary: &str, aliases: &[String], value: &str) -> bool {
    let primary_norm = normalize_key(primary);
    let mut replaced = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        let Some(eq) = trimmed.find('=') else {
            continue;
        };
        let key = normalize_key(&trimmed[..eq]);
        if key == primary_norm || aliases.iter().any(|a| a == &key) {
            *line = format!("{primary} = {value}");
            replaced = true;
        }
    }
    replaced
}

/// What: Join lines back into a single string, ensuring non-empty content has a
/// trailing newline.
///
/// Inputs:
/// - `lines`: Final ordered list of lines.
/// - `fallback_key`: When `lines` is empty, this key/value pair seeds a
///   one-line file.
/// - `fallback_value`: See `fallback_key`.
///
/// Output:
/// - Full file content as `String`.
fn lines_to_content(lines: &[String], fallback_key: &str, fallback_value: &str) -> String {
    if lines.is_empty() {
        return format!("{fallback_key} = {fallback_value}\n");
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// What: Atomically replace `path` with `content`.
///
/// Inputs:
/// - `path`: Target path. Parent directory is created on demand.
/// - `content`: Bytes to write.
///
/// Output:
/// - `Ok(())` on success.
/// - `ConfigWriteError::Io` describing the failed step.
///
/// Details:
/// - Writes to a sibling temp file with `create_new(true)` (refuses to follow a
///   symlink) and `rename`s into place so partially-written files never become
///   visible. On Unix, the temp file is created with mode `0o600` to avoid
///   leaking secrets such as `virustotal_api_key` while the rename is racing.
fn atomic_write(path: &Path, content: &str) -> Result<(), ConfigWriteError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|source| ConfigWriteError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
    }
    let tmp = make_temp_path(path);
    write_temp_file(&tmp, content)?;
    fs::rename(&tmp, path).map_err(|source| {
        // Best-effort cleanup so we don't leave a stray temp file behind.
        let _ = fs::remove_file(&tmp);
        ConfigWriteError::Io {
            path: path.to_path_buf(),
            source,
        }
    })?;
    Ok(())
}

/// What: Build a unique temp path adjacent to `target`.
///
/// Inputs:
/// - `target`: Final destination file.
///
/// Output:
/// - Path in the same directory using PID and nanosecond timestamp as suffix.
///
/// Details:
/// - Same-directory placement guarantees that `rename` is atomic on the same
///   filesystem, which is the universal case for `~/.config/pacsea/`.
fn make_temp_path(target: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let pid = std::process::id();
    let leaf = target.file_name().map_or_else(
        || "pacsea.conf".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let dir = target
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    dir.join(format!(".{leaf}.tmp.{pid}.{nanos}"))
}

/// What: Write `content` to a fresh temp file with restrictive permissions.
///
/// Inputs:
/// - `tmp`: Temp file path; must not yet exist.
/// - `content`: Bytes to write.
///
/// Output:
/// - `Ok(())` on success or `ConfigWriteError::Io` describing the failure.
///
/// Details:
/// - Uses `create_new(true)` so a pre-existing temp file or symlink causes the
///   call to fail rather than overwrite something unexpected.
/// - Calls `sync_all` to flush data and metadata before the rename so a crash
///   cannot leave a zero-length file.
fn write_temp_file(tmp: &Path, content: &str) -> Result<(), ConfigWriteError> {
    use std::io::Write;
    let mut opts = fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(tmp).map_err(|source| ConfigWriteError::Io {
        path: tmp.to_path_buf(),
        source,
    })?;
    f.write_all(content.as_bytes())
        .map_err(|source| ConfigWriteError::Io {
            path: tmp.to_path_buf(),
            source,
        })?;
    f.sync_all().map_err(|source| ConfigWriteError::Io {
        path: tmp.to_path_buf(),
        source,
    })?;
    Ok(())
}

/// What: Apply a single key/value patch to a Pacsea config file.
///
/// Inputs:
/// - `req`: [`PatchRequest`] describing target file, key (with aliases), value,
///   and dry-run flag.
///
/// Output:
/// - [`PatchOutcome::NoChange`] when the file already holds the requested value.
/// - [`PatchOutcome::Written`] when the file was updated atomically.
/// - [`PatchOutcome::DryRun`] when `req.dry_run` is `true` and a change would
///   have been made.
///
/// # Errors
/// - [`ConfigWriteError::Invalid`] when `req.key` is empty after trimming.
/// - [`ConfigWriteError::Io`] when reading the existing file or performing the
///   atomic temp-file write/rename fails.
///
/// Details:
/// - Comments and unrelated keys are preserved: only the matched line is
///   rewritten in place. When no existing line matches, the canonical
///   `key = value` pair is appended.
/// - Aliases are migrated in the same pass: an alias line on disk is rewritten
///   with the primary key.
/// - File is bootstrapped from the matching skeleton when missing or empty.
/// - Writes go through [`atomic_write`] (temp file + rename, mode `0o600` on
///   Unix), so a partially-written file is never visible.
pub fn patch_key(req: &PatchRequest<'_>) -> Result<PatchOutcome, ConfigWriteError> {
    if req.key.trim().is_empty() {
        return Err(ConfigWriteError::Invalid(
            "patch key must not be empty".into(),
        ));
    }
    let path = resolve_path(req.file);
    let alias_norms: Vec<String> = req
        .aliases
        .iter()
        .map(|a| normalize_key(a))
        .filter(|a| !a.is_empty())
        .collect();
    let mut lines = read_or_seed_lines(&path, req.file)?;
    let replaced = replace_in_place(&mut lines, req.key, &alias_norms, req.value);
    if !replaced {
        lines.push(format!("{} = {}", req.key, req.value));
    }
    let new_content = lines_to_content(&lines, req.key, req.value);

    // Compare against current on-disk state to short-circuit no-op writes.
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() == Some(new_content.as_str()) {
        return Ok(PatchOutcome::NoChange { path });
    }

    if req.dry_run {
        return Ok(PatchOutcome::DryRun {
            path,
            proposed: new_content,
        });
    }
    atomic_write(&path, &new_content)?;
    Ok(PatchOutcome::Written { path })
}

/// What: Atomically replace the entire content of a config file.
///
/// Inputs:
/// - `file`: Target [`ConfigFile`].
/// - `content`: Full new content.
/// - `dry_run`: When `true`, do not modify disk.
///
/// Output:
/// - [`PatchOutcome`] mirroring [`patch_key`] semantics.
///
/// # Errors
/// - [`ConfigWriteError::Io`] when the atomic temp-file write or rename fails.
///
/// Details:
/// - Used by callers that have already produced a fully serialized buffer
///   (e.g. theme pre-commit validation that writes to a temp path, validates,
///   then commits).
pub fn write_full_content(
    file: ConfigFile,
    content: &str,
    dry_run: bool,
) -> Result<PatchOutcome, ConfigWriteError> {
    let path = resolve_path(file);
    let current = fs::read_to_string(&path).ok();
    if current.as_deref() == Some(content) {
        return Ok(PatchOutcome::NoChange { path });
    }
    if dry_run {
        return Ok(PatchOutcome::DryRun {
            path,
            proposed: content.to_string(),
        });
    }
    atomic_write(&path, content)?;
    Ok(PatchOutcome::Written { path })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// What: RAII guard that points HOME/XDG to a temp dir and restores them.
    ///
    /// Details:
    /// - Mirrors the helper inside `theme/config/tests.rs` but lives here so
    ///   patch tests can be self-contained.
    struct EnvGuard {
        base: PathBuf,
        cfg_dir: PathBuf,
        orig_home: Option<std::ffi::OsString>,
        orig_xdg: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn new(tag: &str) -> Self {
            let orig_home = std::env::var_os("HOME");
            let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");
            let base = std::env::temp_dir().join(format!(
                "pacsea_patch_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
            let cfg_dir = base.join(".config").join("pacsea");
            fs::create_dir_all(&cfg_dir).expect("create cfg dir");
            unsafe {
                std::env::set_var("HOME", base.display().to_string());
                std::env::remove_var("XDG_CONFIG_HOME");
            }
            Self {
                base,
                cfg_dir,
                orig_home,
                orig_xdg,
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            unsafe {
                if let Some(v) = self.orig_home.as_ref() {
                    std::env::set_var("HOME", v);
                } else {
                    std::env::remove_var("HOME");
                }
                if let Some(v) = self.orig_xdg.as_ref() {
                    std::env::set_var("XDG_CONFIG_HOME", v);
                } else {
                    std::env::remove_var("XDG_CONFIG_HOME");
                }
            }
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    #[test]
    fn patch_dry_run_does_not_touch_disk() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("dry_run");
        let path = env.cfg_dir.join("settings.conf");
        fs::write(&path, "sort_mode = best_matches\n").expect("write");

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &["results_sort"],
            value: "alphabetical",
            dry_run: true,
        };
        let outcome = patch_key(&req).expect("patch ok");
        match outcome {
            PatchOutcome::DryRun { path: p, proposed } => {
                assert_eq!(p, path);
                assert!(
                    proposed.contains("sort_mode = alphabetical"),
                    "proposed should contain new value, got: {proposed}"
                );
            }
            other => panic!("expected DryRun, got {other:?}"),
        }
        let actual = fs::read_to_string(&path).expect("read after dry-run");
        assert_eq!(
            actual, "sort_mode = best_matches\n",
            "dry-run must not modify disk"
        );
        drop(env);
    }

    #[test]
    fn patch_writes_and_preserves_comments() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("preserve");
        let path = env.cfg_dir.join("settings.conf");
        let original = "# header comment\n\
             # describes sort_mode\n\
             sort_mode = best_matches\n\
             # below is unrelated\n\
             show_install_pane = true\n";
        fs::write(&path, original).expect("write");

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &[],
            value: "alphabetical",
            dry_run: false,
        };
        let outcome = patch_key(&req).expect("patch ok");
        assert!(matches!(outcome, PatchOutcome::Written { .. }));

        let after = fs::read_to_string(&path).expect("read after");
        assert!(after.contains("# header comment"));
        assert!(after.contains("# describes sort_mode"));
        assert!(after.contains("sort_mode = alphabetical"));
        assert!(after.contains("show_install_pane = true"));
        assert!(!after.contains("sort_mode = best_matches"));
        drop(env);
    }

    #[test]
    fn patch_no_change_when_value_matches() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("nochange");
        let path = env.cfg_dir.join("settings.conf");
        fs::write(&path, "sort_mode = alphabetical\n").expect("write");

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &[],
            value: "alphabetical",
            dry_run: false,
        };
        let outcome = patch_key(&req).expect("patch ok");
        assert!(matches!(outcome, PatchOutcome::NoChange { .. }));
        drop(env);
    }

    #[test]
    fn patch_migrates_alias_to_primary_key() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("alias");
        let path = env.cfg_dir.join("settings.conf");
        fs::write(&path, "results_sort = best_matches\n").expect("write");

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &["results_sort"],
            value: "alphabetical",
            dry_run: false,
        };
        let outcome = patch_key(&req).expect("patch ok");
        assert!(matches!(outcome, PatchOutcome::Written { .. }));

        let after = fs::read_to_string(&path).expect("read after");
        assert!(
            after.contains("sort_mode = alphabetical"),
            "alias should migrate to primary key, got: {after}"
        );
        assert!(
            !after.contains("results_sort = best_matches"),
            "alias line should be replaced, got: {after}"
        );
        drop(env);
    }

    #[test]
    fn patch_appends_when_key_missing() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("append");
        let path = env.cfg_dir.join("settings.conf");
        fs::write(&path, "show_install_pane = true\n").expect("write");

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &[],
            value: "alphabetical",
            dry_run: false,
        };
        let outcome = patch_key(&req).expect("patch ok");
        assert!(matches!(outcome, PatchOutcome::Written { .. }));

        let after = fs::read_to_string(&path).expect("read after");
        assert!(after.contains("show_install_pane = true"));
        assert!(after.contains("sort_mode = alphabetical"));
        drop(env);
    }

    #[test]
    fn patch_seeds_skeleton_when_file_missing() {
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("seed");
        let path = env.cfg_dir.join("settings.conf");
        assert!(!path.exists());

        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &[],
            value: "alphabetical",
            dry_run: false,
        };
        let outcome = patch_key(&req).expect("patch ok");
        assert!(matches!(outcome, PatchOutcome::Written { .. }));
        let after = fs::read_to_string(&path).expect("read after");
        // Skeleton text should be present (header sentinel comment)
        assert!(
            after.contains("Pacsea settings"),
            "should bootstrap from skeleton, got: {after}"
        );
        assert!(after.contains("sort_mode = alphabetical"));
        drop(env);
    }

    #[test]
    fn patch_rejects_empty_key() {
        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "  ",
            aliases: &[],
            value: "x",
            dry_run: true,
        };
        let err = patch_key(&req).expect_err("should reject empty key");
        assert!(matches!(err, ConfigWriteError::Invalid(_)));
    }

    #[test]
    #[cfg(unix)]
    fn atomic_write_uses_restrictive_mode() {
        use std::os::unix::fs::PermissionsExt;
        let _g = crate::theme::test_mutex().lock().expect("mutex");
        let env = EnvGuard::new("mode");
        let path = env.cfg_dir.join("settings.conf");
        let req = PatchRequest {
            file: ConfigFile::Settings,
            key: "sort_mode",
            aliases: &[],
            value: "alphabetical",
            dry_run: false,
        };
        patch_key(&req).expect("patch ok");
        let mode = fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "atomic write should leave file mode 0o600"
        );
        drop(env);
    }
}
