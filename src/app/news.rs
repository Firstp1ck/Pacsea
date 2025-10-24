/// Return today's UTC date as (year, month, day) using the system `date` command.
pub fn today_ymd_utc() -> Option<(i32, u32, u32)> {
    let out = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%d"]) // e.g., 2025-10-11
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

/// Try to parse various short date formats used by Arch RSS into (Y,M,D).
pub fn parse_news_date_to_ymd(s: &str) -> Option<(i32, u32, u32)> {
    let t = s.trim();
    // Case 1: ISO: YYYY-MM-DD
    if t.len() >= 10 && t.as_bytes().get(4) == Some(&b'-') && t.as_bytes().get(7) == Some(&b'-') {
        let y = t[0..4].parse::<i32>().ok()?;
        let m = t[5..7].parse::<u32>().ok()?;
        let d = t[8..10].parse::<u32>().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        return Some((y, m, d));
    }
    // Case 2: "Sat, 05 Oct 2024" or "05 Oct 2024"
    let part = if let Some((_, rhs)) = t.split_once(',') {
        rhs.trim()
    } else {
        t
    };
    let mut it = part.split_whitespace();
    let d_s = it.next()?; // e.g., 05
    let m_s = it.next()?; // e.g., Oct
    let y_s = it.next()?; // e.g., 2024
    let d = d_s.parse::<u32>().ok()?;
    if !(1..=31).contains(&d) {
        return None;
    }
    let y = y_s.parse::<i32>().ok()?;
    let m = match m_s {
        "Jan" | "January" => 1,
        "Feb" | "February" => 2,
        "Mar" | "March" => 3,
        "Apr" | "April" => 4,
        "May" => 5,
        "Jun" | "June" => 6,
        "Jul" | "July" => 7,
        "Aug" | "August" => 8,
        "Sep" | "Sept" | "September" => 9,
        "Oct" | "October" => 10,
        "Nov" | "November" => 11,
        "Dec" | "December" => 12,
        _ => return None,
    };
    Some((y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Parse various Arch news date formats into (Y,M,D)
    ///
    /// - Input: ISO "2024-10-05", RFC-like "Sat, 05 Oct 2024", and "05 Oct 2024"
    /// - Output: Some((2024,10,5)) for all supported formats; None for invalid
    fn parse_news_date_variants() {
        assert_eq!(parse_news_date_to_ymd("2024-10-05"), Some((2024, 10, 5)));
        assert_eq!(
            parse_news_date_to_ymd("Sat, 05 Oct 2024"),
            Some((2024, 10, 5))
        );
        assert_eq!(parse_news_date_to_ymd("05 Oct 2024"), Some((2024, 10, 5)));
        assert_eq!(
            parse_news_date_to_ymd("05 October 2024"),
            Some((2024, 10, 5))
        );
        assert_eq!(parse_news_date_to_ymd("05 Sept 2024"), Some((2024, 9, 5)));
        assert_eq!(parse_news_date_to_ymd("not a date"), None);
        assert_eq!(parse_news_date_to_ymd("2024-13-40"), None);
    }
}
