//! Small utility helpers for encoding, JSON extraction, ranking, and time formatting.
//!
//! The functions in this module are intentionally lightweight and dependency-free
//! to keep hot paths fast and reduce compile times. They are used by networking,
//! indexing, and UI code.
use serde_json::Value;

/// Percent-encode a string for use in URLs.
///
/// Encoding rules:
///
/// - Unreserved characters as per RFC 3986 (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`)
///   are left as-is.
/// - Space is encoded as `%20` (not `+`).
/// - All other bytes are encoded as two uppercase hexadecimal digits prefixed by `%`.
///
/// The function operates on raw bytes from the input string. Any non-ASCII bytes
/// are hex-escaped.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push_str("%20"),
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
    out
}

/// Extract a string value from a JSON object by key, defaulting to empty string.
///
/// Returns `""` if the key is missing or not a string.
pub fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
/// Extract the first available string from a list of candidate keys.
///
/// Returns `Some(String)` for the first key that maps to a JSON string, or `None`
/// if none match.
pub fn ss(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            return Some(s.to_owned());
        }
    }
    None
}
/// Extract an array of strings from a JSON object by trying keys in order.
///
/// Returns the first found array as `Vec<String>`, filtering out non-string elements.
/// If no array of strings is found, returns an empty vector.
pub fn arrs(v: &Value, keys: &[&str]) -> Vec<String> {
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr
                .iter()
                .filter_map(|e| e.as_str().map(|s| s.to_owned()))
                .collect();
        }
    }
    Vec::new()
}
/// Extract an unsigned 64-bit integer by trying multiple keys and representations.
///
/// Accepts any of the following representations for the first matching key:
///
/// - JSON `u64`
/// - JSON `i64` convertible to `u64`
/// - String that parses as `u64`
///
/// Returns `None` if no usable value is found.
pub fn u64_of(v: &Value, keys: &[&str]) -> Option<u64> {
    for k in keys {
        if let Some(n) = v.get(*k) {
            if let Some(u) = n.as_u64() {
                return Some(u);
            }
            if let Some(i) = n.as_i64()
                && let Ok(u) = u64::try_from(i)
            {
                return Some(u);
            }
            if let Some(s) = n.as_str()
                && let Ok(p) = s.parse::<u64>()
            {
                return Some(p);
            }
        }
    }
    None
}

use crate::state::Source;

/// Determine ordering weight for a package source.
///
/// Lower values indicate higher priority. Used to sort results such that
/// official repositories precede AUR, and core repos precede others.
///
/// Order:
///
/// - `core` => 0
/// - `extra` => 1
/// - other official repos => 2
/// - AUR => 3
pub fn repo_order(src: &Source) -> u8 {
    match src {
        Source::Official { repo, .. } => {
            if repo.eq_ignore_ascii_case("core") {
                0
            } else if repo.eq_ignore_ascii_case("extra") {
                1
            } else {
                2
            }
        }
        Source::Aur => 3,
    }
}
/// Rank how well a package name matches a query (lower is better).
///
/// Expects `query_lower` to be lowercase; the name is lowercased internally.
///
/// Ranking:
///
/// - 0: exact match
/// - 1: prefix match (`starts_with`)
/// - 2: substring match (`contains`)
/// - 3: no match
pub fn match_rank(name: &str, query_lower: &str) -> u8 {
    let n = name.to_lowercase();
    if !query_lower.is_empty() {
        if n == query_lower {
            return 0;
        }
        if n.starts_with(query_lower) {
            return 1;
        }
        if n.contains(query_lower) {
            return 2;
        }
    }
    3
}

/// Convert an optional Unix timestamp (seconds) to a UTC date-time string.
///
/// - Returns an empty string for `None`.
/// - Negative timestamps are returned as their numeric string representation.
/// - Output format: `YYYY-MM-DD HH:MM:SS` (UTC)
///
/// This implementation performs a simple conversion using loops and does not
/// account for leap seconds.
pub fn ts_to_date(ts: Option<i64>) -> String {
    let t = match ts {
        Some(v) => v,
        None => return String::new(),
    };
    if t < 0 {
        return t.to_string();
    }

    // Split into days and seconds-of-day
    let mut days = t / 86_400;
    let mut sod = t % 86_400; // 0..86399
    if sod < 0 {
        sod += 86_400;
        days -= 1;
    }

    let hour = (sod / 3600) as u32;
    sod %= 3600;
    let minute = (sod / 60) as u32;
    let second = (sod % 60) as u32;

    // Convert days since 1970-01-01 to Y-M-D (UTC) using simple loops
    let mut year: i32 = 1970;
    loop {
        let leap = is_leap(year);
        let diy = if leap { 366 } else { 365 } as i64;
        if days >= diy {
            days -= diy;
            year += 1;
        } else {
            break;
        }
    }
    let leap = is_leap(year);
    let mut month: u32 = 1;
    let mdays = [
        31,
        if leap { 29 } else { 28 },
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
    for &len in mdays.iter() {
        if days >= len as i64 {
            days -= len as i64;
            month += 1;
        } else {
            break;
        }
    }
    let day = (days + 1) as u32;

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// Leap year predicate for the proleptic Gregorian calendar.
/// Return `true` if year `y` is a leap year.
///
/// Inputs:
/// - `y`: Year (Gregorian calendar)
///
/// Output:
/// - `true` when `y` is a leap year; `false` otherwise.
///
/// Notes:
/// - Follows Gregorian rule: divisible by 4 and not by 100, unless divisible by 400.
fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// Return today's UTC date formatted as `YYYYMMDD` using only the standard library.
///
/// This uses a simple conversion from Unix epoch seconds to a UTC calendar date,
/// matching the same leap-year logic as `ts_to_date`.
pub fn today_yyyymmdd_utc() -> String {
    let secs = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(dur) => dur.as_secs() as i64,
        Err(_) => 0, // fallback to epoch if clock is before 1970
    };
    let mut days = secs / 86_400;
    // Derive year
    let mut year: i32 = 1970;
    loop {
        let leap = is_leap(year);
        let diy = if leap { 366 } else { 365 } as i64;
        if days >= diy {
            days -= diy;
            year += 1;
        } else {
            break;
        }
    }
    // Derive month/day within the year
    let leap = is_leap(year);
    let mut month: u32 = 1;
    let mdays = [
        31,
        if leap { 29 } else { 28 },
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
    for &len in mdays.iter() {
        if days >= len as i64 {
            days -= len as i64;
            month += 1;
        } else {
            break;
        }
    }
    let day = (days + 1) as u32;
    format!("{year:04}{month:02}{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Source;

    #[test]
    /// What: Verify URL percent-encoding rules (unreserved, space, plus, unicode)
    ///
    /// - Input: "", "abc-_.~", "a b", "C++", "π"
    /// - Output: "", "abc-_.~", "a%20b", "C%2B%2B", "%CF%80"
    fn util_percent_encode() {
        assert_eq!(percent_encode(""), "");
        assert_eq!(percent_encode("abc-_.~"), "abc-_.~");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("C++"), "C%2B%2B");
        assert_eq!(percent_encode("π"), "%CF%80");
    }

    #[test]
    /// What: Validate JSON extractors (s/ss/arrs/u64_of) over mixed types
    ///
    /// - Input: JSON with strings, array, u64, negative i64, numeric string
    /// - Output: Correct extractions; negatives rejected for u64; missing -> defaults
    fn util_json_extractors_and_u64() {
        let v: serde_json::Value = serde_json::json!({
            "a": "str",
            "b": ["x", 1, "y"],
            "c": 42u64,
            "d": -5,
            "e": "123",
        });
        assert_eq!(s(&v, "a"), "str");
        assert_eq!(s(&v, "missing"), "");
        assert_eq!(ss(&v, &["z", "a"]).as_deref(), Some("str"));
        assert_eq!(
            arrs(&v, &["b", "missing"]),
            vec!["x".to_string(), "y".to_string()]
        );
        assert_eq!(u64_of(&v, &["c"]), Some(42));
        assert_eq!(u64_of(&v, &["d"]), None);
        assert_eq!(u64_of(&v, &["e"]), Some(123));
        assert_eq!(u64_of(&v, &["missing"]), None);
    }

    #[test]
    /// What: Ensure repo ordering and name-match ranking are correct
    ///
    /// - Input: Sources core/extra/other/AUR; names vs queries
    /// - Output: core < extra < other < AUR; exact/prefix/substr/no-match ranks 0/1/2/3
    fn util_repo_order_and_rank() {
        let core = Source::Official {
            repo: "core".into(),
            arch: "x86_64".into(),
        };
        let extra = Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        };
        let other = Source::Official {
            repo: "community".into(),
            arch: "x86_64".into(),
        };
        let aur = Source::Aur;
        assert!(repo_order(&core) < repo_order(&extra));
        assert!(repo_order(&extra) < repo_order(&other));
        assert!(repo_order(&other) < repo_order(&aur));

        assert_eq!(match_rank("ripgrep", "ripgrep"), 0);
        assert_eq!(match_rank("ripgrep", "rip"), 1);
        assert_eq!(match_rank("ripgrep", "pg"), 2);
        assert_eq!(match_rank("ripgrep", "zzz"), 3);
    }

    #[test]
    /// What: Convert timestamps to UTC date strings, including leap year
    ///
    /// - Input: None, -1, 0, 951782400 (2000-02-29)
    /// - Output: "", "-1", epoch start, and leap day string
    fn util_ts_to_date_and_leap() {
        assert_eq!(ts_to_date(None), "");
        assert_eq!(ts_to_date(Some(-1)), "-1");
        assert_eq!(ts_to_date(Some(0)), "1970-01-01 00:00:00");
        assert_eq!(ts_to_date(Some(951_782_400)), "2000-02-29 00:00:00");
    }

    #[test]
    /// What: Boundary around Y2K conversion for ts_to_date
    ///
    /// - Input: 946684800 (2000-01-01), 946684799 (one second before)
    /// - Output: "2000-01-01 00:00:00" and "1999-12-31 23:59:59"
    fn util_ts_to_date_boundaries() {
        assert_eq!(ts_to_date(Some(946_684_800)), "2000-01-01 00:00:00");
        assert_eq!(ts_to_date(Some(946_684_799)), "1999-12-31 23:59:59");
    }
}
