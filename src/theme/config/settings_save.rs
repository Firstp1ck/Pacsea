use std::fs;
use std::path::Path;

use crate::theme::config::skeletons::SETTINGS_SKELETON_CONTENT;
use crate::theme::paths::resolve_settings_config_path;

/// Persist selected sort mode back to settings.conf (or legacy pacsea.conf), preserving comments and other keys.
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
    let file_empty = meta.map(|m| m.len() == 0).unwrap_or(true);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        if let Ok(content) = fs::read_to_string(&p) {
            content.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(|s| s.to_string())
            .collect()
    };
    let mut replaced = false;
    for line in lines.iter_mut() {
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

/// Persist a boolean key to settings.conf (or legacy pacsea.conf), preserving other content.
fn save_boolean_key(key_norm: &str, value: bool) {
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
    let file_empty = meta.map(|m| m.len() == 0).unwrap_or(true);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        if let Ok(content) = fs::read_to_string(&p) {
            content.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(|s| s.to_string())
            .collect()
    };
    let mut replaced = false;
    for line in lines.iter_mut() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if let Some(eq) = trimmed.find('=') {
            let (kraw, _) = trimmed.split_at(eq);
            let key = kraw.trim().to_lowercase().replace(['.', '-', ' '], "_");
            if key == key_norm {
                *line = format!("{} = {}", key_norm, if value { "true" } else { "false" });
                replaced = true;
            }
        }
    }
    if !replaced {
        if let Some(dir) = p.parent() {
            let _ = fs::create_dir_all(dir);
        }
        lines.push(format!(
            "{} = {}",
            key_norm,
            if value { "true" } else { "false" }
        ));
    }
    let new_content = if lines.is_empty() {
        format!("{} = {}\n", key_norm, if value { "true" } else { "false" })
    } else {
        lines.join("\n")
    };
    let _ = fs::write(p, new_content);
}

/// Persist Recent and Install pane visibility toggles.
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
    let file_empty = meta.map(|m| m.len() == 0).unwrap_or(true);

    let mut lines: Vec<String> = if file_exists && !file_empty {
        // File exists and has content - read it
        if let Ok(content) = fs::read_to_string(&p) {
            content.lines().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        }
    } else {
        // File doesn't exist or is empty - start with skeleton
        SETTINGS_SKELETON_CONTENT
            .lines()
            .map(|s| s.to_string())
            .collect()
    };
    let mut replaced = false;
    for line in lines.iter_mut() {
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

pub fn save_show_recent_pane(value: bool) {
    save_boolean_key("show_recent_pane", value)
}
pub fn save_show_install_pane(value: bool) {
    save_boolean_key("show_install_pane", value)
}
pub fn save_show_keybinds_footer(value: bool) {
    save_boolean_key("show_keybinds_footer", value)
}

/// Persist mirror settings
pub fn save_selected_countries(value: &str) {
    save_string_key("selected_countries", value)
}
pub fn save_mirror_count(value: u16) {
    save_string_key("mirror_count", &value.to_string())
}

pub fn save_virustotal_api_key(value: &str) {
    save_string_key("virustotal_api_key", value)
}

pub fn save_scan_do_clamav(value: bool) {
    save_boolean_key("scan_do_clamav", value)
}
pub fn save_scan_do_trivy(value: bool) {
    save_boolean_key("scan_do_trivy", value)
}
pub fn save_scan_do_semgrep(value: bool) {
    save_boolean_key("scan_do_semgrep", value)
}
pub fn save_scan_do_shellcheck(value: bool) {
    save_boolean_key("scan_do_shellcheck", value)
}
pub fn save_scan_do_virustotal(value: bool) {
    save_boolean_key("scan_do_virustotal", value)
}
pub fn save_scan_do_custom(value: bool) {
    save_boolean_key("scan_do_custom", value)
}

pub fn save_scan_do_sleuth(value: bool) {
    save_boolean_key("scan_do_sleuth", value)
}
