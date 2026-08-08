//! Shared arch-toolkit client policy and domain adapters.

use std::sync::Arc;
use std::time::Duration;

use arch_toolkit::{ArchClient, RetryPolicy};

/// AUR-specific arch-toolkit conversions and operations.
pub mod aur;
/// Dependency parsing and resolution adapters.
pub mod deps;
/// Official-index adapters.
pub mod index;
/// Install-planning adapters.
pub mod install;
/// News and advisory adapters.
pub mod news;
/// Sandbox-analysis adapters.
pub mod sandbox;

/// What: Own the configured arch-toolkit clients shared by Pacsea workers.
///
/// Inputs:
/// - Constructed with [`ToolkitContext::new`] at a runtime or CLI boundary.
///
/// Output:
/// - Cloneable access to the AUR client and caller-owned HTTP client.
///
/// Details:
/// - AUR retries and toolkit caches are disabled so Pacsea retains its existing policy.
/// - No global mutable state is used; clones share only the immutable AUR client.
#[derive(Clone, Debug)]
pub struct ToolkitContext {
    /// Configured AUR client shared by runtime workers.
    aur_client: Arc<ArchClient>,
    /// Caller-owned client for toolkit APIs that accept a reqwest client.
    http_client: reqwest::Client,
}

impl ToolkitContext {
    /// What: Build the arch-toolkit clients with Pacsea's explicit network policy.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - A configured context or an actionable construction error.
    ///
    /// Details:
    /// - AUR operations use a 10-second timeout with retries and toolkit caching disabled.
    /// - Other toolkit network operations use a 30-second caller-owned client.
    pub fn new() -> Result<Self, ToolkitContextError> {
        let user_agent = format!("Pacsea/{}", env!("CARGO_PKG_VERSION"));
        let retry_policy = RetryPolicy {
            enabled: false,
            max_retries: 0,
            retry_search: false,
            retry_info: false,
            retry_comments: false,
            retry_pkgbuild: false,
            ..RetryPolicy::default()
        };
        let aur_client = ArchClient::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(&user_agent)
            .retry_policy(retry_policy)
            .build()
            .map_err(|error| {
                ToolkitContextError(format!(
                    "failed to initialize arch-toolkit AUR client: {error}; check TLS and proxy configuration"
                ))
            })?;
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent)
            .build()
            .map_err(|error| {
                ToolkitContextError(format!(
                    "failed to initialize arch-toolkit HTTP client: {error}; check TLS and proxy configuration"
                ))
            })?;
        Ok(Self {
            aur_client: Arc::new(aur_client),
            http_client,
        })
    }

    /// What: Borrow the shared AUR client.
    ///
    /// Inputs:
    /// - `self`: Configured integration context.
    ///
    /// Output:
    /// - Shared arch-toolkit AUR client.
    ///
    /// Details:
    /// - The returned `Arc` can be cloned into asynchronous workers.
    pub const fn aur_client(&self) -> &Arc<ArchClient> {
        &self.aur_client
    }

    /// What: Borrow the caller-owned HTTP client.
    ///
    /// Inputs:
    /// - `self`: Configured integration context.
    ///
    /// Output:
    /// - Reqwest client used by non-AUR toolkit fetchers.
    ///
    /// Details:
    /// - Request-specific byte and candidate bounds remain enforced by toolkit APIs.
    pub const fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}

/// What: Report an actionable integration-client construction failure.
///
/// Inputs:
/// - Contains a human-readable error message.
///
/// Output:
/// - Implements standard display and error contracts.
///
/// Details:
/// - This private boundary prevents arch-toolkit error types from leaking into Pacsea APIs.
#[derive(Debug)]
pub struct ToolkitContextError(String);

impl std::fmt::Display for ToolkitContextError {
    /// What: Render the actionable context error.
    ///
    /// Inputs:
    /// - `self`: Error to render.
    /// - `formatter`: Destination formatter.
    ///
    /// Output:
    /// - Standard formatting result.
    ///
    /// Details:
    /// - No sensitive request data is included.
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToolkitContextError {}

#[cfg(test)]
mod tests {
    /// What: Verify the shared toolkit context can be constructed and cloned.
    ///
    /// Inputs:
    /// - Pacsea's fixed client policy.
    ///
    /// Output:
    /// - A usable clone without global state.
    ///
    /// Details:
    /// - No network request is performed by client construction.
    #[test]
    fn toolkit_context_constructs_and_clones() {
        let context = super::ToolkitContext::new().expect("toolkit context should construct");
        let clone = context.clone();
        assert!(std::sync::Arc::ptr_eq(
            context.aur_client(),
            clone.aur_client()
        ));
    }
}
