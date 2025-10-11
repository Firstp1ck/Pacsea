use crate::state::ArchStatusColor;

type Result<T> = super::Result<T>;

pub async fn fetch_arch_status_text() -> Result<(String, ArchStatusColor)> {
    let url = "https://status.archlinux.org";
    let body = tokio::task::spawn_blocking(move || super::curl_text(url)).await??;
    let lowered = body.to_lowercase();
    let has_all_ok = lowered.contains("all systems operational");

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
            return Ok((
                "AUR outage (see status)".to_string(),
                ArchStatusColor::IncidentToday,
            ));
        }
        if has_all_ok {
            return Ok((
                "All systems operational".to_string(),
                ArchStatusColor::Operational,
            ));
        }
        return Ok(("Arch systems nominal".to_string(), ArchStatusColor::None));
    }

    if has_all_ok {
        return Ok((
            "All systems operational".to_string(),
            ArchStatusColor::Operational,
        ));
    }

    Ok(("Arch systems nominal".to_string(), ArchStatusColor::None))
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
