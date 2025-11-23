#![allow(dead_code)]
/// What: Return today's UTC date as (year, month, day) using Rust's standard library.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Some((year, month, day))` when available; `None` if the conversion fails.
#[cfg_attr(not(test), allow(dead_code))]
pub fn today_ymd_utc() -> Option<(i32, u32, u32)> {
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

    for &days_in_m in &days_in_month {
        if day < days_in_m as u64 {
            break;
        }
        day -= days_in_m as u64;
        month += 1;
    }

    Some((
        year,
        month,
        u32::try_from(day).expect("day fits in u32") + 1,
    )) // +1 because day is 0-indexed
}

const fn is_leap_year(year: i32) -> bool {
    // What: Determine whether a Gregorian calendar year is a leap year.
    // Inputs: `year` is a four-digit Gregorian year expressed as an i32.
    // Output: true when the year is divisible by 4 but not 100, unless divisible by 400.
    // Details: Mirrors the Gregorian leap-year rules (same logic chrono uses).
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// What: Parse month name string into month number (1-12).
///
/// Inputs:
/// - `m_s`: Month name string (e.g., "Jan", "January", "Oct", "October").
///
/// Output:
/// - `Some(1..=12)` for recognized month names; `None` otherwise.
///
/// Details:
/// - Supports both abbreviated and full month names.
#[cfg_attr(not(test), allow(dead_code))]
fn parse_month_name(m_s: &str) -> Option<u32> {
    match m_s {
        "Jan" | "January" => Some(1),
        "Feb" | "February" => Some(2),
        "Mar" | "March" => Some(3),
        "Apr" | "April" => Some(4),
        "May" => Some(5),
        "Jun" | "June" => Some(6),
        "Jul" | "July" => Some(7),
        "Aug" | "August" => Some(8),
        "Sep" | "Sept" | "September" => Some(9),
        "Oct" | "October" => Some(10),
        "Nov" | "November" => Some(11),
        "Dec" | "December" => Some(12),
        _ => None,
    }
}

/// What: Parse various short Arch news date formats into `(year, month, day)`.
///
/// Inputs:
/// - `s`: Input date string. Supported formats include:
///   - ISO: `YYYY-MM-DD`
///   - RFC-like: `Sat, 05 Oct 2024`
///   - Short: `05 Oct 2024` or `05 October 2024`
///
/// Output:
/// - `Some((y, m, d))` for recognized and valid dates; `None` otherwise.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::many_single_char_names)]
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
    let m = parse_month_name(m_s)?;
    Some((y, m, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Parse various Arch news date string formats into `(year, month, day)` tuples.
    ///
    /// Inputs:
    /// - Sample ISO date, RFC-like header, short `DD Mon YYYY` form, and invalid strings.
    ///
    /// Output:
    /// - Returns `Some((2024, 10, 5))` for valid formats and `None` for malformed dates.
    ///
    /// Details:
    /// - Confirms month alias handling and basic range validation for day/month values.
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
