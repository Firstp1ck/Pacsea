//! Deterministic suggestions with all machine-specific facts injected at popup open.
use crate::theme::{EditableSetting, Sensitivity, ValueKind};

/// Why a dropdown value is useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionTag {
    /// Effective value at opening.
    Current,
    /// Codebase default.
    Default,
    /// Empty value enables automatic discovery.
    AutoDetect,
    /// Zero disables the cap.
    Unlimited,
    /// Executable found on PATH.
    Installed,
    /// Value observed in the environment.
    Detected,
    /// Schema lower bound.
    Minimum,
    /// Schema upper bound.
    Maximum,
}

/// One deduplicated candidate for a configuration value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Suggestion {
    /// Canonical token or complete value.
    pub value: String,
    /// All applicable provenance labels.
    pub tags: Vec<SuggestionTag>,
    /// Seconds represented by a duration preset.
    pub duration_secs: Option<u64>,
    /// RGB color preview.
    pub swatch: Option<(u8, u8, u8)>,
    /// Optional translated choice description.
    pub label_key: Option<String>,
}

/// Facts captured once when opening a popup; tests can supply every field.
#[derive(Debug, Clone, Default)]
pub struct SuggestionContext {
    /// Opening value.
    pub current: String,
    /// Codebase default.
    pub default: String,
    /// Executable terminal names found on PATH.
    pub installed_terminals: Vec<String>,
    /// Verified provider/model pairs from the Pi setup wizard.
    pub pi_routes: Vec<(String, String)>,
    /// HTTPS proxy from the environment.
    pub env_proxy: Option<String>,
    /// Other effective theme keys and their colors.
    pub palette: Vec<(String, String)>,
    /// Resolved Pi executable path.
    pub pi_binary: Option<String>,
}

/// Binary/package pairs reused by suggestions and the optional-dependencies menu.
pub(crate) const TERMINAL_CANDIDATES: &[(&str, &str)] = &[
    ("alacritty", "alacritty"),
    ("ghostty", "ghostty"),
    ("kitty", "kitty"),
    ("xterm", "xterm"),
    ("gnome-terminal", "gnome-terminal"),
    ("konsole", "konsole"),
    ("xfce4-terminal", "xfce4-terminal"),
    ("tilix", "tilix"),
    ("mate-terminal", "mate-terminal"),
];
/// Country presets matching the system-update picker.
const COUNTRIES: &[&str] = &[
    "Worldwide",
    "Germany",
    "United States",
    "United Kingdom",
    "France",
    "Netherlands",
    "Sweden",
    "Canada",
    "Australia",
    "Japan",
];

impl SuggestionContext {
    /// What: Capture suggestions' external facts once, without running processes.
    ///
    /// Inputs: Application state, schema entry, and shared value snapshot.
    ///
    /// Output: Owned context suitable for deterministic suggestion generation.
    ///
    /// Details: PATH resolution uses the existing filesystem/executable-bit helper.
    #[must_use]
    pub fn capture(
        app: &crate::state::AppState,
        entry: &EditableSetting,
        values: &super::ValueSnapshot,
    ) -> Self {
        Self {
            current: values.value_for(entry),
            default: values.default_for(entry),
            installed_terminals: TERMINAL_CANDIDATES
                .iter()
                .filter(|(name, _)| crate::install::command_on_path(name))
                .map(|(name, _)| (*name).to_string())
                .collect(),
            pi_routes: app
                .pi_scan
                .wizard
                .as_ref()
                .and_then(|wizard| wizard.verified.as_ref())
                .map(|facts| facts.routes.clone())
                .unwrap_or_default(),
            env_proxy: std::env::var("HTTPS_PROXY")
                .ok()
                .filter(|value| value.starts_with("https://") && !value.contains('@'))
                .or_else(|| {
                    std::env::var("https_proxy")
                        .ok()
                        .filter(|value| value.starts_with("https://") && !value.contains('@'))
                }),
            palette: crate::theme::EDITABLE_THEME
                .iter()
                .filter(|other| other.key != entry.key)
                .map(|other| (other.key.to_string(), values.value_for(other)))
                .collect(),
            pi_binary: crate::install::resolve_command_on_path("pi")
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }
}

/// What: Build stable, deduplicated candidates for one schema entry.
///
/// Inputs: Entry and injected context.
///
/// Output: Suggestions with merged tags, optional durations, swatches, and descriptions.
///
/// Details: Secrets produce no candidates; comma-list candidates represent only the last token.
#[must_use]
pub fn suggestions_for(entry: &EditableSetting, ctx: &SuggestionContext) -> Vec<Suggestion> {
    if entry.sensitivity == Sensitivity::Sensitive || matches!(entry.kind, ValueKind::Secret) {
        return Vec::new();
    }
    let mut out = Vec::new();
    match entry.kind {
        ValueKind::Bool => add_values(&mut out, &["false", "true"]),
        ValueKind::Enum { choices } => add_values(&mut out, choices),
        ValueKind::MainPaneOrder => {
            for value in main_pane_choices() {
                add(&mut out, &value, None);
            }
        }
        _ => {
            if comma_list(entry.key) {
                for (source, tag) in [
                    (&ctx.current, SuggestionTag::Current),
                    (&ctx.default, SuggestionTag::Default),
                ] {
                    for value in source
                        .split(',')
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        add(&mut out, value, Some(tag));
                    }
                }
            } else {
                add(&mut out, &ctx.current, Some(SuggestionTag::Current));
                add(&mut out, &ctx.default, Some(SuggestionTag::Default));
            }
            add_kind_values(&mut out, entry, ctx);
            add_key_values(&mut out, entry.key, ctx);
        }
    }
    for suggestion in &mut out {
        if suggestion.value == ctx.current {
            merge_tag(&mut suggestion.tags, SuggestionTag::Current);
        }
        if suggestion.value == ctx.default {
            merge_tag(&mut suggestion.tags, SuggestionTag::Default);
        }
        if matches!(
            entry.kind,
            ValueKind::Enum { .. } | ValueKind::Bool | ValueKind::MainPaneOrder
        ) {
            let choice = if suggestion.value.is_empty() {
                "_empty"
            } else {
                &suggestion.value
            };
            suggestion.label_key = Some(format!(
                "app.modals.config_editor.choices.{}.{choice}",
                entry.key
            ));
        }
        if matches!(entry.kind, ValueKind::Color) {
            suggestion.swatch = parse_swatch(&suggestion.value);
        }
        if entry.key.ends_with("_seconds") || entry.key == "updates_refresh_interval" {
            suggestion.duration_secs = suggestion.value.parse().ok();
        }
    }
    out
}

/// Add candidates derived from a schema kind.
fn add_kind_values(out: &mut Vec<Suggestion>, entry: &EditableSetting, ctx: &SuggestionContext) {
    match entry.kind {
        ValueKind::IntRange { min, max } => {
            add(out, &min.to_string(), Some(SuggestionTag::Minimum));
            for value in int_presets(entry.key)
                .iter()
                .copied()
                .filter(|value| *value >= min && *value <= max)
            {
                add(out, &value.to_string(), None);
            }
            add(out, &max.to_string(), Some(SuggestionTag::Maximum));
        }
        ValueKind::OptionalUnsignedOrAll => {
            add_values(out, &["all", "1", "3", "7", "14", "30", "90"]);
        }
        ValueKind::Color => {
            for (key, value) in &ctx.palette {
                add(out, value, None);
                if let Some(candidate) = out.iter_mut().find(|candidate| candidate.value == *value)
                {
                    candidate.label_key.get_or_insert_with(|| {
                        format!("app.modals.config_editor.settings.{key}.label")
                    });
                }
            }
        }
        ValueKind::Path if entry.key == "pi_scan_binary" => {
            add(out, "pi", None);
            if let Some(path) = &ctx.pi_binary {
                add(out, path, Some(SuggestionTag::Installed));
            }
        }
        _ => {}
    }
}

/// Choose compact numeric presets with meaningful time intervals and limits.
fn int_presets(key: &str) -> &'static [i64] {
    if key.ends_with("_seconds") || key == "updates_refresh_interval" {
        return &[1, 5, 10, 30, 60, 120, 300, 600, 900, 1800, 3600, 86400];
    }
    if key.ends_with("_days") {
        return &[1, 3, 7, 14, 30, 90, 365];
    }
    if key.starts_with("layout_") {
        return &[10, 20, 25, 30, 40, 50, 60, 75, 100];
    }
    &[1, 3, 5, 10, 15, 20, 25, 50, 100]
}

/// Add known values for string-backed settings and environment-derived candidates.
fn add_key_values(out: &mut Vec<Suggestion>, key: &str, ctx: &SuggestionContext) {
    match key {
        "preferred_terminal" => {
            add(out, "", Some(SuggestionTag::AutoDetect));
            for (name, _) in TERMINAL_CANDIDATES {
                add(out, name, None);
            }
            for name in &ctx.installed_terminals {
                add(out, name, Some(SuggestionTag::Installed));
            }
        }
        "selected_countries" => add_values(out, COUNTRIES),
        "news_read_symbol" | "news_unread_symbol" => {
            add_values(out, &["●", "○", "✓", "•", "★", "☆", "+", "-"]);
        }
        "pkgbuild_shellcheck_exclude" => add_values(
            out,
            &["SC2034", "SC2154", "SC2164", "SC2086", "SC1090", "SC1091"],
        ),
        "aur_vote_ssh_command" => add(out, "ssh", None),
        "pi_scan_provider" => {
            for (provider, _) in &ctx.pi_routes {
                add(out, provider, Some(SuggestionTag::Detected));
            }
        }
        "pi_scan_model" => {
            for (_, model) in &ctx.pi_routes {
                add(out, model, Some(SuggestionTag::Detected));
            }
        }
        "pi_scan_fallback_models" => {
            for (provider, model) in &ctx.pi_routes {
                add(
                    out,
                    &format!("{provider}/{model}"),
                    Some(SuggestionTag::Detected),
                );
            }
        }
        "pi_scan_background_starts_per_hour" => add(out, "0", Some(SuggestionTag::Unlimited)),
        "pi_scan_background_token_cap_24h" => {
            add(out, "0", Some(SuggestionTag::Unlimited));
            add_values(out, &["10000", "100000", "1000000"]);
        }
        "pi_scan_background_cost_cap_24h" => {
            add(out, "0", Some(SuggestionTag::Unlimited));
            add_values(out, &["0.10", "1.00", "5.00", "10.00"]);
        }
        "pi_scan_https_proxy" => {
            // Empty means no proxy rather than auto-detection, so it carries no tag.
            add(out, "", None);
            if let Some(proxy) = &ctx.env_proxy
                && proxy.starts_with("https://")
                && !proxy.contains('@')
            {
                add(out, proxy, Some(SuggestionTag::Detected));
            }
        }
        _ => {}
    }
}

/// Keys whose popup completes only the final comma-separated token.
#[must_use]
pub fn comma_list(key: &str) -> bool {
    matches!(
        key,
        "selected_countries" | "pkgbuild_shellcheck_exclude" | "pi_scan_fallback_models"
    )
}

/// Six canonical pane permutations, in stable picker order.
#[must_use]
pub fn main_pane_choices() -> Vec<String> {
    use crate::state::MainVerticalPane::{Middle as M, PackageInfo as P, Results as R};
    [
        [R, M, P],
        [R, P, M],
        [M, R, P],
        [M, P, R],
        [P, R, M],
        [P, M, R],
    ]
    .iter()
    .map(crate::state::format_main_pane_order)
    .collect()
}

/// Merge one provenance tag without duplicating it.
fn merge_tag(tags: &mut Vec<SuggestionTag>, tag: SuggestionTag) {
    if !tags.contains(&tag) {
        tags.push(tag);
    }
}

/// Insert a value once, merging provenance when another source already supplied it.
fn add(out: &mut Vec<Suggestion>, value: &str, tag: Option<SuggestionTag>) {
    let index = out
        .iter()
        .position(|candidate| candidate.value == value)
        .unwrap_or_else(|| {
            out.push(Suggestion {
                value: value.to_string(),
                tags: Vec::new(),
                duration_secs: None,
                swatch: None,
                label_key: None,
            });
            out.len() - 1
        });
    if let Some(tag) = tag {
        merge_tag(&mut out[index].tags, tag);
    }
}

/// Insert a fixed sequence of values without changing its order.
fn add_values(out: &mut Vec<Suggestion>, values: &[&str]) {
    for value in values {
        add(out, value, None);
    }
}

/// What: Parse a hex (`#rrggbb`) or `R,G,B` color into its components.
///
/// Inputs: Raw color text as typed or stored.
///
/// Output: RGB components, or `None` when the text is not a complete color.
///
/// Details: Shared by dropdown swatches and the renderer's input preview so both agree.
#[must_use]
pub fn parse_swatch(value: &str) -> Option<(u8, u8, u8)> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() == 6 && hex.is_ascii() {
        return Some((
            u8::from_str_radix(&hex[..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..], 16).ok()?,
        ));
    }
    let values: Vec<u8> = value
        .split(',')
        .map(str::trim)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    if let [r, g, b] = values.as_slice() {
        Some((*r, *g, *b))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic candidates for one schema key.
    fn candidates(key: &str, ctx: &SuggestionContext) -> Vec<Suggestion> {
        suggestions_for(crate::theme::find_setting(key).expect("schema"), ctx)
    }

    /// Detected proxies must never offer credentials or insecure transport.
    #[test]
    fn regression_proxy_candidates_require_credential_free_https() {
        for proxy in [
            "https://user:password@proxy.invalid",
            "http://proxy.invalid",
        ] {
            let ctx = SuggestionContext {
                env_proxy: Some(proxy.into()),
                ..SuggestionContext::default()
            };
            assert!(
                !candidates("pi_scan_https_proxy", &ctx)
                    .iter()
                    .any(|s| s.value == proxy),
                "unsafe environment proxy was suggested"
            );
        }
        let ctx = SuggestionContext {
            env_proxy: Some("https://proxy.invalid".into()),
            ..SuggestionContext::default()
        };
        assert!(
            candidates("pi_scan_https_proxy", &ctx)
                .iter()
                .any(|s| s.value == "https://proxy.invalid"
                    && s.tags.contains(&SuggestionTag::Detected))
        );
    }

    /// Palette candidates identify the source theme key.
    #[test]
    fn regression_palette_candidates_name_source() {
        let entry = crate::theme::find_setting("semantic_error").expect("schema");
        let ctx = SuggestionContext {
            palette: vec![("background_base".into(), "#123456".into())],
            ..SuggestionContext::default()
        };
        let color = "#123456";
        let suggestions = suggestions_for(entry, &ctx);
        let candidate = suggestions
            .iter()
            .find(|s| s.value == color)
            .expect("palette color");
        assert_eq!(
            candidate.label_key.as_deref(),
            Some("app.modals.config_editor.settings.background_base.label")
        );
    }

    /// Provenance merges by value, and secret values are excluded entirely.
    #[test]
    fn dedupe_and_secret_exclusion() {
        let ctx = SuggestionContext {
            current: "kitty".into(),
            default: "kitty".into(),
            installed_terminals: vec!["kitty".into()],
            ..SuggestionContext::default()
        };
        let suggestions = candidates("preferred_terminal", &ctx);
        let kitty: Vec<_> = suggestions
            .iter()
            .filter(|value| value.value == "kitty")
            .collect();
        assert_eq!(kitty.len(), 1);
        assert_eq!(
            kitty[0].tags,
            [
                SuggestionTag::Current,
                SuggestionTag::Default,
                SuggestionTag::Installed
            ]
        );
        assert!(
            suggestions
                .iter()
                .any(|value| value.tags.contains(&SuggestionTag::AutoDetect))
        );
        assert!(candidates("virustotal_api_key", &ctx).is_empty());
    }

    /// Known strings, durations, optional days, palette, and bindings receive useful presets.
    #[test]
    fn candidates_cover_schema_kinds() {
        let ctx = SuggestionContext {
            current: "Germany, France".into(),
            default: "SC2034, SC2154".into(),
            palette: vec![("background_base".into(), "#123456".into())],
            ..SuggestionContext::default()
        };
        assert!(
            candidates("selected_countries", &ctx)
                .iter()
                .any(|value| value.value == "Worldwide")
        );
        assert!(
            !candidates("selected_countries", &ctx)
                .iter()
                .any(|value| value.value.contains(','))
        );
        assert!(
            candidates("pkgbuild_shellcheck_exclude", &ctx)
                .iter()
                .any(|value| value.value == "SC1091")
        );
        assert!(
            candidates("updates_refresh_interval", &ctx)
                .iter()
                .any(|value| value.duration_secs == Some(3600))
        );
        assert!(
            candidates("news_max_age_days", &ctx)
                .iter()
                .any(|value| value.value == "all")
        );
        assert!(
            candidates("semantic_error", &ctx)
                .iter()
                .any(|value| value.swatch == Some((0x12, 0x34, 0x56)))
        );
        let binding = SuggestionContext {
            current: "F9".into(),
            default: "Ctrl+r".into(),
            ..SuggestionContext::default()
        };
        assert!(
            candidates("keybind_reload_config", &binding)
                .iter()
                .any(
                    |value| value.value == "Ctrl+r" && value.tags.contains(&SuggestionTag::Default)
                )
        );
    }

    /// Pi facts, caps, executable paths, and proxy detection are injected, never probed here.
    #[test]
    fn pi_presets_use_injected_facts() {
        let ctx = SuggestionContext {
            pi_routes: vec![("provider".into(), "model".into())],
            env_proxy: Some("https://proxy.invalid".into()),
            pi_binary: Some("/test/bin/pi".into()),
            ..SuggestionContext::default()
        };
        for (key, expected) in [
            ("pi_scan_provider", "provider"),
            ("pi_scan_model", "model"),
            ("pi_scan_fallback_models", "provider/model"),
            ("pi_scan_https_proxy", "https://proxy.invalid"),
            ("pi_scan_binary", "/test/bin/pi"),
        ] {
            assert!(
                candidates(key, &ctx)
                    .iter()
                    .any(|value| value.value == expected),
                "{key}"
            );
        }
        for key in [
            "pi_scan_background_token_cap_24h",
            "pi_scan_background_cost_cap_24h",
            "pi_scan_background_starts_per_hour",
        ] {
            assert!(
                candidates(key, &ctx)
                    .iter()
                    .any(|value| value.value == "0"
                        && value.tags.contains(&SuggestionTag::Unlimited))
            );
        }
        let thinking = candidates("pi_scan_thinking", &ctx);
        assert_eq!(
            thinking
                .iter()
                .map(|value| value.value.as_str())
                .collect::<Vec<_>>(),
            ["off", "minimal", "low", "medium", "high", "xhigh", "max"]
        );
        assert_eq!(main_pane_choices().len(), 6);
        for choice in main_pane_choices() {
            assert!(crate::state::parse_main_pane_order(&choice).is_some());
        }
    }
}

#[cfg(test)]
mod bounds_tests {
    use super::*;

    /// Integer presets stay in range and retain all provenance when bounds/defaults coincide.
    #[test]
    fn numeric_bounds_and_dedupe() {
        let entry = crate::theme::find_setting("mirror_count").expect("schema");
        let suggestions = suggestions_for(
            entry,
            &SuggestionContext {
                current: "1".into(),
                default: "1".into(),
                ..SuggestionContext::default()
            },
        );
        assert!(suggestions.iter().all(|value| {
            value
                .value
                .parse::<i64>()
                .is_ok_and(|value| (1..=200).contains(&value))
        }));
        let min = suggestions
            .iter()
            .find(|value| value.value == "1")
            .expect("minimum");
        assert!(
            min.tags.contains(&SuggestionTag::Current)
                && min.tags.contains(&SuggestionTag::Default)
                && min.tags.contains(&SuggestionTag::Minimum)
        );
        assert!(
            suggestions
                .iter()
                .any(|value| value.value == "200" && value.tags.contains(&SuggestionTag::Maximum))
        );
    }
}
