//! Rate limiting and circuit breaker for network requests.
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use rand::Rng;
use tracing::{debug, warn};

use super::Result;

/// Rate limiter for news feed network requests.
/// Tracks the last request time to enforce minimum delay between requests.
static RATE_LIMITER: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));
/// Minimum delay between news feed network requests (500ms).
const RATE_LIMIT_DELAY_MS: u64 = 500;

/// Rate limiter state for archlinux.org with exponential backoff.
struct ArchLinuxRateLimiter {
    /// Last request timestamp.
    last_request: Instant,
    /// Current backoff delay in milliseconds (starts at base delay, increases exponentially).
    current_backoff_ms: u64,
    /// Number of consecutive failures/rate limits.
    consecutive_failures: u32,
}

/// Rate limiter for archlinux.org requests with exponential backoff.
/// Tracks last request time and implements progressive delays on failures.
static ARCHLINUX_RATE_LIMITER: LazyLock<Mutex<ArchLinuxRateLimiter>> = LazyLock::new(|| {
    Mutex::new(ArchLinuxRateLimiter {
        last_request: Instant::now(),
        current_backoff_ms: 500, // Start with 500ms base delay (reduced from 2s for faster initial requests)
        consecutive_failures: 0,
    })
});

/// Semaphore to serialize archlinux.org requests (only 1 concurrent request allowed).
/// This prevents multiple async tasks from overwhelming the server even when rate limiting
/// is applied, because the rate limiter alone doesn't prevent concurrent requests that
/// start at nearly the same time from all proceeding simultaneously.
static ARCHLINUX_REQUEST_SEMAPHORE: LazyLock<std::sync::Arc<tokio::sync::Semaphore>> =
    LazyLock::new(|| std::sync::Arc::new(tokio::sync::Semaphore::new(1)));

/// Base delay for archlinux.org requests (2 seconds).
const ARCHLINUX_BASE_DELAY_MS: u64 = 500; // Reduced from 2000ms for faster initial requests
/// Maximum backoff delay (60 seconds).
const ARCHLINUX_MAX_BACKOFF_MS: u64 = 60000;

/// Circuit breaker state for tracking failures per endpoint type.
#[derive(Debug, Clone)]
enum CircuitState {
    /// Circuit is closed - normal operation.
    Closed,
    /// Circuit is open - blocking requests due to failures.
    Open {
        /// Timestamp when circuit was opened (for cooldown calculation).
        opened_at: Instant,
    },
    /// Circuit is half-open - allowing one test request.
    HalfOpen,
}

/// Circuit breaker state tracking failures per endpoint pattern.
struct CircuitBreakerState {
    /// Current circuit state.
    state: CircuitState,
    /// Recent request outcomes (true = success, false = failure).
    /// Tracks last 10 requests to calculate failure rate.
    recent_outcomes: Vec<bool>,
    /// Endpoint pattern this breaker tracks (e.g., "/feeds/news/", "/packages/*/json/").
    /// Stored for debugging/logging purposes.
    #[allow(dead_code)]
    endpoint_pattern: String,
}

/// Circuit breakers per endpoint pattern.
/// Key: endpoint pattern (e.g., "/feeds/news/", "/packages/*/json/")
static CIRCUIT_BREAKERS: LazyLock<Mutex<HashMap<String, CircuitBreakerState>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Maximum number of recent outcomes to track for failure rate calculation.
const CIRCUIT_BREAKER_HISTORY_SIZE: usize = 10;
/// Failure rate threshold to open circuit (50% = 5 failures out of 10).
/// Used in calculation: `failure_count * 2 >= total_count` (equivalent to `>= 0.5`).
#[allow(dead_code)]
const CIRCUIT_BREAKER_FAILURE_THRESHOLD: f64 = 0.5;
/// Cooldown period before transitioning from `Open` to `HalfOpen` (60 seconds).
const CIRCUIT_BREAKER_COOLDOWN_SECS: u64 = 60;

/// Flag indicating a network error occurred during the last news fetch.
/// This can be checked by the UI to show a toast message.
static NETWORK_ERROR_FLAG: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// What: Check and clear the network error flag.
///
/// Inputs: None
///
/// Output: `true` if a network error occurred since the last check, `false` otherwise.
///
/// Details:
/// - Atomically loads and clears the flag.
/// - Used by the UI to show a toast when news fetch had network issues.
#[must_use]
pub fn take_network_error() -> bool {
    NETWORK_ERROR_FLAG.swap(false, std::sync::atomic::Ordering::SeqCst)
}

/// What: Set the network error flag.
///
/// Inputs: None
///
/// Output: None
///
/// Details:
/// - Called when a network error occurs during news fetching.
pub(super) fn set_network_error() {
    NETWORK_ERROR_FLAG.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// What: Retry a network operation with exponential backoff on failure.
///
/// Inputs:
/// - `operation`: Async closure that returns a Result
/// - `max_retries`: Maximum number of retry attempts
///
/// Output:
/// - Result from the operation, or error if all retries fail
///
/// Details:
/// - On failure, waits with exponential backoff: 1s, 2s, 4s...
/// - Stops retrying after `max_retries` attempts
pub(super) async fn retry_with_backoff<T, E, F, Fut>(
    mut operation: F,
    max_retries: usize,
) -> std::result::Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if attempt >= max_retries {
                    return Err(e);
                }
                attempt += 1;
                let backoff_secs = 1u64 << (attempt - 1); // Exponential: 1, 2, 4, 8...
                warn!(
                    attempt,
                    max_retries,
                    backoff_secs,
                    "network request failed, retrying with exponential backoff"
                );
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
            }
        }
    }
}

/// What: Apply rate limiting before making a network request.
///
/// Inputs: None
///
/// Output: None (async sleep if needed)
///
/// Details:
/// - Ensures minimum delay between network requests to avoid overwhelming servers.
/// - Thread-safe via mutex guarding the last request timestamp.
pub(super) async fn rate_limit() {
    let delay_needed = {
        let mut last_request = match RATE_LIMITER.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let elapsed = last_request.elapsed();
        let min_delay = Duration::from_millis(RATE_LIMIT_DELAY_MS);
        let delay = if elapsed < min_delay {
            // Safe to unwrap because we checked elapsed < min_delay above
            #[allow(clippy::unwrap_used)]
            min_delay.checked_sub(elapsed).unwrap()
        } else {
            Duration::ZERO
        };
        *last_request = Instant::now();
        delay
    };
    if !delay_needed.is_zero() {
        tokio::time::sleep(delay_needed).await;
    }
}

/// Maximum jitter in milliseconds to add to rate limiting delays (prevents thundering herd).
const JITTER_MAX_MS: u64 = 500;

/// What: Apply rate limiting specifically for archlinux.org requests with exponential backoff.
///
/// Inputs: None
///
/// Output: `OwnedSemaphorePermit` that the caller MUST hold during the request.
///
/// # Panics
/// - Panics if the archlinux.org request semaphore is closed (should never happen in practice).
///
/// Details:
/// - Acquires a semaphore permit to serialize archlinux.org requests (only 1 at a time).
/// - Uses longer base delay (2 seconds) for archlinux.org to reduce request frequency.
/// - Implements exponential backoff: increases delay on consecutive failures (2s → 4s → 8s → 16s, max 60s).
/// - Adds random jitter (0-500ms) to prevent thundering herd when multiple clients retry simultaneously.
/// - Resets backoff after successful requests.
/// - Thread-safe via mutex guarding the rate limiter state.
/// - The returned permit MUST be held until the HTTP request completes to ensure serialization.
/// - If the permit is dropped before the HTTP request completes, another request may start concurrently,
///   defeating the serialization and potentially causing race conditions or overwhelming the server.
pub async fn rate_limit_archlinux() -> tokio::sync::OwnedSemaphorePermit {
    // 1. Acquire semaphore to serialize requests (waits if another request is in progress)
    // This is the key change - ensures only one archlinux.org request at a time
    let permit = ARCHLINUX_REQUEST_SEMAPHORE
        .clone()
        .acquire_owned()
        .await
        // Semaphore is never closed, so this cannot fail in practice
        .expect("archlinux.org request semaphore should never be closed");

    // 2. Now that we have exclusive access, compute and apply the rate limiting delay
    let delay_needed = {
        let mut limiter = match ARCHLINUX_RATE_LIMITER.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let elapsed = limiter.last_request.elapsed();
        let min_delay = Duration::from_millis(limiter.current_backoff_ms);
        let delay = if elapsed < min_delay {
            // Safe to unwrap because we checked elapsed < min_delay above
            #[allow(clippy::unwrap_used)]
            min_delay.checked_sub(elapsed).unwrap()
        } else {
            Duration::ZERO
        };
        limiter.last_request = Instant::now();
        delay
    };

    if !delay_needed.is_zero() {
        // Add random jitter to prevent thundering herd when multiple clients retry simultaneously
        let jitter_ms = rand::rng().random_range(0..=JITTER_MAX_MS);
        let delay_with_jitter = delay_needed + Duration::from_millis(jitter_ms);
        // Safe to unwrap: delay_ms will be small (max 60s = 60000ms, well within u64)
        #[allow(clippy::cast_possible_truncation)]
        let delay_ms = delay_needed.as_millis() as u64;
        debug!(
            delay_ms,
            jitter_ms,
            total_ms = delay_with_jitter.as_millis(),
            "rate limiting archlinux.org request with jitter"
        );
        tokio::time::sleep(delay_with_jitter).await;
    }

    // 3. Return the permit - caller MUST hold it during the request
    permit
}

/// What: Extract endpoint pattern from URL for circuit breaker tracking.
///
/// Inputs:
/// - `url`: Full URL to extract pattern from
///
/// Output:
/// - Endpoint pattern string (e.g., "/feeds/news/", "/packages/*/json/")
///
/// Details:
/// - Normalizes URLs to endpoint patterns for grouping similar requests.
/// - Replaces specific package names with "*" for JSON endpoints.
#[must_use]
pub fn extract_endpoint_pattern(url: &str) -> String {
    // Extract path from URL
    if let Some(path_start) = url.find("://")
        && let Some(path_pos) = url[path_start + 3..].find('/')
    {
        let path = &url[path_start + 3 + path_pos..];
        // Normalize package-specific endpoints
        if path.contains("/packages/") && path.contains("/json/") {
            // Pattern: /packages/{repo}/{arch}/{name}/json/ -> /packages/*/json/
            if let Some(json_pos) = path.find("/json/") {
                let base = &path[..json_pos];
                if let Some(last_slash) = base.rfind('/') {
                    return format!("{}/*/json/", &base[..=last_slash]);
                }
            }
        }
        // For feeds, use the full path
        if path.starts_with("/feeds/") {
            return path.to_string();
        }
        // For news articles, use /news/ pattern
        if path.contains("/news/")
            && !path.ends_with('/')
            && let Some(news_pos) = path.find("/news/")
        {
            return format!("{}/*", &path[..news_pos + "/news/".len()]);
        }
        return path.to_string();
    }
    url.to_string()
}

/// What: Check circuit breaker state before making a request.
///
/// Inputs:
/// - `endpoint_pattern`: Endpoint pattern to check circuit breaker for
///
/// Output:
/// - `Ok(())` if request should proceed, `Err` with cached error if circuit is open
///
/// # Errors
/// - Returns `Err` if the circuit breaker is open and cooldown period has not expired.
///
/// Details:
/// - Returns error immediately if circuit is Open and cooldown not expired.
/// - Allows request if circuit is Closed or `HalfOpen`.
/// - Automatically transitions `Open` → `HalfOpen` after cooldown period.
#[allow(clippy::significant_drop_tightening)]
pub fn check_circuit_breaker(endpoint_pattern: &str) -> Result<()> {
    // MutexGuard must be held for entire function to modify breaker state
    let mut breakers = match CIRCUIT_BREAKERS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let breaker = breakers
        .entry(endpoint_pattern.to_string())
        .or_insert_with(|| CircuitBreakerState {
            state: CircuitState::Closed,
            recent_outcomes: Vec::new(),
            endpoint_pattern: endpoint_pattern.to_string(),
        });

    match &breaker.state {
        CircuitState::Open { opened_at } => {
            let elapsed = opened_at.elapsed();
            if elapsed.as_secs() >= CIRCUIT_BREAKER_COOLDOWN_SECS {
                // Transition to HalfOpen after cooldown
                breaker.state = CircuitState::HalfOpen;
                debug!(
                    endpoint_pattern,
                    "circuit breaker transitioning Open → HalfOpen after cooldown"
                );
                Ok(())
            } else {
                // Still in cooldown, block request
                let remaining = CIRCUIT_BREAKER_COOLDOWN_SECS - elapsed.as_secs();
                warn!(
                    endpoint_pattern,
                    remaining_secs = remaining,
                    "circuit breaker is Open, blocking request"
                );
                Err(format!(
                    "Circuit breaker is Open for {endpoint_pattern} (cooldown: {remaining}s remaining)"
                )
                .into())
            }
        }
        CircuitState::HalfOpen | CircuitState::Closed => Ok(()),
    }
}

/// What: Record request outcome in circuit breaker.
///
/// Inputs:
/// - `endpoint_pattern`: Endpoint pattern for this request
/// - `success`: `true` if request succeeded, `false` if it failed
///
/// Output: None
///
/// Details:
/// - Records outcome in recent history (max 10 entries).
/// - On success: resets failure count, moves to Closed.
/// - On failure: increments failure count, opens circuit if >50% failure rate.
#[allow(clippy::significant_drop_tightening)]
pub fn record_circuit_breaker_outcome(endpoint_pattern: &str, success: bool) {
    // MutexGuard must be held for entire function to modify breaker state
    let mut breakers = match CIRCUIT_BREAKERS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    let breaker = breakers
        .entry(endpoint_pattern.to_string())
        .or_insert_with(|| CircuitBreakerState {
            state: CircuitState::Closed,
            recent_outcomes: Vec::new(),
            endpoint_pattern: endpoint_pattern.to_string(),
        });

    // Add outcome to history (keep last N)
    breaker.recent_outcomes.push(success);
    if breaker.recent_outcomes.len() > CIRCUIT_BREAKER_HISTORY_SIZE {
        breaker.recent_outcomes.remove(0);
    }

    if success {
        // On success, reset to Closed
        breaker.state = CircuitState::Closed;
        if !breaker.recent_outcomes.iter().all(|&x| x) {
            debug!(
                endpoint_pattern,
                "circuit breaker: request succeeded, resetting to Closed"
            );
        }
    } else {
        // On failure, check if we should open circuit
        let failure_count = breaker
            .recent_outcomes
            .iter()
            .filter(|&&outcome| !outcome)
            .count();
        // Calculate failure rate using integer comparison to avoid precision loss
        // Threshold is 0.5 (50%), so we check: failure_count * 2 >= total_count
        let total_count = breaker.recent_outcomes.len();

        if failure_count * 2 >= total_count && total_count >= CIRCUIT_BREAKER_HISTORY_SIZE {
            // Open circuit
            breaker.state = CircuitState::Open {
                opened_at: Instant::now(),
            };
            warn!(
                endpoint_pattern,
                failure_count,
                total = breaker.recent_outcomes.len(),
                failure_percentage = (failure_count * 100) / total_count,
                "circuit breaker opened due to high failure rate"
            );
        } else if matches!(breaker.state, CircuitState::HalfOpen) {
            // HalfOpen test failed, go back to Open
            breaker.state = CircuitState::Open {
                opened_at: Instant::now(),
            };
            warn!(
                endpoint_pattern,
                "circuit breaker: HalfOpen test failed, reopening"
            );
        }
    }
}

/// What: Extract Retry-After value from error message string.
///
/// Inputs:
/// - `error_msg`: Error message that may contain Retry-After information
///
/// Output:
/// - `Some(seconds)` if Retry-After found in error message, `None` otherwise
///
/// Details:
/// - Parses format: "error message (Retry-After: Ns)" where N is seconds.
#[must_use]
pub fn extract_retry_after_from_error(error_msg: &str) -> Option<u64> {
    if let Some(start) = error_msg.find("Retry-After: ") {
        let after_start = start + "Retry-After: ".len();
        let remaining = &error_msg[after_start..];
        if let Some(end) = remaining.find('s') {
            let seconds_str = &remaining[..end];
            return seconds_str.trim().parse::<u64>().ok();
        }
    }
    None
}

/// What: Increase backoff delay for archlinux.org after a failure or rate limit.
///
/// Inputs:
/// - `retry_after_seconds`: Optional Retry-After value from server (in seconds)
///
/// Output: None
///
/// Details:
/// - If Retry-After is provided, uses that value (capped at maximum delay).
/// - Otherwise, doubles the current backoff delay (exponential backoff).
/// - Caps at maximum delay (60 seconds).
/// - Increments consecutive failure counter.
pub fn increase_archlinux_backoff(retry_after_seconds: Option<u64>) {
    let mut limiter = match ARCHLINUX_RATE_LIMITER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    limiter.consecutive_failures += 1;
    // Use Retry-After value if provided, otherwise use exponential backoff
    if let Some(retry_after) = retry_after_seconds {
        // Convert seconds to milliseconds, cap at maximum
        let retry_after_ms = (retry_after * 1000).min(ARCHLINUX_MAX_BACKOFF_MS);
        limiter.current_backoff_ms = retry_after_ms;
        warn!(
            consecutive_failures = limiter.consecutive_failures,
            retry_after_seconds = retry_after,
            backoff_ms = limiter.current_backoff_ms,
            "increased archlinux.org backoff delay using Retry-After header"
        );
    } else {
        // Double the backoff delay, capped at maximum
        limiter.current_backoff_ms = (limiter.current_backoff_ms * 2).min(ARCHLINUX_MAX_BACKOFF_MS);
        warn!(
            consecutive_failures = limiter.consecutive_failures,
            backoff_ms = limiter.current_backoff_ms,
            "increased archlinux.org backoff delay"
        );
    }
}

/// What: Reset backoff delay for archlinux.org after a successful request.
///
/// Inputs: None
///
/// Output: None
///
/// Details:
/// - Resets backoff to base delay (2 seconds).
/// - Resets consecutive failure counter.
pub fn reset_archlinux_backoff() {
    let mut limiter = match ARCHLINUX_RATE_LIMITER.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if limiter.consecutive_failures > 0 {
        debug!(
            previous_failures = limiter.consecutive_failures,
            previous_backoff_ms = limiter.current_backoff_ms,
            "resetting archlinux.org backoff after successful request"
        );
    }
    limiter.current_backoff_ms = ARCHLINUX_BASE_DELAY_MS;
    limiter.consecutive_failures = 0;
}
