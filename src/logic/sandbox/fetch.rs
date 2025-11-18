//! Fetching functions for AUR package metadata.

use crate::util::{curl_args, percent_encode};
use std::process::{Command, Stdio};

/// What: Fetch .SRCINFO content for an AUR package using async HTTP.
///
/// Inputs:
/// - `client`: Reqwest HTTP client.
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
pub(crate) async fn fetch_srcinfo_async(
    client: &reqwest::Client,
    name: &str,
) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP request failed with status: {}",
            response.status()
        ));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {}", e))?;

    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    // Check if we got an HTML error page instead of .SRCINFO content
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO".to_string());
    }

    Ok(text)
}

/// What: Fetch .SRCINFO content for an AUR package (legacy synchronous version using curl).
///
/// Inputs:
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
#[allow(dead_code)]
pub(crate) fn fetch_srcinfo(name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let args = curl_args(&url, &[]);
    let output = Command::new("curl")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("curl failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "curl failed with status: {:?}",
            output.status.code()
        ));
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    // Check if we got an HTML error page instead of .SRCINFO content
    // AUR sometimes returns HTML error pages (e.g., 502 Bad Gateway) with curl exit code 0
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO content".to_string());
    }

    // Validate that it looks like .SRCINFO format (should have pkgbase or pkgname)
    if !text.contains("pkgbase =") && !text.contains("pkgname =") {
        return Err("Response does not appear to be valid .SRCINFO format".to_string());
    }

    Ok(text)
}
