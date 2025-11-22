use crate::state::ArchStatusColor;

/// What: Check for "Status: Arch systems..." pattern and detect if "Some Systems down" or "Down" is shown.
///
/// Inputs:
/// - `body`: Raw HTML content to analyze.
///
/// Output:
/// - `Some(ArchStatusColor)` if "Some Systems down" or "Down" is detected in the status context, `None` otherwise.
///
/// Details: Searches for the "Status: Arch systems..." pattern and checks if it's followed by "Some Systems down" or "Down" indicators.
/// Also checks for "Some systems down" as a standalone heading/text and the monitors section for individual systems showing "Down" status.
pub(super) fn extract_arch_systems_status_color(body: &str) -> Option<ArchStatusColor> {
    let lowered = body.to_lowercase();

    let mut found_severe = false;
    let mut found_moderate = false;

    // First, check for "Some systems down" as a standalone heading/text (most common case)
    // This appears as a heading on the status page when systems are down
    if lowered.contains("some systems down") {
        found_severe = true;
    }

    // Look for "Status: Arch systems..." pattern (case-insensitive)
    let status_patterns = ["status: arch systems", "status arch systems"];

    // Check for overall status message pattern
    for pattern in status_patterns.iter() {
        if let Some(pos) = lowered.find(pattern) {
            // Check in a window around the pattern (500 chars after)
            let window_start = pos;
            let window_end = std::cmp::min(body.len(), pos + pattern.len() + 500);
            let window = &lowered[window_start..window_end];

            // Check for "Some Systems down" or "Down" in the context
            if window.contains("some systems down") {
                found_severe = true;
            } else if window.contains("down") {
                // Only treat as incident if not part of "operational" or "all systems operational"
                if !window.contains("all systems operational") && !window.contains("operational") {
                    found_moderate = true;
                }
            }
        }
    }

    // Also check the monitors section for "Down" status
    // Look for the monitors section and check if any monitor shows "Down"
    if let Some(monitors_pos) = lowered.find("monitors") {
        let monitors_window_start = monitors_pos;
        let monitors_window_end = std::cmp::min(body.len(), monitors_pos + 5000);
        let monitors_window = &lowered[monitors_window_start..monitors_window_end];

        // Check if "Down" appears in the monitors section (but not as part of "operational")
        // Look for patterns like ">Down<" or "Down" in quotes, indicating a status
        if monitors_window.contains("down") {
            // More specific check: look for "Down" that appears to be a status indicator
            // This avoids false positives from words containing "down" like "download"
            // Check for HTML patterns that indicate status: >down< or "down" or 'down'
            if monitors_window.contains(">down<")
                || monitors_window.contains("\"down\"")
                || monitors_window.contains("'down'")
            {
                // Verify it's not part of "operational" or other positive status
                if !monitors_window.contains("operational") {
                    found_moderate = true;
                }
            }

            // Check for "Some Systems down" in monitors context
            if monitors_window.contains("some systems down") {
                found_severe = true;
            }
        }
    }

    if found_severe {
        Some(ArchStatusColor::IncidentSevereToday)
    } else if found_moderate {
        Some(ArchStatusColor::IncidentToday)
    } else {
        None
    }
}

/// What: Choose the more severe `ArchStatusColor` between two candidates.
///
/// Input: Two color severities `a` and `b`.
/// Output: The color with higher impact according to predefined ordering.
///
/// Details: Converts colors to integer ranks and returns the higher rank so callers can merge
/// multiple heuristics without under-reporting incidents.
pub(super) fn severity_max(a: ArchStatusColor, b: ArchStatusColor) -> ArchStatusColor {
    fn rank(c: ArchStatusColor) -> u8 {
        match c {
            ArchStatusColor::None => 0,
            ArchStatusColor::Operational => 1,
            ArchStatusColor::IncidentToday => 2,
            ArchStatusColor::IncidentSevereToday => 3,
        }
    }
    if rank(a) >= rank(b) { a } else { b }
}

/// Return today's UTC date as (year, month, day) using the system `date` command.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Some((year, month, day))` on success; `None` if the conversion fails.
pub(super) fn today_ymd_utc() -> Option<(i32, u32, u32)> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();

    // Convert Unix timestamp to UTC date using a simple algorithm
    // This is a simplified version that works for dates from 1970 onwards
    let days_since_epoch = now / 86400;

    // Calculate year, month, day from days since epoch
    // Using a simple approximation (not accounting for leap seconds, but good enough for our use case)
    let mut year = 1970;
    let mut days = days_since_epoch;

    // Account for leap years
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    // Calculate month and day
    let days_in_month = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month: u32 = 1;
    let mut day: u64 = days;

    for &days_in_m in days_in_month.iter() {
        if day < days_in_m as u64 {
            break;
        }
        day -= days_in_m as u64;
        month += 1;
    }

    Some((year, month, day as u32 + 1)) // +1 because day is 0-indexed
}

/// What: Determine whether a given year is a leap year in the Gregorian calendar.
///
/// Input: Four-digit year as signed integer.
/// Output: `true` when the year has 366 days; `false` otherwise.
///
/// Details: Applies the standard divisible-by-4 rule with century and 400-year exceptions.
#[inline]
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Attempt to extract today's AUR uptime percentage from the Arch status page HTML.
/// Heuristic-based parsing: look near "Uptime Last 90 days" → "Monitors (default)" → "AUR",
/// then find today's date string and the closest percentage like "97%" around it.
/// Heuristically extract today's AUR uptime percentage from status HTML.
///
/// Inputs:
/// - `body`: Full HTML body string
///
/// Output:
/// - `Some(percent)` like 97 for today's cell; `None` if not found.
pub(super) fn extract_aur_today_percent(body: &str) -> Option<u32> {
    let (year, month, day) = today_ymd_utc()?;
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
    let month_name = months.get((month.saturating_sub(1)) as usize)?;
    let date_str = format!("{month_name} {day}, {year}");
    let lowered = body.to_lowercase();

    // Narrow down to the 90-day uptime monitors section around AUR
    let base = "uptime last 90 days";
    let mut base_pos = lowered.find(base)?;
    if let Some(p) = lowered[base_pos..].find("monitors (default)") {
        base_pos += p;
    }
    if let Some(p) = lowered[base_pos..].find("aur") {
        base_pos += p;
    }
    let region_end = std::cmp::min(body.len(), base_pos.saturating_add(4000));
    let region = &body[base_pos..region_end];
    let region_lower = region.to_lowercase();

    let date_pos = region_lower.find(&date_str.to_lowercase())?;
    // Search in a small window around the date for a percentage like "97%"
    let win_start = date_pos.saturating_sub(120);
    let win_end = std::cmp::min(region_lower.len(), date_pos + 160);
    let window = &region_lower[win_start..win_end];
    // Find the first '%' closest to the date by scanning forward from date_pos within the window
    // Prefer after-date occurrences; if none, fall back to before-date occurrences
    let after_slice = &window[(date_pos - win_start)..];
    if let Some(rel_idx) = after_slice.find('%') {
        let abs_idx = win_start + (date_pos - win_start) + rel_idx;
        if let Some(p) = digits_before_percent(&region_lower, abs_idx) {
            return p.parse::<u32>().ok();
        }
    }
    // Fallback: search any percentage within the window
    if let Some(rel_idx) = window.find('%') {
        let abs_idx = win_start + rel_idx;
        if let Some(p) = digits_before_percent(&region_lower, abs_idx) {
            return p.parse::<u32>().ok();
        }
    }
    None
}

/// Collect up to 3 digits immediately preceding a '%' at `pct_idx` in `s`.
///
/// Inputs:
/// - `s`: Source string (typically a lowercase HTML slice)
/// - `pct_idx`: Index of the '%' character in `s`
///
/// Output:
/// - `Some(String)` containing the digits if present; otherwise `None`.
fn digits_before_percent(s: &str, pct_idx: usize) -> Option<String> {
    if pct_idx == 0 || pct_idx > s.len() {
        return None;
    }
    let mut i = pct_idx.saturating_sub(1);
    let bytes = s.as_bytes();
    let mut digits: Vec<u8> = Vec::new();
    // Collect up to 3 digits directly preceding '%'
    for _ in 0..3 {
        if i < s.len() && bytes[i].is_ascii_digit() {
            digits.push(bytes[i]);
            if i == 0 {
                break;
            }
            i = i.saturating_sub(1);
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return None;
    }
    digits.reverse();
    let s = String::from_utf8(digits).ok()?;
    Some(s)
}

/// What: Infer today's AUR uptime color from the SVG heatmap on the status page.
///
/// Input: Full HTML string captured from status.archlinux.org.
/// Output: `Some(ArchStatusColor)` when a nearby `<rect>` fill value maps to a known palette; `None` otherwise.
///
/// Details: Focuses on the "Uptime Last 90 days" AUR section, locates today's date label, then scans
/// surrounding SVG rectangles for Tailwind-like fill colors that indicate green/yellow/red severity.
pub(super) fn extract_aur_today_rect_color(body: &str) -> Option<ArchStatusColor> {
    let (year, month, day) = today_ymd_utc()?;
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
    let month_name = months.get((month.saturating_sub(1)) as usize)?;
    let date_str = format!("{month_name} {day}, {year}");
    let lowered = body.to_lowercase();

    // Limit to the AUR monitors section
    let base = "uptime last 90 days";
    let mut base_pos = lowered.find(base)?;
    if let Some(p) = lowered[base_pos..].find("monitors (default)") {
        base_pos += p;
    }
    if let Some(p) = lowered[base_pos..].find("aur") {
        base_pos += p;
    }
    let region_end = std::cmp::min(body.len(), base_pos.saturating_add(6000));
    let region = &body[base_pos..region_end];
    let region_lower = region.to_lowercase();
    let date_pos = region_lower.find(&date_str.to_lowercase())?;

    // Look around the date for the nearest preceding <rect ... fill="...">
    let head = &region_lower[..date_pos];
    if let Some(rect_pos) = head.rfind("<rect") {
        // Extract attributes between <rect and the next '>' (bounded)
        let tail = &region_lower[rect_pos..std::cmp::min(region_lower.len(), rect_pos + 400)];
        if let Some(fill_idx) = tail.find("fill=") {
            let after = &tail[fill_idx + 5..]; // skip 'fill='
            // Accept values like "#f59e0b" or 'rgb(245 158 11)'
            // Strip any leading quotes
            let after = after.trim_start_matches(' ');
            let quote = after.chars().next().unwrap_or('"');
            let after = if quote == '"' || quote == '\'' {
                &after[1..]
            } else {
                after
            };
            let value: String = after
                .chars()
                .take_while(|&c| c != '"' && c != '\'' && c != ' ' && c != '>')
                .collect();
            let v = value.to_lowercase();
            // Common tailwind/statuspage palette guesses
            if v.contains("#10b981") || v.contains("rgb(16 185 129)") {
                return Some(ArchStatusColor::Operational);
            }
            if v.contains("#f59e0b") || v.contains("rgb(245 158 11)") || v.contains("#fbbf24") {
                return Some(ArchStatusColor::IncidentToday);
            }
            if v.contains("#ef4444") || v.contains("rgb(239 68 68)") || v.contains("#dc2626") {
                return Some(ArchStatusColor::IncidentSevereToday);
            }
        }
    }
    // Fallback: look forward as well (rect could trail the label)
    let tail = &region_lower[date_pos..std::cmp::min(region_lower.len(), date_pos + 400)];
    if let Some(rect_rel) = tail.find("<rect") {
        let start = date_pos + rect_rel;
        let slice = &region_lower[start..std::cmp::min(region_lower.len(), start + 400)];
        if let Some(fill_idx) = slice.find("fill=") {
            let after = &slice[fill_idx + 5..];
            let after = after.trim_start_matches(' ');
            let quote = after.chars().next().unwrap_or('"');
            let after = if quote == '"' || quote == '\'' {
                &after[1..]
            } else {
                after
            };
            let value: String = after
                .chars()
                .take_while(|&c| c != '"' && c != '\'' && c != ' ' && c != '>')
                .collect();
            let v = value.to_lowercase();
            if v.contains("#10b981") || v.contains("rgb(16 185 129)") {
                return Some(ArchStatusColor::Operational);
            }
            if v.contains("#f59e0b") || v.contains("rgb(245 158 11)") || v.contains("#fbbf24") {
                return Some(ArchStatusColor::IncidentToday);
            }
            if v.contains("#ef4444") || v.contains("rgb(239 68 68)") || v.contains("#dc2626") {
                return Some(ArchStatusColor::IncidentSevereToday);
            }
        }
    }
    None
}

/// What: Derive today's AUR severity from the textual "Status updates" section.
///
/// Input: Raw status page HTML used for keyword scanning.
/// Output: `Some(ArchStatusColor)` when today's entry references the AUR with notable keywords; `None` otherwise.
///
/// Details: Narrows to the status updates block, finds today's date string, and searches a sliding window for
/// AUR mentions coupled with severe or moderate keywords to upgrade incident severity heuristics.
pub(super) fn extract_status_updates_today_color(body: &str) -> Option<ArchStatusColor> {
    let (year, month, day) = today_ymd_utc()?;
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
    let month_name = months.get((month.saturating_sub(1)) as usize)?;
    let date_str = format!("{month_name} {day}, {year}");
    let lowered = body.to_lowercase();

    // Find the "Status updates" section
    let base = "status updates";
    let mut base_pos = lowered.find(base)?;

    // Look for "Last 30 days" or "Last 7 days" or similar
    let days_patterns = ["last 30 days", "last 7 days", "last 14 days"];
    for pattern in days_patterns.iter() {
        if let Some(p) = lowered[base_pos..].find(pattern) {
            base_pos += p;
            break;
        }
    }

    // Search for today's date in the status updates section
    // Look in a reasonable window (up to 10000 chars after "Status updates")
    let region_end = std::cmp::min(body.len(), base_pos.saturating_add(10000));
    let region = &body[base_pos..region_end];
    let region_lower = region.to_lowercase();

    // Find today's date in the region
    let date_pos = region_lower.find(&date_str.to_lowercase())?;

    // Look for keywords in a window around today's date (500 chars before and after)
    let window_start = date_pos.saturating_sub(500);
    let window_end = std::cmp::min(region_lower.len(), date_pos + 500);
    let window = &region_lower[window_start..window_end];

    // Keywords that indicate problems
    let severe_keywords = [
        "outage",
        "down",
        "unavailable",
        "offline",
        "failure",
        "critical",
        "major incident",
    ];
    let moderate_keywords = [
        "degraded",
        "slow",
        "intermittent",
        "issues",
        "problems",
        "maintenance",
        "partial",
    ];

    // Check if AUR is mentioned in the context
    let mentions_aur = window.contains("aur") || window.contains("arch user repository");

    if !mentions_aur {
        return None; // Not AUR-related
    }

    // Check for severe keywords
    for keyword in severe_keywords.iter() {
        if window.contains(keyword) {
            return Some(ArchStatusColor::IncidentSevereToday);
        }
    }

    // Check for moderate keywords
    for keyword in moderate_keywords.iter() {
        if window.contains(keyword) {
            return Some(ArchStatusColor::IncidentToday);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Detect "Status: Arch systems..." pattern with "Some Systems down" or "Down" indicators.
    ///
    /// Inputs:
    /// - HTML snippets containing "Status: Arch systems..." followed by "Some Systems down" or "Down" status.
    /// - HTML with monitors section showing "Down" status.
    ///
    /// Output:
    /// - Returns `ArchStatusColor::IncidentSevereToday` for "Some Systems down", `IncidentToday` for "Down", `None` when no issues detected.
    ///
    /// Details:
    /// - Verifies the function correctly identifies status patterns and monitor down indicators while avoiding false positives.
    fn status_extract_arch_systems_status_color() {
        // Test "Some Systems down" pattern
        let html_severe =
            "<html><body><div>Status: Arch systems Some Systems down</div></body></html>";
        let color = extract_arch_systems_status_color(html_severe);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Down" pattern in status context
        let html_moderate = "<html><body><div>Status: Arch systems Down</div></body></html>";
        let color = extract_arch_systems_status_color(html_moderate);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section
        let html_monitors_down = "<html><body><div>Monitors (default)</div><div>AUR</div><div>Down</div></body></html>";
        let color = extract_arch_systems_status_color(html_monitors_down);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section with HTML tags
        let html_monitors_html =
            "<html><body><div>Monitors</div><div>>Down<</div></body></html>";
        let color = extract_arch_systems_status_color(html_monitors_html);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section with quotes
        let html_monitors_quotes =
            "<html><body><div>Monitors</div><div>\"Down\"</div></body></html>";
        let color = extract_arch_systems_status_color(html_monitors_quotes);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Some Systems down" in monitors section
        let html_monitors_severe =
            "<html><body><div>Monitors</div><div>Some Systems down</div></body></html>";
        let color = extract_arch_systems_status_color(html_monitors_severe);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test no issues (should return None)
        let html_ok = "<html><body><div>Status: Arch systems Operational</div><div>Monitors</div><div>Operational</div></body></html>";
        let color = extract_arch_systems_status_color(html_ok);
        assert_eq!(color, None);

        // Test "Down" but with "operational" context (should not trigger false positive)
        let html_operational = "<html><body><div>Status: Arch systems Operational</div><div>Monitors</div><div>All systems operational</div></body></html>";
        let color = extract_arch_systems_status_color(html_operational);
        assert_eq!(color, None);

        // Test case-insensitive matching
        let html_lowercase =
            "<html><body><div>status: arch systems some systems down</div></body></html>";
        let color = extract_arch_systems_status_color(html_lowercase);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Down" without colon (alternative pattern)
        let html_no_colon = "<html><body><div>Status Arch systems Down</div></body></html>";
        let color = extract_arch_systems_status_color(html_no_colon);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Some systems down" as a standalone heading (actual status page format)
        let html_standalone_heading =
            "<html><body><h2>Some systems down</h2><div>Monitors</div></body></html>";
        let color = extract_arch_systems_status_color(html_standalone_heading);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Some systems down" in body text
        let html_body_text = "<html><body><div>Some systems down</div></body></html>";
        let color = extract_arch_systems_status_color(html_body_text);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));
    }
}
