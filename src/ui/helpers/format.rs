//! Formatting utilities for UI display.
//!
//! This module provides functions for formatting package details, byte sizes, and other
//! UI elements into human-readable strings and ratatui lines.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{i18n, state::AppState, theme::Theme};

/// What: Format the current [`AppState::details`] into themed `ratatui` lines.
///
/// Inputs:
/// - `app`: Read-only application state; uses `app.details` to render fields
/// - `_area_width`: Reserved for future wrapping/layout needs (currently unused)
/// - `th`: Active theme for colors/styles
///
/// Output:
/// - Vector of formatted lines for the Details pane, ending with a Show/Hide PKGBUILD action line.
///
/// Details:
/// - Applies repo-specific heuristics, formats numeric sizes via `human_bytes`, and appends a
///   clickable PKGBUILD toggle line using accent styling.
pub fn format_details_lines(app: &AppState, _area_width: u16, th: &Theme) -> Vec<Line<'static>> {
    /// What: Build a themed key-value line for the details pane.
    ///
    /// Inputs:
    /// - `key`: Label to display (styled in accent color)
    /// - `val`: Value text rendered in primary color
    /// - `th`: Active theme for colors/modifiers
    ///
    /// Output:
    /// - `Line` combining the key/value segments with appropriate styling.
    ///
    /// Details:
    /// - Renders the key in bold accent with a trailing colon and the value in standard text color.
    fn kv(key: &str, val: String, th: &Theme) -> Line<'static> {
        Line::from(vec![
            Span::styled(
                format!("{key}: "),
                Style::default()
                    .fg(th.sapphire)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(val, Style::default().fg(th.text)),
        ])
    }
    let d = &app.details;
    // Compute display repository using unified Manjaro detection (name prefix or owner).
    let repo_display = if crate::index::is_manjaro_name_or_owner(&d.name, &d.owner) {
        "manjaro".to_string()
    } else {
        d.repository.clone()
    };
    // Each line is a label/value pair derived from the current details view.
    let mut lines = vec![
        kv(
            &i18n::t(app, "app.details.fields.repository"),
            repo_display,
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.package_name"),
            d.name.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.version"),
            d.version.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.description"),
            d.description.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.architecture"),
            d.architecture.clone(),
            th,
        ),
        kv(&i18n::t(app, "app.details.fields.url"), d.url.clone(), th),
        kv(
            &i18n::t(app, "app.details.fields.licences"),
            join(&d.licenses),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.provides"),
            join(&d.provides),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.depends_on"),
            join(&d.depends),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.optional_dependencies"),
            join(&d.opt_depends),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.required_by"),
            join(&d.required_by),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.optional_for"),
            join(&d.optional_for),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.conflicts_with"),
            join(&d.conflicts),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.replaces"),
            join(&d.replaces),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.download_size"),
            d.download_size.map_or_else(
                || i18n::t(app, "app.details.fields.not_available"),
                human_bytes,
            ),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.install_size"),
            d.install_size.map_or_else(
                || i18n::t(app, "app.details.fields.not_available"),
                human_bytes,
            ),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.package_owner"),
            d.owner.clone(),
            th,
        ),
        kv(
            &i18n::t(app, "app.details.fields.build_date"),
            d.build_date.clone(),
            th,
        ),
    ];
    // Add a clickable helper line to Show/Hide PKGBUILD below Build date
    let pkgb_label = if app.pkgb_visible {
        i18n::t(app, "app.details.hide_pkgbuild")
    } else {
        i18n::t(app, "app.details.show_pkgbuild")
    };
    lines.push(Line::from(vec![Span::styled(
        pkgb_label,
        Style::default()
            .fg(th.mauve)
            .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
    )]));
    lines
}

/// What: Join a slice of strings with `", "`, falling back to "-" when empty.
///
/// Inputs:
/// - `list`: Slice of strings to format
///
/// Output:
/// - Joined string or "-" when no entries are present.
///
/// Details:
/// - Keeps the details pane compact by representing empty lists with a single dash.
pub(crate) fn join(list: &[String]) -> String {
    if list.is_empty() {
        "-".into()
    } else {
        list.join(", ")
    }
}

/// What: Format bytes into human-readable string with appropriate unit.
///
/// Inputs:
/// - `value`: Number of bytes to format.
///
/// Output:
/// - Returns a formatted string like "1.5 MiB" or "1024 B".
///
/// Details:
/// - Uses binary units (KiB, MiB, GiB, etc.) and shows integer for bytes < 1024, otherwise 1 decimal place.
#[must_use]
pub fn format_bytes(value: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    #[allow(clippy::cast_precision_loss)]
    let mut size = value as f64;
    let mut unit_index = 0usize;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{value} {}", UNITS[unit_index])
    } else {
        format!("{size:.1} {}", UNITS[unit_index])
    }
}

/// What: Format signed bytes into human-readable string with +/- prefix.
///
/// Inputs:
/// - `value`: Signed number of bytes to format.
///
/// Output:
/// - Returns a formatted string like "+1.5 MiB" or "-512 KiB" or "0 B".
///
/// Details:
/// - Uses `format_bytes` for magnitude and adds +/- prefix based on sign.
#[must_use]
pub fn format_signed_bytes(value: i64) -> String {
    if value == 0 {
        return "0 B".to_string();
    }
    let magnitude = value.unsigned_abs();
    if value > 0 {
        format!("+{}", format_bytes(magnitude))
    } else {
        format!("-{}", format_bytes(magnitude))
    }
}

/// What: Format a byte count using binary units with one decimal place.
///
/// Inputs:
/// - `n`: Raw byte count to format
///
/// Output:
/// - Size string such as "1.5 KiB" using 1024-based units.
///
/// Details:
/// - Iteratively divides by 1024 up to PiB, retaining one decimal place for readability.
/// - Always shows decimal place (unlike `format_bytes` which shows integer for bytes < 1024).
#[must_use]
pub fn human_bytes(n: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    #[allow(clippy::cast_precision_loss)]
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    format!("{v:.1} {}", UNITS[i])
}
