use crate::state::ArchStatusColor;

mod api;
mod html;
mod utils;

use api::{parse_status_api_summary, parse_uptimerobot_api};
use html::{is_aur_down_in_monitors, parse_arch_status_from_html};
use utils::{extract_aur_today_percent, extract_aur_today_rect_color, severity_max};

type Result<T> = super::Result<T>;

/// Fetch a short status text and color indicator from status.archlinux.org.
///
/// Inputs: none
///
/// Output:
/// - `Ok((text, color))` where `text` summarizes current status and `color` indicates severity.
/// - `Err` on network or parse failures.
///
/// # Errors
/// - Returns `Err` when network request fails (curl execution error)
/// - Returns `Err` when status API response cannot be fetched or parsed
/// - Returns `Err` when task spawn fails
///
pub async fn fetch_arch_status_text() -> Result<(String, ArchStatusColor)> {
    // 1) Prefer the official Statuspage API (reliable for active incidents and component states)
    let api_url = "https://status.archlinux.org/api/v2/summary.json";
    let api_result =
        tokio::task::spawn_blocking(move || crate::util::curl::curl_json(api_url)).await;

    if let Ok(Ok(v)) = api_result {
        let (mut text, mut color, suffix) = parse_status_api_summary(&v);

        // Always fetch HTML to check the visual indicator (rect color/beam) which may differ from API status
        if let Ok(Ok(html)) = tokio::task::spawn_blocking(|| {
            crate::util::curl::curl_text("https://status.archlinux.org")
        })
        .await
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
                                .is_some_and(|n| n.to_lowercase().contains("aur"))
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
        tokio::task::spawn_blocking(move || crate::util::curl::curl_json(uptimerobot_api_url))
            .await;

    if let Ok(Ok(v)) = uptimerobot_result
        && let Some((mut text, mut color)) = parse_uptimerobot_api(&v)
    {
        // Also fetch HTML to check if AUR specifically shows "Down" status
        // This takes priority over API response
        if let Ok(Ok(html)) = tokio::task::spawn_blocking(|| {
            crate::util::curl::curl_text("https://status.archlinux.org")
        })
        .await
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
    let body = tokio::task::spawn_blocking(move || crate::util::curl::curl_text(url)).await??;

    // Skip AUR homepage keyword heuristic to avoid false outage flags

    let (text, color) = parse_arch_status_from_html(&body);
    // Heuristic banner scan disabled in fallback to avoid false positives.

    Ok((text, color))
}
