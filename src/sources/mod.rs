//! Network and system data retrieval module split into submodules.

use serde_json::Value;

mod details;
mod news;
mod pkgbuild;
mod search;
mod status;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

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
