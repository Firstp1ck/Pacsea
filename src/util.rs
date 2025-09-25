use serde_json::Value;

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

pub fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
pub fn ss(v: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(s) = v.get(*k).and_then(|x| x.as_str()) {
            return Some(s.to_owned());
        }
    }
    None
}
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

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        year, month, day, hour, minute, second
    )
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}
