use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::theme::config::skeletons::{
    KEYBINDS_SKELETON_CONTENT, REPOS_SKELETON_CONTENT, SETTINGS_SKELETON_CONTENT,
    THEME_SKELETON_CONTENT,
};
use crate::theme::config::theme_loader::{THEME_REQUIRED_CANONICAL, resolved_theme_canonical_keys};
use crate::theme::parsing::canonical_for_key;
use crate::theme::paths::{
    config_dir, resolve_keybinds_config_path, resolve_repos_config_path,
    resolve_settings_config_path, resolve_theme_config_path,
};
use crate::theme::types::Settings;

/// What: Convert a boolean value to a config string.
///
/// Inputs:
/// - `value`: Boolean value to convert
///
/// Output:
/// - "true" or "false" string
fn bool_to_string(value: bool) -> String {
    if value {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

/// What: Convert an optional integer to a config string.
///
/// Inputs:
/// - `value`: Optional integer value
///
/// Output:
/// - String representation of the value, or "all" if None
fn optional_int_to_string(value: Option<u32>) -> String {
    value.map_or_else(|| "all".to_string(), |v| v.to_string())
}

/// What: Get layout-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_layout_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "layout_left_pct" => Some(prefs.layout_left_pct.to_string()),
        "layout_center_pct" => Some(prefs.layout_center_pct.to_string()),
        "layout_right_pct" => Some(prefs.layout_right_pct.to_string()),
        _ => None,
    }
}

/// What: Get app/UI-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_app_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "app_dry_run_default" => Some(bool_to_string(prefs.app_dry_run_default)),
        "sort_mode" => Some(prefs.sort_mode.as_config_key().to_string()),
        "clipboard_suffix" => Some(prefs.clipboard_suffix.clone()),
        "show_recent_pane" | "show_search_history_pane" => {
            Some(bool_to_string(prefs.show_recent_pane))
        }
        "show_install_pane" => Some(bool_to_string(prefs.show_install_pane)),
        "show_keybinds_footer" => Some(bool_to_string(prefs.show_keybinds_footer)),
        "package_marker" => {
            let marker_str = match prefs.package_marker {
                crate::theme::types::PackageMarker::FullLine => "full_line",
                crate::theme::types::PackageMarker::Front => "front",
                crate::theme::types::PackageMarker::End => "end",
            };
            Some(marker_str.to_string())
        }
        "app_start_mode" => {
            let mode = if prefs.start_in_news {
                "news"
            } else {
                "package"
            };
            Some(mode.to_string())
        }
        "skip_preflight" => Some(bool_to_string(prefs.skip_preflight)),
        "search_startup_mode" => {
            let mode = if prefs.search_startup_mode {
                "normal_mode"
            } else {
                "insert_mode"
            };
            Some(mode.to_string())
        }
        "locale" => Some(prefs.locale.clone()),
        "preferred_terminal" => Some(prefs.preferred_terminal.clone()),
        "privilege_tool" => Some(prefs.privilege_mode.as_config_key().to_string()),
        "auth_mode" => Some(prefs.auth_mode.as_config_key().to_string()),
        "use_terminal_theme" => Some(bool_to_string(prefs.use_terminal_theme)),
        "aur_vote_enabled" => Some(bool_to_string(prefs.aur_vote_enabled)),
        "aur_vote_ssh_timeout_seconds" => Some(prefs.aur_vote_ssh_timeout_seconds.to_string()),
        "aur_vote_ssh_command" => Some(prefs.aur_vote_ssh_command.clone()),
        _ => None,
    }
}

/// What: Get mirror-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_mirror_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "selected_countries" => Some(prefs.selected_countries.clone()),
        "mirror_count" => Some(prefs.mirror_count.to_string()),
        "virustotal_api_key" => Some(prefs.virustotal_api_key.clone()),
        _ => None,
    }
}

/// What: Get news-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_news_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "news_read_symbol" => Some(prefs.news_read_symbol.clone()),
        "news_unread_symbol" => Some(prefs.news_unread_symbol.clone()),
        "news_filter_show_arch_news" => Some(bool_to_string(prefs.news_filter_show_arch_news)),
        "news_filter_show_advisories" => Some(bool_to_string(prefs.news_filter_show_advisories)),
        "news_filter_show_pkg_updates" => Some(bool_to_string(prefs.news_filter_show_pkg_updates)),
        "news_filter_show_aur_updates" => Some(bool_to_string(prefs.news_filter_show_aur_updates)),
        "news_filter_show_aur_comments" => {
            Some(bool_to_string(prefs.news_filter_show_aur_comments))
        }
        "news_filter_installed_only" => Some(bool_to_string(prefs.news_filter_installed_only)),
        "news_max_age_days" => Some(optional_int_to_string(prefs.news_max_age_days)),
        "startup_news_configured" => Some(bool_to_string(prefs.startup_news_configured)),
        "startup_news_show_arch_news" => Some(bool_to_string(prefs.startup_news_show_arch_news)),
        "startup_news_show_advisories" => Some(bool_to_string(prefs.startup_news_show_advisories)),
        "startup_news_show_aur_updates" => {
            Some(bool_to_string(prefs.startup_news_show_aur_updates))
        }
        "startup_news_show_aur_comments" => {
            Some(bool_to_string(prefs.startup_news_show_aur_comments))
        }
        "startup_news_show_pkg_updates" => {
            Some(bool_to_string(prefs.startup_news_show_pkg_updates))
        }
        "startup_news_max_age_days" => {
            Some(optional_int_to_string(prefs.startup_news_max_age_days))
        }
        "news_cache_ttl_days" => Some(prefs.news_cache_ttl_days.to_string()),
        _ => None,
    }
}

/// What: Get updates/refresh-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_updates_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "updates_refresh_interval" | "updates_interval" | "refresh_interval" => {
            Some(prefs.updates_refresh_interval.to_string())
        }
        _ => None,
    }
}

/// What: Get scan-related setting values.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(String) if key was handled, None otherwise
fn get_scan_value(key: &str, _prefs: &Settings) -> Option<String> {
    match key {
        "scan_do_clamav" | "scan_do_trivy" | "scan_do_semgrep" | "scan_do_shellcheck"
        | "scan_do_virustotal" | "scan_do_custom" | "scan_do_sleuth" => {
            // Scan keys default to true
            Some("true".to_string())
        }
        _ => None,
    }
}

/// What: Get PKGBUILD static-check related setting values for `ensure_settings_keys_present`.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - Some(value) when the key is handled, else None
///
/// Details:
/// - Used when appending missing keys so user prefs override skeleton defaults.
fn get_pkgbuild_static_check_value(key: &str, prefs: &Settings) -> Option<String> {
    match key {
        "pkgbuild_shellcheck_exclude" => Some(prefs.pkgbuild_shellcheck_exclude.clone()),
        "pkgbuild_checks_show_raw_output" => {
            Some(bool_to_string(prefs.pkgbuild_checks_show_raw_output))
        }
        _ => None,
    }
}

/// What: Get the value for a setting key, preferring prefs over skeleton default.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `skeleton_value`: Default value from skeleton
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - String value to use for the setting
///
/// Details:
/// - Delegates to category-specific functions to reduce complexity.
/// - Mirrors the parsing architecture for consistency.
fn get_setting_value(key: &str, skeleton_value: String, prefs: &Settings) -> String {
    get_layout_value(key, prefs)
        .or_else(|| get_app_value(key, prefs))
        .or_else(|| get_mirror_value(key, prefs))
        .or_else(|| get_news_value(key, prefs))
        .or_else(|| get_updates_value(key, prefs))
        .or_else(|| get_scan_value(key, prefs))
        .or_else(|| get_pkgbuild_static_check_value(key, prefs))
        .unwrap_or(skeleton_value)
}

/// What: Parse skeleton and extract missing settings with comments.
///
/// Inputs:
/// - `skeleton_lines`: Lines from the settings skeleton
/// - `have`: Set of existing keys
/// - `prefs`: Current settings to get values from
///
/// Output:
/// - Vector of (`setting_line`, `optional_comment`) tuples
fn parse_missing_settings(
    skeleton_lines: &[&str],
    have: &HashSet<String>,
    prefs: &Settings,
) -> Vec<(String, Option<String>)> {
    let mut missing_settings: Vec<(String, Option<String>)> = Vec::new();
    let mut current_comment: Option<String> = None;

    for line in skeleton_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            current_comment = None;
            continue;
        }
        if trimmed.starts_with('#') {
            // Check if this is a comment for a setting (not a section header or empty comment)
            if !trimmed.contains("—")
                && !trimmed.starts_with("# Pacsea")
                && trimmed.len() > 1
                && !trimmed.starts_with("# Available countries")
            {
                current_comment = Some(trimmed.to_string());
            } else {
                current_comment = None;
            }
            continue;
        }
        if trimmed.starts_with("//") {
            current_comment = None;
            continue;
        }
        if trimmed.contains('=') {
            let mut parts = trimmed.splitn(2, '=');
            let raw_key = parts.next().unwrap_or("");
            let skeleton_value = parts.next().unwrap_or("").trim().to_string();
            let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if have.contains(&key) {
                current_comment = None;
            } else {
                // Use value from prefs if available, otherwise use skeleton value
                let value = get_setting_value(&key, skeleton_value, prefs);
                let setting_line = format!("{} = {}", raw_key.trim(), value);
                missing_settings.push((setting_line, current_comment.take()));
            }
        }
    }
    missing_settings
}

/// What: Create `repos.conf` from the built-in skeleton when the file does not exist yet.
///
/// Inputs:
/// - None.
///
/// Output:
/// - None.
///
/// Details:
/// - Best-effort: ignores write failures (same pattern as keybinds seeding).
/// - Target path matches [`resolve_repos_config_path`] when a candidate file already exists; if none
///   exist yet, writes to `config_dir()/repos.conf` (same default as the Config menu and Repositories modal).
fn ensure_repos_conf_skeleton() {
    let repos_path = resolve_repos_config_path().unwrap_or_else(|| config_dir().join("repos.conf"));
    if repos_path.exists() {
        return;
    }
    if let Some(dir) = repos_path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&repos_path, REPOS_SKELETON_CONTENT);
}

/// What: Ensure all expected settings keys exist in `settings.conf`, appending defaults as needed.
///
/// Inputs:
/// - `prefs`: Current in-memory settings whose values seed the file when keys are missing.
///
/// Output:
/// - None.
///
/// # Panics
/// - Panics if `lines.last()` is called on an empty vector after checking `!lines.is_empty()` (should not happen due to the check)
///
/// Details:
/// - Preserves existing lines and comments while adding only absent keys.
/// - Creates the settings file from the skeleton when it is missing or empty.
pub fn ensure_settings_keys_present(prefs: &Settings) {
    // Always resolve to HOME/XDG path similar to save_sort_mode
    // This ensures we always have a path, even if the file doesn't exist yet
    let p = resolve_settings_config_path().or_else(|| {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| Path::new(&h).join(".config"))
            })
            .map(|base| base.join("pacsea").join("settings.conf"))
    });
    let Some(p) = p else {
        // This should never happen (HOME should always be set), but if it does, we can't proceed
        return;
    };

    // Ensure directory exists
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.is_none_or(|m| m.len() == 0);
    let created_new = !file_exists || file_empty;

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        fs::read_to_string(&p)
            .map(|content| content.lines().map(ToString::to_string).collect())
            .unwrap_or_default()
    } else {
        // File doesn't exist or is empty - start with skeleton
        Vec::new()
    };

    // If file is missing or empty, seed with the built-in skeleton content first
    if created_new || lines.is_empty() {
        lines = SETTINGS_SKELETON_CONTENT
            .lines()
            .map(ToString::to_string)
            .collect();
    }
    // Parse existing settings keys (normalize keys like the parser does)
    let mut have: HashSet<String> = HashSet::new();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (kraw, _) = trimmed.split_at(eq);
            let key = kraw.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if key == "show_recent_pane" {
                have.insert("show_search_history_pane".to_string());
            }
            have.insert(key);
        }
    }

    // Parse skeleton to extract settings entries with their comments
    let skeleton_lines: Vec<&str> = SETTINGS_SKELETON_CONTENT.lines().collect();
    let missing_settings = parse_missing_settings(&skeleton_lines, &have, prefs);

    // Update settings file if needed
    if created_new || !missing_settings.is_empty() {
        // Append missing settings to the file
        // Add separator and header comment for auto-added settings
        if !created_new
            && !lines.is_empty()
            && !lines
                .last()
                .expect("lines should not be empty after is_empty() check")
                .trim()
                .is_empty()
        {
            lines.push(String::new());
        }
        if !missing_settings.is_empty() {
            lines.push("# Missing settings added automatically".to_string());
            lines.push(String::new());
        }

        for (setting_line, comment) in &missing_settings {
            if let Some(comment) = comment {
                lines.push(comment.clone());
            }
            lines.push(setting_line.clone());
        }

        let new_content = lines.join("\n");
        let _ = fs::write(p, new_content);
    }

    // Ensure keybinds file exists with skeleton if missing (best-effort)
    // Try to use the same path resolution as reading, but fall back to config_dir if file doesn't exist yet
    let kb = resolve_keybinds_config_path().unwrap_or_else(|| config_dir().join("keybinds.conf"));
    if kb.exists() {
        // Append missing keybinds to existing file
        ensure_keybinds_present(&kb);
    } else {
        if let Some(dir) = kb.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(&kb, KEYBINDS_SKELETON_CONTENT);
    }

    ensure_repos_conf_skeleton();
}

/// What: Ensure all expected keybind entries exist in `keybinds.conf`, appending defaults as needed.
///
/// Inputs:
/// - `keybinds_path`: Path to the keybinds.conf file.
///
/// Output:
/// - None.
///
/// Details:
/// - Preserves existing lines and comments while adding only absent keybinds.
/// - Parses the skeleton to extract all keybind entries with their associated comments.
/// - Appends missing keybinds with their comments in the correct sections.
fn ensure_keybinds_present(keybinds_path: &Path) {
    // Read existing file
    let Ok(existing_content) = fs::read_to_string(keybinds_path) else {
        return; // Can't read, skip
    };

    // Parse existing keybinds (normalize keys like the parser does)
    let mut have: HashSet<String> = HashSet::new();
    for line in existing_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('=') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("");
        let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
        have.insert(key);
    }

    // Parse skeleton to extract keybind entries with their comments
    let skeleton_lines: Vec<&str> = KEYBINDS_SKELETON_CONTENT.lines().collect();
    let mut missing_keybinds: Vec<(String, Option<String>)> = Vec::new();
    let mut current_comment: Option<String> = None;
    let mut current_section_header: Option<String> = None;

    for line in skeleton_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // Clear descriptive comment on empty lines (section header persists until next keybind)
            current_comment = None;
            continue;
        }
        if trimmed.starts_with('#') {
            // Check if this is a section header (contains "—" or is a special header)
            if trimmed.contains("—")
                || trimmed.starts_with("# Pacsea")
                || trimmed.starts_with("# Modifiers")
            {
                current_section_header = Some(trimmed.to_string());
                current_comment = None;
            } else {
                // Descriptive comment for a keybind
                current_comment = Some(trimmed.to_string());
            }
            continue;
        }
        if trimmed.starts_with("//") {
            current_comment = None;
            continue;
        }
        if trimmed.contains('=') {
            let mut parts = trimmed.splitn(2, '=');
            let raw_key = parts.next().unwrap_or("");
            let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if !have.contains(&key) {
                // Key is missing, build comment string with section header and descriptive comment
                let mut comment_parts = Vec::new();
                if let Some(ref section) = current_section_header {
                    comment_parts.push(section.clone());
                }
                if let Some(ref desc) = current_comment {
                    comment_parts.push(desc.clone());
                }
                let combined_comment = if comment_parts.is_empty() {
                    None
                } else {
                    Some(comment_parts.join("\n"))
                };
                missing_keybinds.push((trimmed.to_string(), combined_comment));
            }
            // Clear both descriptive comment and section header after processing keybind
            // (whether the keybind exists or not). Only the first missing keybind in a section
            // will include the section header in its comment.
            current_comment = None;
            current_section_header = None;
        }
    }

    // If no missing keybinds, nothing to do
    if missing_keybinds.is_empty() {
        return;
    }

    // Append missing keybinds to the file
    let mut new_lines: Vec<String> = existing_content.lines().map(ToString::to_string).collect();

    // Add separator and header comment for auto-added keybinds
    if !new_lines.is_empty()
        && !new_lines
            .last()
            .expect("new_lines should not be empty after is_empty() check")
            .trim()
            .is_empty()
    {
        new_lines.push(String::new());
    }
    if !missing_keybinds.is_empty() {
        new_lines.push("# Missing keybinds added automatically".to_string());
        new_lines.push(String::new());
    }

    for (keybind_line, comment) in &missing_keybinds {
        if let Some(comment) = comment {
            new_lines.push(comment.clone());
        }
        new_lines.push(keybind_line.clone());
    }

    let new_content = new_lines.join("\n");
    let _ = fs::write(keybinds_path, new_content);
}

/// What: Ensure `theme.conf` (or legacy theme file) defines every required theme key, appending skeleton defaults for any gaps.
///
/// Inputs:
/// - None.
///
/// Output:
/// - None.
///
/// Details:
/// - Resolves the same path as theme loading (`resolve_theme_config_path` or `config_dir()/theme.conf`).
/// - Writes the full theme skeleton when the file is missing or empty.
/// - Otherwise appends `key = value` lines from `THEME_SKELETON_CONTENT` for each missing canonical color.
/// - Must run before the first `theme()` load so incomplete files are repaired on disk first.
pub fn ensure_theme_keys_present() {
    let p = resolve_theme_config_path().unwrap_or_else(|| config_dir().join("theme.conf"));
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    let meta = fs::metadata(&p).ok();
    let file_missing = meta.is_none();
    let file_empty = meta.is_none_or(|m| m.len() == 0);

    if file_missing || file_empty {
        let _ = fs::write(&p, THEME_SKELETON_CONTENT);
        return;
    }

    let Ok(content) = fs::read_to_string(&p) else {
        return;
    };

    let have = resolved_theme_canonical_keys(&content);
    let missing: Vec<&str> = THEME_REQUIRED_CANONICAL
        .iter()
        .copied()
        .filter(|k| !have.contains(*k))
        .collect();

    if missing.is_empty() {
        return;
    }

    let mut defaults: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for line in THEME_SKELETON_CONTENT.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('=') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("").trim();
        let norm = raw_key.to_lowercase().replace(['.', '-', ' '], "_");
        let Some(canon) = canonical_for_key(&norm) else {
            continue;
        };
        defaults
            .entry(canon.to_string())
            .or_insert_with(|| trimmed.to_string());
    }

    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    if !lines.is_empty() && !lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.push(String::new());
    }
    lines.push("# Missing theme keys added automatically".to_string());
    lines.push(String::new());
    for canon in missing {
        if let Some(line) = defaults.get(canon) {
            lines.push(line.clone());
        } else {
            tracing::warn!(
                canon = canon,
                "theme skeleton had no default line for missing canonical key"
            );
        }
    }
    let _ = fs::write(&p, lines.join("\n"));
}

/// What: Migrate legacy `pacsea.conf` into the split `theme.conf` and `settings.conf` files.
///
/// Inputs:
/// - None.
///
/// Output:
/// - None.
///
/// Details:
/// - Copies non-preference keys to `theme.conf` and preference keys (excluding keybinds) to `settings.conf`.
/// - Seeds missing files with skeleton content when the legacy file is absent or empty.
/// - Leaves existing, non-empty split configs untouched to avoid overwriting user changes.
pub fn maybe_migrate_legacy_confs() {
    let base = config_dir();
    let legacy = base.join("pacsea.conf");
    if !legacy.is_file() {
        // No legacy file: ensure split configs exist with skeletons
        let theme_path = base.join("theme.conf");
        let settings_path = base.join("settings.conf");
        let keybinds_path = base.join("keybinds.conf");

        // theme.conf
        let theme_missing_or_empty = std::fs::metadata(&theme_path)
            .ok()
            .is_none_or(|m| m.len() == 0);
        if theme_missing_or_empty {
            if let Some(dir) = theme_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&theme_path, THEME_SKELETON_CONTENT);
        }

        // settings.conf
        let settings_missing_or_empty = std::fs::metadata(&settings_path)
            .ok()
            .is_none_or(|m| m.len() == 0);
        if settings_missing_or_empty {
            if let Some(dir) = settings_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&settings_path, SETTINGS_SKELETON_CONTENT);
        }

        // keybinds.conf
        let keybinds_missing_or_empty = std::fs::metadata(&keybinds_path)
            .ok()
            .is_none_or(|m| m.len() == 0);
        if keybinds_missing_or_empty {
            if let Some(dir) = keybinds_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&keybinds_path, KEYBINDS_SKELETON_CONTENT);
        }
        return;
    }
    let theme_path = base.join("theme.conf");
    let settings_path = base.join("settings.conf");

    let theme_missing_or_empty = std::fs::metadata(&theme_path)
        .ok()
        .is_none_or(|m| m.len() == 0);
    let settings_missing_or_empty = std::fs::metadata(&settings_path)
        .ok()
        .is_none_or(|m| m.len() == 0);
    if !theme_missing_or_empty && !settings_missing_or_empty {
        // Nothing to do
        return;
    }
    let Ok(content) = fs::read_to_string(&legacy) else {
        return;
    };

    let mut theme_lines: Vec<String> = Vec::new();
    let mut settings_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('=') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("");
        let key = raw_key.trim();
        let norm = key.to_lowercase().replace(['.', '-', ' '], "_");
        // Same classification as theme parsing: treat these as non-theme preference keys
        let is_pref_key = norm.starts_with("pref_")
            || norm.starts_with("settings_")
            || norm.starts_with("layout_")
            || norm.starts_with("keybind_")
            || norm.starts_with("app_")
            || norm.starts_with("sort_")
            || norm.starts_with("clipboard_")
            || norm.starts_with("show_")
            || norm == "results_sort";
        if is_pref_key {
            // Exclude keybinds from settings.conf; those live in keybinds.conf
            if !norm.starts_with("keybind_") {
                settings_lines.push(trimmed.to_string());
            }
        } else {
            theme_lines.push(trimmed.to_string());
        }
    }

    if theme_missing_or_empty {
        if let Some(dir) = theme_path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if theme_lines.is_empty() {
            let _ = fs::write(&theme_path, THEME_SKELETON_CONTENT);
        } else {
            let mut out = String::new();
            out.push_str("# Pacsea theme configuration (migrated from pacsea.conf)\n");
            out.push_str(&theme_lines.join("\n"));
            out.push('\n');
            let _ = fs::write(&theme_path, out);
        }
    }

    if settings_missing_or_empty {
        if let Some(dir) = settings_path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if settings_lines.is_empty() {
            let _ = fs::write(&settings_path, SETTINGS_SKELETON_CONTENT);
        } else {
            let mut out = String::new();
            out.push_str("# Pacsea settings configuration (migrated from pacsea.conf)\n");
            out.push_str(&settings_lines.join("\n"));
            out.push('\n');
            let _ = fs::write(&settings_path, out);
        }
    }
}
