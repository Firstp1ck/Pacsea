use std::fs;
use std::path::Path;

use crate::theme::config::skeletons::SETTINGS_SKELETON_CONTENT;
use crate::theme::paths::resolve_settings_config_path;

/// What: Persist the user-selected sort mode into `settings.conf` (or legacy `pacsea.conf`).
///
/// Inputs:
/// - `sm`: Sort mode chosen in the UI, expressed as `crate::state::SortMode`.
///
/// Output:
/// - None.
///
/// Details:
/// - Ensures the target file exists by seeding from the skeleton when missing.
/// - Replaces existing `sort_mode`/`results_sort` entries while preserving comments.
pub fn save_sort_mode(sm: crate::state::SortMode) {
    let path = resolve_settings_config_path().or_else(|| {
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
    let Some(p) = path else {
        return;
    };

    // Ensure directory exists
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    // If file doesn't exist or is empty, initialize with skeleton
    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.is_none_or(|m| m.len() == 0);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        fs::read_to_string(&p)
            .map(|content| content.lines().map(ToString::to_string).collect())
            .unwrap_or_default()
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(ToString::to_string)
            .collect()
    };
    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (kraw, _) = trimmed.split_at(eq);
            let key = kraw.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if key == "sort_mode" || key == "results_sort" {
                *line = format!("sort_mode = {}", sm.as_config_key());
                replaced = true;
            }
        }
    }
    if !replaced {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        lines.push(format!("sort_mode = {}", sm.as_config_key()));
    }
    let new_content = if lines.is_empty() {
        format!("sort_mode = {}\n", sm.as_config_key())
    } else {
        lines.join("\n")
    };
    let _ = fs::write(p, new_content);
}

/// What: Persist a single boolean toggle within `settings.conf` while preserving unrelated content.
///
/// Inputs:
/// - `primary_key`: Primary key name to update (lowercase, underscore-separated recommended).
/// - `aliases`: Optional aliases that should map to the same setting (legacy compatibility).
/// - `value`: Boolean flag to serialize as `true` or `false`.
///
/// Output:
/// - None.
///
/// Details:
/// - Creates the configuration file from the skeleton when it is missing or empty.
/// - Rewrites existing entries (including aliases) in place; otherwise appends the primary key.
/// - When an alias is encountered, it is replaced with the primary key to migrate configs forward.
fn save_boolean_key_with_aliases(primary_key: &str, aliases: &[&str], value: bool) {
    let path = resolve_settings_config_path().or_else(|| {
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
    let Some(p) = path else {
        return;
    };

    // Ensure directory exists
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    // If file doesn't exist or is empty, initialize with skeleton
    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.is_none_or(|m| m.len() == 0);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        fs::read_to_string(&p)
            .map(|content| content.lines().map(ToString::to_string).collect())
            .unwrap_or_default()
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(ToString::to_string)
            .collect()
    };
    let bool_text = if value { "true" } else { "false" };
    let primary_norm = primary_key
        .trim()
        .to_lowercase()
        .replace(['.', '-', ' '], "_");
    let alias_norms: Vec<String> = aliases
        .iter()
        .map(|k| k.trim().to_lowercase().replace(['.', '-', ' '], "_"))
        .collect();
    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (kraw, _) = trimmed.split_at(eq);
            let key = kraw.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if key == primary_norm || alias_norms.iter().any(|alias| alias == &key) {
                *line = format!("{primary_key} = {bool_text}");
                replaced = true;
            }
        }
    }
    if !replaced {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        lines.push(format!("{primary_key} = {bool_text}"));
    }
    let new_content = if lines.is_empty() {
        format!("{primary_key} = {bool_text}\n")
    } else {
        lines.join("\n")
    };
    let _ = fs::write(p, new_content);
}

/// What: Persist a single boolean toggle within `settings.conf` while preserving unrelated content.
///
/// Inputs:
/// - `key_norm`: Normalized (lowercase, underscore-separated) key name to update.
/// - `value`: Boolean flag to serialize as `true` or `false`.
///
/// Output:
/// - None.
///
/// Details:
/// - Convenience wrapper that delegates to `save_boolean_key_with_aliases` without aliases.
fn save_boolean_key(key_norm: &str, value: bool) {
    save_boolean_key_with_aliases(key_norm, &[], value);
}

/// What: Persist a string-valued setting inside `settings.conf` without disturbing other keys.
///
/// Inputs:
/// - `key_norm`: Normalized key to update.
/// - `value`: String payload that should be written verbatim after trimming handled by the caller.
///
/// Output:
/// - None.
///
/// Details:
/// - Bootstraps the configuration file from the skeleton if necessary.
/// - Updates the existing key in place or appends a new line when absent.
fn save_string_key(key_norm: &str, value: &str) {
    let path = resolve_settings_config_path().or_else(|| {
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
    let Some(p) = path else {
        return;
    };

    // Ensure directory exists
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    // If file doesn't exist or is empty, initialize with skeleton
    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.is_none_or(|m| m.len() == 0);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        fs::read_to_string(&p)
            .map(|content| content.lines().map(ToString::to_string).collect())
            .unwrap_or_default()
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(ToString::to_string)
            .collect()
    };
    let mut replaced = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (kraw, _) = trimmed.split_at(eq);
            let key = kraw.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if key == key_norm {
                *line = format!("{key_norm} = {value}");
                replaced = true;
            }
        }
    }
    if !replaced {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        lines.push(format!("{key_norm} = {value}"));
    }
    let new_content = if lines.is_empty() {
        format!("{key_norm} = {value}\n")
    } else {
        lines.join("\n")
    };
    let _ = fs::write(p, new_content);
}

/// What: Persist the visibility flag for the Search history pane.
///
/// Inputs:
/// - `value`: Whether the Search history pane should be shown on startup.
///
/// Output:
/// - None.
///
/// Details:
/// - Writes to the canonical `show_search_history_pane` key while migrating legacy
///   `show_recent_pane` entries.
pub fn save_show_recent_pane(value: bool) {
    save_boolean_key_with_aliases("show_search_history_pane", &["show_recent_pane"], value);
}
/// What: Persist the visibility flag for the Install pane.
///
/// Inputs:
/// - `value`: Whether the Install pane should be shown on startup.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("show_install_pane", value)`.
pub fn save_show_install_pane(value: bool) {
    save_boolean_key("show_install_pane", value);
}
/// What: Persist the visibility flag for the keybinds footer.
///
/// Inputs:
/// - `value`: Whether the footer should be rendered.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("show_keybinds_footer", value)`.
pub fn save_show_keybinds_footer(value: bool) {
    save_boolean_key("show_keybinds_footer", value);
}

/// What: Persist the comma-separated list of preferred mirror countries.
///
/// Inputs:
/// - `value`: Country list string (already normalized by caller).
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_string_key("selected_countries", ...)`.
pub fn save_selected_countries(value: &str) {
    save_string_key("selected_countries", value);
}
/// What: Persist the numeric limit on ranked mirrors.
///
/// Inputs:
/// - `value`: Mirror count to record.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_string_key("mirror_count", value)` after converting to text.
pub fn save_mirror_count(value: u16) {
    save_string_key("mirror_count", &value.to_string());
}

/// Persist start mode (package/news).
pub fn save_app_start_mode(start_in_news: bool) {
    let v = if start_in_news { "news" } else { "package" };
    save_string_key("app_start_mode", v);
}

/// Persist whether to show Arch news items.
pub fn save_news_filter_show_arch_news(value: bool) {
    save_boolean_key("news_filter_show_arch_news", value);
}

/// Persist whether to show security advisories.
pub fn save_news_filter_show_advisories(value: bool) {
    save_boolean_key("news_filter_show_advisories", value);
}

/// Persist whether to show installed package updates in the News view.
pub fn save_news_filter_show_pkg_updates(value: bool) {
    save_boolean_key("news_filter_show_pkg_updates", value);
}

/// Persist whether to show AUR comments in the News view.
pub fn save_news_filter_show_aur_comments(value: bool) {
    save_boolean_key("news_filter_show_aur_comments", value);
}

/// Persist whether to restrict advisories to installed packages.
pub fn save_news_filter_installed_only(value: bool) {
    save_boolean_key("news_filter_installed_only", value);
}

/// Persist the maximum age of news items (None = all).
pub fn save_news_max_age_days(value: Option<u32>) {
    let v = value.map_or_else(|| "all".to_string(), |d| d.to_string());
    save_string_key("news_max_age_days", &v);
}

/// What: Persist the `VirusTotal` API key used for scanning packages.
///
/// Inputs:
/// - `value`: API key string supplied by the user.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_string_key("virustotal_api_key", ...)`.
pub fn save_virustotal_api_key(value: &str) {
    save_string_key("virustotal_api_key", value);
}

/// What: Persist the `ClamAV` scan toggle.
///
/// Inputs:
/// - `value`: Whether `ClamAV` scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_clamav", value)`.
pub fn save_scan_do_clamav(value: bool) {
    save_boolean_key("scan_do_clamav", value);
}
/// What: Persist the Trivy scan toggle.
///
/// Inputs:
/// - `value`: Whether Trivy scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_trivy", value)`.
pub fn save_scan_do_trivy(value: bool) {
    save_boolean_key("scan_do_trivy", value);
}
/// What: Persist the Semgrep scan toggle.
///
/// Inputs:
/// - `value`: Whether Semgrep scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_semgrep", value)`.
pub fn save_scan_do_semgrep(value: bool) {
    save_boolean_key("scan_do_semgrep", value);
}
/// What: Persist the `ShellCheck` scan toggle.
///
/// Inputs:
/// - `value`: Whether `ShellCheck` scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_shellcheck", value)`.
pub fn save_scan_do_shellcheck(value: bool) {
    save_boolean_key("scan_do_shellcheck", value);
}
/// What: Persist the `VirusTotal` scan toggle.
///
/// Inputs:
/// - `value`: Whether `VirusTotal` scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_virustotal", value)`.
pub fn save_scan_do_virustotal(value: bool) {
    save_boolean_key("scan_do_virustotal", value);
}
/// What: Persist the custom scan toggle.
///
/// Inputs:
/// - `value`: Whether user-defined custom scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_custom", value)`.
pub fn save_scan_do_custom(value: bool) {
    save_boolean_key("scan_do_custom", value);
}

/// What: Persist the Sleuth scan toggle.
///
/// Inputs:
/// - `value`: Whether Sleuth scans should run by default.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("scan_do_sleuth", value)`.
pub fn save_scan_do_sleuth(value: bool) {
    save_boolean_key("scan_do_sleuth", value);
}

/// What: Persist the fuzzy search toggle.
///
/// Inputs:
/// - `value`: Whether fuzzy search should be enabled.
///
/// Output:
/// - None.
///
/// Details:
/// - Delegates to `save_boolean_key("fuzzy_search", value)`.
pub fn save_fuzzy_search(value: bool) {
    save_boolean_key("fuzzy_search", value);
}
