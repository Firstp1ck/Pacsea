//! Curl-based HTTP utilities for fetching JSON and text content.
//!
//! This module provides functions for executing curl commands and handling
//! common error cases with user-friendly error messages.

use super::curl_args;
use serde_json::Value;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Map curl exit codes to user-friendly error messages.
///
/// Inputs:
/// - `code`: Optional exit code from curl command
/// - `status`: Exit status for fallback error message
///
/// Output:
/// - User-friendly error message string
///
/// Details:
/// - Maps common curl exit codes (22, 6, 7, 28) to descriptive messages
/// - Falls back to generic error message if code is unknown
fn map_curl_error(code: Option<i32>, status: &std::process::ExitStatus) -> String {
    if let Some(code) = code {
        match code {
            22 => "HTTP error from server (likely 502/503/504 - server temporarily unavailable)"
                .to_string(),
            6 => "Could not resolve host (DNS/network issue)".to_string(),
            7 => "Failed to connect to host (network unreachable)".to_string(),
            28 => "Operation timeout".to_string(),
            _ => format!("curl failed with exit code {code}"),
        }
    } else {
        // Process was terminated by a signal or other reason
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(signal) = status.signal() {
                format!("curl process terminated by signal {signal}")
            } else {
                format!("curl process failed: {:?}", status)
            }
        }
        #[cfg(not(unix))]
        {
            format!("curl process failed: {:?}", status)
        }
    }
}

/// What: Fetch JSON from a URL using curl and parse into `serde_json::Value`.
///
/// Inputs:
/// - `url`: HTTP(S) URL to request
///
/// Output:
/// - `Ok(Value)` on success; `Err` if curl fails or the response is not valid JSON
///
/// Details:
/// - Executes curl with appropriate flags and parses the UTF-8 body with `serde_json`.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Provides user-friendly error messages for common curl failure cases.
pub fn curl_json(url: &str) -> Result<Value> {
    let args = curl_args(url, &[]);
    #[cfg(target_os = "windows")]
    {
        // On Windows, log the actual curl command being executed for debugging
        tracing::debug!(
            curl_args = ?args,
            url = %url,
            "Executing curl command on Windows"
        );
    }
    let out = std::process::Command::new("curl").args(&args).output()?;
    if !out.status.success() {
        let error_msg = map_curl_error(out.status.code(), &out.status);
        #[cfg(target_os = "windows")]
        {
            // On Windows, also log stderr for debugging
            if !out.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!(stderr = %stderr, url = %url, "curl stderr output on Windows");
            }
            // Also log stdout in case there's useful info there
            if !out.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                tracing::debug!(stdout = %stdout, url = %url, "curl stdout on Windows (non-success)");
            }
        }
        return Err(error_msg.into());
    }
    let body = String::from_utf8(out.stdout)?;
    #[cfg(target_os = "windows")]
    {
        // On Windows, log response details for debugging API issues
        if body.len() < 500 {
            tracing::debug!(
                url = %url,
                response_length = body.len(),
                response_body = %body,
                "curl response received on Windows"
            );
        } else {
            tracing::debug!(
                url = %url,
                response_length = body.len(),
                response_preview = %format!("{}...", &body[..500]),
                "curl response received on Windows (truncated)"
            );
        }
    }
    let v: Value = serde_json::from_str(&body)?;
    Ok(v)
}

/// What: Fetch plain text from a URL using curl.
///
/// Inputs:
/// - `url`: URL to request
///
/// Output:
/// - `Ok(String)` with response body; `Err` if curl or UTF-8 decoding fails
///
/// Details:
/// - Executes curl with appropriate flags and returns the raw body as a `String`.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Provides user-friendly error messages for common curl failure cases.
pub fn curl_text(url: &str) -> Result<String> {
    curl_text_with_args(url, &[])
}

/// What: Fetch plain text from a URL using curl with custom arguments.
///
/// Inputs:
/// - `url`: URL to request
/// - `extra_args`: Additional curl arguments (e.g., `["--max-time", "10"]`)
///
/// Output:
/// - `Ok(String)` with response body; `Err` if curl or UTF-8 decoding fails
///
/// Details:
/// - Executes curl with appropriate flags plus extra arguments.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Provides user-friendly error messages for common curl failure cases.
pub fn curl_text_with_args(url: &str, extra_args: &[&str]) -> Result<String> {
    let args = curl_args(url, extra_args);
    let out = std::process::Command::new("curl")
        .args(&args)
        .output()
        .map_err(|e| {
            format!("curl command failed to execute: {e} (is curl installed and in PATH?)")
        })?;
    if !out.status.success() {
        let error_msg = map_curl_error(out.status.code(), &out.status);
        return Err(error_msg.into());
    }
    Ok(String::from_utf8(out.stdout)?)
}
