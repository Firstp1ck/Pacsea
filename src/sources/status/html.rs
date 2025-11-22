use crate::state::ArchStatusColor;

use super::utils::{
    extract_arch_systems_status_color, extract_aur_today_percent, extract_aur_today_rect_color,
    extract_status_updates_today_color, severity_max, today_ymd_utc,
};

/// What: Check if AUR specifically shows "Down" status in the monitors section.
///
/// Inputs:
/// - `body`: Raw HTML content to analyze.
///
/// Output:
/// - `true` if AUR monitor shows "Down" status, `false` otherwise.
///
/// Details: Searches the monitors section for AUR and checks if it has "Down" status indicator.
/// Looks for patterns like `title="Down"`, `>Down<`, or "Down" text near AUR.
/// The actual HTML structure has: `<a title="AUR"...>` followed by `<div... title="Down">`
pub(super) fn is_aur_down_in_monitors(body: &str) -> bool {
    let lowered = body.to_lowercase();

    // Find the monitors section
    if let Some(monitors_pos) = lowered.find("monitors") {
        let monitors_window_start = monitors_pos;
        let monitors_window_end = std::cmp::min(body.len(), monitors_pos + 15000);
        let monitors_window = &lowered[monitors_window_start..monitors_window_end];

        // Look for AUR in the monitors section - be more specific: look for title="aur" or "aur" in monitor context
        // The actual HTML has: <a title="AUR" class="psp-monitor-name...
        let aur_patterns = ["title=\"aur\"", "title='aur'", ">aur<", "\"aur\""];
        let mut aur_pos_opt = None;

        for pattern in aur_patterns {
            if let Some(pos) = monitors_window.find(pattern) {
                aur_pos_opt = Some(pos);
                break;
            }
        }

        // Fallback: just search for "aur" if pattern search didn't work
        if aur_pos_opt.is_none() {
            aur_pos_opt = monitors_window.find("aur");
        }

        if let Some(aur_pos) = aur_pos_opt {
            // Check in a much larger window around AUR (2000 chars after) for "Down" status
            // The actual HTML structure has quite a bit of content between AUR and Down
            let aur_window_start = aur_pos;
            let aur_window_end = std::cmp::min(monitors_window.len(), aur_pos + 2000);
            let aur_window = &monitors_window[aur_window_start..aur_window_end];

            // Check for "Down" status indicators near AUR
            // Look for various patterns:
            // 1. title="Down" or title='Down' (most reliable - this is what the actual HTML has)
            // 2. psp-monitor-row-status-inner with title="Down" (specific to status page)
            // 3. >down< (text content between tags)
            // 4. "down" or 'down' (in quotes)
            let has_title_down =
                aur_window.contains("title=\"down\"") || aur_window.contains("title='down'");
            let has_status_inner_down = aur_window.contains("psp-monitor-row-status-inner")
                && (aur_window.contains("title=\"down\"") || aur_window.contains("title='down'"));
            let has_tagged_down = aur_window.contains(">down<");
            let has_quoted_down = aur_window.contains("\"down\"") || aur_window.contains("'down'");

            // Check for plain "down" text, but make sure it's a standalone word
            // Look for word boundaries (space, >, <, etc.) before and after "down"
            let has_plain_down = aur_window.contains(" down ")
                || aur_window.contains(">down<")
                || aur_window.contains(">down ")
                || aur_window.contains(" down<");

            if has_title_down
                || has_status_inner_down
                || has_tagged_down
                || has_quoted_down
                || has_plain_down
            {
                // Verify it's not part of "operational" or other positive status
                // Also check that we're not seeing "download" or similar words
                if !aur_window.contains("operational")
                    && !aur_window.contains("download")
                    && !aur_window.contains("shutdown")
                    && !aur_window.contains("breakdown")
                {
                    return true;
                }
            }
        }
    }

    false
}

/// Parse a status message and color from the HTML of a status page.
///
/// Inputs:
/// - `body`: Raw HTML content to analyze.
///
/// Output:
/// - Tuple `(message, color)` representing a concise status and visual color classification.
pub(super) fn parse_arch_status_from_html(body: &str) -> (String, ArchStatusColor) {
    let lowered = body.to_lowercase();
    let has_all_ok = lowered.contains("all systems operational");

    // Check for "Status: Arch systems..." with "Some Systems down" or "Down"
    // This must be checked FIRST as it represents the overall system status
    let arch_systems_status_color = extract_arch_systems_status_color(body);

    // Check if AUR specifically shows "Down" status - this should be prioritized
    let aur_is_down = is_aur_down_in_monitors(body);

    // If arch systems status shows a problem, check if AUR is specifically down
    if let Some(systems_status_color) = arch_systems_status_color
        && matches!(
            systems_status_color,
            ArchStatusColor::IncidentToday | ArchStatusColor::IncidentSevereToday
        )
    {
        let aur_pct_opt = extract_aur_today_percent(body);
        let aur_pct_suffix = aur_pct_opt
            .map(|p| format!(" — AUR today: {p}%"))
            .unwrap_or_default();

        // If AUR is specifically down, show AUR-specific message
        if aur_is_down {
            let text = format!("Status: AUR Down{aur_pct_suffix}");
            return (text, ArchStatusColor::IncidentSevereToday);
        }

        // Otherwise show generic systems down message
        let text = match systems_status_color {
            ArchStatusColor::IncidentSevereToday => {
                format!("Some Arch systems down (see status){aur_pct_suffix}")
            }
            ArchStatusColor::IncidentToday => {
                format!("Arch systems degraded (see status){aur_pct_suffix}")
            }
            _ => format!("Arch systems nominal{aur_pct_suffix}"),
        };
        return (text, systems_status_color);
    }

    // Also check if AUR is down even if overall status doesn't show problems
    if aur_is_down {
        let aur_pct_opt = extract_aur_today_percent(body);
        let aur_pct_suffix = aur_pct_opt
            .map(|p| format!(" — AUR today: {p}%"))
            .unwrap_or_default();
        let text = format!("Status: AUR Down{aur_pct_suffix}");
        return (text, ArchStatusColor::IncidentSevereToday);
    }

    // Try to extract today's AUR uptime percentage from the Monitors/90-day uptime section
    let aur_pct_opt = extract_aur_today_percent(body);
    let aur_pct_suffix = aur_pct_opt
        .map(|p| format!(" — AUR today: {p}%"))
        .unwrap_or_default();
    let aur_color_from_pct = aur_pct_opt.map(|p| {
        if p > 95 {
            ArchStatusColor::Operational
        } else if p >= 90 {
            ArchStatusColor::IncidentToday
        } else {
            ArchStatusColor::IncidentSevereToday
        }
    });
    // Prefer the SVG rect color for today's cell if present (authoritative UI color)
    let aur_color_from_rect = extract_aur_today_rect_color(body);

    // Check status updates for today's date and keywords
    let status_update_color = extract_status_updates_today_color(body);

    // Prioritize: arch systems status > rect color > status updates > percentage > default
    let final_color = arch_systems_status_color
        .or(aur_color_from_rect)
        .or(status_update_color)
        .or(aur_color_from_pct);

    let outage_key = "the aur is currently experiencing an outage";
    if let Some(pos) = lowered.find(outage_key) {
        let start = pos.saturating_sub(220);
        let region = &body[start..std::cmp::min(body.len(), pos + 220)];
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let mut is_today = false;
        'outer: for m in &months {
            if let Some(mi) = region.find(m) {
                let mut idx = mi + m.len();
                let rbytes = region.as_bytes();
                while idx < region.len() && (rbytes[idx] == b' ' || rbytes[idx] == b',') {
                    idx += 1;
                }
                let day_start = idx;
                while idx < region.len() && rbytes[idx].is_ascii_digit() {
                    idx += 1;
                }
                if idx == day_start {
                    continue;
                }
                let day_s = &region[day_start..idx];
                while idx < region.len() && (rbytes[idx] == b' ' || rbytes[idx] == b',') {
                    idx += 1;
                }
                let year_start = idx;
                let mut count = 0;
                while idx < region.len() && rbytes[idx].is_ascii_digit() && count < 4 {
                    idx += 1;
                    count += 1;
                }
                if count == 4
                    && let (Ok(day), Some((ty, tm, td))) =
                        (day_s.trim().parse::<u32>(), today_ymd_utc())
                {
                    let month_idx = months
                        .iter()
                        .position(|mm| *mm == *m)
                        .expect("month should be found in months array since it came from there")
                        as u32
                        + 1;
                    let year_s = &region[year_start..(year_start + 4)];
                    is_today = tm == month_idx && td == day && ty.to_string() == year_s;
                }
                break 'outer;
            }
        }

        if is_today {
            // During an outage today, force at least yellow; use red only for <90%
            let forced_color = match aur_pct_opt {
                Some(p) if p < 90 => ArchStatusColor::IncidentSevereToday,
                _ => ArchStatusColor::IncidentToday,
            };
            return (
                format!("AUR outage (see status){aur_pct_suffix}"),
                severity_max(
                    forced_color,
                    final_color.unwrap_or(ArchStatusColor::IncidentToday),
                ),
            );
        }
        // Outage announcement present but not today - still check visual indicators
        if has_all_ok {
            return (
                format!("All systems operational{aur_pct_suffix}"),
                final_color.unwrap_or(ArchStatusColor::IncidentToday),
            );
        }
        return (
            format!("Arch systems nominal{aur_pct_suffix}"),
            final_color.unwrap_or(ArchStatusColor::IncidentToday),
        );
    }

    // If rect color shows a problem, prioritize it even if text says "operational"
    if let Some(rect_color) = aur_color_from_rect
        && matches!(
            rect_color,
            ArchStatusColor::IncidentToday | ArchStatusColor::IncidentSevereToday
        )
    {
        let text = if has_all_ok {
            format!("AUR issues detected (see status){aur_pct_suffix}")
        } else {
            format!("AUR degraded (see status){aur_pct_suffix}")
        };
        return (text, rect_color);
    }

    // Check status updates for today
    if let Some(update_color) = status_update_color
        && matches!(
            update_color,
            ArchStatusColor::IncidentToday | ArchStatusColor::IncidentSevereToday
        )
    {
        let text = if has_all_ok {
            format!("AUR issues detected (see status){aur_pct_suffix}")
        } else {
            format!("AUR degraded (see status){aur_pct_suffix}")
        };
        return (text, update_color);
    }

    if has_all_ok {
        return (
            format!("All systems operational{aur_pct_suffix}"),
            final_color.unwrap_or(ArchStatusColor::Operational),
        );
    }

    (
        format!("Arch systems nominal{aur_pct_suffix}"),
        final_color.unwrap_or(ArchStatusColor::None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Verify that "Some systems down" heading is detected and returns correct status.
    ///
    /// Inputs:
    /// - HTML with "Some systems down" as a heading (actual status page format) but AUR is operational.
    ///
    /// Output:
    /// - Returns status text "Some Arch systems down (see status)" with `IncidentSevereToday` color.
    ///
    /// Details:
    /// - Ensures the early check for arch systems status works correctly and isn't overridden by other checks.
    fn status_parse_detects_some_systems_down_heading() {
        // HTML with "Some systems down" but AUR is operational (so it should show generic message)
        let html = "<html><body><h2>Some systems down</h2><div>Monitors (default)</div><div>AUR</div><div>Operational</div></body></html>";
        let (text, color) = parse_arch_status_from_html(html);
        assert_eq!(color, ArchStatusColor::IncidentSevereToday);
        assert!(text.contains("Some Arch systems down"));
    }

    #[test]
    /// What: Parse Arch status HTML to derive AUR color by percentage buckets and outage flag.
    ///
    /// Inputs:
    /// - Synthetic HTML windows around today's date containing percentages of 97, 95, 89 and optional outage headings.
    ///
    /// Output:
    /// - Returns `ArchStatusColor::Operational` above 95%, `IncidentToday` for 90-95%, `IncidentSevereToday` below 90%, and elevates outage cases to at least `IncidentToday`.
    ///
    /// Details:
    /// - Builds several HTML variants to confirm the parser reacts to both high-level outage banners and raw percentages.
    #[allow(clippy::many_single_char_names)]
    fn status_parse_color_by_percentage_and_outage() {
        let (y, m, d) = {
            let out = std::process::Command::new("date")
                .args(["-u", "+%Y-%m-%d"])
                .output();
            let Ok(o) = out else { return };
            if !o.status.success() {
                return;
            }
            let s = match String::from_utf8(o.stdout) {
                Ok(x) => x,
                Err(_) => return,
            };
            let mut it = s.trim().split('-');
            let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else {
                return;
            };
            let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>())
            else {
                return;
            };
            (y, m, d)
        };
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let month_name = months[(m - 1) as usize];
        let date_str = format!("{month_name} {d}, {y}");

        let make_html = |percent: u32, outage: bool| -> String {
            format!(
                "<html><body><h2>Uptime Last 90 days</h2><div>Monitors (default)</div><div>AUR</div><div>{date_str}</div><div>{percent}% uptime</div>{outage_block}</body></html>",
                outage_block = if outage {
                    "<h4>The AUR is currently experiencing an outage</h4>"
                } else {
                    ""
                }
            )
        };

        let html_green = make_html(97, false);
        let (_txt, color) = parse_arch_status_from_html(&html_green);
        assert_eq!(color, ArchStatusColor::Operational);

        let html_yellow = make_html(95, false);
        let (_txt, color) = parse_arch_status_from_html(&html_yellow);
        assert_eq!(color, ArchStatusColor::IncidentToday);

        let html_red = make_html(89, false);
        let (_txt, color) = parse_arch_status_from_html(&html_red);
        assert_eq!(color, ArchStatusColor::IncidentSevereToday);

        let html_outage = make_html(97, true);
        let (_txt, color) = parse_arch_status_from_html(&html_outage);
        assert_eq!(color, ArchStatusColor::IncidentToday);

        let html_outage_red = make_html(80, true);
        let (_txt, color) = parse_arch_status_from_html(&html_outage_red);
        assert_eq!(color, ArchStatusColor::IncidentSevereToday);
    }

    #[test]
    /// What: Prefer the SVG rect fill color over the textual percentage when both are present.
    ///
    /// Inputs:
    /// - HTML snippet around today's date with a green percentage but an SVG rect fill attribute set to yellow.
    ///
    /// Output:
    /// - Returns `ArchStatusColor::IncidentToday`, honoring the SVG-derived color.
    ///
    /// Details:
    /// - Ensures the parser checks the SVG dataset first so maintenance banners with stale percentages still reflect current outages.
    #[allow(clippy::many_single_char_names)]
    fn status_parse_prefers_svg_rect_color() {
        let (y, m, d) = {
            let out = std::process::Command::new("date")
                .args(["-u", "+%Y-%m-%d"])
                .output();
            let Ok(o) = out else { return };
            if !o.status.success() {
                return;
            }
            let s = match String::from_utf8(o.stdout) {
                Ok(x) => x,
                Err(_) => return,
            };
            let mut it = s.trim().split('-');
            let (Some(y), Some(m), Some(d)) = (it.next(), it.next(), it.next()) else {
                return;
            };
            let (Ok(y), Ok(m), Ok(d)) = (y.parse::<i32>(), m.parse::<u32>(), d.parse::<u32>())
            else {
                return;
            };
            (y, m, d)
        };
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let month_name = months[(m - 1) as usize];
        let date_str = format!("{month_name} {d}, {y}");

        let html = format!(
            "<html>\n  <body>\n    <h2>Uptime Last 90 days</h2>\n    <div>Monitors (default)</div>\n    <div>AUR</div>\n    <svg>\n      <rect x=\"900\" y=\"0\" width=\"10\" height=\"10\" fill=\"#f59e0b\"></rect>\n    </svg>\n    <div>{date_str}</div>\n    <div>97% uptime</div>\n  </body>\n</html>"
        );
        let (_txt, color) = parse_arch_status_from_html(&html);
        assert_eq!(color, ArchStatusColor::IncidentToday);
    }

    #[test]
    /// What: Verify that AUR "Down" status is detected and returns "Status: AUR Down" message.
    ///
    /// Inputs:
    /// - HTML with "Some systems down" heading and AUR showing "Down" in monitors section.
    ///
    /// Output:
    /// - Returns status text "Status: AUR Down" with `IncidentSevereToday` color.
    ///
    /// Details:
    /// - Ensures AUR-specific "Down" status is prioritized over generic "Some systems down" message.
    fn status_parse_prioritizes_aur_down_over_some_systems_down() {
        // HTML with "Some systems down" heading and AUR showing "Down"
        let html = "<html><body><h2>Some systems down</h2><div>Monitors (default)</div><div>AUR</div><div>>Down<</div></body></html>";
        let (text, color) = parse_arch_status_from_html(html);
        assert_eq!(color, ArchStatusColor::IncidentSevereToday);
        assert!(
            text.contains("Status: AUR Down"),
            "Expected 'Status: AUR Down' but got: {}",
            text
        );
    }

    #[test]
    /// What: Verify that `is_aur_down_in_monitors` correctly detects AUR "Down" status.
    ///
    /// Inputs:
    /// - HTML snippets with AUR showing "Down" in various formats in monitors section.
    ///
    /// Output:
    /// - Returns `true` when AUR shows "Down", `false` otherwise.
    ///
    /// Details:
    /// - Tests various HTML patterns for AUR "Down" status detection.
    fn status_is_aur_down_in_monitors() {
        // Test AUR Down with title="Down" pattern (actual status page format)
        let html_title = "<html><body><div>Monitors</div><div>AUR</div><div title=\"Down\">Status</div></body></html>";
        assert!(is_aur_down_in_monitors(html_title));

        // Test AUR Down with title='Down' pattern
        let html_title_single = "<html><body><div>Monitors</div><div>AUR</div><div title='Down'>Status</div></body></html>";
        assert!(is_aur_down_in_monitors(html_title_single));

        // Test AUR Down with actual status page HTML structure
        let html_real = "<div class=\"psp-monitor-row\"><div>Monitors</div><div>AUR</div><div class=\"psp-monitor-row-status-inner\" title=\"Down\"><span>Down</span></div></div>";
        assert!(is_aur_down_in_monitors(html_real));

        // Test with actual website HTML structure (from status.archlinux.org)
        // Must include "Monitors" text for the function to find the monitors section
        let html_actual = "<div>Monitors (default)</div><div class=\"psp-monitor-row\"><div class=\"uk-flex uk-flex-between uk-flex-wrap\"><div class=\"psp-monitor-row-header uk-text-muted uk-flex uk-flex-auto\"><a title=\"AUR\" class=\"psp-monitor-name uk-text-truncate uk-display-inline-block\" href=\"https://status.archlinux.org/788139639\">AUR<svg class=\"icon icon-plus-square uk-flex-none\"><use href=\"/assets/symbol-defs.svg#icon-arrow-right\"></use></svg></a><div class=\"uk-flex-none\"><span class=\"m-r-5 m-l-5 uk-visible@s\">|</span><span class=\"uk-text-primary uk-visible@s\">94.864%</span><div class=\"uk-hidden@s uk-margin-small-left\"><div class=\"uk-text-danger psp-monitor-row-status-inner\" title=\"Down\"><span class=\"dot is-error\" aria-hidden=\"true\"></span><span class=\"uk-visible@s\">Down</span></div></div></div></div></div><div class=\"psp-monitor-row-status uk-visible@s\"><div class=\"uk-text-danger psp-monitor-row-status-inner\" title=\"Down\"><span class=\"dot is-error\" aria-hidden=\"true\"></span><span class=\"uk-visible@s\">Down</span></div></div></div>";
        assert!(
            is_aur_down_in_monitors(html_actual),
            "Should detect AUR Down in actual website HTML structure"
        );

        // Test AUR Down with >down< pattern
        let html1 = "<html><body><div>Monitors</div><div>AUR</div><div>>Down<</div></body></html>";
        assert!(is_aur_down_in_monitors(html1));

        // Test AUR Down with "down" pattern
        let html2 =
            "<html><body><div>Monitors</div><div>AUR</div><div>\"Down\"</div></body></html>";
        assert!(is_aur_down_in_monitors(html2));

        // Test AUR Down with 'down' pattern
        let html3 = "<html><body><div>Monitors</div><div>AUR</div><div>'Down'</div></body></html>";
        assert!(is_aur_down_in_monitors(html3));

        // Test AUR Operational (should return false)
        let html4 =
            "<html><body><div>Monitors</div><div>AUR</div><div>Operational</div></body></html>";
        assert!(!is_aur_down_in_monitors(html4));

        // Test no monitors section (should return false)
        let html5 = "<html><body><div>AUR</div><div>Down</div></body></html>";
        assert!(!is_aur_down_in_monitors(html5));
    }
}
