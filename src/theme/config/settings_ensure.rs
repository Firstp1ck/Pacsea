use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::theme::config::skeletons::{
    KEYBINDS_SKELETON_CONTENT, SETTINGS_SKELETON_CONTENT, THEME_SKELETON_CONTENT,
};
use crate::theme::paths::{config_dir, resolve_settings_config_path};
use crate::theme::types::Settings;

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
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }

    let meta = std::fs::metadata(&p).ok();
    let file_exists = meta.is_some();
    let file_empty = meta.map(|m| m.len() == 0).unwrap_or(true);
    let created_new = !file_exists || file_empty;

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        if let Ok(content) = fs::read_to_string(&p) {
            content.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        // File doesn't exist or is empty - start with skeleton
        Vec::new()
    };

    // If file is missing or empty, seed with the built-in skeleton content first
    if created_new || lines.is_empty() {
        lines = SETTINGS_SKELETON_CONTENT
            .lines()
            .map(|s| s.to_string())
            .collect();
    }
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
    // Desired keys and their values from prefs
    let pairs: [(&str, String); 17] = [
        ("layout_left_pct", prefs.layout_left_pct.to_string()),
        ("layout_center_pct", prefs.layout_center_pct.to_string()),
        ("layout_right_pct", prefs.layout_right_pct.to_string()),
        (
            "app_dry_run_default",
            if prefs.app_dry_run_default {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        ("sort_mode", prefs.sort_mode.as_config_key().to_string()),
        ("clipboard_suffix", prefs.clipboard_suffix.clone()),
        (
            "show_recent_pane",
            if prefs.show_recent_pane {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        (
            "show_install_pane",
            if prefs.show_install_pane {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        (
            "show_keybinds_footer",
            if prefs.show_keybinds_footer {
                "true"
            } else {
                "false"
            }
            .to_string(),
        ),
        ("selected_countries", prefs.selected_countries.clone()),
        ("mirror_count", prefs.mirror_count.to_string()),
        ("virustotal_api_key", prefs.virustotal_api_key.clone()),
        ("news_read_symbol", prefs.news_read_symbol.clone()),
        ("news_unread_symbol", prefs.news_unread_symbol.clone()),
        ("preferred_terminal", prefs.preferred_terminal.clone()),
        (
            "package_marker",
            match prefs.package_marker {
                crate::theme::types::PackageMarker::FullLine => "full_line",
                crate::theme::types::PackageMarker::Front => "front",
                crate::theme::types::PackageMarker::End => "end",
            }
            .to_string(),
        ),
        ("locale", prefs.locale.clone()),
    ];
    let mut appended_any = false;
    // Ensure scan toggles exist; default to true when missing
    let scan_keys: [(&str, &str); 7] = [
        ("scan_do_clamav", "true"),
        ("scan_do_trivy", "true"),
        ("scan_do_semgrep", "true"),
        ("scan_do_shellcheck", "true"),
        ("scan_do_virustotal", "true"),
        ("scan_do_custom", "true"),
        ("scan_do_sleuth", "true"),
    ];
    for (k, v) in scan_keys.iter() {
        if !have.contains(*k) {
            lines.push(format!("{k} = {v}"));
            appended_any = true;
        }
    }
    for (k, v) in pairs.iter() {
        if !have.contains(*k) {
            lines.push(format!("{k} = {v}"));
            appended_any = true;
        }
    }
    if created_new || appended_any {
        let new_content = lines.join("\n");
        let _ = fs::write(p, new_content);
    }

    // Ensure keybinds file exists with skeleton if missing (best-effort)
    let kb = config_dir().join("keybinds.conf");
    if !kb.exists() {
        if let Some(dir) = kb.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let _ = fs::write(kb, KEYBINDS_SKELETON_CONTENT);
    }
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
        let theme_missing_or_empty = match std::fs::metadata(&theme_path) {
            Ok(m) => m.len() == 0,
            Err(_) => true,
        };
        if theme_missing_or_empty {
            if let Some(dir) = theme_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&theme_path, THEME_SKELETON_CONTENT);
        }

        // settings.conf
        let settings_missing_or_empty = match std::fs::metadata(&settings_path) {
            Ok(m) => m.len() == 0,
            Err(_) => true,
        };
        if settings_missing_or_empty {
            if let Some(dir) = settings_path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&settings_path, SETTINGS_SKELETON_CONTENT);
        }

        // keybinds.conf
        let keybinds_missing_or_empty = match std::fs::metadata(&keybinds_path) {
            Ok(m) => m.len() == 0,
            Err(_) => true,
        };
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

    let theme_missing_or_empty = match std::fs::metadata(&theme_path) {
        Ok(m) => m.len() == 0,
        Err(_) => true,
    };
    let settings_missing_or_empty = match std::fs::metadata(&settings_path) {
        Ok(m) => m.len() == 0,
        Err(_) => true,
    };
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
