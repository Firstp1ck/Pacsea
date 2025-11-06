use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use super::parsing::{apply_override_to_map, canonical_to_preferred};
use super::paths::{config_dir, resolve_settings_config_path};
use super::types::{Settings, Theme};

/// Skeleton configuration file content with default color values.
pub(crate) const THEME_SKELETON_CONTENT: &str = "# Pacsea theme configuration\n\
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
#-----------------------------------------------------------------------------------------------------------------------\n\
#\n\
# ---------- Catppuccin Mocha (dark) ----------\n\
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
# ---------- Alternative Theme (Tokyo Night — Night) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #1a1b26\n\
# background_mantle = #16161e\n\
# background_crust = #0f0f14\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #24283b\n\
# surface_level2 = #1f2335\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #414868\n\
# overlay_secondary = #565f89\n\
#\n\
# # Text hierarchy\n\
# text_primary = #c0caf5\n\
# text_secondary = #a9b1d6\n\
# text_tertiary = #9aa5ce\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #7aa2f7\n\
# accent_heading = #bb9af7\n\
# accent_emphasis = #7dcfff\n\
# semantic_success = #9ece6a\n\
# semantic_warning = #e0af68\n\
# semantic_error = #f7768e\n\
\n\
# ---------- Alternative Theme (Nord) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #2e3440\n\
# background_mantle = #3b4252\n\
# background_crust = #434c5e\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #3b4252\n\
# surface_level2 = #4c566a\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #4c566a\n\
# overlay_secondary = #616e88\n\
#\n\
# # Text hierarchy\n\
# text_primary = #e5e9f0\n\
# text_secondary = #d8dee9\n\
# text_tertiary = #eceff4\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #81a1c1\n\
# accent_heading = #b48ead\n\
# accent_emphasis = #88c0d0\n\
# semantic_success = #a3be8c\n\
# semantic_warning = #ebcb8b\n\
# semantic_error = #bf616a\n\
\n\
# ---------- Alternative Theme (Dracula) ----------\n\
#\n\
# # Background layers (from darkest to lightest)\n\
# background_base = #282a36\n\
# background_mantle = #21222c\n\
# background_crust = #44475a\n\
#\n\
# # Component surfaces\n\
# surface_level1 = #44475a\n\
# surface_level2 = #343746\n\
#\n\
# # Low-contrast lines/borders\n\
# overlay_primary = #44475a\n\
# overlay_secondary = #6272a4\n\
#\n\
# # Text hierarchy\n\
# text_primary = #f8f8f2\n\
# text_secondary = #e2e2e6\n\
# text_tertiary = #d6d6de\n\
#\n\
# # Accents and semantic colors\n\
# accent_interactive = #8be9fd\n\
# accent_heading = #bd93f9\n\
# accent_emphasis = #ff79c6\n\
# semantic_success = #50fa7b\n\
# semantic_warning = #f1fa8c\n\
# semantic_error = #ff5555\n\
#\n\
#-----------------------------------------------------------------------------------------------------------------------\n";

/// Standalone settings skeleton used when initializing a separate settings.conf
pub(crate) const SETTINGS_SKELETON_CONTENT: &str = "# Pacsea settings configuration\n\
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
# Mirrors\n\
# Select one or more countries (comma-separated). Example: \"Switzerland, Germany, Austria\"\n\
selected_countries = Worldwide\n\
# Number of HTTPS mirrors to consider when updating\n\
mirror_count = 20\n\
# Available countries (commented list; edit selected_countries above as needed):\n\
# Worldwide\n\
# Albania\n\
# Algeria\n\
# Argentina\n\
# Armenia\n\
# Australia\n\
# Austria\n\
# Azerbaijan\n\
# Belarus\n\
# Belgium\n\
# Bosnia and Herzegovina\n\
# Brazil\n\
# Bulgaria\n\
# Cambodia\n\
# Canada\n\
# Chile\n\
# China\n\
# Colombia\n\
# Costa Rica\n\
# Croatia\n\
# Cyprus\n\
# Czechia\n\
# Denmark\n\
# Ecuador\n\
# Estonia\n\
# Finland\n\
# France\n\
# Georgia\n\
# Germany\n\
# Greece\n\
# Hong Kong\n\
# Hungary\n\
# Iceland\n\
# India\n\
# Indonesia\n\
# Iran\n\
# Ireland\n\
# Israel\n\
# Italy\n\
# Japan\n\
# Kazakhstan\n\
# Latvia\n\
# Lithuania\n\
# Luxembourg\n\
# Malaysia\n\
# Mexico\n\
# Moldova\n\
# Netherlands\n\
# New Caledonia\n\
# New Zealand\n\
# Norway\n\
# Peru\n\
# Philippines\n\
# Poland\n\
# Portugal\n\
# Romania\n\
# Russia\n\
# Serbia\n\
# Singapore\n\
# Slovakia\n\
# Slovenia\n\
# South Africa\n\
# South Korea\n\
# Spain\n\
# Sweden\n\
# Switzerland\n\
# Taiwan\n\
# Thailand\n\
# Turkey\n\
# Ukraine\n\
# United Kingdom\n\
# United States\n\
# Uruguay\n\
# Vietnam\n\
\n\
# Scans\n\
# Default scan configuration used when opening Scan Configuration\n\
scan_do_clamav = true\n\
scan_do_trivy = true\n\
scan_do_semgrep = true\n\
scan_do_shellcheck = true\n\
scan_do_virustotal = true\n\
scan_do_custom = true\n\
scan_do_sleuth = true\n\
\n\
# News\n\
# Symbols for read/unread indicators in the News popup\n\
news_read_symbol = ✓\n\
news_unread_symbol = ∘\n\
\n\
# VirusTotal\n\
# API key used for VirusTotal scans (optional)\n\
virustotal_api_key = \n\
\n\
# Terminal\n\
# Preferred terminal emulator binary (optional): e.g., alacritty, kitty, gnome-terminal\n\
preferred_terminal = \n\
\n\
# Package selection marker\n\
# Visual marker for packages added to Install/Remove/Downgrade lists.\n\
# Allowed values: full_line | front | end\n\
# - full_line: color the entire line\n\
# - front: add marker at the front of the line (default)\n\
# - end: add marker at the end of the line\n\
package_marker = front\n";

/// Standalone keybinds skeleton used when initializing a separate keybinds.conf
pub(crate) const KEYBINDS_SKELETON_CONTENT: &str = "# Pacsea keybindings configuration\n\
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
keybind_search_normal_clear = Shift+Del\n\
\n\
# SEARCH — Normal Mode (Menus)\n\
# Toggle dropdown menus while in Normal Mode\n\
keybind_toggle_config = Shift+C\n\
keybind_toggle_options = Shift+O\n\
keybind_toggle_panels = Shift+P\n\
\n\
# SEARCH — Normal Mode (Other)\n\
# Open Arch status page in default browser\n\
keybind_search_normal_open_status = Shift+S\n\
# Import packages list into Install list\n\
keybind_search_normal_import = Shift+I\n\
# Export current Install list to a file\n\
keybind_search_normal_export = Shift+E\n\
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
keybind_install_focus_left = Left\n\
\n\
# NEWS — Actions\n\
keybind_news_mark_read = r\n\
keybind_news_mark_all_read = CTRL+R\n";

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

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    #[test]
    fn config_try_load_theme_success_and_errors() {
        use std::fs;
        use std::io::Write;
        use std::path::PathBuf;
        // Minimal valid theme with required canonical keys
        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_theme_cfg_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut p = dir.clone();
        p.push("theme.conf");
        let content = "base=#000000\nmantle=#000000\ncrust=#000000\nsurface1=#000000\nsurface2=#000000\noverlay1=#000000\noverlay2=#000000\ntext=#000000\nsubtext0=#000000\nsubtext1=#000000\nsapphire=#000000\nmauve=#000000\ngreen=#000000\nyellow=#000000\nred=#000000\nlavender=#000000\n";
        fs::write(&p, content).unwrap();
        let t = super::try_load_theme_with_diagnostics(&p).expect("valid theme");
        let _ = t.base; // use

        // Error case: unknown key + missing required
        let mut pe = dir.clone();
        pe.push("bad.conf");
        let mut f = fs::File::create(&pe).unwrap();
        writeln!(f, "unknown_key = #fff").unwrap();
        let err = super::try_load_theme_with_diagnostics(&pe).unwrap_err();
        assert!(err.contains("Unknown key"));
        assert!(err.contains("Missing required keys"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    /// Comprehensive test for settings config parameters:
    /// - Verifies all Settings fields are in skeleton config
    /// - Tests loading/saving from/to config
    /// - Tests defaults are applied when keys are missing
    /// - Tests missing config file is generated with skeleton
    fn config_settings_comprehensive_parameter_check() {
        use std::collections::HashSet;
        use std::fs;

        let _guard = crate::theme::test_mutex().lock().unwrap();
        let orig_home = std::env::var_os("HOME");
        let orig_xdg = std::env::var_os("XDG_CONFIG_HOME");

        // Create temporary test directory
        let base = std::env::temp_dir().join(format!(
            "pacsea_test_config_params_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cfg_dir = base.join(".config").join("pacsea");
        let _ = fs::create_dir_all(&cfg_dir);
        unsafe {
            std::env::set_var("HOME", base.display().to_string());
            std::env::remove_var("XDG_CONFIG_HOME");
        }

        // Test 1: Verify all Settings fields are present in skeleton config
        let skeleton_content = super::SETTINGS_SKELETON_CONTENT;
        let skeleton_keys: HashSet<String> = skeleton_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        // All expected Settings keys (excluding keymap which is in keybinds.conf)
        let expected_keys: HashSet<&str> = [
            "layout_left_pct",
            "layout_center_pct",
            "layout_right_pct",
            "app_dry_run_default",
            "sort_mode",
            "clipboard_suffix",
            "show_recent_pane",
            "show_install_pane",
            "show_keybinds_footer",
            "selected_countries",
            "mirror_count",
            "virustotal_api_key",
            "scan_do_clamav",
            "scan_do_trivy",
            "scan_do_semgrep",
            "scan_do_shellcheck",
            "scan_do_virustotal",
            "scan_do_custom",
            "scan_do_sleuth",
            "news_read_symbol",
            "news_unread_symbol",
            "preferred_terminal",
        ]
        .into_iter()
        .collect();

        for key in &expected_keys {
            assert!(
                skeleton_keys.contains(*key),
                "Missing key '{}' in skeleton config",
                key
            );
        }

        // Test 2: Missing config file is correctly generated with skeleton
        let settings_path = cfg_dir.join("settings.conf");
        assert!(
            !settings_path.exists(),
            "Settings file should not exist initially"
        );

        // Call ensure_settings_keys_present - should create file with skeleton
        let default_prefs = crate::theme::types::Settings::default();
        super::ensure_settings_keys_present(&default_prefs);

        assert!(settings_path.exists(), "Settings file should be created");
        let generated_content = fs::read_to_string(&settings_path).unwrap();
        assert!(
            !generated_content.is_empty(),
            "Generated config file should not be empty"
        );

        // Verify skeleton content matches generated file (ignoring whitespace differences)
        let generated_keys: HashSet<String> = generated_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        for key in &expected_keys {
            assert!(
                generated_keys.contains(*key),
                "Missing key '{}' in generated config file",
                key
            );
        }

        // Test 3: All parameters are loaded with defaults when missing
        // Delete the config file and test loading
        fs::remove_file(&settings_path).unwrap();
        let loaded_settings = crate::theme::settings::settings();
        let default_settings = crate::theme::types::Settings::default();

        // Verify all fields match defaults
        assert_eq!(
            loaded_settings.layout_left_pct, default_settings.layout_left_pct,
            "layout_left_pct should match default"
        );
        assert_eq!(
            loaded_settings.layout_center_pct, default_settings.layout_center_pct,
            "layout_center_pct should match default"
        );
        assert_eq!(
            loaded_settings.layout_right_pct, default_settings.layout_right_pct,
            "layout_right_pct should match default"
        );
        assert_eq!(
            loaded_settings.app_dry_run_default, default_settings.app_dry_run_default,
            "app_dry_run_default should match default"
        );
        assert_eq!(
            loaded_settings.sort_mode.as_config_key(),
            default_settings.sort_mode.as_config_key(),
            "sort_mode should match default"
        );
        assert_eq!(
            loaded_settings.clipboard_suffix, default_settings.clipboard_suffix,
            "clipboard_suffix should match default"
        );
        assert_eq!(
            loaded_settings.show_recent_pane, default_settings.show_recent_pane,
            "show_recent_pane should match default"
        );
        assert_eq!(
            loaded_settings.show_install_pane, default_settings.show_install_pane,
            "show_install_pane should match default"
        );
        assert_eq!(
            loaded_settings.show_keybinds_footer, default_settings.show_keybinds_footer,
            "show_keybinds_footer should match default"
        );
        assert_eq!(
            loaded_settings.selected_countries, default_settings.selected_countries,
            "selected_countries should match default"
        );
        assert_eq!(
            loaded_settings.mirror_count, default_settings.mirror_count,
            "mirror_count should match default"
        );
        assert_eq!(
            loaded_settings.virustotal_api_key, default_settings.virustotal_api_key,
            "virustotal_api_key should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_clamav, default_settings.scan_do_clamav,
            "scan_do_clamav should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_trivy, default_settings.scan_do_trivy,
            "scan_do_trivy should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_semgrep, default_settings.scan_do_semgrep,
            "scan_do_semgrep should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_shellcheck, default_settings.scan_do_shellcheck,
            "scan_do_shellcheck should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_virustotal, default_settings.scan_do_virustotal,
            "scan_do_virustotal should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_custom, default_settings.scan_do_custom,
            "scan_do_custom should match default"
        );
        assert_eq!(
            loaded_settings.scan_do_sleuth, default_settings.scan_do_sleuth,
            "scan_do_sleuth should match default"
        );
        assert_eq!(
            loaded_settings.news_read_symbol, default_settings.news_read_symbol,
            "news_read_symbol should match default"
        );
        assert_eq!(
            loaded_settings.news_unread_symbol, default_settings.news_unread_symbol,
            "news_unread_symbol should match default"
        );
        assert_eq!(
            loaded_settings.preferred_terminal, default_settings.preferred_terminal,
            "preferred_terminal should match default"
        );

        // Test 4: Missing keys are added to config with defaults
        // Create a minimal config file with only one key
        fs::write(
            &settings_path,
            "# Minimal config\nsort_mode = aur_popularity\n",
        )
        .unwrap();

        // Call ensure_settings_keys_present - should add missing keys
        let modified_prefs = crate::theme::types::Settings {
            sort_mode: crate::state::SortMode::AurPopularityThenOfficial,
            ..crate::theme::types::Settings::default()
        };
        super::ensure_settings_keys_present(&modified_prefs);

        // Verify file now contains all keys
        let updated_content = fs::read_to_string(&settings_path).unwrap();
        let updated_keys: HashSet<String> = updated_content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                    return None;
                }
                if let Some(eq_pos) = trimmed.find('=') {
                    let key = trimmed[..eq_pos]
                        .trim()
                        .to_lowercase()
                        .replace(['.', '-', ' '], "_");
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        for key in &expected_keys {
            assert!(
                updated_keys.contains(*key),
                "Missing key '{}' after ensure_settings_keys_present",
                key
            );
        }

        // Verify sort_mode was preserved
        assert!(
            updated_content.contains("sort_mode = aur_popularity")
                || updated_content.contains("sort_mode=aur_popularity"),
            "sort_mode should be preserved in config"
        );

        // Test 5: Parameters can be loaded from config file
        // Write a config file with custom values
        fs::write(
            &settings_path,
            "layout_left_pct = 25\n\
             layout_center_pct = 50\n\
             layout_right_pct = 25\n\
             app_dry_run_default = true\n\
             sort_mode = alphabetical\n\
             clipboard_suffix = Custom suffix\n\
             show_recent_pane = false\n\
             show_install_pane = false\n\
             show_keybinds_footer = false\n\
             selected_countries = Germany, France\n\
             mirror_count = 30\n\
             virustotal_api_key = test_api_key\n\
             scan_do_clamav = false\n\
             scan_do_trivy = false\n\
             scan_do_semgrep = false\n\
             scan_do_shellcheck = false\n\
             scan_do_virustotal = false\n\
             scan_do_custom = false\n\
             scan_do_sleuth = false\n\
             news_read_symbol = READ\n\
             news_unread_symbol = UNREAD\n\
             preferred_terminal = alacritty\n",
        )
        .unwrap();

        let loaded_custom = crate::theme::settings::settings();
        assert_eq!(loaded_custom.layout_left_pct, 25);
        assert_eq!(loaded_custom.layout_center_pct, 50);
        assert_eq!(loaded_custom.layout_right_pct, 25);
        assert!(loaded_custom.app_dry_run_default);
        assert_eq!(loaded_custom.sort_mode.as_config_key(), "alphabetical");
        assert_eq!(loaded_custom.clipboard_suffix, "Custom suffix");
        assert!(!loaded_custom.show_recent_pane);
        assert!(!loaded_custom.show_install_pane);
        assert!(!loaded_custom.show_keybinds_footer);
        assert_eq!(loaded_custom.selected_countries, "Germany, France");
        assert_eq!(loaded_custom.mirror_count, 30);
        assert_eq!(loaded_custom.virustotal_api_key, "test_api_key");
        assert!(!loaded_custom.scan_do_clamav);
        assert!(!loaded_custom.scan_do_trivy);
        assert!(!loaded_custom.scan_do_semgrep);
        assert!(!loaded_custom.scan_do_shellcheck);
        assert!(!loaded_custom.scan_do_virustotal);
        assert!(!loaded_custom.scan_do_custom);
        assert!(!loaded_custom.scan_do_sleuth);
        assert_eq!(loaded_custom.news_read_symbol, "READ");
        assert_eq!(loaded_custom.news_unread_symbol, "UNREAD");
        assert_eq!(loaded_custom.preferred_terminal, "alacritty");

        // Test 6: Save functions persist values correctly
        // Test save_sort_mode
        super::save_sort_mode(crate::state::SortMode::BestMatches);
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content.contains("sort_mode = best_matches")
                || saved_content.contains("sort_mode=best_matches"),
            "save_sort_mode should persist sort_mode"
        );

        // Test save_boolean_key via save_show_recent_pane
        super::save_show_recent_pane(true);
        let saved_content2 = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content2.contains("show_recent_pane = true")
                || saved_content2.contains("show_recent_pane=true"),
            "save_show_recent_pane should persist value"
        );

        // Test save_string_key via save_selected_countries
        super::save_selected_countries("Switzerland, Austria");
        let saved_content3 = fs::read_to_string(&settings_path).unwrap();
        assert!(
            saved_content3.contains("selected_countries = Switzerland, Austria")
                || saved_content3.contains("selected_countries=Switzerland, Austria"),
            "save_selected_countries should persist value"
        );

        // Verify saved values are loaded back
        let reloaded = crate::theme::settings::settings();
        assert_eq!(reloaded.sort_mode.as_config_key(), "best_matches");
        assert!(reloaded.show_recent_pane);
        assert_eq!(reloaded.selected_countries, "Switzerland, Austria");

        // Cleanup
        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
            if let Some(v) = orig_xdg {
                std::env::set_var("XDG_CONFIG_HOME", v);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
        let _ = fs::remove_dir_all(&base);
    }
}

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

/// Ensure core application settings keys exist in the settings file; append missing with current/default values.
///
/// This preserves existing lines and comments, only appending keys that are not present.
/// If the file is missing, it is created with the full skeleton content.
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
    let pairs: [(&str, String); 16] = [
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

/// If legacy `pacsea.conf` is present and the new split configs are missing,
/// generate `theme.conf` and `settings.conf` by taking over the values from `pacsea.conf`.
///
/// - Theme lines are any keys that are NOT recognized as preference/settings keys.
/// - Settings lines are recognized preference keys EXCLUDING any `keybind_*` keys.
/// - Existing non-empty `theme.conf`/`settings.conf` are left untouched.
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
