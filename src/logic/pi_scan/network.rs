//! Production bounded HTTPS and DNS adapters for Pi-scan acquisition.
//!
//! Every request uses a fresh Reqwest client with redirects and ambient proxies disabled.
//! The destination host is resolved first, the complete answer set is validated by the
//! acquisition policy, and the selected address is pinned while Reqwest retains the original
//! hostname for HTTP and TLS verification.

use crate::logic::pi_scan::acquisition::{
    AcquisitionError, AddressResolver, AurRpcData, HttpFetcher, HttpRequest, HttpResponse,
};
use crate::logic::pi_scan::head_source::{validate_https_url, validate_public_addresses};
use futures::StreamExt as _;
use serde::Deserialize;
use std::future::Future;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, Instant};

/// Maximum bytes accepted from one AUR RPC response.
pub const MAX_AUR_RPC_BYTES: u64 = 10 * 1024 * 1024;

/// What: Production system-DNS and Reqwest HTTPS adapter.
///
/// Inputs:
/// - Bounded requests supplied by the acquisition layer.
///
/// Output:
/// - Validated DNS answers and single-hop HTTP responses.
///
/// Details:
/// - The async methods are the primary API. The existing synchronous WS8 seams use an
///   isolated helper thread and current-thread Tokio runtime, so they never nest a runtime.
/// - WS9 already executes blocking acquisition adapters through `spawn_blocking`, keeping
///   the UI runtime free while the compatibility seam waits for that helper.
#[derive(Debug, Default, Clone)]
pub struct SystemNetworkAdapter {
    /// Explicit credential-free HTTPS proxy, or direct-only when absent.
    proxy: Option<String>,
}

impl SystemNetworkAdapter {
    /// What: Construct a stateless production network adapter.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - A reusable adapter.
    ///
    /// Details:
    /// - No client is cached, preventing DNS pins from leaking across hops or hosts.
    #[must_use]
    pub const fn new() -> Self {
        Self { proxy: None }
    }

    /// Construct an adapter with one validated credential-free HTTPS proxy.
    ///
    /// # Errors
    /// - Returns when the proxy is not canonical HTTPS or carries forbidden URL components.
    pub fn with_https_proxy(proxy: &str) -> Result<Self, AcquisitionError> {
        let proxy =
            validate_https_url(proxy, false).map_err(|error| network_error(proxy, error.reason))?;
        Ok(Self { proxy: Some(proxy) })
    }

    /// What: Resolve one HTTPS host under a bounded deadline.
    ///
    /// Inputs:
    /// - `host`: TLS/HTTP hostname.
    /// - `port`: Explicit or HTTPS-default port.
    /// - `timeout`: Maximum resolver wall time.
    ///
    /// Output:
    /// - Deduplicated addresses returned by the system resolver.
    ///
    /// Details:
    /// - Empty, mixed-public/private, documentation, link-local, multicast, and other
    ///   special-use answers are rejected before a connection is attempted.
    ///
    /// # Errors
    /// - Returns a network error on timeout, resolution failure, or address-policy failure.
    pub async fn resolve_async(
        host: String,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<IpAddr>, AcquisitionError> {
        let lookup = tokio::net::lookup_host((host.as_str(), port));
        let resolved = tokio::time::timeout(timeout, lookup)
            .await
            .map_err(|_| network_error(&host, "DNS resolution timed out"))?
            .map_err(|error| network_error(&host, format!("DNS resolution failed: {error}")))?;
        let mut addresses: Vec<IpAddr> = resolved.map(|address| address.ip()).collect();
        addresses.sort_unstable();
        addresses.dedup();
        validate_public_addresses(&addresses)
            .map_err(|error| network_error(&host, error.reason))?;
        Ok(addresses)
    }

    /// What: Fetch exactly one already-resolved HTTPS hop with streaming byte bounds.
    ///
    /// Inputs:
    /// - `request`: Canonical URL, validated pinned address, deadline, and byte cap.
    ///
    /// Output:
    /// - Status, optional redirect location, and a bounded terminal body.
    ///
    /// Details:
    /// - Redirects and ambient proxies are disabled. Rustls uses the platform/system trust
    ///   verifier; no custom CA, insecure verifier, or hostname override is installed.
    /// - The DNS override pins the contacted address but leaves the URL hostname unchanged,
    ///   preserving SNI and certificate hostname validation.
    ///
    /// # Errors
    /// - Returns a network error for client construction, timeout, TLS, transport, header,
    ///   content-length, or streaming-limit failures.
    pub async fn fetch_async(
        request: HttpRequest,
        proxy: Option<String>,
    ) -> Result<HttpResponse, AcquisitionError> {
        let parsed = reqwest::Url::parse(&request.url)
            .map_err(|error| network_error(&request.url, format!("malformed URL: {error}")))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| network_error(&request.url, "URL requires a host"))?;
        let port = parsed.port_or_known_default().unwrap_or(443);
        let socket = SocketAddr::new(request.pinned_address, port);
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .https_only(true)
            .tls_backend_rustls()
            .resolve(host, socket);
        if let Some(proxy) = proxy {
            let configured = reqwest::Proxy::https(&proxy).map_err(|error| {
                network_error(&request.url, format!("HTTPS proxy setup failed: {error}"))
            })?;
            builder = builder.proxy(configured);
        }
        let client = builder.build().map_err(|error| {
            network_error(&request.url, format!("HTTPS client setup failed: {error}"))
        })?;
        let started_at = Instant::now();
        let response = tokio::time::timeout(request.timeout, client.get(parsed).send())
            .await
            .map_err(|_| network_error(&request.url, "HTTPS request timed out"))?
            .map_err(|error| {
                network_error(
                    &request.url,
                    format!("HTTPS request failed: {}", reqwest_error_chain(&error)),
                )
            })?;
        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_string)
                    .map_err(|_| network_error(&request.url, "redirect Location is not ASCII"))
            })
            .transpose()?;
        if status != 200 {
            return Ok(HttpResponse {
                status,
                location,
                body: Vec::new(),
            });
        }
        if response
            .content_length()
            .is_some_and(|length| length > request.max_bytes)
        {
            return Err(network_error(
                &request.url,
                format!(
                    "Content-Length exceeds the {}-byte limit",
                    request.max_bytes
                ),
            ));
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        loop {
            let remaining = request
                .timeout
                .checked_sub(started_at.elapsed())
                .filter(|remaining| !remaining.is_zero())
                .ok_or_else(|| network_error(&request.url, "HTTPS response stream timed out"))?;
            let next = tokio::time::timeout(remaining, stream.next())
                .await
                .map_err(|_| network_error(&request.url, "HTTPS response stream timed out"))?;
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.map_err(|error| {
                network_error(&request.url, format!("response stream failed: {error}"))
            })?;
            if (body.len() as u64).saturating_add(chunk.len() as u64) > request.max_bytes {
                return Err(network_error(
                    &request.url,
                    format!("response exceeds the {}-byte limit", request.max_bytes),
                ));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(HttpResponse {
            status,
            location,
            body,
        })
    }
}

impl AddressResolver for SystemNetworkAdapter {
    fn resolve(&mut self, host: &str, port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
        self.resolve_with_timeout(host, port, Duration::from_secs(15))
    }

    fn resolve_with_timeout(
        &mut self,
        host: &str,
        port: u16,
        timeout: Duration,
    ) -> Result<Vec<IpAddr>, AcquisitionError> {
        run_async_worker(
            "pacsea-pi-scan-dns",
            Self::resolve_async(host.to_string(), port, timeout.min(Duration::from_secs(15))),
            host,
        )
    }
}

impl HttpFetcher for SystemNetworkAdapter {
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, AcquisitionError> {
        run_async_worker(
            "pacsea-pi-scan-http",
            Self::fetch_async(request.clone(), self.proxy.clone()),
            &request.url,
        )
    }
}

/// What: Resolve one package name through the bounded official AUR RPC endpoint.
///
/// Inputs:
/// - `network`: Production network adapter.
/// - `package_name`: Already validated package name text.
///
/// Output:
/// - WS8 RPC mapping data containing only exact returned name/base pairs.
///
/// Details:
/// - URL query encoding is performed by `Url::query_pairs_mut`, never by string or shell
///   interpolation. The response is bounded to 10 MiB before JSON parsing.
///
/// # Errors
/// - Returns a network error for transport, status, or schema failures and an unresolved-package
///   error when the official AUR has no exact package-name result.
pub fn fetch_aur_rpc_package_base(
    network: &mut SystemNetworkAdapter,
    package_name: &str,
) -> Result<AurRpcData, AcquisitionError> {
    fetch_aur_rpc_package_base_with_timeout(network, package_name, Duration::from_secs(30))
}

/// Resolve one package name with a caller-bounded whole-cycle timeout.
///
/// # Errors
/// - Returns a network error for transport, status, or schema failures and an unresolved-package
///   error when the official AUR has no exact package-name result.
pub fn fetch_aur_rpc_package_base_with_timeout(
    network: &mut SystemNetworkAdapter,
    package_name: &str,
    timeout: Duration,
) -> Result<AurRpcData, AcquisitionError> {
    let mut url = reqwest::Url::parse("https://aur.archlinux.org/rpc/v5/info")
        .map_err(|error| network_error("AUR RPC", error.to_string()))?;
    url.query_pairs_mut().append_pair("arg[]", package_name);
    let mut resolver = network.clone();
    let downloaded = crate::logic::pi_scan::acquisition::download_static_source(
        network,
        &mut resolver,
        url.as_str(),
        MAX_AUR_RPC_BYTES,
        timeout.min(Duration::from_secs(30)),
    )?;
    parse_aur_rpc_package_base(&downloaded.bytes, package_name, url.as_str())
}

/// What: Parse one bounded AUR RPC info response into exact package/base mapping data.
///
/// Inputs:
/// - `bytes`: Already bounded response body.
/// - `package_name`: Exact requested package name.
/// - `context`: Sanitized request URL used only in typed diagnostics.
///
/// Output:
/// - Exact matching package/base pair, or a typed acquisition failure.
///
/// Details:
/// - Non-matching results never establish identity and are discarded.
fn parse_aur_rpc_package_base(
    bytes: &[u8],
    package_name: &str,
    context: &str,
) -> Result<AurRpcData, AcquisitionError> {
    let response: AurRpcResponse = serde_json::from_slice(bytes)
        .map_err(|error| network_error(context, format!("invalid AUR RPC JSON: {error}")))?;
    let pairs: Vec<(&str, &str)> = response
        .results
        .iter()
        .filter(|result| result.name == package_name)
        .map(|result| (result.name.as_str(), result.package_base.as_str()))
        .collect();
    if pairs.is_empty() {
        return Err(AcquisitionError::PackageBaseUnresolved {
            package_name: package_name.to_string(),
            reason: "official AUR RPC returned no exact package-name result".to_string(),
        });
    }
    Ok(AurRpcData::from_pairs(&pairs))
}

/// Minimal bounded AUR RPC response schema.
#[derive(Debug, Deserialize)]
struct AurRpcResponse {
    /// Exact package results returned by the info endpoint.
    results: Vec<AurRpcResult>,
}

/// Minimal identity fields consumed from one AUR RPC result.
#[derive(Debug, Deserialize)]
struct AurRpcResult {
    /// Exact package name.
    #[serde(rename = "Name")]
    name: String,
    /// Declared package base.
    #[serde(rename = "PackageBase")]
    package_base: String,
}

/// What: Render a bounded Reqwest source chain for actionable transport diagnostics.
///
/// Inputs:
/// - `error`: Top-level Reqwest request failure.
///
/// Output:
/// - Colon-separated error and source details, capped at four nested causes.
///
/// Details:
/// - Reqwest's display text often stops at `error sending request for url`; nested causes expose
///   whether DNS, TCP, TLS, routing, or timeout handling actually failed.
fn reqwest_error_chain(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    for _ in 0..4 {
        let Some(cause) = source else {
            break;
        };
        let detail = cause.to_string();
        if !message.ends_with(&detail) {
            message.push_str(": ");
            message.push_str(&detail);
        }
        source = cause.source();
    }
    message
}

/// Run one owned async operation on an isolated helper runtime.
fn run_async_worker<T, F>(
    thread_name: &str,
    future: F,
    context: &str,
) -> Result<T, AcquisitionError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, AcquisitionError>> + Send + 'static,
{
    let handle = std::thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| network_error("network runtime", error.to_string()))?;
            runtime.block_on(future)
        })
        .map_err(|error| network_error(context, format!("network worker failed: {error}")))?;
    handle
        .join()
        .map_err(|_| network_error(context, "network worker panicked"))?
}

/// Construct a consistent acquisition network error.
fn network_error(context: &str, reason: impl Into<String>) -> AcquisitionError {
    AcquisitionError::Network {
        url: context.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_aur_rpc_package_base;
    use crate::logic::pi_scan::acquisition::AcquisitionError;

    /// An empty successful info response identifies a non-AUR package, not a network outage.
    #[test]
    fn empty_info_response_is_typed_as_unresolved_package() {
        let result = parse_aur_rpc_package_base(
            br#"{"resultcount":0,"results":[],"type":"multiinfo","version":5}"#,
            "qml-vulkan",
            "https://aur.archlinux.org/rpc/v5/info",
        );

        assert!(matches!(
            result,
            Err(AcquisitionError::PackageBaseUnresolved { package_name, .. })
                if package_name == "qml-vulkan"
        ));
    }
}
