use crate::state::ArchStatusColor;

use super::utils::{severity_max, today_ymd_utc};

/// Parse the UptimeRobot API response to extract the worst status among all monitors (AUR, Forum, Website, Wiki).
///
/// Inputs:
/// - `v`: JSON value from https://status.archlinux.org/api/getMonitorList/vmM5ruWEAB
///
/// Output:
/// - `Some((text, color))` if monitor data is found, `None` otherwise.
///   Always returns the worst (lowest uptime) status among all monitored services.
pub(super) fn parse_uptimerobot_api(v: &serde_json::Value) -> Option<(String, ArchStatusColor)> {
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
pub(super) fn parse_status_api_summary(
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
