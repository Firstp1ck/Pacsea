//! Color palette definitions for Pacsea's TUI.
//!
//! This module exposes a small, opinionated theme used throughout the user
//! interface. Colors are grouped into neutrals (base/mantle/crust/surfaces),
//! overlays/subtexts, and accents for highlighting and semantic states.
use ratatui::style::Color;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::{env, fs};
use crossterm::event::{KeyCode, KeyModifiers};

/// Application theme palette used by rendering code.
///
/// All colors are provided as [`ratatui::style::Color`] and are suitable for
/// direct use with widgets and styles.
#[derive(Clone, Copy)]
pub struct Theme {
    /// Primary background color for the canvas.
    pub base: Color,
    /// Slightly lighter background layer used behind panels.
    pub mantle: Color,
    /// Darkest background shade for deep contrast areas.
    pub crust: Color,
    /// Subtle surface color for component backgrounds (level 1).
    pub surface1: Color,
    /// Subtle surface color for component backgrounds (level 2).
    pub surface2: Color,
    /// Muted overlay line/border color (primary).
    pub overlay1: Color,
    /// Muted overlay line/border color (secondary).
    pub overlay2: Color,
    /// Primary foreground text color.
    pub text: Color,
    /// Secondary text for less prominent content.
    pub subtext0: Color,
    /// Tertiary text for captions and low-emphasis content.
    pub subtext1: Color,
    /// Accent color commonly used for selection and interactive highlights.
    pub sapphire: Color,
    /// Accent color for emphasized headings or selections.
    pub mauve: Color,
    /// Success/positive state color.
    pub green: Color,
    /// Warning/attention state color.
    pub yellow: Color,
    /// Error/danger state color.
    pub red: Color,
    /// Accent color for subtle emphasis and borders.
    pub lavender: Color,
}

// Note: No hardcoded default color values are embedded here.

/// User-configurable application settings parsed from `pacsea.conf`.
#[derive(Clone, Debug)]
pub struct Settings {
    pub layout_left_pct: u16,
    pub layout_center_pct: u16,
    pub layout_right_pct: u16,
    pub app_dry_run_default: bool,
    /// Configurable key bindings parsed from `pacsea.conf`
    pub keymap: KeyMap,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout_left_pct: 20,
            layout_center_pct: 60,
            layout_right_pct: 20,
            app_dry_run_default: false,
            keymap: KeyMap::default(),
        }
    }
}

/// A single keyboard chord (modifiers + key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl KeyChord {
    /// Return a short display label such as "Ctrl+R", "F1", "Shift+Del", "+/ ?".
    pub fn label(&self) -> String {
        let mut parts: Vec<&'static str> = Vec::new();
        if self.mods.contains(KeyModifiers::CONTROL) {
            parts.push("Ctrl");
        }
        if self.mods.contains(KeyModifiers::ALT) {
            parts.push("Alt");
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            parts.push("Shift");
        }
        if self.mods.contains(KeyModifiers::SUPER) {
            parts.push("Super");
        }
        let key = match self.code {
            KeyCode::Char(ch) => {
                // Show uppercase character for display
                let up = ch.to_ascii_uppercase();
                if up == ' ' { "Space".to_string() } else { up.to_string() }
            }
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::BackTab => "Shift+Tab".to_string(),
            KeyCode::Delete => "Del".to_string(),
            KeyCode::Insert => "Ins".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::F(n) => format!("F{n}"),
            _ => "?".to_string(),
        };
        if parts.is_empty() || matches!(self.code, KeyCode::BackTab) {
            key
        } else {
            format!("{}+{}", parts.join("+"), key)
        }
    }
}

/// Application key bindings.
/// Each action can have multiple chords.
#[derive(Clone, Debug)]
pub struct KeyMap {
    // Global
    pub help_overlay: Vec<KeyChord>,
    pub reload_theme: Vec<KeyChord>,
    pub exit: Vec<KeyChord>,
    pub pane_next: Vec<KeyChord>,
    pub pane_prev: Vec<KeyChord>,
    pub pane_left: Vec<KeyChord>,
    pub pane_right: Vec<KeyChord>,

    // Search
    pub search_move_up: Vec<KeyChord>,
    pub search_move_down: Vec<KeyChord>,
    pub search_page_up: Vec<KeyChord>,
    pub search_page_down: Vec<KeyChord>,
    pub search_add: Vec<KeyChord>,
    pub search_install: Vec<KeyChord>,
    pub search_focus_left: Vec<KeyChord>,
    pub search_focus_right: Vec<KeyChord>,
    pub search_backspace: Vec<KeyChord>,

    // Recent
    pub recent_move_up: Vec<KeyChord>,
    pub recent_move_down: Vec<KeyChord>,
    pub recent_find: Vec<KeyChord>,
    pub recent_use: Vec<KeyChord>,
    pub recent_add: Vec<KeyChord>,
    pub recent_to_search: Vec<KeyChord>,
    pub recent_focus_right: Vec<KeyChord>,

    // Install
    pub install_move_up: Vec<KeyChord>,
    pub install_move_down: Vec<KeyChord>,
    pub install_confirm: Vec<KeyChord>,
    pub install_remove: Vec<KeyChord>,
    pub install_clear: Vec<KeyChord>,
    pub install_find: Vec<KeyChord>,
    pub install_to_search: Vec<KeyChord>,
    pub install_focus_left: Vec<KeyChord>,
}

impl Default for KeyMap {
    fn default() -> Self {
        use KeyCode::*;
        let none = KeyModifiers::empty();
        let ctrl = KeyModifiers::CONTROL;
        let shift = KeyModifiers::SHIFT;
        KeyMap {
            help_overlay: vec![KeyChord { code: F(1), mods: none }, KeyChord { code: Char('?'), mods: none }],
            reload_theme: vec![KeyChord { code: Char('r'), mods: ctrl }],
            exit: vec![KeyChord { code: Char('c'), mods: ctrl }],
            pane_next: vec![KeyChord { code: Tab, mods: none }],
            pane_prev: vec![KeyChord { code: BackTab, mods: none }],
            pane_left: vec![KeyChord { code: Left, mods: none }],
            pane_right: vec![KeyChord { code: Right, mods: none }],

            search_move_up: vec![KeyChord { code: Up, mods: none }],
            search_move_down: vec![KeyChord { code: Down, mods: none }],
            search_page_up: vec![KeyChord { code: PageUp, mods: none }],
            search_page_down: vec![KeyChord { code: PageDown, mods: none }],
            search_add: vec![KeyChord { code: Char(' '), mods: none }],
            search_install: vec![KeyChord { code: Enter, mods: none }],
            search_focus_left: vec![KeyChord { code: Left, mods: none }],
            search_focus_right: vec![KeyChord { code: Right, mods: none }],
            search_backspace: vec![KeyChord { code: Backspace, mods: none }],

            recent_move_up: vec![KeyChord { code: Char('k'), mods: none }, KeyChord { code: Up, mods: none }],
            recent_move_down: vec![KeyChord { code: Char('j'), mods: none }, KeyChord { code: Down, mods: none }],
            recent_find: vec![KeyChord { code: Char('/'), mods: none }],
            recent_use: vec![KeyChord { code: Enter, mods: none }],
            recent_add: vec![KeyChord { code: Char(' '), mods: none }],
            recent_to_search: vec![KeyChord { code: Esc, mods: none }],
            recent_focus_right: vec![KeyChord { code: Right, mods: none }],

            install_move_up: vec![KeyChord { code: Char('k'), mods: none }, KeyChord { code: Up, mods: none }],
            install_move_down: vec![KeyChord { code: Char('j'), mods: none }, KeyChord { code: Down, mods: none }],
            install_confirm: vec![KeyChord { code: Enter, mods: none }],
            install_remove: vec![KeyChord { code: Delete, mods: none }],
            install_clear: vec![KeyChord { code: Delete, mods: shift }],
            install_find: vec![KeyChord { code: Char('/'), mods: none }],
            install_to_search: vec![KeyChord { code: Esc, mods: none }],
            install_focus_left: vec![KeyChord { code: Left, mods: none }],
        }
    }
}

fn parse_key_identifier(s: &str) -> Option<KeyCode> {
    let t = s.trim();
    // Function keys
    if let Some(num) = t.strip_prefix('F').and_then(|x| x.parse::<u8>().ok()) {
        return Some(KeyCode::F(num));
    }
    match t.to_ascii_uppercase().as_str() {
        "ESC" => Some(KeyCode::Esc),
        "ENTER" | "RETURN" => Some(KeyCode::Enter),
        "TAB" => Some(KeyCode::Tab),
        "BACKTAB" | "SHIFT+TAB" => Some(KeyCode::BackTab),
        "BACKSPACE" => Some(KeyCode::Backspace),
        "DELETE" | "DEL" => Some(KeyCode::Delete),
        "INSERT" | "INS" => Some(KeyCode::Insert),
        "HOME" => Some(KeyCode::Home),
        "END" => Some(KeyCode::End),
        "PAGEUP" | "PGUP" => Some(KeyCode::PageUp),
        "PAGEDOWN" | "PGDN" => Some(KeyCode::PageDown),
        "UP" | "ARROWUP" => Some(KeyCode::Up),
        "DOWN" | "ARROWDOWN" => Some(KeyCode::Down),
        "LEFT" | "ARROWLEFT" => Some(KeyCode::Left),
        "RIGHT" | "ARROWRIGHT" => Some(KeyCode::Right),
        "SPACE" => Some(KeyCode::Char(' ')),
        _ => {
            // Single visible character, e.g. "?" or "r"; normalize to lowercase
            let mut chars = t.chars();
            if let (Some(ch), None) = (chars.next(), chars.next()) {
                Some(KeyCode::Char(ch.to_ascii_lowercase()))
            } else {
                None
            }
        }
    }
}

fn parse_key_chord(spec: &str) -> Option<KeyChord> {
    // Accept formats like: CTRL+R, Alt+?, Shift+Del, F1, Tab, BackTab, Super+F2
    let mut mods = KeyModifiers::empty();
    let mut key_part: Option<String> = None;
    for part in spec.split('+') {
        let p = part.trim();
        if p.is_empty() { continue; }
        match p.to_ascii_uppercase().as_str() {
            "CTRL" | "CONTROL" => mods |= KeyModifiers::CONTROL,
            "ALT" => mods |= KeyModifiers::ALT,
            "SHIFT" => mods |= KeyModifiers::SHIFT,
            "SUPER" | "META" | "WIN" => mods |= KeyModifiers::SUPER,
            other => {
                key_part = Some(other.to_string());
            }
        }
    }
    // Special-case Shift+Tab -> BackTab (mods cleared)
    if key_part.as_deref() == Some("TAB") && mods.contains(KeyModifiers::SHIFT) {
        return Some(KeyChord { code: KeyCode::BackTab, mods: KeyModifiers::empty() });
    }
    let code = parse_key_identifier(key_part.as_deref().unwrap_or(""))?;
    Some(KeyChord { code, mods })
}

/// Skeleton configuration file content with default color values.
const SKELETON_CONFIG_CONTENT: &str = "# Pacsea theme configuration\n\
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
\
# ---------- Alternative Theme (Light) ----------\n\
# To use this light theme, comment out the dark values above and uncomment the\n\
# lines below, or copy these into your own overrides.\n\
\
# # Background layers (from lightest to darkest)\n\
# background_base = #f5f5f7\n\
# background_mantle = #eaeaee\n\
# background_crust = #dcdce1\n\
\
# # Component surfaces\n\
# surface_level1 = #cfd1d7\n\
# surface_level2 = #b7bac3\n\
\
# # Low-contrast lines/borders and secondary text accents\n\
# overlay_primary = #7a7d86\n\
# overlay_secondary = #63666f\n\
\
# # Text hierarchy\n\
# text_primary = #1c1c22\n\
# text_secondary = #3c3f47\n\
# text_tertiary = #565a64\n\
\
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
\n\
# GLOBAL — Pane switching\n\
keybind_pane_left = Left\n\
keybind_pane_right = Right\n\
keybind_pane_next = Tab\n\
keybind_pane_prev = Shift+Tab\n\
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
# RECENT — Navigation\n\
keybind_recent_move_up = k\n\
keybind_recent_move_down = j\n\
\n\
# RECENT — Actions\n\
keybind_recent_use = Enter\n\
keybind_recent_add = Space\n\
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
keybind_install_clear = Shift+Del\n\
\n\
# INSTALL — Find/Focus\n\
keybind_install_find = /\n\
keybind_install_to_search = Esc\n\
keybind_install_focus_left = Left\n";

/// Parse a color value from a configuration string.
///
/// Supported formats:
/// - "#RRGGBB" (hex)
/// - "R,G,B" (decimal triplet, 0-255)
fn parse_color_value(s: &str) -> Option<Color> {
    // Trim and strip inline comments (support trailing "// ..." and "# ...").
    // Preserve a leading '#' for hex values by searching for '#' only after the first char.
    let mut t = s.trim();
    if let Some(i) = t.find("//") {
        t = &t[..i];
    }
    if let Some(i_rel) = if let Some(stripped) = t.strip_prefix('#') {
        stripped.find('#').map(|j| j + 1)
    } else {
        t.find('#')
    } {
        t = &t[..i_rel];
    }
    t = t.trim();
    if t.is_empty() {
        return None;
    }
    // Hex formats: #RRGGBB or RRGGBB
    let h = t.strip_prefix('#').unwrap_or(t);
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        return Some(Color::Rgb(r, g, b));
    }
    // Decimal triplet: R,G,B
    if let Some((r, g, b)) = t.split(',').collect::<Vec<_>>().get(0..3).and_then(|v| {
        let r = v[0].trim().parse::<u16>().ok()?;
        let g = v[1].trim().parse::<u16>().ok()?;
        let b = v[2].trim().parse::<u16>().ok()?;
        Some((r, g, b))
    })
        && r <= 255 && g <= 255 && b <= 255 {
            return Some(Color::Rgb(r as u8, g as u8, b as u8));
        }
    None
}

/// Apply a single key/value override to the provided theme.
fn canonical_for_key(norm: &str) -> Option<&'static str> {
    match norm {
        // Legacy and comprehensive keys mapped to canonical names
        "base" | "background" | "background_base" => Some("base"),
        "mantle" | "background_mantle" => Some("mantle"),
        "crust" | "background_crust" => Some("crust"),
        "surface1" | "surface_1" | "surface_level1" => Some("surface1"),
        "surface2" | "surface_2" | "surface_level2" => Some("surface2"),
        "overlay1" | "overlay_primary" | "border_primary" => Some("overlay1"),
        "overlay2" | "overlay_secondary" | "border_secondary" => Some("overlay2"),
        "text" | "text_primary" => Some("text"),
        "subtext0" | "text_secondary" => Some("subtext0"),
        "subtext1" | "text_tertiary" => Some("subtext1"),
        "sapphire" | "accent_interactive" | "accent_info" => Some("sapphire"),
        "mauve" | "accent_heading" | "accent_primary" => Some("mauve"),
        "green" | "semantic_success" => Some("green"),
        "yellow" | "semantic_warning" => Some("yellow"),
        "red" | "semantic_error" => Some("red"),
        "lavender" | "accent_emphasis" | "accent_border" => Some("lavender"),
        _ => None,
    }
}

fn canonical_to_preferred(canon: &str) -> String {
    match canon {
        "base" => "background_base",
        "mantle" => "background_mantle",
        "crust" => "background_crust",
        "surface1" => "surface_level1",
        "surface2" => "surface_level2",
        "overlay1" => "overlay_primary",
        "overlay2" => "overlay_secondary",
        "text" => "text_primary",
        "subtext0" => "text_secondary",
        "subtext1" => "text_tertiary",
        "sapphire" => "accent_interactive",
        "mauve" => "accent_heading",
        "green" => "semantic_success",
        "yellow" => "semantic_warning",
        "red" => "semantic_error",
        "lavender" => "accent_emphasis",
        _ => canon,
    }
    .to_string()
}

fn apply_override_to_map(
    map: &mut HashMap<String, Color>,
    key: &str,
    value: &str,
    errors: &mut Vec<String>,
    line_no: usize,
) {
    let norm = key.trim().to_lowercase().replace(['.', '-', ' '], "_");
    let Some(canon) = canonical_for_key(&norm) else {
        let suggestion = nearest_key(&norm);
        if let Some(s) = suggestion {
            errors.push(format!(
                "- Unknown key '{}' on line {} (did you mean '{}'?)",
                key,
                line_no,
                canonical_to_preferred(s)
            ));
        } else {
            errors.push(format!("- Unknown key '{}' on line {}", key, line_no));
        }
        return;
    };
    if value.is_empty() {
        errors.push(format!("- Missing value for '{}' on line {}", key, line_no));
        return;
    }
    if let Some(c) = parse_color_value(value) {
        map.insert(canon.to_string(), c);
    } else {
        errors.push(format!(
            "- Invalid color for '{}' on line {} (use #RRGGBB or R,G,B)",
            key, line_no
        ));
    }
}

fn nearest_key(input: &str) -> Option<&'static str> {
    // Very small domain; simple Levenshtein distance is fine
    const CANON: [&str; 16] = [
        "base", "mantle", "crust", "surface1", "surface2", "overlay1", "overlay2", "text",
        "subtext0", "subtext1", "sapphire", "mauve", "green", "yellow", "red", "lavender",
    ];
    let mut best: Option<(&'static str, usize)> = None;
    for &k in &CANON {
        let d = levenshtein(input, k);
        if best.map(|(_, bd)| d < bd).unwrap_or(true) {
            best = Some((k, d));
        }
    }
    best.and_then(|(k, d)| if d <= 3 { Some(k) } else { None })
}

fn levenshtein(a: &str, b: &str) -> usize {
    let m = b.len();
    let mut dp: Vec<usize> = (0..=m).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = dp[0];
        dp[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let tmp = dp[j + 1];
            let cost = if ca == cb { 0 } else { 1 };
            dp[j + 1] = std::cmp::min(std::cmp::min(dp[j + 1] + 1, dp[j] + 1), prev + cost);
            prev = tmp;
        }
    }
    dp[m]
}

/// Attempt to parse a theme from a configuration file with simple `key = value` pairs.
fn try_load_theme_with_diagnostics(path: &Path) -> Result<Theme, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("{}", e))?;
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
            errors.push(format!("- Missing '=' on line {}", line_no));
            continue;
        }
        let mut parts = trimmed.splitn(2, '=');
        let raw_key = parts.next().unwrap_or("");
        let key = raw_key.trim();
        let val = parts.next().unwrap_or("").trim();
        if key.is_empty() {
            errors.push(format!("- Missing key before '=' on line {}", line_no));
            continue;
        }
        let norm = key.to_lowercase().replace(['.', '-', ' '], "_");
        // Allow non-theme preference keys to live in pacsea.conf without erroring
        let is_pref_key = norm.starts_with("pref_")
            || norm.starts_with("settings_")
            || norm.starts_with("layout_")
            || norm.starts_with("keybind_")
            || norm.starts_with("app_");
        if is_pref_key {
            // Skip theme handling; parsed elsewhere
            continue;
        }
        // Track duplicates (by canonical form if known, otherwise normalized input)
        let canon_or_norm = canonical_for_key(&norm).unwrap_or(norm.as_str());
        if !seen_keys.insert(canon_or_norm.to_string()) {
            errors.push(format!("- Duplicate key '{}' on line {}", key, line_no));
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

fn load_theme_from_file(path: &Path) -> Option<Theme> {
    try_load_theme_with_diagnostics(path).ok()
}

/// Determine the configuration file path for Pacsea's theme, searching in priority order:
/// 1) "$HOME/pacsea.conf"
/// 2) "$HOME/.config/pacsea.conf"
/// 3) "$HOME/.config/pacsea/pacsea.conf"
/// 4) "config/pacsea.conf" (repository-local testing)
fn resolve_config_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok();
    let xdg_config = env::var("XDG_CONFIG_HOME").ok();
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(h) = home.as_deref() {
        candidates.push(
            Path::new(h)
                .join(".config")
                .join("pacsea")
                .join("pacsea.conf"),
        );
    }
    if let Some(xdg) = xdg_config.as_deref() {
        let x = Path::new(xdg);
        candidates.push(x.join("pacsea").join("pacsea.conf"));
        candidates.push(x.join("pacsea.conf"));
    }
    if let Ok(cwd) = env::current_dir() {
        candidates.push(cwd.join("config").join("pacsea.conf"));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Global theme store with live-reload capability.
static THEME_STORE: OnceLock<RwLock<Theme>> = OnceLock::new();

fn load_initial_theme_or_exit() -> Theme {
    if let Some(path) = resolve_config_path() {
        match try_load_theme_with_diagnostics(&path) {
            Ok(t) => return t,
            Err(msg) => {
                eprintln!(
                    "Pacsea: theme configuration errors in {}:\n{}",
                    path.display(),
                    msg
                );
            }
        }
        std::process::exit(1);
    } else {
        // No config found: write default skeleton to $XDG_CONFIG_HOME/pacsea/pacsea.conf
        let xdg_base = env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")));
        if let Some(base) = xdg_base {
            let target = base.join("pacsea").join("pacsea.conf");
            if !target.exists() {
                if let Some(dir) = target.parent() {
                    let _ = fs::create_dir_all(dir);
                }
                // Prefer repository config content if available; otherwise, use the skeleton template
                let content = env::current_dir()
                    .ok()
                    .map(|cwd| cwd.join("config").join("pacsea.conf"))
                    .and_then(|p| fs::read_to_string(p).ok())
                    .unwrap_or_else(|| SKELETON_CONFIG_CONTENT.to_string());
                let _ = fs::write(&target, content);
            }
            if let Some(t) = load_theme_from_file(&target) {
                return t;
            }
        }
        eprintln!(
            "Pacsea: theme configuration missing or incomplete. Please edit $XDG_CONFIG_HOME/pacsea/pacsea.conf (or ~/.config/pacsea/pacsea.conf)."
        );
        std::process::exit(1);
    }
}

// /// Return the directory that contains `pacsea.conf`, creating it if needed.
// /// This is also where other app files (caches, lists) should live.
// removed unused config_dir()

fn xdg_base_dir(var: &str, home_default: &[&str]) -> PathBuf {
    if let Ok(p) = env::var(var)
        && !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let mut base = PathBuf::from(home);
    for seg in home_default {
        base = base.join(seg);
    }
    base
}

/// XDG cache directory for Pacsea (ensured to exist)
pub fn cache_dir() -> PathBuf {
    let base = xdg_base_dir("XDG_CACHE_HOME", &[".cache"]);
    let dir = base.join("pacsea");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// XDG state directory for Pacsea (ensured to exist)
pub fn state_dir() -> PathBuf {
    let base = xdg_base_dir("XDG_STATE_HOME", &[".local", "state"]);
    let dir = base.join("pacsea");
    let _ = fs::create_dir_all(&dir);
    dir
}

// /// XDG data directory for Pacsea (ensured to exist)
// removed unused data_dir()

fn strip_inline_comment(mut s: &str) -> &str {
    if let Some(i) = s.find("//") {
        s = &s[..i];
    }
    if let Some(i_rel) = if let Some(stripped) = s.strip_prefix('#') {
        stripped.find('#').map(|j| j + 1)
    } else {
        s.find('#')
    } {
        s = &s[..i_rel];
    }
    s.trim()
}

/// Load user settings from the same config file as the theme.
/// Falls back to `Settings::default()` when missing or invalid.
pub fn settings() -> Settings {
    let mut out = Settings::default();
    let path = resolve_config_path().or_else(|| {
        env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| env::var("HOME").ok().map(|h| Path::new(&h).join(".config")))
            .map(|base| base.join("pacsea").join("pacsea.conf"))
    });
    let Some(p) = path else {
        return out;
    };
    let Ok(content) = fs::read_to_string(&p) else {
        return out;
    };
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
        let key = raw_key.trim().to_lowercase().replace(['.', '-', ' '], "_");
        let val_raw = parts.next().unwrap_or("").trim();
        let val = strip_inline_comment(val_raw);
        match key.as_str() {
            "layout_left_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    out.layout_left_pct = v;
                }
            }
            "layout_center_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    out.layout_center_pct = v;
                }
            }
            "layout_right_pct" => {
                if let Ok(v) = val.parse::<u16>() {
                    out.layout_right_pct = v;
                }
            }
            "app_dry_run_default" => {
                let lv = val.to_ascii_lowercase();
                out.app_dry_run_default = lv == "true" || lv == "1" || lv == "yes" || lv == "on";
            }
            // Keybindings (single chord per action); overrides full list
            "keybind_help" | "keybind_help_overlay" => { if let Some(ch) = parse_key_chord(val) { out.keymap.help_overlay = vec![ch]; } }
            "keybind_reload_theme" | "keybind_reload" => { if let Some(ch) = parse_key_chord(val) { out.keymap.reload_theme = vec![ch]; } }
            "keybind_exit" | "keybind_quit" => { if let Some(ch) = parse_key_chord(val) { out.keymap.exit = vec![ch]; } }
            "keybind_pane_next" | "keybind_next_pane" | "keybind_switch_pane" => { if let Some(ch) = parse_key_chord(val) { out.keymap.pane_next = vec![ch]; } }
            "keybind_pane_prev" | "keybind_prev_pane" => { if let Some(ch) = parse_key_chord(val) { out.keymap.pane_prev = vec![ch]; } }
            "keybind_pane_left" => { if let Some(ch) = parse_key_chord(val) { out.keymap.pane_left = vec![ch]; } }
            "keybind_pane_right" => { if let Some(ch) = parse_key_chord(val) { out.keymap.pane_right = vec![ch]; } }

            // Search pane
            "keybind_search_move_up" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_move_up = vec![ch]; } }
            "keybind_search_move_down" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_move_down = vec![ch]; } }
            "keybind_search_page_up" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_page_up = vec![ch]; } }
            "keybind_search_page_down" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_page_down = vec![ch]; } }
            "keybind_search_add" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_add = vec![ch]; } }
            "keybind_search_install" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_install = vec![ch]; } }
            "keybind_search_focus_left" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_focus_left = vec![ch]; } }
            "keybind_search_focus_right" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_focus_right = vec![ch]; } }
            "keybind_search_backspace" => { if let Some(ch) = parse_key_chord(val) { out.keymap.search_backspace = vec![ch]; } }

            // Recent pane
            "keybind_recent_move_up" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_move_up = vec![ch]; } }
            "keybind_recent_move_down" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_move_down = vec![ch]; } }
            "keybind_recent_find" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_find = vec![ch]; } }
            "keybind_recent_use" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_use = vec![ch]; } }
            "keybind_recent_add" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_add = vec![ch]; } }
            "keybind_recent_to_search" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_to_search = vec![ch]; } }
            "keybind_recent_focus_right" => { if let Some(ch) = parse_key_chord(val) { out.keymap.recent_focus_right = vec![ch]; } }

            // Install pane
            "keybind_install_move_up" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_move_up = vec![ch]; } }
            "keybind_install_move_down" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_move_down = vec![ch]; } }
            "keybind_install_confirm" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_confirm = vec![ch]; } }
            "keybind_install_remove" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_remove = vec![ch]; } }
            "keybind_install_clear" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_clear = vec![ch]; } }
            "keybind_install_find" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_find = vec![ch]; } }
            "keybind_install_to_search" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_to_search = vec![ch]; } }
            "keybind_install_focus_left" => { if let Some(ch) = parse_key_chord(val) { out.keymap.install_focus_left = vec![ch]; } }
            _ => {}
        }
    }
    // Validate sum; if invalid, revert to defaults
    let sum = out
        .layout_left_pct
        .saturating_add(out.layout_center_pct)
        .saturating_add(out.layout_right_pct);
    if sum != 100
        || out.layout_left_pct == 0
        || out.layout_center_pct == 0
        || out.layout_right_pct == 0
    {
        out = Settings::default();
    }
    out
}

/// Return the application's theme palette, loading from config if available.
///
/// The config file is searched in the following locations (first match wins):
/// - "$HOME/pacsea.conf"
/// - "$HOME/.config/pacsea.conf"
/// - "$HOME/.config/pacsea/pacsea.conf"
/// - "config/pacsea.conf" (useful for repository-local testing)
///
/// Format: key = value, one per line; values are colors as "#RRGGBB" or "R,G,B".
pub fn theme() -> Theme {
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    *lock.read().expect("theme store poisoned")
}

/// Reload the theme from disk without restarting the app.
/// Returns Ok(()) on success; Err(msg) if the config is missing or incomplete.
pub fn reload_theme() -> std::result::Result<(), String> {
    let path = resolve_config_path().or_else(|| {
        env::var("HOME").ok().map(|h| {
            Path::new(&h)
                .join(".config")
                .join("pacsea")
                .join("pacsea.conf")
        })
    });
    let Some(p) = path else {
        return Err("No theme configuration file found".to_string());
    };
    let new_theme = try_load_theme_with_diagnostics(&p)?;
    let lock = THEME_STORE.get_or_init(|| RwLock::new(load_initial_theme_or_exit()));
    if let Ok(mut guard) = lock.write() {
        *guard = new_theme;
        Ok(())
    } else {
        Err("Failed to acquire theme store for writing".to_string())
    }
}
