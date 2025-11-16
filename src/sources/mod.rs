//! Network and system data retrieval module split into submodules.

use crate::util::curl_args;
use serde_json::Value;

mod details;
mod news;
mod pkgbuild;
mod search;
pub mod status;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Fetch JSON from a URL using curl and parse into `serde_json::Value`
///
/// Input: `url` HTTP(S) to request
/// Output: `Ok(Value)` on success; `Err` if curl fails or the response is not valid JSON
///
/// Details: Executes curl with appropriate flags and parses the UTF-8 body with `serde_json`.
/// On Windows, uses `-k` flag to skip SSL certificate verification.
fn curl_json(url: &str) -> Result<Value> {
    let args = curl_args(url, &[]);
    let out = std::process::Command::new("curl").args(&args).output()?;
    if !out.status.success() {
        let error_msg = if let Some(code) = out.status.code() {
            match code {
                22 => "HTTP error from server (likely 502/503/504 - server temporarily unavailable)".to_string(),
                6 => "Could not resolve host (DNS/network issue)".to_string(),
                7 => "Failed to connect to host (network unreachable)".to_string(),
                28 => "Operation timeout".to_string(),
                _ => format!("curl failed with exit code {}", code),
            }
        } else {
            format!("curl failed: {:?}", out.status)
        };
        return Err(error_msg.into());
    }
    let body = String::from_utf8(out.stdout)?;
    let v: Value = serde_json::from_str(&body)?;
    Ok(v)
}

/// What: Fetch plain text from a URL using curl
///
/// Input:
/// - `url` to request
///
/// Output:
/// - `Ok(String)` with response body; `Err` if curl or UTF-8 decoding fails
///
/// Details:
/// - Executes curl with appropriate flags and returns the raw body as a `String`.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
fn curl_text(url: &str) -> Result<String> {
    let args = curl_args(url, &[]);
    let out = std::process::Command::new("curl").args(&args).output()?;
    if !out.status.success() {
        return Err(format!("curl failed: {:?}", out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

pub use details::fetch_details;
pub use news::fetch_arch_news;
pub use pkgbuild::fetch_pkgbuild_fast;
pub use search::fetch_all_with_errors;
pub use status::fetch_arch_status_text;

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
static TEST_MUTEX: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(not(target_os = "windows"))]
#[cfg(test)]
/// What: Provide a shared mutex to serialize tests that mutate PATH or curl shims.
///
/// Input: None.
/// Output: `&'static Mutex<()>` guard to synchronize tests touching global state.
///
/// Details: Lazily initializes a global `Mutex` via `OnceLock` for cross-test coordination.
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
