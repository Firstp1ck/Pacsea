use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::theme::parsing::{apply_override_to_map, canonical_to_preferred};
use crate::theme::types::Theme;

/// What: Parse a theme configuration file containing `key = value` color pairs into a `Theme`.
///
/// Inputs:
/// - `path`: Filesystem location of the theme configuration.
///
/// Output:
/// - `Ok(Theme)` when all required colors are present and valid.
/// - `Err(String)` containing newline-separated diagnostics when parsing fails.
///
/// Details:
/// - Ignores preference keys that belong to other config files for backwards compatibility.
/// - Detects duplicates, missing required keys, and invalid color formats with precise line info.
pub fn try_load_theme_with_diagnostics(path: &Path) -> Result<Theme, String> {
    const REQUIRED: [&str; 16] = [
        "base", "mantle", "crust", "surface1", "surface2", "overlay1", "overlay2", "text",
        "subtext0", "subtext1", "sapphire", "mauve", "green", "yellow", "red", "lavender",
    ];
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
        let canon_or_norm =
            crate::theme::parsing::canonical_for_key(&norm).unwrap_or(norm.as_str());
        if !seen_keys.insert(canon_or_norm.to_string()) {
            errors.push(format!("- Duplicate key '{key}' on line {line_no}"));
        }
        apply_override_to_map(&mut map, key, val, &mut errors, line_no);
    }
    // Check missing required keys
    // Syntax command color defaults to sapphire if not specified
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
    if errors.is_empty() {
        let get = |name: &str| {
            map.get(name)
                .copied()
                .expect("all required keys should be present after validation")
        };
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
    } else {
        Err(errors.join("\n"))
    }
}

/// What: Load a theme from disk while discarding any diagnostics on failure.
///
/// Inputs:
/// - `path`: Filesystem path to the theme configuration.
///
/// Output:
/// - `Some(Theme)` when parsing succeeds.
/// - `None` when the file is missing or contains validation errors.
///
/// Details:
/// - Wraps [`try_load_theme_with_diagnostics`] and converts its error into `None`.
pub fn load_theme_from_file(path: &Path) -> Option<Theme> {
    try_load_theme_with_diagnostics(path).ok()
}
