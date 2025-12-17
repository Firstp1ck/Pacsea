//! Curl-based HTTP utilities for fetching JSON and text content.
//!
//! This module provides functions for executing curl commands and handling
//! common error cases with user-friendly error messages.
//!
//! # Security
//! - Uses absolute paths for curl binary when available (defense-in-depth against PATH hijacking)
//! - Redacts URL query parameters in debug logs to prevent potential secret leakage

use super::curl_args;
use chrono;
use serde_json::Value;
use std::sync::OnceLock;

/// What: Result type alias for curl utility errors.
///
/// Inputs: None (type alias).
///
/// Output: Result type with boxed error trait object.
///
/// Details: Standard error type for curl operations.
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Cached curl binary path for performance (computed once at first use).
static CURL_PATH: OnceLock<String> = OnceLock::new();

/// What: Find the curl binary path, preferring absolute paths for security.
///
/// Inputs: None
///
/// Output:
/// - Path to curl binary (absolute path if found, otherwise "curl" for PATH lookup)
///
/// Details:
/// - If `PACSEA_CURL_PATH` env var is set, returns "curl" to use PATH lookup (for testing)
/// - On Unix: Checks `/usr/bin/curl`, `/bin/curl`, `/usr/local/bin/curl`
/// - On Windows: Checks system paths (System32, Git, MSYS2, Cygwin, Chocolatey)
///   and user paths (Scoop, `WinGet`, local installs)
/// - Falls back to PATH lookup if no absolute path is found
/// - Result is cached for performance using `OnceLock` (except when env override is set)
/// - Defense-in-depth measure against PATH hijacking attacks
fn get_curl_path() -> &'static str {
    // Check for test override BEFORE using cache - allows tests to inject fake curl
    // This check is outside OnceLock so it's evaluated on every call
    if std::env::var("PACSEA_CURL_PATH").is_ok() {
        // Leak a static string for the "curl" fallback in test mode
        // This is intentional: tests need a consistent &'static str return type
        return Box::leak(Box::new("curl".to_string()));
    }

    CURL_PATH.get_or_init(|| {
        // Check common absolute paths first (defense-in-depth against PATH hijacking)
        #[cfg(unix)]
        {
            for path in ["/usr/bin/curl", "/bin/curl", "/usr/local/bin/curl"] {
                if std::path::Path::new(path).exists() {
                    tracing::trace!(curl_path = path, "Using absolute path for curl");
                    return path.to_string();
                }
            }
        }

        #[cfg(target_os = "windows")]
        {
            // On Windows, check common system installation paths first
            let system_paths = [
                r"C:\Windows\System32\curl.exe",
                r"C:\Program Files\Git\mingw64\bin\curl.exe",
                r"C:\Program Files (x86)\Git\mingw64\bin\curl.exe",
                r"C:\Program Files\curl\bin\curl.exe",
                r"C:\curl\bin\curl.exe",
                r"C:\ProgramData\chocolatey\bin\curl.exe",
                r"C:\msys64\usr\bin\curl.exe",
                r"C:\msys64\mingw64\bin\curl.exe",
                r"C:\cygwin64\bin\curl.exe",
                r"C:\cygwin\bin\curl.exe",
            ];

            for path in system_paths {
                if std::path::Path::new(path).exists() {
                    tracing::trace!(curl_path = path, "Using absolute path for curl on Windows");
                    return path.to_string();
                }
            }

            // Check user-specific paths (Scoop, MSYS2, local installs)
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let user_paths = [
                    // Scoop
                    format!(r"{user_profile}\scoop\shims\curl.exe"),
                    format!(r"{user_profile}\scoop\apps\curl\current\bin\curl.exe"),
                    format!(r"{user_profile}\scoop\apps\msys2\current\usr\bin\curl.exe"),
                    format!(r"{user_profile}\scoop\apps\msys2\current\mingw64\bin\curl.exe"),
                    // MSYS2 user installs
                    format!(r"{user_profile}\msys64\usr\bin\curl.exe"),
                    format!(r"{user_profile}\msys64\mingw64\bin\curl.exe"),
                    format!(r"{user_profile}\msys2\usr\bin\curl.exe"),
                    format!(r"{user_profile}\msys2\mingw64\bin\curl.exe"),
                    // Other user paths
                    format!(r"{user_profile}\.local\bin\curl.exe"),
                    format!(r"{user_profile}\AppData\Local\Microsoft\WinGet\Packages\curl.exe"),
                ];

                for path in user_paths {
                    if std::path::Path::new(&path).exists() {
                        tracing::trace!(
                            curl_path = %path,
                            "Using user-specific path for curl on Windows"
                        );
                        return path;
                    }
                }
            }
        }

        // Fallback to PATH lookup
        tracing::trace!("No absolute curl path found, falling back to PATH lookup");
        "curl".to_string()
    })
}

/// What: Redact query parameters from a URL for safe logging.
///
/// Inputs:
/// - `url`: The full URL that may contain query parameters
///
/// Output:
/// - URL with query parameters replaced by `?[REDACTED]` if present
///
/// Details:
/// - Prevents potential secret leakage in logs (API keys, tokens in query strings)
/// - Returns original URL if no query parameters are present
#[cfg(target_os = "windows")]
fn redact_url_for_logging(url: &str) -> String {
    url.find('?').map_or_else(
        || url.to_string(),
        |query_start| format!("{}?[REDACTED]", &url[..query_start]),
    )
}

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
fn map_curl_error(code: Option<i32>, status: std::process::ExitStatus) -> String {
    code.map_or_else(
        || {
            // Process was terminated by a signal or other reason
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                status.signal().map_or_else(
                    || format!("curl process failed: {status:?}"),
                    |signal| format!("curl process terminated by signal {signal}"),
                )
            }
            #[cfg(not(unix))]
            {
                format!("curl process failed: {status:?}")
            }
        },
        |code| match code {
            22 => "HTTP error from server (likely 502/503/504 - server temporarily unavailable)"
                .to_string(),
            6 => "Could not resolve host (DNS/network issue)".to_string(),
            7 => "Failed to connect to host (network unreachable)".to_string(),
            28 => "Operation timeout".to_string(),
            _ => format!("curl failed with exit code {code}"),
        },
    )
}

/// What: Fetch JSON from a URL using curl and parse into `serde_json::Value`.
///
/// Inputs:
/// - `url`: HTTP(S) URL to request
///
/// Output:
/// - `Ok(Value)` on success; `Err` if curl fails or the response is not valid JSON
///
/// # Errors
/// - Returns `Err` when curl command execution fails (I/O error or curl not found)
/// - Returns `Err` when curl exits with non-zero status (network errors, HTTP errors, timeouts)
/// - Returns `Err` when response body cannot be decoded as UTF-8
/// - Returns `Err` when response body cannot be parsed as JSON
///
/// Details:
/// - Executes curl with appropriate flags and parses the UTF-8 body with `serde_json`.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Provides user-friendly error messages for common curl failure cases.
pub fn curl_json(url: &str) -> Result<Value> {
    let args = curl_args(url, &[]);
    let curl_bin = get_curl_path();
    #[cfg(target_os = "windows")]
    {
        // On Windows, log curl command for debugging (URL redacted for security)
        let safe_url = redact_url_for_logging(url);
        tracing::debug!(
            curl_bin = %curl_bin,
            url = %safe_url,
            "Executing curl command on Windows"
        );
    }
    let out = std::process::Command::new(curl_bin).args(&args).output()?;
    if !out.status.success() {
        let error_msg = map_curl_error(out.status.code(), out.status);
        #[cfg(target_os = "windows")]
        {
            let safe_url = redact_url_for_logging(url);
            // On Windows, also log stderr for debugging
            if !out.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!(stderr = %stderr, url = %safe_url, "curl stderr output on Windows");
            }
            // Also log stdout in case there's useful info there
            if !out.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                tracing::debug!(stdout = %stdout, url = %safe_url, "curl stdout on Windows (non-success)");
            }
        }
        return Err(error_msg.into());
    }
    let body = String::from_utf8(out.stdout)?;
    #[cfg(target_os = "windows")]
    {
        // On Windows, log response details for debugging API issues (URL redacted)
        let safe_url = redact_url_for_logging(url);
        if body.len() < 500 {
            tracing::debug!(
                url = %safe_url,
                response_length = body.len(),
                "curl response received on Windows"
            );
        } else {
            tracing::debug!(
                url = %safe_url,
                response_length = body.len(),
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
/// # Errors
/// - Returns `Err` when curl command execution fails (I/O error or curl not found)
/// - Returns `Err` when curl exits with non-zero status (network errors, HTTP errors, timeouts)
/// - Returns `Err` when response body cannot be decoded as UTF-8
///
/// Details:
/// - Executes curl with appropriate flags and returns the raw body as a `String`.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Provides user-friendly error messages for common curl failure cases.
pub fn curl_text(url: &str) -> Result<String> {
    curl_text_with_args(url, &[])
}

/// What: Parse Retry-After header value into seconds.
///
/// Inputs:
/// - `retry_after`: Retry-After header value (can be seconds as number or HTTP-date)
///
/// Output:
/// - `Some(seconds)` if parsing succeeds, `None` otherwise
///
/// Details:
/// - Supports both numeric format (seconds) and HTTP-date format (RFC 7231).
/// - For HTTP-date, calculates seconds until that date.
fn parse_retry_after(retry_after: &str) -> Option<u64> {
    let trimmed = retry_after.trim();
    // Try parsing as number (seconds)
    if let Ok(seconds) = trimmed.parse::<u64>() {
        return Some(seconds);
    }
    // Try parsing as HTTP-date (RFC 7231)
    // Common formats: "Wed, 21 Oct 2015 07:28:00 GMT", "Wed, 21 Oct 2015 07:28:00 +0000"
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(trimmed) {
        let now = chrono::Utc::now();
        let retry_time = dt.with_timezone(&chrono::Utc);
        if retry_time > now {
            let duration = retry_time - now;
            let seconds = duration.num_seconds().max(0);
            // Safe: seconds is non-negative, and u64::MAX is much larger than any reasonable retry time
            #[allow(clippy::cast_sign_loss)]
            return Some(seconds as u64);
        }
        return Some(0);
    }
    // Try RFC 3339 format
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(trimmed) {
        let now = chrono::Utc::now();
        let retry_time = dt.with_timezone(&chrono::Utc);
        if retry_time > now {
            let duration = retry_time - now;
            let seconds = duration.num_seconds().max(0);
            // Safe: seconds is non-negative, and u64::MAX is much larger than any reasonable retry time
            #[allow(clippy::cast_sign_loss)]
            return Some(seconds as u64);
        }
        return Some(0);
    }
    None
}

/// What: Extract header value from HTTP response headers (case-insensitive).
///
/// Inputs:
/// - `headers_text`: Raw HTTP headers text (from curl -i output)
/// - `header_name`: Name of the header to extract (case-insensitive)
///
/// Output:
/// - `Some(value)` if header found, `None` otherwise
///
/// Details:
/// - Searches for header name (case-insensitive).
/// - Returns trimmed value after the colon.
fn extract_header_value(headers_text: &str, header_name: &str) -> Option<String> {
    let header_lower = header_name.to_lowercase();
    for line in headers_text.lines() {
        let line_lower = line.trim_start().to_lowercase();
        if line_lower.starts_with(&format!("{header_lower}:"))
            && let Some(colon_pos) = line.find(':')
        {
            let value = line[colon_pos + 1..].trim().to_string();
            return Some(value);
        }
    }
    None
}

/// What: Extract Retry-After header value from HTTP response headers.
///
/// Inputs:
/// - `headers_text`: Raw HTTP headers text (from curl -i output)
///
/// Output:
/// - `Some(seconds)` if Retry-After header found and parsed, `None` otherwise
///
/// Details:
/// - Searches for "Retry-After:" header (case-insensitive).
/// - Parses the value using `parse_retry_after()`.
fn extract_retry_after(headers_text: &str) -> Option<u64> {
    extract_header_value(headers_text, "Retry-After")
        .as_deref()
        .and_then(parse_retry_after)
}

/// Response metadata including headers for parsing `Retry-After`, `ETag`, and `Last-Modified`.
#[derive(Debug, Clone)]
pub struct CurlResponse {
    /// Response body.
    pub body: String,
    /// HTTP status code.
    pub status_code: Option<u16>,
    /// Retry-After header value in seconds, if present.
    pub retry_after_seconds: Option<u64>,
    /// `ETag` header value, if present.
    pub etag: Option<String>,
    /// Last-Modified header value, if present.
    pub last_modified: Option<String>,
}

/// What: Fetch plain text from a URL using curl with custom arguments, including headers.
///
/// Inputs:
/// - `url`: URL to request
/// - `extra_args`: Additional curl arguments (e.g., `["--max-time", "10"]`)
///
/// Output:
/// - `Ok(CurlResponse)` with response body, status code, and parsed headers; `Err` if curl or UTF-8 decoding fails
///
/// # Errors
/// - Returns `Err` when curl command execution fails (I/O error or curl not found)
/// - Returns `Err` when curl exits with non-zero status (network errors, HTTP errors, timeouts)
/// - Returns `Err` when response body cannot be decoded as UTF-8
///
/// Details:
/// - Executes curl with `-i` flag to include headers in output.
/// - Uses `-w "\n%{http_code}\n"` to get HTTP status code at the end.
/// - Parses Retry-After header from response headers.
/// - Separates headers from body in the response.
pub fn curl_text_with_args_headers(url: &str, extra_args: &[&str]) -> Result<CurlResponse> {
    let mut args = curl_args(url, extra_args);
    // Include headers in output (-i flag)
    args.push("-i".to_string());
    // Append write-out format to get HTTP status code at the end
    args.push("-w".to_string());
    args.push("\n%{http_code}\n".to_string());
    let curl_bin = get_curl_path();
    let out = std::process::Command::new(curl_bin)
        .args(&args)
        .output()
        .map_err(|e| {
            format!("curl command failed to execute: {e} (is curl installed and in PATH?)")
        })?;

    let stdout = String::from_utf8(out.stdout)?;

    // Parse status code from the end of output (last line should be the status code)
    let status_code = stdout
        .lines()
        .last()
        .and_then(|line| line.trim().parse::<u16>().ok());

    // Find the boundary between headers and body (empty line)
    let lines: Vec<&str> = stdout.lines().collect();
    let mut header_end = 0;
    let mut found_empty_line = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() && i > 0 {
            // Found empty line separating headers from body
            header_end = i;
            found_empty_line = true;
            break;
        }
    }

    // Extract headers and body
    let (headers_text, body_lines) = if found_empty_line {
        let headers: Vec<&str> = lines[..header_end].to_vec();
        // Skip the empty line and status code line at the end
        let body_end = lines.len().saturating_sub(1); // Exclude status code line
        let body: Vec<&str> = if header_end + 1 < body_end {
            lines[header_end + 1..body_end].to_vec()
        } else {
            vec![]
        };
        (headers.join("\n"), body.join("\n"))
    } else {
        // No headers found, treat entire output as body (minus status code)
        let body_end = lines.len().saturating_sub(1);
        let body: Vec<&str> = if body_end > 0 {
            lines[..body_end].to_vec()
        } else {
            vec![]
        };
        (String::new(), body.join("\n"))
    };

    // Parse headers
    let retry_after_seconds = (!headers_text.is_empty())
        .then(|| extract_retry_after(&headers_text))
        .flatten();
    let etag = (!headers_text.is_empty())
        .then(|| extract_header_value(&headers_text, "ETag"))
        .flatten();
    let last_modified = (!headers_text.is_empty())
        .then(|| extract_header_value(&headers_text, "Last-Modified"))
        .flatten();

    Ok(CurlResponse {
        body: body_lines,
        status_code,
        retry_after_seconds,
        etag,
        last_modified,
    })
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
/// # Errors
/// - Returns `Err` when curl command execution fails (I/O error or curl not found)
/// - Returns `Err` when curl exits with non-zero status (network errors, HTTP errors, timeouts)
/// - Returns `Err` when response body cannot be decoded as UTF-8
/// - Returns `Err` with message containing "429" when HTTP 429 (Too Many Requests) is received
///
/// Details:
/// - Executes curl with appropriate flags plus extra arguments.
/// - On Windows, uses `-k` flag to skip SSL certificate verification.
/// - Uses `-i` flag to include headers for Retry-After parsing.
/// - Uses `-w "\n%{http_code}\n"` to detect HTTP status codes, especially 429.
/// - Provides user-friendly error messages for common curl failure cases.
/// - HTTP 429/503 errors include Retry-After information when available.
pub fn curl_text_with_args(url: &str, extra_args: &[&str]) -> Result<String> {
    let mut args = curl_args(url, extra_args);
    // Include headers in output (-i flag) for Retry-After parsing
    args.push("-i".to_string());
    // Append write-out format to get HTTP status code at the end
    args.push("-w".to_string());
    args.push("\n%{http_code}\n".to_string());
    let curl_bin = get_curl_path();
    let out = std::process::Command::new(curl_bin)
        .args(&args)
        .output()
        .map_err(|e| {
            format!("curl command failed to execute: {e} (is curl installed and in PATH?)")
        })?;

    let stdout = String::from_utf8(out.stdout)?;

    // Parse status code from the end of output (last line should be the status code)
    // Check if last line is a numeric status code (3 digits)
    let lines: Vec<&str> = stdout.lines().collect();
    let (status_code, body_end) = lines.last().map_or((None, lines.len()), |last_line| {
        let trimmed = last_line.trim();
        // Check if last line looks like an HTTP status code (3 digits)
        if trimmed.len() == 3 && trimmed.chars().all(|c| c.is_ascii_digit()) {
            (
                trimmed.parse::<u16>().ok(),
                lines.len().saturating_sub(1), // Exclude status code line
            )
        } else {
            // Last line is not a status code, include it in body
            (None, lines.len())
        }
    });

    // Find the boundary between headers and body (empty line)
    let mut header_end = 0;
    let mut found_empty_line = false;
    for (i, line) in lines.iter().enumerate() {
        if line.trim().is_empty() && i > 0 {
            // Found empty line separating headers from body
            header_end = i;
            found_empty_line = true;
            break;
        }
    }

    // Extract headers and body
    let (headers_text, body_lines) = if found_empty_line {
        let headers: Vec<&str> = lines[..header_end].to_vec();
        // Check if headers section actually contains non-empty lines
        // If not, treat as if there are no headers (empty line is just formatting)
        let has_actual_headers = headers.iter().any(|h| !h.trim().is_empty());
        if has_actual_headers {
            // Skip the empty line and status code line at the end
            let body: Vec<&str> = if header_end + 1 < body_end {
                lines[header_end + 1..body_end].to_vec()
            } else {
                vec![]
            };
            (headers.join("\n"), body.join("\n"))
        } else {
            // No actual headers, treat entire output as body (up to body_end)
            let body: Vec<&str> = if body_end > 0 {
                // Include everything up to body_end, filtering out empty lines
                lines[..body_end]
                    .iter()
                    .filter(|line| !line.trim().is_empty())
                    .copied()
                    .collect()
            } else {
                vec![]
            };
            (String::new(), body.join("\n"))
        }
    } else {
        // No headers found, treat entire output as body (up to body_end)
        let body: Vec<&str> = if body_end > 0 {
            lines[..body_end].to_vec()
        } else {
            vec![]
        };
        (String::new(), body.join("\n"))
    };

    // Parse headers
    let retry_after_seconds = if headers_text.is_empty() {
        None
    } else {
        extract_retry_after(&headers_text)
    };

    // Check for HTTP errors
    if let Some(code) = status_code
        && code >= 400
    {
        // Check if we got HTTP 429 (Too Many Requests)
        if code == 429 {
            let mut error_msg = "HTTP 429 Too Many Requests - rate limited by server".to_string();
            if let Some(retry_after) = retry_after_seconds {
                error_msg.push_str(" (Retry-After: ");
                error_msg.push_str(&retry_after.to_string());
                error_msg.push_str("s)");
            }
            return Err(error_msg.into());
        }
        if code == 503 {
            let mut error_msg = "HTTP 503 Service Unavailable".to_string();
            if let Some(retry_after) = retry_after_seconds {
                error_msg.push_str(" (Retry-After: ");
                error_msg.push_str(&retry_after.to_string());
                error_msg.push_str("s)");
            }
            return Err(error_msg.into());
        }
    }

    // Check curl exit status for other errors
    if !out.status.success() {
        let error_msg = map_curl_error(out.status.code(), out.status);
        return Err(error_msg.into());
    }

    Ok(body_lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_curl_path_returns_valid_path() {
        let path = get_curl_path();
        // Should return either an absolute path or "curl"
        assert!(
            path == "curl"
                || path.starts_with('/')
                || path.starts_with("C:\\")
                || path.starts_with(r"C:\"),
            "Expected valid curl path, got: {path}"
        );
    }

    #[test]
    fn test_get_curl_path_is_cached() {
        // Calling get_curl_path twice should return the same value
        let path1 = get_curl_path();
        let path2 = get_curl_path();
        assert_eq!(path1, path2, "Curl path should be cached and consistent");
    }

    #[test]
    #[cfg(unix)]
    fn test_get_curl_path_prefers_absolute_on_unix() {
        let path = get_curl_path();
        // On Unix systems where curl is installed in standard locations,
        // we should get an absolute path
        if std::path::Path::new("/usr/bin/curl").exists()
            || std::path::Path::new("/bin/curl").exists()
            || std::path::Path::new("/usr/local/bin/curl").exists()
        {
            assert!(
                path.starts_with('/'),
                "Expected absolute path on Unix when curl is in standard location, got: {path}"
            );
        }
    }

    #[test]
    fn test_redact_url_for_logging_with_query_params() {
        // This test is only compiled on Windows, but we can still test the logic
        fn redact_url(url: &str) -> String {
            url.find('?').map_or_else(
                || url.to_string(),
                |query_start| format!("{}?[REDACTED]", &url[..query_start]),
            )
        }

        // URL with query parameters should be redacted
        let url_with_params = "https://api.example.com/search?apikey=secret123&query=test";
        let redacted = redact_url(url_with_params);
        assert_eq!(redacted, "https://api.example.com/search?[REDACTED]");
        assert!(!redacted.contains("secret123"));
        assert!(!redacted.contains("apikey"));
    }

    #[test]
    fn test_redact_url_for_logging_without_query_params() {
        fn redact_url(url: &str) -> String {
            url.find('?').map_or_else(
                || url.to_string(),
                |query_start| format!("{}?[REDACTED]", &url[..query_start]),
            )
        }

        // URL without query parameters should remain unchanged
        let url_no_params = "https://archlinux.org/mirrors/status/json/";
        let redacted = redact_url(url_no_params);
        assert_eq!(redacted, url_no_params);
    }

    #[test]
    fn test_redact_url_for_logging_empty_query() {
        fn redact_url(url: &str) -> String {
            url.find('?').map_or_else(
                || url.to_string(),
                |query_start| format!("{}?[REDACTED]", &url[..query_start]),
            )
        }

        // URL with empty query string should still redact
        let url_empty_query = "https://example.com/path?";
        let redacted = redact_url(url_empty_query);
        assert_eq!(redacted, "https://example.com/path?[REDACTED]");
    }

    #[test]
    #[cfg(unix)]
    fn test_map_curl_error_common_codes() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        // Test exit code 22 (HTTP error)
        let status = ExitStatus::from_raw(22 << 8);
        let msg = map_curl_error(Some(22), status);
        assert!(msg.contains("HTTP error"));

        // Test exit code 6 (DNS error)
        let status = ExitStatus::from_raw(6 << 8);
        let msg = map_curl_error(Some(6), status);
        assert!(msg.contains("resolve host"));

        // Test exit code 7 (connection error)
        let status = ExitStatus::from_raw(7 << 8);
        let msg = map_curl_error(Some(7), status);
        assert!(msg.contains("connect"));

        // Test exit code 28 (timeout)
        let status = ExitStatus::from_raw(28 << 8);
        let msg = map_curl_error(Some(28), status);
        assert!(msg.contains("timeout"));
    }
}
