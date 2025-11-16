use crate::state::ArchStatusColor;

type Result<T> = super::Result<T>;

/// Fetch a short status text and color indicator from status.archlinux.org.
///
/// Inputs: none
///
/// Output:
/// - `Ok((text, color))` where `text` summarizes current status and `color` indicates severity.
/// - `Err` on network or parse failures.
pub async fn fetch_arch_status_text() -> Result<(String, ArchStatusColor)> {
    // 1) Prefer the official Statuspage API (reliable for active incidents and component states)
    let api_url = "https://status.archlinux.org/api/v2/summary.json";
    let api_result = tokio::task::spawn_blocking(move || super::curl_json(api_url)).await;

    if let Ok(Ok(v)) = api_result {
        let (mut text, mut color, suffix) = parse_status_api_summary(&v);

        // Always fetch HTML to check the visual indicator (rect color/beam) which may differ from API status
        if let Ok(Ok(html)) =
            tokio::task::spawn_blocking(|| super::curl_text("https://status.archlinux.org")).await
        {
            // FIRST PRIORITY: Check if AUR specifically shows "Down" status in monitors section
            // This must be checked before anything else as it's the most specific indicator
            if is_aur_down_in_monitors(&html) {
                let aur_pct_opt = extract_aur_today_percent(&html);
                let aur_pct_suffix = aur_pct_opt
                    .map(|p| format!(" — AUR today: {p}%"))
                    .unwrap_or_default();
                let text = format!("Status: AUR Down{aur_pct_suffix}");
                return Ok((text, ArchStatusColor::IncidentSevereToday));
            }

            // Extract today's AUR uptime percentage (best-effort)
            let aur_pct_opt = extract_aur_today_percent(&html);
            if let Some(p) = aur_pct_opt {
                text.push_str(&format!(" — AUR today: {p}%"));
            }

            // Check the visual indicator (rect color/beam) - this is authoritative for current status
            // The beam color can show red/yellow even when API says "operational"
            if let Some(rect_color) = extract_aur_today_rect_color(&html) {
                // If the visual indicator shows a problem but API says operational, trust the visual indicator
                let api_says_operational = matches!(
                    v.get("components")
                        .and_then(|c| c.as_array())
                        .and_then(|arr| arr.iter().find(|c| {
                            c.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n.to_lowercase().contains("aur"))
                                .unwrap_or(false)
                        }))
                        .and_then(|c| c.get("status").and_then(|s| s.as_str())),
                    Some("operational")
                );

                if api_says_operational
                    && matches!(
                        rect_color,
                        ArchStatusColor::IncidentToday | ArchStatusColor::IncidentSevereToday
                    )
                {
                    // Visual indicator shows a problem but API says operational - trust the visual indicator
                    color = rect_color;
                    // Update text to reflect the visual indicator discrepancy
                    let text_lower = text.to_lowercase();
                    let pct_suffix = if text_lower.contains("aur today") {
                        String::new() // Already added
                    } else {
                        aur_pct_opt
                            .map(|p| format!(" — AUR today: {p}%"))
                            .unwrap_or_default()
                    };
                    match rect_color {
                        ArchStatusColor::IncidentSevereToday => {
                            if !text_lower.contains("outage") && !text_lower.contains("issues") {
                                text = format!("AUR issues detected (see status){pct_suffix}");
                            }
                        }
                        ArchStatusColor::IncidentToday => {
                            if !text_lower.contains("degraded")
                                && !text_lower.contains("outage")
                                && !text_lower.contains("issues")
                            {
                                text = format!("AUR degraded (see status){pct_suffix}");
                            }
                        }
                        _ => {}
                    }
                } else {
                    // Use the more severe of API color or rect color
                    color = severity_max(color, rect_color);
                }
            }
        }

        if let Some(sfx) = suffix
            && !text.to_lowercase().contains(&sfx.to_lowercase())
        {
            text = format!("{text} {sfx}");
        }

        return Ok((text, color));
    }

    // 2) Try the UptimeRobot API endpoint (the actual API the status page uses)
    let uptimerobot_api_url = "https://status.archlinux.org/api/getMonitorList/vmM5ruWEAB";
    let uptimerobot_result =
        tokio::task::spawn_blocking(move || super::curl_json(uptimerobot_api_url)).await;

    if let Ok(Ok(v)) = uptimerobot_result
        && let Some((mut text, mut color)) = parse_uptimerobot_api(&v)
    {
        // Also fetch HTML to check if AUR specifically shows "Down" status
        // This takes priority over API response
        if let Ok(Ok(html)) =
            tokio::task::spawn_blocking(|| super::curl_text("https://status.archlinux.org")).await
            && is_aur_down_in_monitors(&html)
        {
            let aur_pct_opt = extract_aur_today_percent(&html);
            let aur_pct_suffix = aur_pct_opt
                .map(|p| format!(" — AUR today: {p}%"))
                .unwrap_or_default();
            text = format!("Status: AUR Down{aur_pct_suffix}");
            color = ArchStatusColor::IncidentSevereToday;
        }
        return Ok((text, color));
    }

    // 3) Fallback: use the existing HTML parser + banner heuristic if APIs are unavailable
    let url = "https://status.archlinux.org";
    let body = tokio::task::spawn_blocking(move || super::curl_text(url)).await??;

    // Skip AUR homepage keyword heuristic to avoid false outage flags

    let (text, color) = parse_arch_status_from_html(&body);
    // Heuristic banner scan disabled in fallback to avoid false positives.

    Ok((text, color))
}

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
fn extract_arch_systems_status_color(body: &str) -> Option<ArchStatusColor> {
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
fn is_aur_down_in_monitors(body: &str) -> bool {
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

        for pattern in aur_patterns.iter() {
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
pub fn parse_arch_status_from_html(body: &str) -> (String, ArchStatusColor) {
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
        'outer: for m in months.iter() {
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
                    let month_idx = months.iter().position(|mm| *mm == *m).unwrap() as u32 + 1;
                    let _year_s = &region[year_start..(year_start + 4)];
                    is_today = tm == month_idx && td == day && ty.to_string() == _year_s;
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

/// Heuristically detect whether the provided HTML/text contains a DDoS-related banner/message.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AurBannerCategory {
    DdosProtection,
    PushDisabled,
    SshUnavailable,
    ScheduledMaintenance,
    Outage,
    RpcDegraded,
    AccountActionsLimited,
    SecurityIncident,
}

/// What: Classify AUR homepage banners into coarse categories.
///
/// Input: Raw banner text to inspect.
/// Output: `Some(AurBannerCategory)` when a known keyword is detected; `None` otherwise.
///
/// Details: Normalizes text to lowercase and checks ordered keyword sets so the most severe
/// scenarios (security incident, outage) are caught before more general matches.
#[allow(dead_code)]
fn categorize_aur_banner(s: &str) -> Option<AurBannerCategory> {
    let t = s.to_lowercase();
    // Order matters: match most severe/explicit first
    if t.contains("security incident")
        || t.contains("compromised package")
        || t.contains("host keys rotated")
        || (t.contains("security") && t.contains("incident"))
    {
        return Some(AurBannerCategory::SecurityIncident);
    }
    // Only match specific phrases indicating a CURRENT outage, not historical mentions
    if t.contains("the aur is currently experiencing an outage")
        || t.contains("aur is currently experiencing an outage")
        || t.contains("currently experiencing an outage")
        || (t.contains("aur")
            && t.contains("currently")
            && (t.contains("unreachable") || t.contains("down")))
    {
        return Some(AurBannerCategory::Outage);
    }
    if t.contains("pushing to the aur currently not possible")
        || t.contains("push disabled")
        || t.contains("uploads disabled")
        || t.contains("submission disabled")
        || t.contains("read-only mode")
    {
        return Some(AurBannerCategory::PushDisabled);
    }
    if t.contains("ddos")
        || t.contains("ddos protection")
        || t.contains("rate limiting")
        || t.contains("429")
    {
        return Some(AurBannerCategory::DdosProtection);
    }
    if t.contains("port 22")
        || t.contains("ssh unavailable")
        || t.contains("git over ssh unavailable")
        || (t.contains("ssh") && t.contains("unavailable"))
    {
        return Some(AurBannerCategory::SshUnavailable);
    }
    if t.contains("maintenance")
        || t.contains("maintenance window")
        || t.contains("down for maintenance")
        || t.contains("scheduled maintenance")
    {
        return Some(AurBannerCategory::ScheduledMaintenance);
    }
    if t.contains("rpc v5 degraded")
        || t.contains("search api degraded")
        || t.contains("slow responses")
        || t.contains("timeouts")
        || t.contains("degraded")
    {
        return Some(AurBannerCategory::RpcDegraded);
    }
    if t.contains("registration disabled")
        || t.contains("login disabled")
        || t.contains("password reset disabled")
        || t.contains("email delivery delayed")
    {
        return Some(AurBannerCategory::AccountActionsLimited);
    }
    None
}

/// What: Map an `AurBannerCategory` to the baseline `ArchStatusColor` severity.
///
/// Input: Banner category deduced from AUR messaging.
/// Output: Color representing minimum severity level to report.
///
/// Details: Groups similar banner classes (e.g., outages, security incidents) to a consistent
/// severity palette so later logic can escalate but never downgrade the reported color.
#[allow(dead_code)]
fn category_base_color(cat: &AurBannerCategory) -> ArchStatusColor {
    match cat {
        AurBannerCategory::SecurityIncident => ArchStatusColor::IncidentSevereToday,
        AurBannerCategory::Outage => ArchStatusColor::IncidentSevereToday,
        AurBannerCategory::PushDisabled => ArchStatusColor::IncidentSevereToday,
        AurBannerCategory::SshUnavailable => ArchStatusColor::IncidentToday,
        AurBannerCategory::ScheduledMaintenance => ArchStatusColor::IncidentToday,
        AurBannerCategory::RpcDegraded => ArchStatusColor::IncidentToday,
        AurBannerCategory::AccountActionsLimited => ArchStatusColor::IncidentToday,
        AurBannerCategory::DdosProtection => ArchStatusColor::IncidentToday,
    }
}

/// What: Provide a short human-readable suffix describing a banner category.
///
/// Input: `AurBannerCategory` classification.
/// Output: Static string appended to status summaries.
///
/// Details: Keeps display text centralized for banner-derived annotations, ensuring consistent
/// phrasing for each category.
#[allow(dead_code)]
fn category_suffix(cat: &AurBannerCategory) -> &'static str {
    match cat {
        AurBannerCategory::DdosProtection => "— AUR DDoS/protection active",
        AurBannerCategory::PushDisabled => "— AUR push disabled (read-only)",
        AurBannerCategory::SshUnavailable => "— SSH unavailable (use HTTPS)",
        AurBannerCategory::ScheduledMaintenance => "— Maintenance ongoing",
        AurBannerCategory::Outage => "— AUR outage",
        AurBannerCategory::RpcDegraded => "— AUR RPC degraded",
        AurBannerCategory::AccountActionsLimited => "— Account actions limited",
        AurBannerCategory::SecurityIncident => "— Security incident (see details)",
    }
}

/// What: Choose the more severe `ArchStatusColor` between two candidates.
///
/// Input: Two color severities `a` and `b`.
/// Output: The color with higher impact according to predefined ordering.
///
/// Details: Converts colors to integer ranks and returns the higher rank so callers can merge
/// multiple heuristics without under-reporting incidents.
fn severity_max(a: ArchStatusColor, b: ArchStatusColor) -> ArchStatusColor {
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

/// Parse the UptimeRobot API response to extract the worst status among all monitors (AUR, Forum, Website, Wiki).
///
/// Inputs:
/// - `v`: JSON value from https://status.archlinux.org/api/getMonitorList/vmM5ruWEAB
///
/// Output:
/// - `Some((text, color))` if monitor data is found, `None` otherwise.
///   Always returns the worst (lowest uptime) status among all monitored services.
fn parse_uptimerobot_api(v: &serde_json::Value) -> Option<(String, ArchStatusColor)> {
    let data = v.get("data")?.as_array()?;

    // Get today's date in YYYY-MM-DD format
    let (year, month, day) = today_ymd_utc()?;
    let today_str = format!("{year}-{month:02}-{day:02}");

    // Monitor names we care about
    let monitor_names = ["AUR", "Forum", "Website", "Wiki"];

    // Collect today's status for all monitors
    let mut monitor_statuses: Vec<(String, f64, &str, &str)> = Vec::new();

    for monitor in data.iter() {
        let name = monitor.get("name")?.as_str()?;
        if !monitor_names.iter().any(|&n| n.eq_ignore_ascii_case(name)) {
            continue;
        }

        let daily_ratios = monitor.get("dailyRatios")?.as_array()?;
        if let Some(today_data) = daily_ratios.iter().find(|d| {
            d.get("date")
                .and_then(|date| date.as_str())
                .map(|date| date == today_str)
                .unwrap_or(false)
        }) {
            let ratio_str = today_data.get("ratio")?.as_str()?;
            if let Ok(ratio) = ratio_str.parse::<f64>() {
                let color_str = today_data.get("color")?.as_str()?;
                let label = today_data.get("label")?.as_str()?;
                monitor_statuses.push((name.to_string(), ratio, color_str, label));
            }
        }
    }

    if monitor_statuses.is_empty() {
        return None;
    }

    // Find the worst status (lowest ratio, or if equal, worst color)
    let worst = monitor_statuses.iter().min_by(|a, b| {
        // First compare by ratio (lower is worse)
        let ratio_cmp = a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal);
        if ratio_cmp != std::cmp::Ordering::Equal {
            return ratio_cmp;
        }
        // If ratios are equal, compare by color severity (red > yellow/blue > green)
        let color_rank_a = match a.2 {
            "red" => 3,
            "yellow" | "blue" => 2,
            "green" => 1,
            _ => 0,
        };
        let color_rank_b = match b.2 {
            "red" => 3,
            "yellow" | "blue" => 2,
            "green" => 1,
            _ => 0,
        };
        color_rank_b.cmp(&color_rank_a) // Reverse because we want worst first
    })?;

    // Find AUR status separately
    let aur_status = monitor_statuses
        .iter()
        .find(|s| s.0.eq_ignore_ascii_case("aur"));

    let (name, ratio, color_str, label) = worst;

    // Map UptimeRobot colors to our ArchStatusColor
    let color = match *color_str {
        "green" => ArchStatusColor::Operational,
        "yellow" | "blue" => ArchStatusColor::IncidentToday,
        "red" => ArchStatusColor::IncidentSevereToday,
        _ => ArchStatusColor::None,
    };

    // Determine text based on ratio, label, and service name
    let mut text = if *ratio < 90.0 {
        format!(
            "{} outage (see status) — {} today: {:.1}%",
            name, name, ratio
        )
    } else if *ratio < 95.0 {
        format!(
            "{} degraded (see status) — {} today: {:.1}%",
            name, name, ratio
        )
    } else if *label == "poor" || *color_str == "red" {
        format!(
            "{} issues detected (see status) — {} today: {:.1}%",
            name, name, ratio
        )
    } else {
        format!("Arch systems nominal — {} today: {:.1}%", name, ratio)
    };

    // Always append AUR status in parentheses if AUR is not the worst service AND AUR has issues
    if let Some((aur_name, aur_ratio, aur_color_str, _)) = aur_status
        && !aur_name.eq_ignore_ascii_case(name)
        && (*aur_ratio < 100.0 || *aur_color_str != "green")
    {
        text.push_str(&format!(" (AUR: {:.1}%)", aur_ratio));
    }

    Some((text, color))
}

/// Parse the Arch Status API summary JSON into a concise status line, color, and optional suffix.
///
/// Inputs:
/// - `v`: JSON value from <https://status.archlinux.org/api/v2/summary.json>
///
/// Output:
/// - `(text, color, suffix)` where `text` is either "All systems operational" or "Arch systems nominal",
///   `color` reflects severity, and `suffix` indicates the AUR component state when not operational.
pub fn parse_status_api_summary(
    v: &serde_json::Value,
) -> (String, ArchStatusColor, Option<String>) {
    // Overall indicator severity
    let indicator = v
        .get("status")
        .and_then(|s| s.get("indicator"))
        .and_then(|i| i.as_str())
        .unwrap_or("none");
    let mut color = match indicator {
        "none" => ArchStatusColor::Operational,
        "minor" => ArchStatusColor::IncidentToday,
        "major" | "critical" => ArchStatusColor::IncidentSevereToday,
        _ => ArchStatusColor::None,
    };

    // AUR component detection and suffix mapping
    let suffix: Option<String> = None;
    let mut aur_state: Option<&str> = None;
    if let Some(components) = v.get("components").and_then(|c| c.as_array())
        && let Some(aur_comp) = components.iter().find(|c| {
            c.get("name")
                .and_then(|n| n.as_str())
                .map(|n| n.to_lowercase().contains("aur"))
                .unwrap_or(false)
        })
        && let Some(state) = aur_comp.get("status").and_then(|s| s.as_str())
    {
        aur_state = Some(state);
        match state {
            "operational" => { /* no suffix */ }
            "degraded_performance" => {
                // Don't set suffix - text will already say "AUR RPC degraded"
                color = severity_max(color, ArchStatusColor::IncidentToday);
            }
            "partial_outage" => {
                // Don't set suffix - text will already say "AUR partial outage"
                color = severity_max(color, ArchStatusColor::IncidentToday);
            }
            "major_outage" => {
                // Don't set suffix - text will already say "AUR outage (see status)"
                color = severity_max(color, ArchStatusColor::IncidentSevereToday);
            }
            "under_maintenance" => {
                // Don't set suffix - text will already say "AUR maintenance ongoing"
                color = severity_max(color, ArchStatusColor::IncidentToday);
            }
            _ => {}
        }
    }

    // If AUR has a non-operational status, prioritize that in the text
    let text = if let Some(state) = aur_state {
        match state {
            "operational" => {
                if indicator == "none" {
                    "All systems operational".to_string()
                } else {
                    "Arch systems nominal".to_string()
                }
            }
            "major_outage" => "AUR outage (see status)".to_string(),
            "partial_outage" => "AUR partial outage".to_string(),
            "degraded_performance" => "AUR RPC degraded".to_string(),
            "under_maintenance" => "AUR maintenance ongoing".to_string(),
            _ => {
                if indicator == "none" {
                    "All systems operational".to_string()
                } else {
                    "Arch systems nominal".to_string()
                }
            }
        }
    } else if indicator == "none" {
        "All systems operational".to_string()
    } else {
        "Arch systems nominal".to_string()
    };

    (text, color, suffix)
}

/// Return today's UTC date as (year, month, day) using the system `date` command.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Some((year, month, day))` on success; `None` if the conversion fails.
fn today_ymd_utc() -> Option<(i32, u32, u32)> {
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
fn extract_aur_today_percent(body: &str) -> Option<u32> {
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
fn extract_aur_today_rect_color(body: &str) -> Option<ArchStatusColor> {
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
fn extract_status_updates_today_color(body: &str) -> Option<ArchStatusColor> {
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
        let html = r#"<html><body><h2>Some systems down</h2><div>Monitors (default)</div><div>AUR</div><div>Operational</div></body></html>"#;
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
                r#"<html><body><h2>Uptime Last 90 days</h2><div>Monitors (default)</div><div>AUR</div><div>{date_str}</div><div>{percent}% uptime</div>{outage_block}</body></html>"#,
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
            r#"<html><body><div>Status: Arch systems Some Systems down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_severe);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Down" pattern in status context
        let html_moderate = r#"<html><body><div>Status: Arch systems Down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_moderate);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section
        let html_monitors_down = r#"<html><body><div>Monitors (default)</div><div>AUR</div><div>Down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_monitors_down);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section with HTML tags
        let html_monitors_html =
            r#"<html><body><div>Monitors</div><div>>Down<</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_monitors_html);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Down" in monitors section with quotes
        let html_monitors_quotes =
            r#"<html><body><div>Monitors</div><div>"Down"</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_monitors_quotes);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Some Systems down" in monitors section
        let html_monitors_severe =
            r#"<html><body><div>Monitors</div><div>Some Systems down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_monitors_severe);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test no issues (should return None)
        let html_ok = r#"<html><body><div>Status: Arch systems Operational</div><div>Monitors</div><div>Operational</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_ok);
        assert_eq!(color, None);

        // Test "Down" but with "operational" context (should not trigger false positive)
        let html_operational = r#"<html><body><div>Status: Arch systems Operational</div><div>Monitors</div><div>All systems operational</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_operational);
        assert_eq!(color, None);

        // Test case-insensitive matching
        let html_lowercase =
            r#"<html><body><div>status: arch systems some systems down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_lowercase);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Down" without colon (alternative pattern)
        let html_no_colon = r#"<html><body><div>Status Arch systems Down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_no_colon);
        assert_eq!(color, Some(ArchStatusColor::IncidentToday));

        // Test "Some systems down" as a standalone heading (actual status page format)
        let html_standalone_heading =
            r#"<html><body><h2>Some systems down</h2><div>Monitors</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_standalone_heading);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));

        // Test "Some systems down" in body text
        let html_body_text = r#"<html><body><div>Some systems down</div></body></html>"#;
        let color = extract_arch_systems_status_color(html_body_text);
        assert_eq!(color, Some(ArchStatusColor::IncidentSevereToday));
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
        let html = r#"<html><body><h2>Some systems down</h2><div>Monitors (default)</div><div>AUR</div><div>>Down<</div></body></html>"#;
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
        let html_title = r#"<html><body><div>Monitors</div><div>AUR</div><div title="Down">Status</div></body></html>"#;
        assert!(is_aur_down_in_monitors(html_title));

        // Test AUR Down with title='Down' pattern
        let html_title_single = r#"<html><body><div>Monitors</div><div>AUR</div><div title='Down'>Status</div></body></html>"#;
        assert!(is_aur_down_in_monitors(html_title_single));

        // Test AUR Down with actual status page HTML structure
        let html_real = r#"<div class="psp-monitor-row"><div>Monitors</div><div>AUR</div><div class="psp-monitor-row-status-inner" title="Down"><span>Down</span></div></div>"#;
        assert!(is_aur_down_in_monitors(html_real));

        // Test with actual website HTML structure (from status.archlinux.org)
        // Must include "Monitors" text for the function to find the monitors section
        let html_actual = r#"<div>Monitors (default)</div><div class="psp-monitor-row"><div class="uk-flex uk-flex-between uk-flex-wrap"><div class="psp-monitor-row-header uk-text-muted uk-flex uk-flex-auto"><a title="AUR" class="psp-monitor-name uk-text-truncate uk-display-inline-block" href="https://status.archlinux.org/788139639">AUR<svg class="icon icon-plus-square uk-flex-none"><use href="/assets/symbol-defs.svg#icon-arrow-right"></use></svg></a><div class="uk-flex-none"><span class="m-r-5 m-l-5 uk-visible@s">|</span><span class="uk-text-primary uk-visible@s">94.864%</span><div class="uk-hidden@s uk-margin-small-left"><div class="uk-text-danger psp-monitor-row-status-inner" title="Down"><span class="dot is-error" aria-hidden="true"></span><span class="uk-visible@s">Down</span></div></div></div></div></div><div class="psp-monitor-row-status uk-visible@s"><div class="uk-text-danger psp-monitor-row-status-inner" title="Down"><span class="dot is-error" aria-hidden="true"></span><span class="uk-visible@s">Down</span></div></div></div>"#;
        assert!(
            is_aur_down_in_monitors(html_actual),
            "Should detect AUR Down in actual website HTML structure"
        );

        // Test AUR Down with >down< pattern
        let html1 =
            r#"<html><body><div>Monitors</div><div>AUR</div><div>>Down<</div></body></html>"#;
        assert!(is_aur_down_in_monitors(html1));

        // Test AUR Down with "down" pattern
        let html2 =
            r#"<html><body><div>Monitors</div><div>AUR</div><div>"Down"</div></body></html>"#;
        assert!(is_aur_down_in_monitors(html2));

        // Test AUR Down with 'down' pattern
        let html3 =
            r#"<html><body><div>Monitors</div><div>AUR</div><div>'Down'</div></body></html>"#;
        assert!(is_aur_down_in_monitors(html3));

        // Test AUR Operational (should return false)
        let html4 =
            r#"<html><body><div>Monitors</div><div>AUR</div><div>Operational</div></body></html>"#;
        assert!(!is_aur_down_in_monitors(html4));

        // Test no monitors section (should return false)
        let html5 = r#"<html><body><div>AUR</div><div>Down</div></body></html>"#;
        assert!(!is_aur_down_in_monitors(html5));
    }
}
