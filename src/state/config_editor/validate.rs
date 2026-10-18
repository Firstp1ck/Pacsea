//! Shared validation for popup edits and saves.
use crate::theme::{EDITABLE_KEYBINDS, EditableSetting, KeyChord, ValueKind, keybind_scope};
use crossterm::event::{KeyCode, KeyModifiers};
/// What: Validate a candidate value against its schema before writing.
///
/// Inputs:
/// - `setting`: Schema entry carrying the kind and semantic constraints.
/// - `value`: Canonical string the user wants to save.
///
/// Output:
/// - `Ok(())` when the value is acceptable; `Err(reason)` otherwise.
///
/// Details:
/// - Enum values must match a schema choice; other constraints are checked
///   here or by the settings parser as appropriate.
/// # Errors
/// Returns an actionable reason when syntax or schema constraints are violated.
pub fn validate_value(setting: &EditableSetting, value: &str) -> Result<(), String> {
    match setting.kind {
        ValueKind::IntRange { min, max } => {
            let parsed: i64 = value
                .trim()
                .parse()
                .map_err(|_| format!("expected integer in {min}..={max}"))?;
            if parsed < min || parsed > max {
                return Err(format!("must be within {min}..={max}"));
            }
            Ok(())
        }
        ValueKind::OptionalUnsignedOrAll => {
            let normalized = value.trim().to_ascii_lowercase();
            if matches!(normalized.as_str(), "all" | "none" | "unlimited" | "") {
                return Ok(());
            }
            normalized
                .parse::<u32>()
                .map(|_| ())
                .map_err(|_| "expected a non-negative integer or 'all'".to_string())
        }
        ValueKind::MainPaneOrder => crate::state::parse_main_pane_order(value)
            .map(|_| ())
            .ok_or_else(|| {
                "expected three distinct roles: results, search, package_info".to_string()
            }),
        ValueKind::Path => {
            if value.trim().is_empty() {
                Err("path must not be empty".to_string())
            } else {
                Ok(())
            }
        }
        ValueKind::Color => validate_color_value(value),
        ValueKind::KeyChord => validate_key_chord(value),
        ValueKind::String => validate_semantic_string_key(setting.key, value),
        ValueKind::Enum { choices } => {
            if choices.contains(&value) {
                Ok(())
            } else {
                Err("value must be one of the available choices".to_string())
            }
        }
        ValueKind::Secret | ValueKind::Bool => Ok(()),
    }
}

/// What: Apply stricter semantic checks for selected string-backed setting keys.
///
/// Inputs:
/// - `key`: Canonical setting key.
/// - `value`: Candidate value from popup save flow.
///
/// Output:
/// - `Ok(())` when the key/value combination passes semantic checks.
/// - `Err(reason)` when the value is syntactically valid text but semantically invalid.
///
/// Details:
/// - Used for Phase 1 hardening without introducing per-key validators for every setting.
/// - Unknown keys remain accepted to preserve existing behavior until dedicated validators are added.
fn validate_semantic_string_key(key: &str, value: &str) -> Result<(), String> {
    match key {
        "preferred_terminal" => validate_preferred_terminal_value(value),
        "selected_countries" => validate_selected_countries_value(value),
        "pi_scan_https_proxy" => {
            let proxy = value.trim();
            if proxy.is_empty() || (proxy.starts_with("https://") && !proxy.contains('@')) {
                Ok(())
            } else {
                Err("use a credential-free https:// proxy URL (remove user:password@), or leave empty to disable the proxy".to_string())
            }
        }
        "pi_scan_background_token_cap_24h" => validate_u64_decimal(value),
        "pi_scan_background_cost_cap_24h" => crate::theme::parse_pi_scan_cost_microusd(value)
            .map(|_| ())
            .map_err(|error| error.replace("pi_scan_background_cost_cap_24h ", "")),
        _ => Ok(()),
    }
}

/// Validate one unsigned 64-bit decimal setting without imposing a product-specific maximum.
fn validate_u64_decimal(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Err("expected a non-negative decimal integer".to_string());
    }
    trimmed
        .parse::<u64>()
        .map(|_| ())
        .map_err(|_| "value exceeds the maximum unsigned 64-bit integer".to_string())
}

/// What: Validate terminal command preference.
///
/// Inputs:
/// - `value`: Preferred terminal command or path, or empty for auto-detect.
///
/// Output:
/// - `Ok(())` when the value is empty or safe single-token command text.
pub(crate) fn validate_preferred_terminal_value(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    if trimmed.chars().any(char::is_control) {
        return Err("terminal command must not contain control characters".to_string());
    }
    if trimmed.chars().any(char::is_whitespace) {
        return Err("terminal command must be a single token (no spaces)".to_string());
    }
    Ok(())
}

/// What: Validate `selected_countries` CSV-like value.
///
/// Inputs:
/// - `value`: Country list candidate (e.g., `Worldwide` or `Germany, France`).
///
/// Output:
/// - `Ok(())` when all entries are non-empty and composed of common country-name characters.
pub(crate) fn validate_selected_countries_value(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("selected countries must not be empty".to_string());
    }
    if trimmed.eq_ignore_ascii_case("worldwide") {
        return Ok(());
    }
    for country in trimmed.split(',') {
        let token = country.trim();
        if token.is_empty() {
            return Err("country entries must not be empty".to_string());
        }
        if token.chars().any(char::is_control) {
            return Err("country entries must not contain control characters".to_string());
        }
        if !token
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '\'' | '.' | '_'))
        {
            return Err(format!("invalid country entry '{token}'"));
        }
    }
    Ok(())
}

/// What: Validate a color literal accepted by config/theme parsing.
///
/// Inputs:
/// - `value`: Candidate color string.
///
/// Output:
/// - `Ok(())` when the value is `#RRGGBB`, `RRGGBB`, or `R,G,B` (0..=255 each).
pub(crate) fn validate_color_value(value: &str) -> Result<(), String> {
    let t = value.trim();
    if t.is_empty() {
        return Err("color must not be empty".to_string());
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(());
    }
    let parts: Vec<&str> = t.split(',').collect();
    if parts.len() == 3 {
        let parsed: Option<Vec<u16>> = parts
            .iter()
            .map(|p| p.trim().parse::<u16>().ok())
            .collect::<Option<Vec<u16>>>();
        if let Some(rgb) = parsed
            && rgb.iter().all(|v| *v <= 255)
        {
            return Ok(());
        }
    }
    Err("expected #RRGGBB or R,G,B (0..255)".to_string())
}

/// What: Validate a key-chord string accepted by keybind parsing.
///
/// Inputs:
/// - `value`: Candidate key-chord specification (e.g. `Ctrl+R`, `Shift+Tab`).
///
/// Output:
/// - `Ok(())` when the value parses into a supported key token plus optional modifiers.
pub(crate) fn validate_key_chord(value: &str) -> Result<(), String> {
    parse_validated_key_chord(value).map(|_| ())
}

/// What: Parse a key-chord string into a [`KeyChord`] using the same rules as
/// `theme::parsing::parse_key_chord`, surfacing actionable errors.
///
/// Inputs:
/// - `value`: Candidate key-chord specification (e.g. `Ctrl+R`).
///
/// Output:
/// - `Ok(KeyChord)` when the value is well-formed.
/// - `Err(reason)` with a user-facing message otherwise.
///
/// Details:
/// - `Shift+Tab` is canonicalized to `KeyCode::BackTab` with empty modifiers,
///   matching the in-memory representation used by the runtime keymap and the
///   shipped parser.
pub(crate) fn parse_validated_key_chord(value: &str) -> Result<KeyChord, String> {
    let mut mods = KeyModifiers::empty();
    let mut key_part: Option<String> = None;
    for part in value.split('+') {
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
    let key = key_part.unwrap_or_default();
    if key == "TAB" && mods.contains(KeyModifiers::SHIFT) {
        return Ok(KeyChord {
            code: KeyCode::BackTab,
            mods: KeyModifiers::empty(),
        });
    }
    if key.is_empty() {
        return Err("missing key token".to_string());
    }
    let code = parse_key_identifier(&key)
        .ok_or_else(|| "invalid key chord; expected forms like Ctrl+R or Shift+Tab".to_string())?;
    Ok(KeyChord { code, mods })
}

/// What: Detect cross-action keybind conflicts before patching disk.
///
/// Inputs:
/// - `setting`: Schema row for the keybind being edited.
/// - `value`: Proposed canonical chord string about to be saved.
///
/// Output:
/// - `Ok(())` when no other action in the same scope already uses the chord.
/// - `Err(reason)` naming the conflicting action so the status line can
///   explain what to rebind first.
///
/// Details:
/// - Compares against the latest [`crate::theme::settings`] snapshot so users
///   see the same picture the running app uses for dispatch.
/// - Same-chord overlaps across scopes (e.g. `Left` shared by global pane
///   navigation and search-pane focus) are intentional and not flagged.
/// - Multi-bind actions like `keybind_recent_remove` are still checked; the
///   editor only writes the first chord, but existing extra chords on other
///   actions are honored when scanning for collisions.
pub(crate) fn validate_keybind_conflict(
    setting: &EditableSetting,
    value: &str,
) -> Result<(), String> {
    let proposed = parse_validated_key_chord(value)?;
    let scope = keybind_scope(setting.key);
    let snapshot = crate::theme::settings();
    for other in EDITABLE_KEYBINDS {
        if other.key == setting.key {
            continue;
        }
        if keybind_scope(other.key) != scope {
            continue;
        }
        let chords =
            crate::state::config_editor::keybind_chords_for_key(other.key, &snapshot.keymap);
        if chords
            .iter()
            .any(|c| c.code == proposed.code && c.mods == proposed.mods)
        {
            return Err(format!(
                "{} already uses that chord (scope: {scope}); rebind it first",
                other.key
            ));
        }
    }
    Ok(())
}

/// What: Parse one key identifier token used by key-chord validation.
///
/// Inputs:
/// - `token`: Key token (for example `F5`, `Esc`, `?`, `r`).
///
/// Output:
/// - `Some(KeyCode)` when recognized, otherwise `None`.
fn parse_key_identifier(token: &str) -> Option<KeyCode> {
    let t = token.trim();
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
            let mut chars = t.chars();
            if let (Some(ch), None) = (chars.next(), chars.next()) {
                Some(KeyCode::Char(ch.to_ascii_lowercase()))
            } else {
                None
            }
        }
    }
}
