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
    let url = "https://status.archlinux.org";
    let body = tokio::task::spawn_blocking(move || super::curl_text(url)).await??;

    // Also try to read the AUR homepage where outage/maintenance banners are posted.
    let aur_body_opt: Option<String> = match tokio::task::spawn_blocking(|| {
        super::curl_text("https://aur.archlinux.org/")
    })
    .await
    {
        Ok(Ok(s)) => Some(s),
        _ => None,
    };

    let (mut text, mut color) = parse_arch_status_from_html(&body);

    // Categorize any banner present on either page and raise severity/append suffix.
    let aur_pct_opt = extract_aur_today_percent(&body);
    let cat = aur_body_opt
        .as_deref()
        .and_then(categorize_aur_banner)
        .or_else(|| categorize_aur_banner(&body));
    if let Some(cat) = cat {
        let mut suggested = category_base_color(&cat);
        // If DDoS and uptime <90%, escalate to red.
        if matches!(cat, AurBannerCategory::DdosProtection)
            && aur_pct_opt.map(|p| p < 90).unwrap_or(false)
        {
            suggested = ArchStatusColor::IncidentSevereToday;
        }
        color = severity_max(color, suggested);
        let suffix = category_suffix(&cat);
        if !text.to_lowercase().contains(&suffix.to_lowercase()) {
            text = format!("{text} {suffix}");
        }
    }

    Ok((text, color))
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
                forced_color,
            );
        }
        if has_all_ok {
            return (
                format!("All systems operational{aur_pct_suffix}"),
                // Outage announcement present: if we can't parse a color, default to yellow
                aur_color_from_rect
                    .or(aur_color_from_pct)
                    .unwrap_or(ArchStatusColor::IncidentToday),
            );
        }
        return (
            format!("Arch systems nominal{aur_pct_suffix}"),
            // Outage announcement present: if we can't parse a color, default to yellow
            aur_color_from_rect
                .or(aur_color_from_pct)
                .unwrap_or(ArchStatusColor::IncidentToday),
        );
    }

    if has_all_ok {
        return (
            format!("All systems operational{aur_pct_suffix}"),
            aur_color_from_rect
                .or(aur_color_from_pct)
                .unwrap_or(ArchStatusColor::Operational),
        );
    }

    (
        format!("Arch systems nominal{aur_pct_suffix}"),
        aur_color_from_rect
            .or(aur_color_from_pct)
            .unwrap_or(ArchStatusColor::None),
    )
}

/// Heuristically detect whether the provided HTML/text contains a DDoS-related banner/message.
//

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
    if t.contains("outage") || t.contains("unreachable") || t.contains("service interruption") {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Parse Arch status HTML to derive AUR color by % and outage
    ///
    /// - Input: Synthetic HTML around today's date with 97/95/89% and outage flag
    /// - Output: Green for >95, Yellow for 90-95, Red for <90; outage forces >= Yellow
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
    /// What: Prefer SVG rect fill color over percentage when present
    ///
    /// - Input: HTML with greenish % but rect fill set to yellow near today
    /// - Output: Yellow color classification
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
}

/// Return today's UTC date as (year, month, day) using the system `date` command.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Some((year, month, day))` on success; `None` if the command is unavailable or parsing fails.
fn today_ymd_utc() -> Option<(i32, u32, u32)> {
    let out = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim();
    let mut it = s.split('-');
    let y = it.next()?.parse::<i32>().ok()?;
    let m = it.next()?.parse::<u32>().ok()?;
    let d = it.next()?.parse::<u32>().ok()?;
    Some((y, m, d))
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

/// Attempt to extract today's AUR cell color from the SVG rect fill.
/// Returns a color classification if we can find a nearby <rect ... fill="#..."> around today's date.
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
