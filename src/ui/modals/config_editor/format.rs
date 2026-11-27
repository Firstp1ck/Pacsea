//! Localized labels and type-aware value formatting for the config editor.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Span,
};

use super::FrameCtx;
use crate::state::config_editor::suggestions::{comma_list, main_pane_choices, parse_swatch};
use crate::state::config_editor::{SuggestionTag, setting_group};
use crate::theme::{EditableSetting, ReloadBehavior, ValueKind};

/// Prefix shared by every config-editor translation key.
pub(super) const KEY_PREFIX: &str = "app.modals.config_editor.";

/// Masked display used for a non-empty secret.
const SECRET_MASK: &str = "••••••••";

/// String settings whose empty value enables automatic discovery.
const EMPTY_AUTO_DETECT_KEYS: &[&str] = &["preferred_terminal"];

/// Seconds per minute, hour, and day for duration humanizing.
const MINUTE: u64 = 60;
/// Seconds per hour.
const HOUR: u64 = 60 * MINUTE;
/// Seconds per day.
const DAY: u64 = 24 * HOUR;

impl FrameCtx<'_> {
    /// Translate a key below `app.modals.config_editor.`.
    pub(super) fn t(&self, suffix: &str) -> String {
        crate::i18n::t(self.app, &format!("{KEY_PREFIX}{suffix}"))
    }

    /// Translate a key below `app.modals.config_editor.` with ordered `{}` arguments.
    pub(super) fn t_args(&self, suffix: &str, args: &[&dyn std::fmt::Display]) -> String {
        crate::i18n::t_fmt(self.app, &format!("{KEY_PREFIX}{suffix}"), args)
    }

    /// Translate a full key, or `None` when no locale defines it.
    ///
    /// `crate::i18n::t` returns the key itself when a translation is missing, so a
    /// result equal to the key means "untranslated".
    pub(super) fn translated(&self, key: &str) -> Option<String> {
        let text = crate::i18n::t(self.app, key);
        (text != key).then_some(text)
    }
}

/// Localized row label with a readable fallback derived from the key.
pub(super) fn setting_label(ctx: &FrameCtx<'_>, entry: &EditableSetting) -> String {
    ctx.translated(&entry.label_i18n_key())
        .unwrap_or_else(|| entry.key.replace('_', " "))
}

/// Localized summary, or `None` when the key has no translated summary.
pub(super) fn setting_summary(ctx: &FrameCtx<'_>, entry: &EditableSetting) -> Option<String> {
    ctx.translated(&entry.summary_i18n_key())
}

/// Localized group heading for the entry's display group.
pub(super) fn group_label(ctx: &FrameCtx<'_>, entry: &EditableSetting) -> String {
    let key = setting_group(entry).i18n_key();
    ctx.translated(key)
        .unwrap_or_else(|| key.rsplit('.').next().unwrap_or(key).replace('_', " "))
}

/// Localized description of how a saved value reaches the running app.
pub(super) fn reload_label(ctx: &FrameCtx<'_>, reload: ReloadBehavior) -> String {
    ctx.t(match reload {
        ReloadBehavior::AutoReload => "applies.auto_reload",
        ReloadBehavior::AppliesOnSave => "applies.on_save",
        ReloadBehavior::RequiresRestart => "applies.restart",
    })
}

/// Color used for the apply-behavior text so restart-bound keys stand out.
pub(super) const fn reload_color(ctx: &FrameCtx<'_>, reload: ReloadBehavior) -> Color {
    match reload {
        ReloadBehavior::AutoReload => ctx.th.green,
        ReloadBehavior::AppliesOnSave => ctx.th.sapphire,
        ReloadBehavior::RequiresRestart => ctx.th.red,
    }
}

/// Whether integer values of this key are durations in seconds.
///
/// Mirrors the rule used by the suggestion engine to set `duration_secs`.
fn is_seconds_key(key: &str) -> bool {
    key.ends_with("_seconds") || key == "updates_refresh_interval"
}

/// What: Describe the accepted value type and range of a setting.
///
/// Inputs:
/// - `ctx`: Frame context (translations).
/// - `entry`: Schema row.
///
/// Output:
/// - Localized text such as `integer 1–200` or `key chord · conflict scope: Search pane`.
///
/// Details:
/// - Second-based integer ranges also show their humanized bounds.
/// - The keybind conflict scope reuses the localized group heading.
pub(super) fn type_label(ctx: &FrameCtx<'_>, entry: &EditableSetting) -> String {
    match entry.kind {
        ValueKind::Bool => ctx.t("types.bool"),
        ValueKind::Enum { choices } => ctx.t_args("types.enum", &[&choices.len()]),
        ValueKind::String if comma_list(entry.key) => ctx.t("types.list"),
        ValueKind::String => ctx.t("types.text"),
        ValueKind::Path => ctx.t("types.path"),
        ValueKind::Secret => ctx.t("types.secret"),
        ValueKind::IntRange { min, max } => {
            let base = ctx.t_args("types.int", &[&min, &max]);
            match (u64::try_from(min), u64::try_from(max)) {
                (Ok(lo), Ok(hi)) if is_seconds_key(entry.key) => format!(
                    "{base} ({}–{})",
                    humanize_duration(ctx, lo),
                    humanize_duration(ctx, hi)
                ),
                _ => base,
            }
        }
        ValueKind::OptionalUnsignedOrAll => ctx.t("types.days"),
        ValueKind::Color => ctx.t("types.color"),
        ValueKind::MainPaneOrder => ctx.t_args("types.pane_order", &[&main_pane_choices().len()]),
        ValueKind::KeyChord => ctx.t_args("types.chord", &[&group_label(ctx, entry)]),
    }
}

/// What: Render a duration in the largest whole unit (e.g. `5 min`, `1 h`, `1 day`).
///
/// Inputs:
/// - `ctx`: Frame context (translations).
/// - `secs`: Duration in seconds.
///
/// Output:
/// - Localized duration text.
///
/// Details:
/// - Falls back to seconds when the value is not a whole number of minutes.
pub(super) fn humanize_duration(ctx: &FrameCtx<'_>, secs: u64) -> String {
    let (count, key) = if secs >= DAY && secs.is_multiple_of(DAY) {
        let days = secs / DAY;
        (
            days,
            if days == 1 {
                "duration.day"
            } else {
                "duration.days"
            },
        )
    } else if secs >= HOUR && secs.is_multiple_of(HOUR) {
        (secs / HOUR, "duration.hours")
    } else if secs >= MINUTE && secs.is_multiple_of(MINUTE) {
        (secs / MINUTE, "duration.minutes")
    } else {
        (secs, "duration.seconds")
    };
    ctx.t_args(key, &[&count])
}

/// Localized badge text for a suggestion provenance tag.
pub(super) fn tag_label(ctx: &FrameCtx<'_>, tag: SuggestionTag) -> String {
    ctx.t(match tag {
        SuggestionTag::Current => "tags.current",
        SuggestionTag::Default => "tags.default",
        SuggestionTag::AutoDetect => "tags.auto_detect",
        SuggestionTag::Unlimited => "tags.unlimited",
        SuggestionTag::Installed => "tags.installed",
        SuggestionTag::Detected => "tags.detected",
        SuggestionTag::Minimum => "tags.minimum",
        SuggestionTag::Maximum => "tags.maximum",
    })
}

/// Badge color so the current value and the default are recognizable at a glance.
pub(super) const fn tag_color(ctx: &FrameCtx<'_>, tag: SuggestionTag) -> Color {
    match tag {
        SuggestionTag::Current => ctx.th.green,
        SuggestionTag::Default => ctx.th.sapphire,
        SuggestionTag::Minimum | SuggestionTag::Maximum => ctx.th.overlay2,
        SuggestionTag::AutoDetect
        | SuggestionTag::Unlimited
        | SuggestionTag::Installed
        | SuggestionTag::Detected => ctx.th.lavender,
    }
}

/// Localized placeholder for an empty value of `entry`.
pub(super) fn empty_label(ctx: &FrameCtx<'_>, entry: &EditableSetting) -> String {
    match entry.kind {
        _ if EMPTY_AUTO_DETECT_KEYS.contains(&entry.key) => ctx.t("popup_empty_choice"),
        ValueKind::Enum { choices } if choices.contains(&"") => ctx.t("popup_empty_choice"),
        ValueKind::KeyChord => ctx.t("values.unbound"),
        _ => ctx.t("values.empty"),
    }
}

/// Localized on/off text for a canonical boolean string.
pub(super) fn bool_label(ctx: &FrameCtx<'_>, raw: &str) -> String {
    ctx.t(if raw == "true" {
        "values.on"
    } else {
        "values.off"
    })
}

/// What: Format a stored or candidate value for display.
///
/// Inputs:
/// - `ctx`: Frame context (theme + translations).
/// - `entry`: Schema row that owns the value.
/// - `raw`: Canonical value text.
/// - `reveal_secret`: Show a secret in clear text (popup reveal toggle).
///
/// Output:
/// - Styled spans: on/off for bools, `██ #hex` swatches for colors, masks for secrets,
///   and localized placeholders for empty values.
///
/// Details:
/// - Secrets are never shown unless `reveal_secret` is true.
pub(super) fn value_spans(
    ctx: &FrameCtx<'_>,
    entry: &EditableSetting,
    raw: &str,
    reveal_secret: bool,
) -> Vec<Span<'static>> {
    let th = ctx.th;
    let dim = Style::default()
        .fg(th.overlay1)
        .add_modifier(Modifier::ITALIC);
    match entry.kind {
        ValueKind::Bool => {
            let color = if raw == "true" { th.green } else { th.overlay2 };
            vec![Span::styled(
                bool_label(ctx, raw),
                Style::default().fg(color),
            )]
        }
        ValueKind::Secret if raw.is_empty() => {
            vec![Span::styled(ctx.t("values.secret_unset"), dim)]
        }
        ValueKind::Secret if !reveal_secret => {
            vec![Span::styled(SECRET_MASK, Style::default().fg(th.subtext0))]
        }
        _ if raw.is_empty() => vec![Span::styled(empty_label(ctx, entry), dim)],
        ValueKind::Color => match parse_swatch(raw) {
            Some((r, g, b)) => vec![
                Span::styled("██", Style::default().fg(Color::Rgb(r, g, b))),
                Span::raw(" "),
                Span::styled(raw.to_string(), Style::default().fg(th.text)),
            ],
            None => vec![Span::styled(raw.to_string(), Style::default().fg(th.text))],
        },
        _ => vec![Span::styled(raw.to_string(), Style::default().fg(th.text))],
    }
}

/// What: Display text for a dropdown suggestion value.
///
/// Inputs:
/// - `ctx`: Frame context.
/// - `entry`: Schema row of the popup.
/// - `value`: Canonical suggestion value.
///
/// Output:
/// - On/off for bools, localized placeholder for the empty value, otherwise `value`.
///
/// Details:
/// - Secrets never have suggestions, so no masking is needed here.
pub(super) fn suggestion_text(ctx: &FrameCtx<'_>, entry: &EditableSetting, value: &str) -> String {
    if matches!(entry.kind, ValueKind::Bool) {
        bool_label(ctx, value)
    } else if value.is_empty() {
        empty_label(ctx, entry)
    } else {
        value.to_string()
    }
}
