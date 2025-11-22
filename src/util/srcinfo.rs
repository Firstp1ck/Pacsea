//! .SRCINFO fetching utilities for AUR packages.
//!
//! This module provides functions for fetching .SRCINFO files from the AUR,
//! with support for both synchronous (curl) and asynchronous (reqwest) fetching.

use crate::util::{curl, percent_encode};

/// What: Fetch .SRCINFO content for an AUR package synchronously using curl.
///
/// Inputs:
/// - `name`: AUR package name.
/// - `timeout_seconds`: Optional timeout in seconds (None = no timeout).
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
///
/// # Errors
/// - Returns `Err` when network request fails (curl execution error)
/// - Returns `Err` when .SRCINFO cannot be fetched from AUR
/// - Returns `Err` when response is empty or contains HTML error page
/// - Returns `Err` when response does not appear to be valid .SRCINFO format
///
/// Details:
/// - Downloads .SRCINFO from AUR cgit repository.
/// - Validates that the response is not empty, not HTML, and contains .SRCINFO format markers.
pub fn fetch_srcinfo(name: &str, timeout_seconds: Option<u64>) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let text = if let Some(timeout) = timeout_seconds {
        let timeout_str = timeout.to_string();
        curl::curl_text_with_args(&url, &["--max-time", &timeout_str])
            .map_err(|e| format!("curl failed: {e}"))?
    } else {
        curl::curl_text(&url).map_err(|e| format!("curl failed: {e}"))?
    };

    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    // Check if we got an HTML error page instead of .SRCINFO content
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO".to_string());
    }

    // Validate that it looks like .SRCINFO format (should have pkgbase or pkgname)
    if !text.contains("pkgbase =") && !text.contains("pkgname =") {
        return Err("Response does not appear to be valid .SRCINFO format".to_string());
    }

    Ok(text)
}

/// What: Fetch .SRCINFO content for an AUR package using async HTTP.
///
/// Inputs:
/// - `client`: Reqwest HTTP client.
/// - `name`: AUR package name.
///
/// Output:
/// - Returns .SRCINFO content as a string, or an error if fetch fails.
///
/// # Errors
/// - Returns `Err` when HTTP request fails (network error or client error)
/// - Returns `Err` when HTTP response status is not successful
/// - Returns `Err` when response body cannot be read
/// - Returns `Err` when response is empty or contains HTML error page
///
/// Details:
/// - Uses reqwest for async fetching with built-in timeout handling.
/// - Validates that the response is not empty and not HTML.
pub async fn fetch_srcinfo_async(client: &reqwest::Client, name: &str) -> Result<String, String> {
    let url = format!(
        "https://aur.archlinux.org/cgit/aur.git/plain/.SRCINFO?h={}",
        percent_encode(name)
    );
    tracing::debug!("Fetching .SRCINFO from: {}", url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "HTTP request failed with status: {}",
            response.status()
        ));
    }

    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if text.trim().is_empty() {
        return Err("Empty .SRCINFO content".to_string());
    }

    // Check if we got an HTML error page instead of .SRCINFO content
    if text.trim_start().starts_with("<html") || text.trim_start().starts_with("<!DOCTYPE") {
        return Err("Received HTML error page instead of .SRCINFO".to_string());
    }

    Ok(text)
}
