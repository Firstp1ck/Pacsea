use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::theme::config::skeletons::{
    KEYBINDS_SKELETON_CONTENT, SETTINGS_SKELETON_CONTENT, THEME_SKELETON_CONTENT,
};
use crate::theme::paths::{config_dir, resolve_settings_config_path};
use crate::theme::types::Settings;

/// What: Get the value for a setting key, preferring prefs over skeleton default.
///
/// Inputs:
/// - `key`: Normalized key name
/// - `skeleton_value`: Default value from skeleton
/// - `prefs`: Current in-memory settings
///
/// Output:
/// - String value to use for the setting
fn get_setting_value(key: &str, skeleton_value: String, prefs: &Settings) -> String {
    match key {
        "layout_left_pct" => prefs.layout_left_pct.to_string(),
        "layout_center_pct" => prefs.layout_center_pct.to_string(),
        "layout_right_pct" => prefs.layout_right_pct.to_string(),
        "app_dry_run_default" => if prefs.app_dry_run_default {
            "true"
        } else {
            "false"
        }
        .to_string(),
        "sort_mode" => prefs.sort_mode.as_config_key().to_string(),
        "clipboard_suffix" => prefs.clipboard_suffix.clone(),
        "show_recent_pane" => if prefs.show_recent_pane {
            "true"
        } else {
            "false"
        }
        .to_string(),
        "show_install_pane" => if prefs.show_install_pane {
            "true"
        } else {
            "false"
        }
        .to_string(),
        "show_keybinds_footer" => if prefs.show_keybinds_footer {
            "true"
        } else {
            "false"
        }
        .to_string(),
        "selected_countries" => prefs.selected_countries.clone(),
        "mirror_count" => prefs.mirror_count.to_string(),
        "virustotal_api_key" => prefs.virustotal_api_key.clone(),
        "news_read_symbol" => prefs.news_read_symbol.clone(),
        "news_unread_symbol" => prefs.news_unread_symbol.clone(),
        "preferred_terminal" => prefs.preferred_terminal.clone(),
        "package_marker" => match prefs.package_marker {
            crate::theme::types::PackageMarker::FullLine => "full_line",
            crate::theme::types::PackageMarker::Front => "front",
            crate::theme::types::PackageMarker::End => "end",
        }
        .to_string(),
        "locale" => prefs.locale.clone(),
        "skip_preflight" => if prefs.skip_preflight {
            "true"
        } else {
            "false"
        }
        .to_string(),
        "scan_do_clamav" | "scan_do_trivy" | "scan_do_semgrep" | "scan_do_shellcheck"
        | "scan_do_virustotal" | "scan_do_custom" | "scan_do_sleuth" => {
            // Scan keys default to true
            "true".to_string()
        }
        _ => skeleton_value,
    }
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

/// What: Ensure all expected settings keys exist in `settings.conf`, appending defaults as needed.
///
/// Inputs:
/// - `prefs`: Current in-memory settings whose values seed the file when keys are missing.
///
/// Output:
/// - None.
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
    p.parent().map(|dir| {
        let _ = fs::create_dir_all(dir);
    });

    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.map(|m| m.len() == 0).unwrap_or(true);
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
            have.insert(key);
        }
    }

    // Parse skeleton to extract settings entries with their comments
    let skeleton_lines: Vec<&str> = SETTINGS_SKELETON_CONTENT.lines().collect();
    let missing_settings = parse_missing_settings(&skeleton_lines, &have, prefs);

    // If no missing settings, nothing to do (unless file was just created)
    if !created_new && missing_settings.is_empty() {
        return;
    }

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

    // Ensure keybinds file exists with skeleton if missing (best-effort)
    let kb = config_dir().join("keybinds.conf");
    if kb.exists() {
        // Append missing keybinds to existing file
        ensure_keybinds_present(&kb);
    } else {
        if let Some(dir) = kb.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(kb, KEYBINDS_SKELETON_CONTENT);
    }
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
    let existing_content = match fs::read_to_string(keybinds_path) {
        Ok(content) => content,
        Err(_) => return, // Can't read, skip
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

    for line in skeleton_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            current_comment = None;
            continue;
        }
        if trimmed.starts_with('#') {
            // Check if this is a comment for a keybind (not a section header)
            if !trimmed.contains("—")
                && !trimmed.starts_with("# Pacsea")
                && !trimmed.starts_with("# Modifiers")
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
            let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if have.contains(&key) {
                current_comment = None;
            } else {
                missing_keybinds.push((trimmed.to_string(), current_comment.take()));
            }
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
