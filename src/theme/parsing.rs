use crossterm::event::KeyCode;
use ratatui::style::Color;

use super::types::KeyChord;
use crossterm::event::KeyModifiers;

pub(crate) fn parse_key_identifier(s: &str) -> Option<KeyCode> {
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

pub(crate) fn parse_key_chord(spec: &str) -> Option<KeyChord> {
    // Accept formats like: CTRL+R, Alt+?, Shift+Del, F1, Tab, BackTab, Super+F2
    let mut mods = KeyModifiers::empty();
    let mut key_part: Option<String> = None;
    for part in spec.split('+') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
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
        return Some(KeyChord {
            code: KeyCode::BackTab,
            mods: KeyModifiers::empty(),
        });
    }
    let code = parse_key_identifier(key_part.as_deref().unwrap_or(""))?;
    Some(KeyChord { code, mods })
}

/// Parse a color value from a configuration string.
///
/// Supported formats:
/// - "#RRGGBB" (hex)
/// - "R,G,B" (decimal triplet, 0-255)
pub(crate) fn parse_color_value(s: &str) -> Option<Color> {
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
    }) && r <= 255
        && g <= 255
        && b <= 255
    {
        return Some(Color::Rgb(r as u8, g as u8, b as u8));
    }
    None
}

pub(crate) fn canonical_for_key(norm: &str) -> Option<&'static str> {
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

pub(crate) fn canonical_to_preferred(canon: &str) -> String {
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

pub(crate) fn apply_override_to_map(
    map: &mut std::collections::HashMap<String, Color>,
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
            errors.push(format!("- Unknown key '{key}' on line {line_no}"));
        }
        return;
    };
    if value.is_empty() {
        errors.push(format!("- Missing value for '{key}' on line {line_no}"));
        return;
    }
    if let Some(c) = parse_color_value(value) {
        map.insert(canon.to_string(), c);
    } else {
        errors.push(format!(
            "- Invalid color for '{key}' on line {line_no} (use #RRGGBB or R,G,B)"
        ));
    }
}

pub(crate) fn nearest_key(input: &str) -> Option<&'static str> {
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

pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
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

pub(crate) fn strip_inline_comment(mut s: &str) -> &str {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsing_key_identifier_and_chord() {
        assert_eq!(parse_key_identifier("F5"), Some(KeyCode::F(5)));
        assert_eq!(parse_key_identifier("?"), Some(KeyCode::Char('?')));
        assert_eq!(parse_key_identifier("Backspace"), Some(KeyCode::Backspace));
        let kc = parse_key_chord("Ctrl+R").unwrap();
        assert_eq!(kc.code, KeyCode::Char('r'));
        assert!(kc.mods.contains(KeyModifiers::CONTROL));
        let bt = parse_key_chord("Shift+Tab").unwrap();
        assert_eq!(bt.code, KeyCode::BackTab);
        assert!(bt.mods.is_empty());
    }

    #[test]
    fn parsing_color_and_canon() {
        assert_eq!(parse_color_value("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color_value("255,0,10"), Some(Color::Rgb(255, 0, 10)));
        assert!(parse_color_value("").is_none());
        assert_eq!(canonical_for_key("background_base"), Some("base"));
        assert_eq!(canonical_to_preferred("overlay1"), "overlay_primary");
        assert!(nearest_key("bas").is_some());
    }

    #[test]
    fn parsing_strip_inline_comment_variants() {
        // Leading '#' preserved for hex; we only strip after first character to allow '#RRGGBB'
        assert_eq!(strip_inline_comment("#foo"), "#foo");
        assert_eq!(strip_inline_comment("abc // hi"), "abc");
        assert_eq!(strip_inline_comment("#ff00ff # tail"), "#ff00ff");
    }
}
