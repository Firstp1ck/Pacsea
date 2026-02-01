//! Unified theme resolution logic.
//!
//! This module implements the decision flow for determining which theme source
//! to use: theme.conf, terminal colors via OSC 10/11, or codebase default.
//!
//! ## Decision Flow
//!
//! 1. Try to load theme from theme.conf
//! 2. If valid theme from file:
//!    - If `use_terminal_theme` is true and terminal is supported: query OSC, use terminal theme if successful
//!    - Otherwise: use file theme
//! 3. If no valid theme from file:
//!    - If terminal is supported: query OSC, use terminal theme if successful
//!    - Otherwise: use codebase default
//! 4. If OSC query fails: fall back to file theme if valid, else codebase default

use super::config::{THEME_SKELETON_CONTENT, try_load_theme_with_diagnostics};
use super::paths::{config_dir, resolve_theme_config_path};
use super::settings::settings;
use super::terminal_detect::is_supported_terminal_for_theme;
use super::terminal_query::{query_terminal_colors, theme_from_fg_bg};
use super::types::Theme;

/// Source of the resolved theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    /// Theme loaded from theme.conf file.
    File,
    /// Theme derived from terminal colors via OSC 10/11.
    Terminal,
    /// Codebase default theme (Catppuccin Mocha).
    Default,
}

/// Result of theme resolution.
#[derive(Debug)]
pub struct ResolvedTheme {
    /// The resolved theme.
    pub theme: Theme,
    /// Where the theme came from.
    pub source: ThemeSource,
}

/// What: Resolve the theme using the unified decision flow.
///
/// Inputs:
/// - None (reads settings, theme.conf, and queries terminal as needed).
///
/// Output:
/// - A `ResolvedTheme` containing the theme and its source.
///
/// Details:
/// - Implements the full decision flow from the plan.
/// - Always returns a valid theme (never fails).
/// - Logs the resolution path for debugging.
#[must_use]
pub fn resolve_theme() -> ResolvedTheme {
    let prefs = settings();
    let use_terminal = prefs.use_terminal_theme;

    // Step 1: Try to load theme from file
    let file_theme = try_load_file_theme();

    // Step 2: Determine resolution path
    match file_theme {
        Some(theme) if !use_terminal => {
            // Valid file theme and user doesn't want terminal theme
            tracing::info!("Using theme from theme.conf");
            ResolvedTheme {
                theme,
                source: ThemeSource::File,
            }
        }
        Some(file_theme) => {
            // Valid file theme but user wants terminal theme
            // Try terminal if supported, fall back to file
            if is_supported_terminal_for_theme() {
                if let Some(resolved) = try_terminal_theme() {
                    tracing::info!(
                        "Using terminal theme (use_terminal_theme=true, supported terminal)"
                    );
                    return resolved;
                }
                tracing::info!("Terminal theme query failed, falling back to theme.conf");
            } else {
                tracing::info!(
                    "use_terminal_theme=true but terminal not supported, using theme.conf"
                );
            }
            ResolvedTheme {
                theme: file_theme,
                source: ThemeSource::File,
            }
        }
        None => {
            // No valid file theme - try terminal if supported, else default
            if is_supported_terminal_for_theme() {
                if let Some(resolved) = try_terminal_theme() {
                    tracing::info!(
                        "No valid theme.conf, using terminal theme (supported terminal)"
                    );
                    return resolved;
                }
                tracing::info!(
                    "No valid theme.conf and terminal query failed, using codebase default"
                );
            } else {
                tracing::info!(
                    "No valid theme.conf and terminal not supported, using codebase default"
                );
            }
            ResolvedTheme {
                theme: default_theme(),
                source: ThemeSource::Default,
            }
        }
    }
}

/// Try to load theme from the theme.conf file.
fn try_load_file_theme() -> Option<Theme> {
    let path = resolve_theme_config_path().or_else(|| {
        let target = config_dir().join("theme.conf");
        if target.exists() { Some(target) } else { None }
    })?;

    // Check if file exists and is not empty
    let meta = std::fs::metadata(&path).ok()?;
    if meta.len() == 0 {
        return None;
    }

    match try_load_theme_with_diagnostics(&path) {
        Ok(theme) => Some(theme),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Failed to load theme.conf");
            None
        }
    }
}

/// Try to get theme from terminal via OSC 10/11.
fn try_terminal_theme() -> Option<ResolvedTheme> {
    let (fg, bg) = query_terminal_colors()?;
    let theme = theme_from_fg_bg(fg, bg);
    Some(ResolvedTheme {
        theme,
        source: ThemeSource::Terminal,
    })
}

/// Get the codebase default theme (Catppuccin Mocha).
///
/// This parses the skeleton content to get the default theme colors.
fn default_theme() -> Theme {
    // Parse the skeleton content to extract default colors
    // This is the same as what would be written to a new theme.conf
    parse_skeleton_theme().unwrap_or_else(hardcoded_default_theme)
}

/// Parse the theme skeleton to get default colors.
fn parse_skeleton_theme() -> Option<Theme> {
    use ratatui::style::Color;
    use std::collections::HashMap;

    let mut map: HashMap<String, Color> = HashMap::new();

    for line in THEME_SKELETON_CONTENT.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.contains('=') {
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let key = parts.next()?.trim();
        let val = parts.next()?.trim();

        // Parse color value
        if let Some(color) = parse_color(val) {
            let norm = normalize_key(key);
            map.insert(norm, color);
        }
    }

    // Check we have all required keys
    let required = [
        "base", "mantle", "crust", "surface1", "surface2", "overlay1", "overlay2", "text",
        "subtext0", "subtext1", "sapphire", "mauve", "green", "yellow", "red", "lavender",
    ];

    for k in required {
        if !map.contains_key(k) {
            return None;
        }
    }

    Some(Theme {
        base: map["base"],
        mantle: map["mantle"],
        crust: map["crust"],
        surface1: map["surface1"],
        surface2: map["surface2"],
        overlay1: map["overlay1"],
        overlay2: map["overlay2"],
        text: map["text"],
        subtext0: map["subtext0"],
        subtext1: map["subtext1"],
        sapphire: map["sapphire"],
        mauve: map["mauve"],
        green: map["green"],
        yellow: map["yellow"],
        red: map["red"],
        lavender: map["lavender"],
    })
}

/// Normalize a theme key to canonical form.
fn normalize_key(key: &str) -> String {
    let norm = key.to_lowercase().replace(['.', '-', ' '], "_");
    // Map preferred names to canonical names
    match norm.as_str() {
        "background_base" => "base".to_string(),
        "background_mantle" => "mantle".to_string(),
        "background_crust" => "crust".to_string(),
        "surface_level1" => "surface1".to_string(),
        "surface_level2" => "surface2".to_string(),
        "overlay_primary" => "overlay1".to_string(),
        "overlay_secondary" => "overlay2".to_string(),
        "text_primary" => "text".to_string(),
        "text_secondary" => "subtext0".to_string(),
        "text_tertiary" => "subtext1".to_string(),
        "accent_interactive" => "sapphire".to_string(),
        "accent_heading" => "mauve".to_string(),
        "accent_emphasis" => "lavender".to_string(),
        "semantic_success" => "green".to_string(),
        "semantic_warning" => "yellow".to_string(),
        "semantic_error" => "red".to_string(),
        _ => norm,
    }
}

/// Parse a color value from string.
fn parse_color(val: &str) -> Option<ratatui::style::Color> {
    let val = val.trim();

    // Handle hex format #RRGGBB
    if val.starts_with('#') && val.len() == 7 {
        let r = u8::from_str_radix(&val[1..3], 16).ok()?;
        let g = u8::from_str_radix(&val[3..5], 16).ok()?;
        let b = u8::from_str_radix(&val[5..7], 16).ok()?;
        return Some(ratatui::style::Color::Rgb(r, g, b));
    }

    // Handle decimal format R,G,B
    if val.contains(',') {
        let parts: Vec<&str> = val.split(',').collect();
        if parts.len() >= 3 {
            let r: u8 = parts[0].trim().parse().ok()?;
            let g: u8 = parts[1].trim().parse().ok()?;
            let b: u8 = parts[2].trim().parse().ok()?;
            return Some(ratatui::style::Color::Rgb(r, g, b));
        }
    }

    None
}

/// Hardcoded fallback theme if skeleton parsing fails.
/// This is Catppuccin Mocha.
const fn hardcoded_default_theme() -> Theme {
    use ratatui::style::Color;

    Theme {
        base: Color::Rgb(30, 30, 46),        // #1e1e2e
        mantle: Color::Rgb(24, 24, 37),      // #181825
        crust: Color::Rgb(17, 17, 27),       // #11111b
        surface1: Color::Rgb(69, 71, 90),    // #45475a
        surface2: Color::Rgb(88, 91, 112),   // #585b70
        overlay1: Color::Rgb(127, 132, 156), // #7f849c
        overlay2: Color::Rgb(147, 153, 178), // #9399b2
        text: Color::Rgb(205, 214, 244),     // #cdd6f4
        subtext0: Color::Rgb(166, 173, 200), // #a6adc8
        subtext1: Color::Rgb(186, 194, 222), // #bac2de
        sapphire: Color::Rgb(116, 199, 236), // #74c7ec
        mauve: Color::Rgb(203, 166, 247),    // #cba6f7
        green: Color::Rgb(166, 227, 161),    // #a6e3a1
        yellow: Color::Rgb(249, 226, 175),   // #f9e2af
        red: Color::Rgb(243, 139, 168),      // #f38ba8
        lavender: Color::Rgb(180, 190, 254), // #b4befe
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_hex() {
        let color = parse_color("#1e1e2e");
        let color = color.expect("hex color should parse");
        assert!(matches!(color, ratatui::style::Color::Rgb(30, 30, 46)));
    }

    #[test]
    fn test_parse_color_decimal() {
        let color = parse_color("205, 214, 244");
        let color = color.expect("decimal color should parse");
        assert!(matches!(color, ratatui::style::Color::Rgb(205, 214, 244)));
    }

    #[test]
    fn test_normalize_key() {
        assert_eq!(normalize_key("background_base"), "base");
        assert_eq!(normalize_key("text_primary"), "text");
        assert_eq!(normalize_key("accent_interactive"), "sapphire");
        assert_eq!(normalize_key("base"), "base"); // Already canonical
    }

    #[test]
    fn test_hardcoded_default_theme() {
        let theme = hardcoded_default_theme();
        // Just verify it returns a valid theme
        assert!(matches!(theme.base, ratatui::style::Color::Rgb(30, 30, 46)));
    }

    #[test]
    fn test_parse_skeleton_theme() {
        let theme = parse_skeleton_theme();
        let theme = theme.expect("Should be able to parse skeleton theme");
        // Verify a few key colors from Catppuccin Mocha
        assert!(matches!(theme.base, ratatui::style::Color::Rgb(30, 30, 46)));
        assert!(matches!(
            theme.text,
            ratatui::style::Color::Rgb(205, 214, 244)
        ));
    }
}
