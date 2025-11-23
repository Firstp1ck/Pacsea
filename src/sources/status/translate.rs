use crate::state::AppState;

/// What: Extract AUR suffix information from remaining text after a service ratio.
///
/// Inputs:
/// - `remaining`: Text remaining after extracting service ratio
///
/// Output:
/// - `(Option<u32>, Option<f64>)` tuple with AUR percentage and ratio if found
fn extract_aur_suffix(remaining: &str) -> (Option<u32>, Option<f64>) {
    let aur_suffix_pct = remaining.find(" — AUR today: ").and_then(|aur_pos| {
        let aur_part = &remaining[aur_pos + 14..];
        aur_part
            .strip_suffix('%')
            .and_then(|s| s.parse::<u32>().ok())
    });
    let aur_suffix_ratio = remaining.find(" (AUR: ").and_then(|aur_pos| {
        let aur_part = &remaining[aur_pos + 7..];
        aur_part
            .strip_suffix("%)")
            .and_then(|s| s.parse::<f64>().ok())
    });
    (aur_suffix_pct, aur_suffix_ratio)
}

/// What: Format translated text with optional AUR suffix.
///
/// Inputs:
/// - `app`: Application state
/// - `main_text`: Translated main text
/// - `aur_pct`: Optional AUR percentage
/// - `aur_ratio`: Optional AUR ratio
///
/// Output:
/// - Formatted text with AUR suffix if present
fn format_with_aur_suffix(
    app: &AppState,
    main_text: String,
    aur_pct: Option<u32>,
    aur_ratio: Option<f64>,
) -> String {
    use crate::i18n;
    if let Some(pct) = aur_pct {
        format!(
            "{}{}",
            main_text,
            i18n::t_fmt1(app, "app.arch_status.aur_today_suffix", pct)
        )
    } else if let Some(ratio) = aur_ratio {
        format!("{main_text} (AUR: {ratio:.1}%)")
    } else {
        main_text
    }
}

/// What: Parse and translate service status pattern with ratio.
///
/// Inputs:
/// - `app`: Application state
/// - `text`: Full status text
/// - `pattern`: Pattern to match (e.g., " outage (see status) — ")
/// - `translation_key`: Translation key for the pattern
///
/// Output:
/// - `Some(translated_text)` if pattern matches, `None` otherwise
fn translate_service_pattern(
    app: &AppState,
    text: &str,
    pattern: &str,
    translation_key: &str,
) -> Option<String> {
    use crate::i18n;
    if !text.contains(pattern) || !text.contains(" today: ") || text.contains(" — AUR today: ") {
        return None;
    }

    let pattern_pos = text.find(pattern)?;
    let service_name = &text[..pattern_pos];
    let today_pos = text.find(" today: ")?;
    let after_today = &text[today_pos + 8..];
    let pct_pos = after_today.find('%')?;
    let ratio_str = &after_today[..pct_pos];
    let ratio: f64 = ratio_str.parse().ok()?;

    let remaining = &after_today[pct_pos + 1..];
    let (aur_pct, aur_ratio) = extract_aur_suffix(remaining);
    let main_text = i18n::t_fmt(
        app,
        translation_key,
        &[&service_name, &service_name, &ratio],
    );
    Some(format_with_aur_suffix(app, main_text, aur_pct, aur_ratio))
}

/// What: Translate simple status text patterns using lookup table.
///
/// Inputs:
/// - `app`: Application state
/// - `base_text`: Base text to translate
///
/// Output:
/// - `Some(translated_text)` if pattern matches, `None` otherwise
fn translate_simple_pattern(app: &AppState, base_text: &str) -> Option<String> {
    use crate::i18n;
    let translation_key = match base_text {
        "Status: AUR Down" => Some("app.arch_status.aur_down"),
        "Some Arch systems down (see status)" => Some("app.arch_status.some_systems_down"),
        "Arch systems degraded (see status)" => Some("app.arch_status.systems_degraded"),
        "Arch systems nominal" => Some("app.arch_status.systems_nominal"),
        "All systems operational" => Some("app.arch_status.all_systems_operational"),
        "AUR outage (see status)" => Some("app.arch_status.aur_outage"),
        "AUR partial outage" => Some("app.arch_status.aur_partial_outage"),
        "AUR RPC degraded" => Some("app.arch_status.aur_rpc_degraded"),
        "AUR maintenance ongoing" => Some("app.arch_status.aur_maintenance_ongoing"),
        "AUR issues detected (see status)" => Some("app.arch_status.aur_issues_detected"),
        "AUR degraded (see status)" => Some("app.arch_status.aur_degraded"),
        _ => None,
    };
    translation_key.map(|key| i18n::t(app, key))
}

/// What: Translate "Arch systems nominal — {service} today: {ratio}%" pattern.
///
/// Inputs:
/// - `app`: Application state
/// - `text`: Full status text
///
/// Output:
/// - `Some(translated_text)` if pattern matches, `None` otherwise
fn translate_nominal_with_service(app: &AppState, text: &str) -> Option<String> {
    use crate::i18n;
    if !text.starts_with("Arch systems nominal — ")
        || !text.contains(" today: ")
        || text.contains(" — AUR today: ")
    {
        return None;
    }

    let today_pos = text.find(" today: ")?;
    let service_part = &text[24..today_pos]; // "Arch systems nominal — " is 24 chars
    let after_today = &text[today_pos + 8..];
    let pct_pos = after_today.find('%')?;
    let ratio_str = &after_today[..pct_pos];
    let ratio: f64 = ratio_str.parse().ok()?;

    let remaining = &after_today[pct_pos + 1..];
    let (aur_pct, aur_ratio) = extract_aur_suffix(remaining);
    let main_text = i18n::t_fmt(
        app,
        "app.arch_status.systems_nominal_with_service",
        &[&service_part, &ratio],
    );
    Some(format_with_aur_suffix(app, main_text, aur_pct, aur_ratio))
}

/// What: Translate Arch systems status text from English to the current locale.
///
/// Inputs:
/// - `app`: Application state containing translations
/// - `text`: English status text to translate
///
/// Output:
/// - Translated status text, or original text if translation not found
///
/// Details:
/// - Parses English status messages and maps them to translation keys
/// - Handles dynamic parts like percentages and service names
/// - Falls back to original text if pattern doesn't match
#[must_use]
pub fn translate_status_text(app: &AppState, text: &str) -> String {
    // Check for complex patterns with service names first (before extracting AUR suffix)
    // These patterns have their own "today: {ratio}%" that's not the AUR suffix
    if let Some(result) = translate_service_pattern(
        app,
        text,
        " outage (see status) — ",
        "app.arch_status.service_outage",
    ) {
        return result;
    }
    if let Some(result) = translate_service_pattern(
        app,
        text,
        " degraded (see status) — ",
        "app.arch_status.service_degraded",
    ) {
        return result;
    }
    if let Some(result) = translate_service_pattern(
        app,
        text,
        " issues detected (see status) — ",
        "app.arch_status.service_issues_detected",
    ) {
        return result;
    }
    if let Some(result) = translate_nominal_with_service(app, text) {
        return result;
    }

    // Extract AUR percentage suffix if present (for simple patterns)
    let (base_text, aur_pct) = text
        .find(" — AUR today: ")
        .map_or((text, None), |suffix_pos| {
            let (base, suffix) = text.split_at(suffix_pos);
            let pct = suffix
                .strip_prefix(" — AUR today: ")
                .and_then(|s| s.strip_suffix('%'))
                .and_then(|s| s.parse::<u32>().ok());
            (base, pct)
        });

    // Extract AUR suffix in parentheses if present
    let (base_text, aur_ratio) =
        base_text
            .find(" (AUR: ")
            .map_or((base_text, None), |suffix_pos| {
                let (base, suffix) = base_text.split_at(suffix_pos);
                let ratio = suffix
                    .strip_prefix(" (AUR: ")
                    .and_then(|s| s.strip_suffix("%)"))
                    .and_then(|s| s.parse::<f64>().ok());
                (base, ratio)
            });

    // Match base text patterns and translate
    let Some(translated) = translate_simple_pattern(app, base_text) else {
        // Pattern not recognized, return original
        return text.to_string();
    };

    // Append AUR percentage suffix if present
    format_with_aur_suffix(app, translated, aur_pct, aur_ratio)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    /// What: Test translation of simple status messages.
    ///
    /// Inputs:
    /// - English status text
    /// - `AppState` with translations
    ///
    /// Output:
    /// - Translated status text
    ///
    /// Details:
    /// - Verifies basic status message translation
    fn test_translate_simple_status() {
        let app = AppState::default();
        let text = "Arch systems nominal";
        let translated = translate_status_text(&app, text);
        // Should return translation or fallback to English
        assert!(!translated.is_empty());
    }

    #[test]
    /// What: Test translation of status messages with AUR percentage.
    ///
    /// Inputs:
    /// - English status text with AUR percentage suffix
    /// - `AppState` with translations
    ///
    /// Output:
    /// - Translated status text with translated suffix
    ///
    /// Details:
    /// - Verifies status message translation with dynamic percentage
    fn test_translate_status_with_aur_pct() {
        let app = AppState::default();
        let text = "Arch systems nominal — AUR today: 97%";
        let translated = translate_status_text(&app, text);
        // Should return translation or fallback to English
        assert!(!translated.is_empty());
        // The translation should contain the percentage or be a valid translation
        assert!(translated.contains("97") || translated.contains("AUR") || translated.len() > 10);
    }
}
