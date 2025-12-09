//! Small utility helpers for encoding, JSON extraction, ranking, and time formatting.
//!
//! The functions in this module are intentionally lightweight and dependency-free
//! to keep hot paths fast and reduce compile times. They are used by networking,
//! indexing, and UI code.

pub mod config;
pub mod curl;
pub mod pacman;
pub mod srcinfo;

use serde_json::Value;
use std::fmt::Write;

/// What: Ensure mouse capture is enabled for the TUI.
///
/// Inputs:
/// - None.
///
/// Output:
/// - No return value; enables mouse capture on stdout if not in headless mode.
///
/// Details:
/// - Should be called after spawning external processes (like terminals) that might disable mouse capture.
/// - Safe to call multiple times.
/// - In headless/test mode (`PACSEA_TEST_HEADLESS=1`), this is a no-op to prevent mouse escape sequences from appearing in test output.
/// - On Windows, this is a no-op as mouse capture is handled differently.
pub fn ensure_mouse_capture() {
    // Skip mouse capture in headless/test mode to prevent escape sequences in test output
    if std::env::var("PACSEA_TEST_HEADLESS").ok().as_deref() == Some("1") {
    } else {
        #[cfg(not(target_os = "windows"))]
        {
            use crossterm::execute;
            let _ = execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
        }
    }
}

/// What: Percent-encode a string for use in URLs according to RFC 3986.
///
/// Inputs:
/// - `input`: String to encode.
///
/// Output:
/// - Returns a percent-encoded string where reserved characters are escaped.
///
/// Details:
/// - Unreserved characters as per RFC 3986 (`A-Z`, `a-z`, `0-9`, `-`, `.`, `_`, `~`) are left as-is.
/// - Space is encoded as `%20` (not `+`).
/// - All other bytes are encoded as two uppercase hexadecimal digits prefixed by `%`.
/// - Operates on raw bytes from the input string; any non-ASCII bytes are hex-escaped.
#[must_use]
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
                let _ = write!(out, "{b:02X}");
            }
        }
    }
    out
}

/// What: Extract a string value from a JSON object by key, defaulting to empty string.
///
/// Inputs:
/// - `v`: JSON value to extract from.
/// - `key`: Key to look up in the JSON object.
///
/// Output:
/// - Returns the string value if found, or an empty string if the key is missing or not a string.
///
/// Details:
/// - Returns `""` if the key is missing or the value is not a string type.
#[must_use]
pub fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
/// What: Extract the first available string from a list of candidate keys.
///
/// Inputs:
/// - `v`: JSON value to extract from.
/// - `keys`: Array of candidate keys to try in order.
///
/// Output:
/// - Returns `Some(String)` for the first key that maps to a JSON string, or `None` if none match.
///
/// Details:
/// - Tries keys in the order provided and returns the first match.
/// - Returns `None` if no key maps to a string value.
#[must_use]
pub fn ss(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            return Some(s.to_owned());
        }
    }
    None
}
/// What: Extract an array of strings from a JSON object by trying keys in order.
///
/// Inputs:
/// - `v`: JSON value to extract from.
/// - `keys`: Array of candidate keys to try in order.
///
/// Output:
/// - Returns the first found array as `Vec<String>`, filtering out non-string elements.
/// - Returns an empty vector if no array of strings is found.
///
/// Details:
/// - Tries keys in the order provided and returns the first array found.
/// - Filters out non-string elements from the array.
/// - Returns an empty vector if no key maps to an array or if all elements are non-string.
#[must_use]
pub fn arrs(v: &Value, keys: &[&str]) -> Vec<String> {
    for k in keys {
        if let Some(arr) = v.get(*k).and_then(|x| x.as_array()) {
            return arr
                .iter()
                .filter_map(|e| e.as_str().map(ToOwned::to_owned))
                .collect();
        }
    }
    Vec::new()
}
/// What: Extract an unsigned 64-bit integer by trying multiple keys and representations.
///
/// Inputs:
/// - `v`: JSON value to extract from.
/// - `keys`: Array of candidate keys to try in order.
///
/// Output:
/// - Returns `Some(u64)` if a valid value is found, or `None` if no usable value is found.
///
/// Details:
/// - Accepts any of the following representations for the first matching key:
///   - JSON `u64`
///   - JSON `i64` convertible to `u64`
///   - String that parses as `u64`
/// - Tries keys in the order provided and returns the first match.
/// - Returns `None` if no key maps to a convertible value.
#[must_use]
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

/// Rank how well a package name matches a query using fuzzy matching (fzf-style) with a provided matcher.
///
/// Inputs:
/// - `name`: Package name to match against
/// - `query`: Query string to match
/// - `matcher`: Reference to a `SkimMatcherV2` instance to reuse across multiple calls
///
/// Output:
/// - `Some(score)` if the query matches the name (higher score = better match), `None` if no match
///
/// Details:
/// - Uses the provided `fuzzy_matcher::skim::SkimMatcherV2` for fzf-style fuzzy matching
/// - Returns scores where higher values indicate better matches
/// - Returns `None` when the query doesn't match at all
/// - This function is optimized for cases where the matcher can be reused across multiple calls
#[must_use]
pub fn fuzzy_match_rank_with_matcher(
    name: &str,
    query: &str,
    matcher: &fuzzy_matcher::skim::SkimMatcherV2,
) -> Option<i64> {
    use fuzzy_matcher::FuzzyMatcher;

    if query.trim().is_empty() {
        return None;
    }

    matcher.fuzzy_match(name, query)
}

/// Rank how well a package name matches a query using fuzzy matching (fzf-style).
///
/// Inputs:
/// - `name`: Package name to match against
/// - `query`: Query string to match
///
/// Output:
/// - `Some(score)` if the query matches the name (higher score = better match), `None` if no match
///
/// Details:
/// - Uses `fuzzy_matcher::skim::SkimMatcherV2` for fzf-style fuzzy matching
/// - Returns scores where higher values indicate better matches
/// - Returns `None` when the query doesn't match at all
/// - For performance-critical code that calls this function multiple times with the same query,
///   consider using `fuzzy_match_rank_with_matcher` instead to reuse the matcher instance
#[must_use]
pub fn fuzzy_match_rank(name: &str, query: &str) -> Option<i64> {
    use fuzzy_matcher::skim::SkimMatcherV2;

    let matcher = SkimMatcherV2::default();
    fuzzy_match_rank_with_matcher(name, query, &matcher)
}

/// What: Determine ordering weight for a package source.
///
/// Inputs:
/// - `src`: Package source to rank.
///
/// Output:
/// - Returns a `u8` weight where lower values indicate higher priority.
///
/// Details:
/// - Used to sort results such that official repositories precede AUR, and core repos precede others.
/// - Order: `core` => 0, `extra` => 1, other official repos => 2, AUR => 3.
/// - Case-insensitive comparison for repository names.
#[must_use]
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
/// What: Rank how well a package name matches a query (lower is better).
///
/// Inputs:
/// - `name`: Package name to match against.
/// - `query_lower`: Query string (must be lowercase).
///
/// Output:
/// - Returns a `u8` rank: 0 = exact match, 1 = prefix match, 2 = substring match, 3 = no match.
///
/// Details:
/// - Expects `query_lower` to be lowercase; the name is lowercased internally.
/// - Returns 3 (no match) if the query is empty.
#[must_use]
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

/// What: Convert an optional Unix timestamp (seconds) to a UTC date-time string.
///
/// Inputs:
/// - `ts`: Optional Unix timestamp in seconds since epoch.
///
/// Output:
/// - Returns a formatted string `YYYY-MM-DD HH:MM:SS` (UTC), or empty string for `None`, or numeric string for negative timestamps.
///
/// Details:
/// - Returns an empty string for `None`.
/// - Negative timestamps are returned as their numeric string representation.
/// - Output format: `YYYY-MM-DD HH:MM:SS` (UTC).
/// - This implementation performs a simple conversion using loops and does not account for leap seconds.
#[must_use]
pub fn ts_to_date(ts: Option<i64>) -> String {
    let Some(t) = ts else {
        return String::new();
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

    let hour = u32::try_from(sod / 3600).unwrap_or(0);
    sod %= 3600;
    let minute = u32::try_from(sod / 60).unwrap_or(0);
    let second = u32::try_from(sod % 60).unwrap_or(0);

    // Convert days since 1970-01-01 to Y-M-D (UTC) using simple loops
    let mut year: i32 = 1970;
    loop {
        let leap = is_leap(year);
        let diy = i64::from(if leap { 366 } else { 365 });
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
    for &len in &mdays {
        if days >= i64::from(len) {
            days -= i64::from(len);
            month += 1;
        } else {
            break;
        }
    }
    let day = u32::try_from(days + 1).unwrap_or(1);

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
const fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

/// What: Open a file in the default editor (cross-platform).
///
/// Inputs:
/// - `path`: Path to the file to open.
///
/// Output:
/// - No return value; spawns a background process to open the file.
///
/// Details:
/// - On Windows, uses `PowerShell`'s `Invoke-Item` to open files with the default application, with fallback to `cmd start`.
/// - On Unix-like systems (Linux/macOS), uses `xdg-open` (Linux) or `open` (macOS).
/// - Spawns the command in a background thread and ignores errors.
pub fn open_file(path: &std::path::Path) {
    std::thread::spawn({
        let path = path.to_path_buf();
        move || {
            #[cfg(target_os = "windows")]
            {
                // Use PowerShell to open file with default application
                let path_str = path.display().to_string().replace('\'', "''");
                let _ = std::process::Command::new("powershell.exe")
                    .args([
                        "-NoProfile",
                        "-Command",
                        &format!("Invoke-Item '{path_str}'"),
                    ])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .or_else(|_| {
                        // Fallback: try cmd start
                        std::process::Command::new("cmd")
                            .args(["/c", "start", "", &path.display().to_string()])
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                    });
            }
            #[cfg(not(target_os = "windows"))]
            {
                // Try xdg-open first (Linux), then open (macOS)
                let _ = std::process::Command::new("xdg-open")
                    .arg(&path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .or_else(|_| {
                        std::process::Command::new("open")
                            .arg(&path)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                    });
            }
        }
    });
}

/// What: Open a URL in the default browser (cross-platform).
///
/// Inputs:
/// - `url`: URL string to open.
///
/// Output:
/// - No return value; spawns a background process to open the URL.
///
/// Details:
/// - On Windows, uses `cmd /c start`, with fallback to `PowerShell` `Start-Process`.
/// - On Unix-like systems (Linux/macOS), uses `xdg-open` (Linux) or `open` (macOS).
/// - Spawns the command in a background thread and ignores errors.
/// - During tests, this is a no-op to avoid opening real browser windows.
#[cfg_attr(test, allow(unused_variables))]
#[allow(clippy::missing_const_for_fn)]
pub fn open_url(url: &str) {
    // Skip actual spawning during tests
    #[cfg(not(test))]
    {
        let url = url.to_string();
        std::thread::spawn(move || {
            #[cfg(target_os = "windows")]
            {
                // Use cmd /c start with empty title to open URL in default browser
                let _ = std::process::Command::new("cmd")
                    .args(["/c", "start", "", &url])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .or_else(|_| {
                        // Fallback: try PowerShell
                        std::process::Command::new("powershell")
                            .args(["-Command", &format!("Start-Process '{url}'")])
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                    });
            }
            #[cfg(not(target_os = "windows"))]
            {
                // Try xdg-open first (Linux), then open (macOS)
                let _ = std::process::Command::new("xdg-open")
                    .arg(&url)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .or_else(|_| {
                        std::process::Command::new("open")
                            .arg(&url)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .spawn()
                    });
            }
        });
    }
}

/// Build curl command arguments for fetching a URL.
///
/// On Windows, adds `-k` flag to skip SSL certificate verification to work around
/// common SSL certificate issues (exit code 77). On other platforms, uses standard
/// SSL verification.
///
/// Inputs:
/// - `url`: The URL to fetch
/// - `extra_args`: Additional curl arguments (e.g., `["--max-time", "10"]`)
///
/// Output:
/// - Vector of curl arguments ready to pass to `Command::args()`
///
/// Details:
/// - Base arguments: `-sSLf` (silent, show errors, follow redirects, fail on HTTP errors)
/// - Windows: Adds `-k` to skip SSL verification
/// - Adds User-Agent header to avoid being blocked by APIs
/// - Appends `extra_args` and `url` at the end
#[must_use]
pub fn curl_args(url: &str, extra_args: &[&str]) -> Vec<String> {
    let mut args = vec!["-sSLf".to_string()];

    #[cfg(target_os = "windows")]
    {
        // Skip SSL certificate verification on Windows to avoid exit code 77
        args.push("-k".to_string());
    }

    // Add default timeouts to prevent indefinite hangs:
    // --connect-timeout 30: fail if connection not established within 30 seconds
    // --max-time 60: fail if entire operation exceeds 60 seconds
    // Note: Some feeds (e.g., archlinux.org/feeds/news/) can be slow to connect
    args.push("--connect-timeout".to_string());
    args.push("30".to_string());
    args.push("--max-time".to_string());
    args.push("60".to_string());

    // Add User-Agent header to avoid being blocked by APIs
    args.push("-H".to_string());
    args.push("User-Agent: Pacsea/1.0".to_string());

    // Add any extra arguments
    for arg in extra_args {
        args.push((*arg).to_string());
    }

    // URL goes last
    args.push(url.to_string());

    args
}

/// What: Parse a single update entry line in the format "name - `old_version` -> name - `new_version`".
///
/// Inputs:
/// - `line`: A trimmed line from the updates file
///
/// Output:
/// - `Some((name, old_version, new_version))` if parsing succeeds, `None` otherwise
///
/// Details:
/// - Parses format: "name - `old_version` -> name - `new_version`"
/// - Returns `None` for empty lines or invalid formats
/// - Uses `rfind` to find the last occurrence of " - " to handle package names that may contain dashes
#[must_use]
pub fn parse_update_entry(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Parse format: "name - old_version -> name - new_version"
    trimmed.find(" -> ").and_then(|arrow_pos| {
        let before_arrow = trimmed[..arrow_pos].trim();
        let after_arrow = trimmed[arrow_pos + 4..].trim();

        // Parse "name - old_version" from before_arrow
        before_arrow.rfind(" - ").and_then(|old_dash_pos| {
            let name = before_arrow[..old_dash_pos].trim().to_string();
            let old_version = before_arrow[old_dash_pos + 3..].trim().to_string();

            // Parse "name - new_version" from after_arrow
            after_arrow.rfind(" - ").map(|new_dash_pos| {
                let new_version = after_arrow[new_dash_pos + 3..].trim().to_string();
                (name, old_version, new_version)
            })
        })
    })
}

/// What: Return today's UTC date formatted as `YYYYMMDD` using only the standard library.
///
/// Inputs:
/// - None (uses current system time).
///
/// Output:
/// - Returns a string in format `YYYYMMDD` representing today's date in UTC.
///
/// Details:
/// - Uses a simple conversion from Unix epoch seconds to a UTC calendar date.
/// - Matches the same leap-year logic as `ts_to_date`.
/// - Falls back to epoch date (1970-01-01) if system time is before 1970.
#[must_use]
pub fn today_yyyymmdd_utc() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|dur| i64::try_from(dur.as_secs()).ok())
        .unwrap_or(0); // fallback to epoch if clock is before 1970
    let mut days = secs / 86_400;
    // Derive year
    let mut year: i32 = 1970;
    loop {
        let leap = is_leap(year);
        let diy = i64::from(if leap { 366 } else { 365 });
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
    for &len in &mdays {
        if days >= i64::from(len) {
            days -= i64::from(len);
            month += 1;
        } else {
            break;
        }
    }
    let day = u32::try_from(days + 1).unwrap_or(1);
    format!("{year:04}{month:02}{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Source;

    #[test]
    /// What: Verify that percent encoding preserves unreserved characters and escapes reserved ones.
    ///
    /// Inputs:
    /// - `cases`: Sample strings covering empty input, ASCII safe set, spaces, plus signs, and unicode.
    ///
    /// Output:
    /// - Encoded results match RFC 3986 expectations for each case.
    ///
    /// Details:
    /// - Exercises `percent_encode` across edge characters to confirm proper handling of special
    ///   symbols and non-ASCII glyphs.
    fn util_percent_encode() {
        assert_eq!(percent_encode(""), "");
        assert_eq!(percent_encode("abc-_.~"), "abc-_.~");
        assert_eq!(percent_encode("a b"), "a%20b");
        assert_eq!(percent_encode("C++"), "C%2B%2B");
        assert_eq!(percent_encode("π"), "%CF%80");
    }

    #[test]
    /// What: Validate JSON helper extractors across strings, arrays, and numeric conversions.
    ///
    /// Inputs:
    /// - `v`: Composite JSON value containing strings, arrays, unsigned ints, negatives, and text numbers.
    ///
    /// Output:
    /// - Helpers return expected values, defaulting or rejecting incompatible types.
    ///
    /// Details:
    /// - Confirms `s`, `ss`, `arrs`, and `u64_of` handle fallbacks, partial arrays, and reject negative
    ///   values while parsing numeric strings.
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
    /// What: Ensure repository ordering and name match ranking align with search heuristics.
    ///
    /// Inputs:
    /// - `sources`: Official repos (core, extra, other) plus AUR source for ordering comparison.
    /// - `queries`: Example name/query pairs for ranking checks.
    ///
    /// Output:
    /// - Ordering places core before extra before other before AUR and match ranks progress 0→3.
    ///
    /// Details:
    /// - Verifies that `repo_order` promotes official repositories and that `match_rank` scores exact,
    ///   prefix, substring, and non-matches as intended.
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
    /// What: Verify fuzzy matching returns scores for valid matches and None for non-matches.
    ///
    /// Inputs:
    /// - Package names and queries covering exact matches, partial matches, and non-matches.
    ///
    /// Output:
    /// - Fuzzy matching returns `Some(score)` for matches (higher = better) and `None` for non-matches.
    ///
    /// Details:
    /// - Tests that fuzzy matching can find non-substring matches (e.g., "rg" matches "ripgrep").
    /// - Verifies empty queries return `None`.
    fn util_fuzzy_match_rank() {
        // Exact match should return a score
        assert!(fuzzy_match_rank("ripgrep", "ripgrep").is_some());

        // Prefix match should return a score
        assert!(fuzzy_match_rank("ripgrep", "rip").is_some());

        // Fuzzy match (non-substring) should return a score
        assert!(fuzzy_match_rank("ripgrep", "rg").is_some());

        // Non-match should return None
        assert!(fuzzy_match_rank("ripgrep", "xyz").is_none());

        // Empty query should return None
        assert!(fuzzy_match_rank("ripgrep", "").is_none());
        assert!(fuzzy_match_rank("ripgrep", "   ").is_none());

        // Case-insensitive matching
        assert!(fuzzy_match_rank("RipGrep", "rg").is_some());
        assert!(fuzzy_match_rank("RIPGREP", "rip").is_some());
    }

    #[test]
    /// What: Convert timestamps into UTC date strings, including leap-year handling.
    ///
    /// Inputs:
    /// - `samples`: `None`, negative, epoch, and leap-day timestamps.
    ///
    /// Output:
    /// - Strings reflect empty/default, passthrough, epoch baseline, and leap day formatting.
    ///
    /// Details:
    /// - Exercises `ts_to_date` across typical edge cases to ensure correct chrono arithmetic.
    fn util_ts_to_date_and_leap() {
        assert_eq!(ts_to_date(None), "");
        assert_eq!(ts_to_date(Some(-1)), "-1");
        assert_eq!(ts_to_date(Some(0)), "1970-01-01 00:00:00");
        assert_eq!(ts_to_date(Some(951_782_400)), "2000-02-29 00:00:00");
    }

    #[test]
    /// What: Validate `ts_to_date` output at the Y2K boundary.
    ///
    /// Inputs:
    /// - `y2k`: Timestamp for 2000-01-01 and the preceding second.
    ///
    /// Output:
    /// - Formatted strings match midnight Y2K and the final second of 1999.
    ///
    /// Details:
    /// - Confirms no off-by-one errors occur when crossing the year boundary.
    fn util_ts_to_date_boundaries() {
        assert_eq!(ts_to_date(Some(946_684_800)), "2000-01-01 00:00:00");
        assert_eq!(ts_to_date(Some(946_684_799)), "1999-12-31 23:59:59");
    }
}
