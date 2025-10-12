use crate::state::ArchStatusColor;

type Result<T> = super::Result<T>;

pub async fn fetch_arch_status_text() -> Result<(String, ArchStatusColor)> {
    let url = "https://status.archlinux.org";
    let body = tokio::task::spawn_blocking(move || super::curl_text(url)).await??;
    Ok(parse_arch_status_from_html(&body))
}

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
