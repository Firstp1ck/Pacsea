//! Network and system data retrieval module split into submodules.

use serde_json::Value;

mod details;
mod news;
mod pkgbuild;
mod search;
pub mod status;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Fetch JSON from a URL using `curl -sSLf` and parse it with serde_json.
///
/// Inputs:
/// - `url`: HTTP(S) URL to request.
///
/// Output:
/// - `Ok(Value)` on success; `Err` if curl fails or the response is not valid JSON.
fn curl_json(url: &str) -> Result<Value> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
    if !out.status.success() {
        return Err(format!("curl failed: {:?}", out.status).into());
    }
    let body = String::from_utf8(out.stdout)?;
    let v: Value = serde_json::from_str(&body)?;
    Ok(v)
}

/// Fetch plain text from a URL using `curl -sSLf`.
///
/// Inputs: `url` to request.
///
/// Output: `Ok(String)` with response body; `Err` if curl or UTF-8 decoding fails.
fn curl_text(url: &str) -> Result<String> {
    let out = std::process::Command::new("curl")
        .args(["-sSLf", url])
        .output()?;
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
pub(crate) fn test_mutex() -> &'static std::sync::Mutex<()> {
    TEST_MUTEX.get_or_init(|| std::sync::Mutex::new(()))
}
