//! Privileged apply plan: managed `pacsea-repos.conf` drop-in, optional `pacman.conf` markers, `pacman-key`.
//!
//! Planning runs without mutating the system. Final command strings use [`crate::logic::privilege::build_privilege_command`].

use std::path::Path;

use crate::install::shell_single_quote;
use crate::logic::privilege::{PrivilegeTool, active_tool, build_privilege_command};
use crate::util::curl_args;

use super::config::{
    RepoRow, ReposConfFile, repo_row_declares_apply_sources, row_is_enabled_for_repos_conf,
};

/// What: Start-of-block marker Pacsea appends to `/etc/pacman.conf`.
pub const PACMAN_MANAGED_BEGIN: &str = "# === pacsea managed begin";
/// What: End-of-block marker Pacsea appends to `/etc/pacman.conf`.
pub const PACMAN_MANAGED_END: &str = "# === pacsea managed end";
/// What: File name for the managed drop-in under `/etc/pacman.d/`.
pub const MANAGED_DROPIN_FILE: &str = "pacsea-repos.conf";
/// What: Default absolute path for the managed drop-in.
pub const DEFAULT_DROPIN_PATH: &str = "/etc/pacman.d/pacsea-repos.conf";
/// What: Default path to the main pacman configuration.
pub const DEFAULT_MAIN_PACMAN_PATH: &str = "/etc/pacman.conf";
/// What: Default `SigLevel` written to the managed drop-in when `sig_level` is omitted in `repos.conf`.
pub const DEFAULT_DROPIN_SIG_LEVEL: &str = "Required DatabaseOptional";

/// What: Human-readable summary lines plus shell commands for [`ExecutorRequest::Update`].
///
/// Inputs:
/// - Created by [`build_repo_apply_bundle`].
///
/// Output:
/// - Shown in logs / optional UI; `commands` are chained with `&&` by the executor.
///
/// Details:
/// - Commands are already prefixed with `sudo` / `doas` via [`build_privilege_command`].
#[derive(Debug)]
pub struct RepoApplyBundle {
    /// What: Short descriptions for user review (e.g. preflight log seeding).
    pub summary_lines: Vec<String>,
    /// What: Privilege-wrapped shell commands executed in order.
    pub commands: Vec<String>,
}

/// What: Build privileged commands to apply enabled `[[repo]]` rows from `repos.conf`.
///
/// Inputs:
/// - `repos`: Parsed document.
/// - `main_pacman_text`: Current contents of the main `pacman.conf` (read from disk by the caller).
/// - `selected_section`: `[repo]` name for the focused modal row (trimmed; case-insensitive). The matching `[[repo]]` must declare apply sources (`server`, `mirrorlist`, or `http`/`https` `mirrorlist_url`), even when `enabled = false`.
///
/// Output:
/// - [`RepoApplyBundle`] or an actionable error string.
///
/// Details:
/// - Regenerates the **entire** managed drop-in from all **enabled** rows that define `server`, local `mirrorlist`, or `mirrorlist_url` (`http`/`https`). When none are enabled, writes a short comment-only stub file.
/// - Optionally downloads `mirrorlist_url` targets with curl (privileged) before writing the drop-in.
/// - Runs `pacman-key --recv-keys` (with optional `--keyserver` from `key_server`) / `--lsign-key` for each distinct fingerprint when `key_id` parses to at least 8 hex digits.
/// - Appends the managed `Include` block only if an **active** (uncommented) [`PACMAN_MANAGED_BEGIN`] line is absent.
/// - Appends `pacman -Sy --noconfirm` after repo files are updated.
/// - Uses [`active_tool`] for privilege wrapping; fails if no tool is configured.
///
/// # Errors
///
/// - When [`active_tool`] returns an error (no privilege tool configured).
/// - When [`build_repo_apply_bundle_with_tool`] rejects the plan (bad selection, unsafe paths, invalid stanzas).
pub fn build_repo_apply_bundle(
    repos: &ReposConfFile,
    main_pacman_text: &str,
    selected_section: &str,
) -> Result<RepoApplyBundle, String> {
    let tool = active_tool()?;
    build_repo_apply_bundle_with_tool(repos, main_pacman_text, selected_section, tool)
}

/// What: Same as [`build_repo_apply_bundle`] but accepts a fixed [`PrivilegeTool`] (for tests).
///
/// Inputs:
/// - `repos`, `main_pacman_text`, `selected_section` as in [`build_repo_apply_bundle`].
/// - `tool`: Privilege tool to wrap shell commands.
///
/// Output:
/// - [`RepoApplyBundle`] or error.
///
/// Details:
/// - Test-only consumers should call this to avoid depending on host `sudo`/`doas`.
///
/// # Errors
///
/// - When `selected_section` is empty, names no `[[repo]]`, or names a row without apply sources.
/// - When drop-in or `pacman.conf` paths fail safety checks, or stanza rendering fails internally.
pub fn build_repo_apply_bundle_with_tool(
    repos: &ReposConfFile,
    main_pacman_text: &str,
    selected_section: &str,
    tool: PrivilegeTool,
) -> Result<RepoApplyBundle, String> {
    let eligible: Vec<&RepoRow> = apply_eligible_rows(repos);
    let want = selected_section.trim().to_lowercase();
    if want.is_empty() {
        return Err("No repository selected.".to_string());
    }
    let Some(selected_row) = find_repo_row_by_lower_name(repos, &want) else {
        return Err(format!(
            "Selected repository \"{selected_section}\" has no matching [[repo]] name in repos.conf."
        ));
    };
    if !row_has_apply_source(selected_row) {
        return Err(format!(
            "Selected repository \"{selected_section}\" needs `server`, `mirrorlist`, or `http`/`https` mirrorlist_url in repos.conf \
             before it can be applied."
        ));
    }

    let mut summary_lines: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();

    if eligible.is_empty() {
        summary_lines.push(
            "No enabled [[repo]] rows: writing an empty managed drop-in (all custom repos disabled)."
                .to_string(),
        );
    }

    for r in &eligible {
        let name = r.name.as_deref().map_or("", str::trim);
        let has_ml = non_empty_trim(r.mirrorlist.as_deref());
        let has_srv = non_empty_trim(r.server.as_deref());
        let has_url = non_empty_trim(r.mirrorlist_url.as_deref());
        if has_ml && has_url {
            summary_lines.push(format!(
                "Note: skipping mirrorlist_url for [{name}] (mirrorlist path is set)"
            ));
        }
        if has_srv && has_url {
            summary_lines.push(format!(
                "Note: skipping mirrorlist_url for [{name}] (server is set)"
            ));
        }
    }

    let mirror_fetches = collect_mirror_fetch_steps(&eligible)?;
    if !mirror_fetches.is_empty() {
        ensure_curl_runnable()?;
    }
    for MirrorFetch { url, dest, name } in &mirror_fetches {
        summary_lines.push(format!(
            "Download mirrorlist for [{name}] via curl to {dest}"
        ));
        commands.push(privileged_curl_fetch_command(tool, url, dest)?);
    }

    let body = render_dropin_body(&eligible)?;

    let key_specs = distinct_key_recv_specs(&eligible);
    for (fpr, ks) in &key_specs {
        let recv_inner = pacman_key_recv_inner(fpr, ks.as_deref());
        summary_lines.push(format!("Receive signing key {fpr} (pacman-key)"));
        commands.push(build_priv_command(tool, &recv_inner));
        summary_lines.push(format!("Locally sign key {fpr} (pacman-key)"));
        commands.push(build_priv_command(
            tool,
            &format!("pacman-key --lsign-key {}", shell_single_quote(fpr)),
        ));
    }

    summary_lines.push(format!("Write managed drop-in {DEFAULT_DROPIN_PATH}"));
    commands.push(write_dropin_command(tool, DEFAULT_DROPIN_PATH, &body)?);

    if main_pacman_has_active_managed_marker(main_pacman_text) {
        summary_lines.push(format!(
            "Skip appending Include ({PACMAN_MANAGED_BEGIN} already active in {DEFAULT_MAIN_PACMAN_PATH})"
        ));
    } else {
        summary_lines.push(format!(
            "Append Pacsea Include block to {DEFAULT_MAIN_PACMAN_PATH}"
        ));
        commands.push(append_managed_include_command(
            tool,
            DEFAULT_MAIN_PACMAN_PATH,
            DEFAULT_DROPIN_PATH,
        )?);
    }

    summary_lines.push("Sync package databases (pacman -Sy --noconfirm)".to_string());
    commands.push(build_priv_command(tool, "pacman -Sy --noconfirm"));

    Ok(RepoApplyBundle {
        summary_lines,
        commands,
    })
}

/// What: Plan privileged `pacman-key` receive + local sign for the selected repo row only.
///
/// Inputs:
/// - `repos`: Parsed `repos.conf` document.
/// - `selected_section`: Pacman `[repo]` name from the modal (case-insensitive trim).
///
/// Output:
/// - [`RepoApplyBundle`] with two commands (recv, lsign) or an error string.
///
/// Details:
/// - Requires `key_id` with at least 8 hex digits after normalization; optional `key_server` is passed
///   to `pacman-key --recv-keys` the same way as full apply.
/// - Does not write drop-ins or run `pacman -Sy`.
///
/// # Errors
///
/// - When [`active_tool`] returns an error (no privilege tool configured).
/// - When no matching `[[repo]]` row exists, `key_id` is missing/invalid, or fingerprint normalization fails.
pub fn build_repo_key_refresh_bundle(
    repos: &ReposConfFile,
    selected_section: &str,
) -> Result<RepoApplyBundle, String> {
    let tool = active_tool()?;
    build_repo_key_refresh_bundle_with_tool(repos, selected_section, tool)
}

/// What: Same as [`build_repo_key_refresh_bundle`] but accepts a fixed [`PrivilegeTool`] (for tests).
///
/// Inputs:
/// - `repos`, `selected_section` as in [`build_repo_key_refresh_bundle`].
/// - `tool`: Privilege tool to wrap shell commands.
///
/// Output:
/// - [`RepoApplyBundle`] or error.
///
/// Details:
/// - Test callers avoid depending on host `sudo`/`doas` configuration.
///
/// # Errors
///
/// - When no matching `[[repo]]` row exists, `key_id` is missing/invalid, or fingerprint normalization fails.
pub fn build_repo_key_refresh_bundle_with_tool(
    repos: &ReposConfFile,
    selected_section: &str,
    tool: PrivilegeTool,
) -> Result<RepoApplyBundle, String> {
    let want = selected_section.trim().to_lowercase();
    let row = repos
        .repo
        .iter()
        .find(|r| {
            r.name
                .as_deref()
                .map(str::trim)
                .is_some_and(|n| n.to_lowercase() == want)
        })
        .ok_or_else(|| {
            format!(
                "No [[repo]] row named \"{}\" in repos.conf.",
                selected_section.trim()
            )
        })?;
    let kid = row
        .key_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "This repository has no key_id in repos.conf; nothing to refresh.".to_string()
        })?;
    let fpr = normalized_fingerprint(kid)
        .ok_or_else(|| "key_id must contain at least 8 hexadecimal digits.".to_string())?;
    let ks = row
        .key_server
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let mut summary_lines: Vec<String> = Vec::new();
    let mut commands: Vec<String> = Vec::new();
    let recv_inner = pacman_key_recv_inner(&fpr, ks);
    summary_lines.push(format!("Receive signing key {fpr} (pacman-key)"));
    commands.push(build_privilege_command(tool, &recv_inner));
    summary_lines.push(format!("Locally sign key {fpr} (pacman-key)"));
    commands.push(build_privilege_command(
        tool,
        &format!("pacman-key --lsign-key {}", shell_single_quote(&fpr)),
    ));
    Ok(RepoApplyBundle {
        summary_lines,
        commands,
    })
}

/// What: Wrap a shell command with the active privilege tool (`sudo` / `doas`).
///
/// Inputs:
/// - `tool`: Privilege backend.
/// - `inner`: Inner shell snippet (no new wrapping).
///
/// Output:
/// - Full command string for the executor.
///
/// Details:
/// - Thin wrapper over [`build_privilege_command`].
fn build_priv_command(tool: PrivilegeTool, inner: &str) -> String {
    build_privilege_command(tool, inner)
}

/// What: One privileged `curl` fetch step generated from `mirrorlist_url`.
///
/// Inputs:
/// - Fields filled by [`collect_mirror_fetch_steps`].
///
/// Output:
/// - Used to build summary lines and shell commands.
///
/// Details:
/// - `dest` is under `/etc/pacman.d/` with a Pacsea-specific filename.
struct MirrorFetch {
    /// Remote mirrorlist URL (`http`/`https` only).
    url: String,
    /// Absolute path written as root (matches `Include =` in the drop-in).
    dest: String,
    /// Pacman section name (for summaries).
    name: String,
}

/// What: Detect an uncommented Pacsea marker line in main `pacman.conf` text.
///
/// Inputs:
/// - `text`: Full file contents.
///
/// Output:
/// - `true` when some line trims to exactly [`PACMAN_MANAGED_BEGIN`].
///
/// Details:
/// - Lines such as `# === pacsea managed begin` do **not** match, so Apply may append the block again.
fn main_pacman_has_active_managed_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == PACMAN_MANAGED_BEGIN)
}

/// What: Non-empty string after trim.
///
/// Inputs:
/// - `s`: Optional string slice.
///
/// Output:
/// - `true` when `s` is `Some` and not empty or whitespace-only.
fn non_empty_trim(s: Option<&str>) -> bool {
    s.map(str::trim).is_some_and(|t| !t.is_empty())
}

/// What: Return rows that participate in the managed drop-in.
///
/// Inputs:
/// - `repos`: Parsed config.
///
/// Output:
/// - Slice references in file order.
///
/// Details:
/// - Skips `enabled = false`. Requires non-empty `name` and [`row_has_apply_source`].
fn apply_eligible_rows(repos: &ReposConfFile) -> Vec<&RepoRow> {
    repos
        .repo
        .iter()
        .filter(|r| {
            row_enabled(r)
                && r.name
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .is_some()
                && row_has_apply_source(r)
        })
        .collect()
}

/// What: Whether a row defines any apply source (server, local mirrorlist, or HTTP(S) mirrorlist URL).
///
/// Inputs:
/// - `r`: Parsed `[[repo]]` row.
///
/// Output:
/// - `true` when at least one source is present and any URL uses `http://` or `https://`.
///
/// Details:
/// - Non-HTTP `mirrorlist_url` values do not qualify alone (returns `false` when that is all there is).
/// - A non-empty `server` is accepted here; [`render_dropin_body`] rejects values that are not HTTP(S) URLs.
fn row_has_apply_source(r: &RepoRow) -> bool {
    repo_row_declares_apply_sources(r)
}

/// What: Accept only obvious HTTP(S) mirrorlist URLs for privileged fetch.
///
/// Inputs:
/// - `u`: Trimmed URL string.
///
/// Output:
/// - `true` for `http://` or `https://` prefixes (ASCII case-insensitive).
fn looks_like_http_url(u: &str) -> bool {
    let lower = u.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// What: Determine whether a row is considered enabled for apply.
///
/// Inputs:
/// - `r`: Parsed `[[repo]]` row.
///
/// Output:
/// - `false` only when `enabled = false`; otherwise `true`.
///
/// Details:
/// - `None` treats the row as enabled.
fn row_enabled(r: &RepoRow) -> bool {
    row_is_enabled_for_repos_conf(r)
}

/// What: Find a `[[repo]]` row by case-insensitive `name`.
///
/// Inputs:
/// - `repos`: Parsed document.
/// - `want_lower`: Lowercased section name (trimmed by caller).
///
/// Output:
/// - Matching row reference, if any.
///
/// Details:
/// - Ignores rows with empty `name` after trim.
fn find_repo_row_by_lower_name<'a>(
    repos: &'a ReposConfFile,
    want_lower: &str,
) -> Option<&'a RepoRow> {
    repos.repo.iter().find(|r| {
        r.name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some_and(|n| n.to_lowercase() == want_lower)
    })
}

/// What: Extract hex fingerprint material from a `key_id` string.
///
/// Inputs:
/// - `key_id`: User-supplied key id (may contain spaces or `0x` prefixes).
///
/// Output:
/// - Uppercase hex of length ≥ 8 when enough digits exist; otherwise `None`.
///
/// Details:
/// - Non-hex characters are stripped; short ids are ignored.
fn normalized_fingerprint(key_id: &str) -> Option<String> {
    let hex: String = key_id.chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() < 8 {
        return None;
    }
    Some(hex.to_uppercase())
}

/// What: Distinct signing key fingerprints plus optional `key_server` for recv.
///
/// Inputs:
/// - `rows`: Apply-eligible rows.
///
/// Output:
/// - Pairs `(fingerprint, key_server)` in first-seen fingerprint order.
///
/// Details:
/// - When the same fingerprint appears in multiple rows, `key_server` is the **first non-empty** trimmed value among those rows in file order.
/// - Skips rows without `key_id` or with ids that normalize to fewer than 8 hex digits.
fn distinct_key_recv_specs(rows: &[&RepoRow]) -> Vec<(String, Option<String>)> {
    let mut specs: Vec<(String, Option<String>)> = Vec::new();
    for r in rows {
        let Some(k) = r.key_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(fpr) = normalized_fingerprint(k) else {
            continue;
        };
        let ks = r
            .key_server
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(std::string::ToString::to_string);
        if let Some(i) = specs.iter().position(|(f, _)| f == &fpr) {
            if specs[i].1.is_none() {
                specs[i].1 = ks;
            }
        } else {
            specs.push((fpr, ks));
        }
    }
    specs
}

/// What: Build inner shell for `pacman-key --recv-keys` with optional `--keyserver`.
///
/// Inputs:
/// - `fpr`: Uppercase hex fingerprint.
/// - `key_server`: Optional keyserver hostname or URL.
///
/// Output:
/// - Unwrapped command string passed to [`build_privilege_command`].
///
/// Details:
/// - Uses [`shell_single_quote`] for all variable fragments.
fn pacman_key_recv_inner(fpr: &str, key_server: Option<&str>) -> String {
    let qf = shell_single_quote(fpr);
    let Some(ks) = key_server.map(str::trim).filter(|s| !s.is_empty()) else {
        return format!("pacman-key --recv-keys {qf}");
    };
    format!(
        "pacman-key --keyserver {} --recv-keys {qf}",
        shell_single_quote(ks)
    )
}

/// What: Plan privileged curl downloads for rows that rely on `mirrorlist_url`.
///
/// Inputs:
/// - `rows`: Apply-eligible rows (file order preserved).
///
/// Output:
/// - Fetch descriptors, or an error when a URL is not HTTP(S) or paths are invalid.
///
/// Details:
/// - Skips rows that have `server` or `mirrorlist` set (those take precedence; see summary notes in the bundle builder).
fn collect_mirror_fetch_steps(rows: &[&RepoRow]) -> Result<Vec<MirrorFetch>, String> {
    let mut out = Vec::new();
    for r in rows {
        if non_empty_trim(r.server.as_deref()) || non_empty_trim(r.mirrorlist.as_deref()) {
            continue;
        }
        let Some(url_raw) = r
            .mirrorlist_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if !looks_like_http_url(url_raw) {
            let name = r.name.as_deref().map_or("", str::trim);
            return Err(format!(
                "repos.conf: mirrorlist_url for [{name}] must start with http:// or https://"
            ));
        }
        let name = r
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "repos.conf: mirrorlist_url row missing name (internal)".to_string())?;
        let dest = mirror_url_dest_path(name)?;
        out.push(MirrorFetch {
            url: url_raw.to_string(),
            dest,
            name: name.to_string(),
        });
    }
    Ok(out)
}

/// What: Verify `curl` runs so `mirrorlist_url` steps can succeed.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Ok(())` when `curl --version` succeeds.
///
/// Details:
/// - Uses [`crate::util::curl::curl_binary_path`] for the executable.
fn ensure_curl_runnable() -> Result<(), String> {
    let bin = crate::util::curl::curl_binary_path();
    match std::process::Command::new(bin)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(format!(
            "curl ('{bin}') is required for mirrorlist_url but did not run successfully. Install curl or use mirrorlist = \"/path\" instead."
        )),
        Err(e) => Err(format!(
            "Could not run curl ('{bin}'): {e}. Install curl or use mirrorlist = \"/path\" instead."
        )),
    }
}

/// What: One privileged curl invocation: fetch URL to a path under `/etc/pacman.d/`.
///
/// Inputs:
/// - `tool`: Privilege wrapper.
/// - `url`: HTTP(S) URL (passed through [`curl_args`]).
/// - `dest`: Absolute destination path.
///
/// Output:
/// - Full privileged command string.
///
/// Details:
/// - Each argv token is [`shell_single_quote`]d; adds `-o dest` via `curl_args` extras.
fn privileged_curl_fetch_command(
    tool: PrivilegeTool,
    url: &str,
    dest: &str,
) -> Result<String, String> {
    if !is_safe_abs_path(dest) {
        return Err("Refusing unsafe mirrorlist destination path.".to_string());
    }
    let bin = crate::util::curl::curl_binary_path();
    let argv = curl_args(url, &["-o", dest]);
    let mut parts = vec![shell_single_quote(bin)];
    for a in argv {
        parts.push(shell_single_quote(&a));
    }
    let inner = parts.join(" ");
    Ok(build_privilege_command(tool, &inner))
}

/// What: Stable on-disk path for a fetched mirrorlist (`Include =` target).
///
/// Inputs:
/// - `section`: Repo section `name`.
///
/// Output:
/// - Absolute path under `/etc/pacman.d/`.
///
/// Details:
/// - Slug is sanitized; a short hash suffix avoids collisions when names normalize alike.
/// - Hash uses FNV-1a (32-bit) so filenames stay stable across Rust compiler versions (unlike `DefaultHasher`).
fn mirror_url_dest_path(section: &str) -> Result<String, String> {
    let slug = sanitize_repo_slug(section)?;
    let short = stable_fnv1a_u32(section.trim().as_bytes());
    let p = format!("/etc/pacman.d/pacsea-mirror-{slug}-{short:x}.list");
    if !is_safe_abs_path(&p) {
        return Err("Refusing unsafe mirrorlist destination path.".to_string());
    }
    Ok(p)
}

/// What: Sanitize repo `name` into a path slug (lowercase, safe chars).
///
/// Inputs:
/// - `section`: Raw section name.
///
/// Output:
/// - Non-empty slug or error.
fn sanitize_repo_slug(section: &str) -> Result<String, String> {
    let mut out = String::new();
    for c in section.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        return Err(
            "repos.conf: repo `name` sanitizes to an empty path slug for mirrorlist_url."
                .to_string(),
        );
    }
    let out = if out.len() > 48 {
        out[..48].trim_end_matches('_').to_string()
    } else {
        out
    };
    if out.is_empty() {
        return Err(
            "repos.conf: repo `name` too short after sanitizing for mirrorlist_url.".to_string(),
        );
    }
    Ok(out)
}

/// What: Render `[repo]` stanzas for the managed drop-in file.
///
/// Inputs:
/// - `rows`: Eligible rows.
///
/// Output:
/// - Full file text ending with a newline.
///
/// Details:
/// - Default `SigLevel` is [`DEFAULT_DROPIN_SIG_LEVEL`] when omitted.
/// - `mirrorlist_url`-only rows emit `Include =` to the same path as [`mirror_url_dest_path`].
/// - Non-empty `server` values must start with `http://` or `https://` (same policy as `mirrorlist_url` fetches).
fn render_dropin_body(rows: &[&RepoRow]) -> Result<String, String> {
    if rows.is_empty() {
        return Ok(
            "# Pacsea managed repositories\n# No enabled [[repo]] rows in repos.conf.\n"
                .to_string(),
        );
    }
    let mut out = String::new();
    for r in rows {
        let name = r
            .name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "repos.conf: missing name on eligible row (internal)".to_string())?;
        out.push('[');
        out.push_str(name);
        out.push_str("]\n");
        let sl_line = r
            .sig_level
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_DROPIN_SIG_LEVEL);
        out.push_str("SigLevel = ");
        out.push_str(sl_line);
        out.push('\n');
        if let Some(srv) = r.server.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if !looks_like_http_url(srv) {
                return Err(format!(
                    "repos.conf: server for [{name}] must start with http:// or https://"
                ));
            }
            out.push_str("Server = ");
            out.push_str(srv);
            out.push('\n');
        } else if let Some(inc) = r
            .mirrorlist
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            out.push_str("Include = ");
            out.push_str(inc);
            out.push('\n');
        } else if non_empty_trim(r.mirrorlist_url.as_deref())
            && !non_empty_trim(r.server.as_deref())
            && !non_empty_trim(r.mirrorlist.as_deref())
        {
            let dest = mirror_url_dest_path(name)?;
            out.push_str("Include = ");
            out.push_str(&dest);
            out.push('\n');
        } else {
            return Err(
                "Internal: row missing server, mirrorlist, and resolvable mirrorlist_url"
                    .to_string(),
            );
        }
        out.push('\n');
    }
    Ok(out)
}

/// What: Build a privileged command that writes the drop-in file atomically via `printf` and `tee`.
///
/// Inputs:
/// - `tool`: Privilege backend.
/// - `dropin_path`: Absolute path under `/etc/pacman.d/` (validated).
/// - `body`: Full drop-in file contents.
///
/// Output:
/// - Wrapped `sh -c` command string or a refusal error.
///
/// Details:
/// - Each line is passed through [`shell_single_quote`] before `printf`.
fn write_dropin_command(
    tool: PrivilegeTool,
    dropin_path: &str,
    body: &str,
) -> Result<String, String> {
    if !dropin_path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '.' | '_'))
    {
        return Err("Refusing unsafe drop-in path.".to_string());
    }
    let pieces: Vec<String> = body.lines().map(shell_single_quote).collect();
    if pieces.is_empty() {
        return Err("Generated drop-in is empty.".to_string());
    }
    let printf_args = pieces.join(" ");
    let inner = format!("printf '%s\\n' {printf_args} | tee {dropin_path} > /dev/null");
    Ok(build_privilege_command(
        tool,
        &format!("sh -c {}", shell_single_quote(&inner)),
    ))
}

/// What: Build a privileged command appending the managed `Include` block to main `pacman.conf`.
///
/// Inputs:
/// - `tool`: Privilege backend.
/// - `main_path`: Absolute path to `pacman.conf` (validated).
/// - `dropin_path`: Absolute path passed to `Include =` (validated).
///
/// Output:
/// - Wrapped `sh -c` command or path safety error.
///
/// Details:
/// - Appends marker lines plus `Include = …` plus blank line via `printf` and `>>`.
fn append_managed_include_command(
    tool: PrivilegeTool,
    main_path: &str,
    dropin_path: &str,
) -> Result<String, String> {
    if !is_safe_abs_path(main_path) || !is_safe_abs_path(dropin_path) {
        return Err("Refusing unsafe pacman path.".to_string());
    }
    let include_line = format!("Include = {dropin_path}");
    let b = shell_single_quote(PACMAN_MANAGED_BEGIN);
    let inc = shell_single_quote(&include_line);
    let e = shell_single_quote(PACMAN_MANAGED_END);
    let q_main = shell_single_quote(main_path);
    let inner = format!("printf '%s\\n' {b} {inc} {e} '' >> {q_main}");
    Ok(build_privilege_command(
        tool,
        &format!("sh -c {}", shell_single_quote(&inner)),
    ))
}

/// What: Reject path strings that could break out of expected `/etc/...` layouts.
///
/// Inputs:
/// - `p`: Path to validate.
///
/// Output:
/// - `true` when the path is absolute and uses only safe characters.
///
/// Details:
/// - Allows alphanumeric, `/`, `-`, `.`, `_`.
/// - Rejects `..` path segments so traversal like `/etc/pacman.d/../../tmp/x` cannot pass.
fn is_safe_abs_path(p: &str) -> bool {
    p.starts_with('/')
        && !p.contains("..")
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '.' | '_'))
}

/// What: 32-bit FNV-1a hash over bytes (stable across toolchains).
///
/// Inputs:
/// - `data`: Bytes to hash (e.g. UTF-8 of a trimmed repo name).
///
/// Output:
/// - Unsigned 32-bit digest.
///
/// Details:
/// - Used for mirrorlist filenames so upgrades to a new `rustc` do not orphan on-disk paths.
fn stable_fnv1a_u32(data: &[u8]) -> u32 {
    const OFFSET_BASIS: u32 = 0x811c_9dc5;
    const PRIME: u32 = 0x0100_0193;
    let mut h = OFFSET_BASIS;
    for &b in data {
        h ^= u32::from(b);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// What: Read main `pacman.conf` for planning (production convenience).
///
/// Inputs:
/// - `path`: Usually [`DEFAULT_MAIN_PACMAN_PATH`].
///
/// Output:
/// - File text or an IO error message.
///
/// Details:
/// - Callers may inject fixture text in tests instead.
///
/// # Errors
///
/// - When `path` cannot be read (permission, missing file, I/O error); message includes the path.
pub fn read_main_pacman_conf_text(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| {
        format!(
            "Could not read {}: {e}. Apply needs the live pacman configuration.",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logic::repos::{ReposConfFile, load_resolve_repos_from_str};

    #[test]
    fn bundle_contains_recv_lsign_write() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://example.com/$repo/os/$arch"
key_id = "AABBCCDDEEFF0011"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let main = "\n";
        let b = build_repo_apply_bundle_with_tool(&file, main, "myrepo", PrivilegeTool::Sudo)
            .expect("bundle");
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacman-key --recv-keys"))
        );
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacman-key --lsign-key"))
        );
        assert!(b.commands.iter().any(|c| c.contains("pacsea-repos.conf")));
        assert!(
            b.summary_lines
                .iter()
                .any(|s| s.contains("Append Pacsea Include"))
        );
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacman -Sy --noconfirm"))
        );
    }

    #[test]
    fn recv_uses_keyserver_when_configured() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://example.com/$repo/os/$arch"
key_id = "AABBCCDDEEFF0011"
key_server = "keyserver.ubuntu.com"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b = build_repo_apply_bundle_with_tool(&file, "\n", "myrepo", PrivilegeTool::Sudo)
            .expect("b");
        assert!(b.commands.iter().any(|c| {
            c.contains("pacman-key")
                && c.contains("--keyserver")
                && c.contains("keyserver.ubuntu.com")
        }));
    }

    #[test]
    fn skips_include_when_marker_present() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://x.test"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let main = format!(
            "\n{PACMAN_MANAGED_BEGIN}\nInclude = /etc/pacman.d/pacsea-repos.conf\n{PACMAN_MANAGED_END}\n"
        );
        let b = build_repo_apply_bundle_with_tool(&file, &main, "myrepo", PrivilegeTool::Sudo)
            .expect("b");
        assert!(
            b.summary_lines
                .iter()
                .any(|s| s.contains("Skip appending") && s.contains("already active"))
        );
        assert!(
            !b.commands
                .iter()
                .any(|c| { c.contains(">>") && c.contains("/etc/pacman.conf") })
        );
    }

    #[test]
    fn commented_marker_line_still_appends_include() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://x.test"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let main = format!("# {PACMAN_MANAGED_BEGIN}\n");
        let b = build_repo_apply_bundle_with_tool(&file, &main, "myrepo", PrivilegeTool::Sudo)
            .expect("b");
        assert!(
            b.summary_lines
                .iter()
                .any(|s| s.contains("Append Pacsea Include"))
        );
        assert!(
            b.commands
                .iter()
                .any(|c| { c.contains(">>") && c.contains("/etc/pacman.conf") })
        );
    }

    #[test]
    fn mirrorlist_url_fetch_and_include() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
mirrorlist_url = "https://archlinux.org/mirrorlist/all/http/"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b = build_repo_apply_bundle_with_tool(&file, "\n", "myrepo", PrivilegeTool::Sudo)
            .expect("b");
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("curl") && c.contains("archlinux.org"))
        );
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacsea-mirror-myrepo-"))
        );
    }

    #[test]
    fn selected_row_without_server_errors() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://x.test"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let err = build_repo_apply_bundle_with_tool(&file, "", "other", PrivilegeTool::Sudo)
            .expect_err("err");
        assert!(
            err.contains("no matching [[repo]]") || err.contains("needs"),
            "{err}"
        );
    }

    #[test]
    fn all_disabled_rows_still_writes_empty_dropin() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://x.test"
enabled = false
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b = build_repo_apply_bundle_with_tool(&file, "\n", "myrepo", PrivilegeTool::Sudo)
            .expect("bundle");
        assert!(
            b.summary_lines
                .iter()
                .any(|s| s.contains("empty managed drop-in")),
            "{:?}",
            b.summary_lines
        );
        assert!(b.commands.iter().any(|c| c.contains("pacsea-repos.conf")));
        assert!(b.commands.iter().any(|c| c.contains("pacman -Sy")));
    }

    #[test]
    fn key_refresh_bundle_only_recv_and_lsign() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://example.com/$repo/os/$arch"
key_id = "AABBCCDDEEFF0011"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b = build_repo_key_refresh_bundle_with_tool(&file, "myrepo", PrivilegeTool::Sudo)
            .expect("bundle");
        assert_eq!(b.commands.len(), 2);
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacman-key") && c.contains("--recv-keys"))
        );
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains("pacman-key --lsign-key"))
        );
        assert!(!b.commands.iter().any(|c| c.contains("pacsea-repos.conf")));
        assert!(!b.commands.iter().any(|c| c.contains("pacman -Sy")));
    }

    #[test]
    fn key_refresh_uses_keyserver_when_configured() {
        let toml = r#"
[[repo]]
name = "kr"
results_filter = "k"
server = "https://x.test"
key_id = "AABBCCDDEEFF0011"
key_server = "keyserver.ubuntu.com"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b =
            build_repo_key_refresh_bundle_with_tool(&file, "kr", PrivilegeTool::Doas).expect("b");
        assert!(b.commands.iter().any(|c| {
            c.contains("pacman-key")
                && c.contains("--keyserver")
                && c.contains("keyserver.ubuntu.com")
        }));
    }

    #[test]
    fn is_safe_abs_path_rejects_dotdot() {
        assert!(!is_safe_abs_path("/etc/pacman.d/../../tmp/evil"));
        assert!(is_safe_abs_path("/etc/pacman.d/pacsea-repos.conf"));
    }

    #[test]
    fn server_must_use_http_or_https() {
        let toml = r#"
[[repo]]
name = "bad"
results_filter = "b"
server = "file:///etc/shadow"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let err = build_repo_apply_bundle_with_tool(&file, "\n", "bad", PrivilegeTool::Sudo)
            .expect_err("err");
        assert!(err.contains("http://") && err.contains("https://"), "{err}");
    }

    #[test]
    fn dropin_command_contains_safe_default_sig_level() {
        let toml = r#"
[[repo]]
name = "myrepo"
results_filter = "mine"
server = "https://example.com/$repo/os/$arch"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let b = build_repo_apply_bundle_with_tool(&file, "\n", "myrepo", PrivilegeTool::Sudo)
            .expect("bundle");
        assert!(
            b.commands
                .iter()
                .any(|c| c.contains(DEFAULT_DROPIN_SIG_LEVEL)),
            "expected drop-in write to use default sig level"
        );
    }

    #[test]
    fn key_refresh_errors_without_key_id() {
        let toml = r#"
[[repo]]
name = "nok"
results_filter = "n"
server = "https://x.test"
"#;
        let (repo, _) = load_resolve_repos_from_str(toml).expect("parse");
        let file = ReposConfFile { repo };
        let err = build_repo_key_refresh_bundle_with_tool(&file, "nok", PrivilegeTool::Sudo)
            .expect_err("err");
        assert!(err.contains("key_id"), "{err}");
    }
}
