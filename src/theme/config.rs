use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::parsing::{apply_override_to_map, canonical_to_preferred};
use super::paths::resolve_config_path;
use super::types::{Settings, Theme};

/// Skeleton configuration file content with default color values.
pub(crate) const SKELETON_CONFIG_CONTENT: &str = "# Pacsea theme configuration\n\
#\n\
# Format: key = value\n\
# Value formats supported:\n\
#   - #RRGGBB (hex)\n\
#   - R,G,B (decimal, 0-255 each)\n\
#   Example (decimal): text_primary = 205,214,244\n\
# Lines starting with # are comments.\n\
#\n\
# Key naming:\n\
#   Comprehensive names are preferred (shown first). Legacy keys remain supported\n\
#   for compatibility (e.g., \"base\", \"surface1\").\n\
#\n\
# Background layers (from darkest to lightest)\n\
background_base = #1e1e2e\n\
background_mantle = #181825\n\
background_crust = #11111b\n\
#\n\
# Component surfaces\n\
surface_level1 = #45475a\n\
surface_level2 = #585b70\n\
#\n\
# Low-contrast lines/borders\n\
overlay_primary = #7f849c\n\
overlay_secondary = #9399b2\n\
#\n\
# Text hierarchy\n\
text_primary = #cdd6f4\n\
text_secondary = #a6adc8\n\
text_tertiary = #bac2de\n\
#\n\
# Accents and semantic colors\n\
accent_interactive = #74c7ec\n\
accent_heading = #cba6f7\n\
accent_emphasis = #b4befe\n\
semantic_success = #a6e3a1\n\
semantic_warning = #f9e2af\n\
semantic_error = #f38ba8\n\
#\n\
# ---------- Alternative Theme (Light) ----------\n\
# To use this light theme, comment out the dark values above and uncomment the\n\
# lines below, or copy these into your own overrides.\n\
#\n\
# # Background layers (from lightest to darkest)\n\
# background_base = #f5f5f7\n\
# background_mantle = #eaeaee\n\
# background_crust = #dcdce1\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #cfd1d7\n\
# surface_level2 = #b7bac3\n\
#\n\
# # Low-contrast lines/borders and secondary text accents\n\
# overlay_primary = #7a7d86\n\
# overlay_secondary = #63666f\n\
#\n\
# # Text hierarchy\n\
# text_primary = #1c1c22\n\
# text_secondary = #3c3f47\n\
# text_tertiary = #565a64\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #1e66f5\n\
# accent_heading = #8839ef\n\
# accent_emphasis = #7287fd\n\
# semantic_success = #40a02b\n\
# semantic_warning = #df8e1d\n\
# semantic_error = #d20f39\n\
\n\
# Application settings\n\
# Layout percentages for the middle row panes (must sum to 100)\n\
layout_left_pct = 20\n\
layout_center_pct = 60\n\
layout_right_pct = 20\n\
# Default dry-run behavior when starting the app (overridden by --dry-run)\n\
app_dry_run_default = false\n\
# Middle row visibility (default true)\n\
show_recent_pane = true\n\
show_install_pane = true\n\
show_keybinds_footer = true\n\
\n\
# Results sorting\n\
# Allowed values: alphabetical | aur_popularity | best_matches\n\
sort_mode = best_matches\n\
\n\
# Clipboard\n\
# Text appended when copying PKGBUILD to the clipboard\n\
clipboard_suffix = Check PKGBUILD and source for suspicious and malicious activities\n\
\n\
# Keybindings (defaults)\n\
# Modifiers can be one of: SUPER, CTRL, SHIFT, ALT.\n\
\n\
# GLOBAL — App\n\
keybind_help = F1\n\
# Alternative help shortcut\n\
keybind_help = ?\n\
keybind_reload_theme = CTRL+R\n\
keybind_exit = CTRL+Q\n\
keybind_show_pkgbuild = CTRL+X\n\
\n\
# GLOBAL — Pane switching\n\
keybind_pane_left = Left\n\
keybind_pane_right = Right\n\
keybind_pane_next = Tab\n\
# GLOBAL — Sorting\n\
keybind_change_sort = BackTab\n\
\n\
# SEARCH — Navigation\n\
keybind_search_move_up = Up\n\
keybind_search_move_down = Down\n\
keybind_search_page_up = PgUp\n\
keybind_search_page_down = PgDn\n\
\n\
# SEARCH — Actions\n\
keybind_search_add = Space\n\
keybind_search_install = Enter\n\
\n\
# SEARCH — Focus/Edit\n\
keybind_search_focus_left = Left\n\
keybind_search_focus_right = Right\n\
keybind_search_backspace = Backspace\n\
\n\
# SEARCH — Normal Mode (Focused Search Window)\n\
keybind_search_normal_toggle = Esc\n\
keybind_search_normal_insert = i\n\
keybind_search_normal_select_left = h\n\
keybind_search_normal_select_right = l\n\
keybind_search_normal_delete = d\n\
\n\
# RECENT — Navigation\n\
keybind_recent_move_up = k\n\
keybind_recent_move_down = j\n\
\n\
# RECENT — Actions\n\
keybind_recent_use = Enter\n\
keybind_recent_add = Space\n\
keybind_recent_remove = d\n\
keybind_recent_remove = Del\n\
\n\
# RECENT — Find/Focus\n\
keybind_recent_find = /\n\
keybind_recent_to_search = Esc\n\
keybind_recent_focus_right = Right\n\
\n\
# INSTALL — Navigation\n\
keybind_install_move_up = k\n\
keybind_install_move_down = j\n\
\n\
# INSTALL — Actions\n\
keybind_install_confirm = Enter\n\
keybind_install_remove = Del\n\
keybind_install_remove = d\n\
keybind_install_clear = Shift+Del\n\
\n\
# INSTALL — Find/Focus\n\
keybind_install_find = /\n\
keybind_install_to_search = Esc\n\
keybind_install_focus_left = Left\n";

/// Attempt to parse a theme from a configuration file with simple `key = value` pairs.
pub(crate) fn try_load_theme_with_diagnostics(path: &Path) -> Result<Theme, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let mut map: HashMap<String, Color> = HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    let mut seen_keys: HashSet<String> = HashSet::new();
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        if !trimmed.contains('=') {
            errors.push(format!("- Missing '=' on line {line_no}"));
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("");
        let key = raw_key.trim();
        let val = parts.next().unwrap_or("").trim();
        if key.is_empty() {
            errors.push(format!("- Missing key before '=' on line {line_no}"));
            continue;
        }
        let norm = key.to_lowercase().replace(['.', '-', ' '], "_");
        // Allow non-theme preference keys to live in pacsea.conf without erroring
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
            // Skip theme handling; parsed elsewhere
            continue;
        }
        // Track duplicates (by canonical form if known, otherwise normalized input)
        let canon_or_norm = super::parsing::canonical_for_key(&norm).unwrap_or(norm.as_str());
        if !seen_keys.insert(canon_or_norm.to_string()) {
            errors.push(format!("- Duplicate key '{key}' on line {line_no}"));
        }
        apply_override_to_map(&mut map, key, val, &mut errors, line_no);
    }
    // Check missing required keys
    const REQUIRED: [&str; 16] = [
        "base", "mantle", "crust", "surface1", "surface2", "overlay1", "overlay2", "text",
        "subtext0", "subtext1", "sapphire", "mauve", "green", "yellow", "red", "lavender",
    ];
    let mut missing: Vec<&str> = Vec::new();
    for k in REQUIRED {
        if !map.contains_key(k) {
            missing.push(k);
        }
    }
    if !missing.is_empty() {
        let preferred: Vec<String> = missing.iter().map(|k| canonical_to_preferred(k)).collect();
        errors.push(format!("- Missing required keys: {}", preferred.join(", ")));
    }
    if !errors.is_empty() {
        Err(errors.join("\n"))
    } else {
        let get = |name: &str| map.get(name).copied().unwrap();
        Ok(Theme {
            base: get("base"),
            mantle: get("mantle"),
            crust: get("crust"),
            surface1: get("surface1"),
            surface2: get("surface2"),
            overlay1: get("overlay1"),
            overlay2: get("overlay2"),
            text: get("text"),
            subtext0: get("subtext0"),
            subtext1: get("subtext1"),
            sapphire: get("sapphire"),
            mauve: get("mauve"),
            green: get("green"),
            yellow: get("yellow"),
            red: get("red"),
            lavender: get("lavender"),
        })
    }
}

pub(crate) fn load_theme_from_file(path: &Path) -> Option<Theme> {
    try_load_theme_with_diagnostics(path).ok()
}

/// Persist selected sort mode back to `pacsea.conf`, preserving comments and other keys.
pub fn save_sort_mode(sm: crate::state::SortMode) {
    let path = resolve_config_path().or_else(|| {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| Path::new(&h).join(".config"))
            })
            .map(|base| base.join("pacsea").join("pacsea.conf"))
    });
    let Some(p) = path else {
        return;
    };
    let mut lines: Vec<String> = if let Ok(content) = fs::read_to_string(&p) {
        content.lines().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
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

/// Persist a boolean key to pacsea.conf, preserving other content.
fn save_boolean_key(key_norm: &str, value: bool) {
    let path = resolve_config_path().or_else(|| {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| Path::new(&h).join(".config"))
            })
            .map(|base| base.join("pacsea").join("pacsea.conf"))
    });
    let Some(p) = path else {
        return;
    };
    let mut lines: Vec<String> = if let Ok(content) = fs::read_to_string(&p) {
        content.lines().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
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
pub fn save_show_recent_pane(value: bool) {
    save_boolean_key("show_recent_pane", value)
}
pub fn save_show_install_pane(value: bool) {
    save_boolean_key("show_install_pane", value)
}
pub fn save_show_keybinds_footer(value: bool) {
    save_boolean_key("show_keybinds_footer", value)
}

/// Ensure core application settings keys exist in the config file; append missing with current/default values.
///
/// This preserves existing lines and comments, only appending keys that are not present.
pub fn ensure_settings_keys_present(prefs: &Settings) {
    // Always resolve to HOME/XDG path similar to save_sort_mode
    let path = resolve_config_path().or_else(|| {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| Path::new(&h).join(".config"))
            })
            .map(|base| base.join("pacsea").join("pacsea.conf"))
    });
    let Some(p) = path else {
        return;
    };
    let meta = std::fs::metadata(&p).ok();
    let created_new = meta.is_none() || meta.map(|m| m.len() == 0).unwrap_or(true);
    let mut lines: Vec<String> = if let Ok(content) = fs::read_to_string(&p) {
        content.lines().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    // If file is missing or empty, seed with the built-in skeleton content first
    if created_new || lines.is_empty() {
        if let Some(dir) = p.parent() { let _ = fs::create_dir_all(dir); }
        lines = SKELETON_CONFIG_CONTENT
            .lines()
            .map(|s| s.to_string())
            .collect();
    }
    use std::collections::HashSet;
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
    let pairs: [(&str, String); 9] = [
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
    ];
    let mut appended_any = false;
    for (k, v) in pairs.iter() {
        if !have.contains(*k) {
            lines.push(format!("{} = {}", k, v));
            appended_any = true;
        }
    }
    if created_new || appended_any {
        let new_content = lines.join("\n");
        let _ = fs::write(p, new_content);
    }
}
