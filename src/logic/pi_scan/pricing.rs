//! Exact provider/model route pricing, accounting class, endpoint privacy class, and freshness.
//!
//! This module never performs network access. Catalog bytes and Pi model records are supplied
//! by the caller through the pure parsing seams below, which keeps pricing deterministic and
//! testable and keeps the single approved HTTPS acquisition boundary in the runtime layer.
//!
//! Matching invariants:
//!
//! - Only exact provider/model route matches are accepted. There is no fuzzy, prefix,
//!   normalized, or nearest-neighbour substitution, because charging one model's price for a
//!   different model would silently break the approved cost caps.
//! - Missing pricing is represented explicitly rather than defaulted to zero.
//! - Recognized subscription routes are dollar-accounted as zero and labelled
//!   subscription-backed; they remain fully token-bounded.

use crate::logic::pi_scan::result::UsageAccounting;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::net::IpAddr;
use std::time::Duration;

/// Interval after which a cached pricing catalog must be refetched.
pub const PRICING_REFRESH_INTERVAL: Duration = Duration::from_hours(7 * 24);

/// Micro-USD per one million tokens derived from a per-token USD rate.
///
/// One USD is 1,000,000 micro-USD and one million tokens is 1,000,000 tokens, so a per-token
/// USD rate is scaled by `1e12` to reach micro-USD per million tokens.
const PER_TOKEN_USD_TO_MICROUSD_PER_MILLION: f64 = 1e12;

/// Micro-USD represented by one USD in Pi's per-million-token model rates.
const PER_MILLION_USD_TO_MICROUSD: f64 = 1e6;

/// Upper bound accepted for any parsed rate, guarding against absurd catalog values.
///
/// Written as a float literal so the bound check needs no lossy integer-to-float cast. One
/// million micro-USD per token is already far beyond any published rate.
const MAX_MICROUSD_PER_MILLION: f64 = 1e12;

/// What: Pricing parsing, matching, or accounting failure with actionable guidance.
///
/// Inputs:
/// - Produced while parsing supplied catalog bytes or resolving a route.
///
/// Output:
/// - A message naming the exact route or field that could not be used.
///
/// Details:
/// - Every variant is inert. No variant causes a refetch, a substitution, or a default price.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PricingError {
    /// Supplied catalog bytes are not the expected JSON document shape.
    MalformedCatalog {
        /// Catalog name for the message.
        catalog: String,
        /// Reason parsing failed.
        reason: String,
    },
    /// A catalog record exists but carries an unusable rate field.
    InvalidRate {
        /// Catalog name for the message.
        catalog: String,
        /// Exact route identifier.
        route: String,
        /// Reason the rate was rejected.
        reason: String,
    },
    /// No exact provider/model route exists in the catalog.
    RouteNotFound {
        /// Exact provider identifier requested.
        provider: String,
        /// Exact model identifier requested.
        model: String,
    },
}

impl fmt::Display for PricingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedCatalog { catalog, reason } => write!(
                formatter,
                "the {catalog} pricing catalog could not be read: {reason}. Cached pricing was \
                 kept unchanged; retry the pricing refresh or select a model with known pricing"
            ),
            Self::InvalidRate {
                catalog,
                route,
                reason,
            } => write!(
                formatter,
                "the {catalog} pricing entry for '{route}' is unusable: {reason}. That route was \
                 skipped; choose a different model or retry the pricing refresh"
            ),
            Self::RouteNotFound { provider, model } => write!(
                formatter,
                "no exact pricing entry exists for provider '{provider}' model '{model}'. Pacsea \
                 never substitutes a similar model's price; refresh pricing or pick a model with \
                 published pricing before running a paid scan"
            ),
        }
    }
}

impl std::error::Error for PricingError {}

/// What: Where one exact pricing record came from.
///
/// Inputs: Set when a record is constructed.
///
/// Output: Persisted provenance for cost claims.
///
/// Details:
/// - `PiModelCost` is the primary source. Catalog sources are used only for exact
///   direct-provider or OpenRouter-routed matches.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum PricingSource {
    /// Reported by the host Pi `Model.cost` field.
    PiModelCost,
    /// Exact record from the `LiteLLM` structured cost map.
    LiteLlmCatalog,
    /// Exact record from the `OpenRouter` models endpoint.
    OpenRouterCatalog,
}

impl PricingSource {
    /// Return the catalog name used in user-facing messages.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PiModelCost => "Pi model cost",
            Self::LiteLlmCatalog => "LiteLLM",
            Self::OpenRouterCatalog => "OpenRouter",
        }
    }
}

/// What: How a route's dollar cost is accounted.
///
/// Inputs: Decided by [`classify_accounting`].
///
/// Output: Budget and disclosure input.
///
/// Details:
/// - `SubscriptionBacked` is dollar-accounted as zero but is never described as free API
///   usage, and it remains subject to the authoritative rolling token cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PricingAccounting {
    /// Charged per token at the recorded rates.
    Metered,
    /// Covered by a recognized subscription; dollar cost is zero, tokens still count.
    SubscriptionBacked,
}

impl PricingAccounting {
    /// Return the exact disclosure label shown to the user.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Metered => "Metered per token",
            Self::SubscriptionBacked => "Subscription-backed (not free API usage)",
        }
    }
}

/// What: Privacy class of a custom model endpoint.
///
/// Inputs: Decided by [`classify_endpoint`].
///
/// Output: Privacy disclosure and consent input.
///
/// Details:
/// - Classification is conservative: anything that is not a literal loopback, Unix socket, or
///   literal RFC1918/ULA address is `Remote`, including every custom hostname. A hostname is
///   never resolved here, so a name that happens to point at a private address is still
///   disclosed as remote rather than being silently downgraded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EndpointClass {
    /// Loopback address or Unix domain socket on this machine.
    Local,
    /// Literal RFC1918 or unique-local address.
    PrivateNetwork,
    /// Everything else, including all custom hostnames.
    Remote,
}

impl EndpointClass {
    /// Return the exact disclosure label shown to the user.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Local => "Local",
            Self::PrivateNetwork => "Private network",
            Self::Remote => "Remote",
        }
    }
}

/// What: Freshness of a cached pricing catalog.
///
/// Inputs: Decided by [`classify_freshness`].
///
/// Output: Stale labelling input for the UI and provenance.
///
/// Details:
/// - Stale pricing may continue to be used under the approved policy, but only with an
///   explicit stale label. The rolling token cap remains authoritative either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PricingFreshness {
    /// Fetched within the weekly refresh interval.
    Fresh {
        /// Age of the cached catalog in seconds.
        age_seconds: u64,
    },
    /// Older than the weekly refresh interval and must be labelled stale.
    Stale {
        /// Age of the cached catalog in seconds.
        age_seconds: u64,
    },
}

impl PricingFreshness {
    /// Return whether this catalog must carry the explicit stale label.
    #[must_use]
    pub const fn is_stale(self) -> bool {
        matches!(self, Self::Stale { .. })
    }

    /// Return the exact disclosure label shown to the user.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Fresh { .. } => "Current pricing",
            Self::Stale { .. } => "Stale cached pricing",
        }
    }
}

/// What: Per-million-token rates for one exact route, in micro-USD.
///
/// Inputs: Parsed from a catalog record or a Pi `Model.cost` value.
///
/// Output: Cost estimation input.
///
/// Details:
/// - Integer micro-USD is used so repeated accounting cannot drift the way float dollars do.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TokenRates {
    /// Micro-USD charged per one million input tokens.
    pub input_microusd_per_million: u64,
    /// Micro-USD charged per one million output tokens.
    pub output_microusd_per_million: u64,
}

impl TokenRates {
    /// What: Estimate the micro-USD cost of an exact token split, rounding up.
    ///
    /// Inputs:
    /// - `input_tokens`: Input tokens charged at the input rate.
    /// - `output_tokens`: Output tokens charged at the output rate.
    ///
    /// Output:
    /// - Saturating micro-USD total, rounded up.
    ///
    /// Details:
    /// - Rounding up and saturating arithmetic keep a reservation conservative; an
    ///   under-estimate would let a scan exceed the approved rolling cost cap.
    #[must_use]
    pub const fn estimate_microusd(self, input_tokens: u64, output_tokens: u64) -> u64 {
        let input = self
            .input_microusd_per_million
            .saturating_mul(input_tokens)
            .div_ceil(1_000_000);
        let output = self
            .output_microusd_per_million
            .saturating_mul(output_tokens)
            .div_ceil(1_000_000);
        input.saturating_add(output)
    }

    /// Return whether both recorded rates are exactly zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.input_microusd_per_million == 0 && self.output_microusd_per_million == 0
    }
}

/// What: One exact provider/model route pricing record.
///
/// Inputs: Produced by a parsing seam or by [`pricing_from_pi_model_cost`].
///
/// Output: Persisted pricing provenance and reservation input.
///
/// Details:
/// - `provider` and `model` are stored verbatim. They are compared with exact equality, never
///   normalized, lower-cased, or trimmed for matching purposes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RoutePricing {
    /// Exact provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// Per-million-token rates in micro-USD.
    pub rates: TokenRates,
    /// Where this record came from.
    pub source: PricingSource,
    /// Dollar accounting class for this route.
    pub accounting: PricingAccounting,
}

impl RoutePricing {
    /// What: Estimate the reserved micro-USD cost for one scan.
    ///
    /// Inputs:
    /// - `input_tokens`: Worst-case input tokens.
    /// - `output_tokens`: Worst-case output tokens.
    ///
    /// Output:
    /// - Zero for subscription-backed routes, otherwise the metered estimate.
    ///
    /// Details:
    /// - Subscription routes report zero dollars but the caller still charges tokens against
    ///   the authoritative rolling token cap.
    #[must_use]
    pub const fn reserve_microusd(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        match self.accounting {
            PricingAccounting::SubscriptionBacked => 0,
            PricingAccounting::Metered => self.rates.estimate_microusd(input_tokens, output_tokens),
        }
    }
}

/// What: An exactly-keyed pricing catalog with a fetch timestamp.
///
/// Inputs: Built from parsed records plus the Unix second they were fetched.
///
/// Output: Exact route lookups and freshness labelling.
///
/// Details:
/// - Keyed by the exact `(provider, model)` pair. A lookup miss is an explicit error, never a
///   nearest match and never a zero price.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PricingCatalog {
    /// Exact `(provider, model)` route records.
    pub records: BTreeMap<String, RoutePricing>,
    /// Unix second the catalog bytes were fetched.
    pub fetched_at_unix: u64,
}

/// Build the exact composite key used for route lookups.
fn route_key(provider: &str, model: &str) -> String {
    format!("{provider}\u{1f}{model}")
}

impl PricingCatalog {
    /// What: Build a catalog from parsed records.
    ///
    /// Inputs:
    /// - `records`: Parsed route records.
    /// - `fetched_at_unix`: Unix second the bytes were fetched.
    ///
    /// Output:
    /// - A catalog keyed by exact provider and model.
    ///
    /// Details:
    /// - A later record for the same exact route replaces an earlier one, so a caller can
    ///   layer a direct-provider catalog over a routed one deterministically.
    #[must_use]
    pub fn new(records: Vec<RoutePricing>, fetched_at_unix: u64) -> Self {
        let mut map = BTreeMap::new();
        for record in records {
            map.insert(route_key(&record.provider, &record.model), record);
        }
        Self {
            records: map,
            fetched_at_unix,
        }
    }

    /// What: Look up one route by exact provider and model.
    ///
    /// Inputs:
    /// - `provider`: Exact provider identifier.
    /// - `model`: Exact model identifier.
    ///
    /// Output:
    /// - The exact record.
    ///
    /// Details:
    /// - Exact equality only. Case differences, whitespace differences, prefixes, suffixes,
    ///   and family names never match.
    ///
    /// # Errors
    /// - Returns `PricingError::RouteNotFound` when no exact record exists.
    pub fn lookup_exact(&self, provider: &str, model: &str) -> Result<&RoutePricing, PricingError> {
        self.records
            .get(&route_key(provider, model))
            .ok_or_else(|| PricingError::RouteNotFound {
                provider: provider.to_string(),
                model: model.to_string(),
            })
    }

    /// What: Label this catalog's freshness against the weekly refresh interval.
    ///
    /// Inputs:
    /// - `now_unix`: Current Unix second.
    ///
    /// Output:
    /// - `Fresh` or `Stale` with the observed age.
    ///
    /// Details:
    /// - A catalog with a fetch time in the future is treated as zero-age rather than
    ///   panicking or wrapping.
    #[must_use]
    pub const fn freshness(&self, now_unix: u64) -> PricingFreshness {
        classify_freshness(self.fetched_at_unix, now_unix)
    }
}

/// What: Classify cached catalog freshness against the weekly refresh interval.
///
/// Inputs:
/// - `fetched_at_unix`: Unix second the catalog was fetched.
/// - `now_unix`: Current Unix second.
///
/// Output:
/// - `Fresh` while the age is at most seven days, otherwise `Stale`.
///
/// Details:
/// - Clock skew that puts the fetch time in the future yields zero age instead of wrapping.
#[must_use]
pub const fn classify_freshness(fetched_at_unix: u64, now_unix: u64) -> PricingFreshness {
    let age_seconds = now_unix.saturating_sub(fetched_at_unix);
    if age_seconds > PRICING_REFRESH_INTERVAL.as_secs() {
        PricingFreshness::Stale { age_seconds }
    } else {
        PricingFreshness::Fresh { age_seconds }
    }
}

/// What: Decide the dollar accounting class for one exact route.
///
/// Inputs:
/// - `provider`: Exact provider identifier.
/// - `model`: Exact model identifier.
/// - `subscription_routes`: Explicitly recognized subscription routes as exact
///   `(provider, model)` pairs.
///
/// Output:
/// - `SubscriptionBacked` for an exact recognized route, otherwise `Metered`.
///
/// Details:
/// - Recognition is exact and explicit. An unrecognized route is never assumed to be covered
///   by a subscription, because that assumption would zero out real spend.
#[must_use]
pub fn classify_accounting(
    provider: &str,
    model: &str,
    subscription_routes: &[(String, String)],
) -> PricingAccounting {
    let recognized = subscription_routes
        .iter()
        .any(|(known_provider, known_model)| known_provider == provider && known_model == model);
    if recognized {
        PricingAccounting::SubscriptionBacked
    } else {
        PricingAccounting::Metered
    }
}

/// What: Classify a custom model endpoint into its privacy class.
///
/// Inputs:
/// - `endpoint`: Endpoint string exactly as configured.
///
/// Output:
/// - `Local`, `PrivateNetwork`, or `Remote`.
///
/// Details:
/// - Unix socket forms (`unix:`, `unix://`, `http+unix://`, or an absolute path) and literal
///   loopback addresses are `Local`.
/// - Literal RFC1918 IPv4 and unique-local IPv6 addresses are `PrivateNetwork`.
/// - Every other value, including all hostnames, is `Remote`. No DNS resolution occurs, so a
///   hostname is never optimistically classified as private.
#[must_use]
pub fn classify_endpoint(endpoint: &str) -> EndpointClass {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return EndpointClass::Remote;
    }
    if trimmed.starts_with('/')
        || trimmed.starts_with("unix:")
        || trimmed.starts_with("http+unix://")
        || trimmed.starts_with("https+unix://")
    {
        return EndpointClass::Local;
    }
    let Some(host) = endpoint_host(trimmed) else {
        return EndpointClass::Remote;
    };
    let Ok(address) = host.parse::<IpAddr>() else {
        return EndpointClass::Remote;
    };
    classify_ip_endpoint(address)
}

/// Classify a literal endpoint address into its privacy class.
fn classify_ip_endpoint(address: IpAddr) -> EndpointClass {
    match address {
        IpAddr::V4(value) => {
            if value.is_loopback() {
                EndpointClass::Local
            } else if value.is_private() {
                EndpointClass::PrivateNetwork
            } else {
                EndpointClass::Remote
            }
        }
        IpAddr::V6(value) => {
            if let Some(mapped) = value.to_ipv4_mapped() {
                return classify_ip_endpoint(IpAddr::V4(mapped));
            }
            if value.is_loopback() {
                EndpointClass::Local
            } else if value.segments()[0] & 0xfe00 == 0xfc00 {
                EndpointClass::PrivateNetwork
            } else {
                EndpointClass::Remote
            }
        }
    }
}

/// Extract the bare host portion of an endpoint without resolving it.
fn endpoint_host(endpoint: &str) -> Option<&str> {
    let after_scheme = endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest);
    if after_scheme.contains('@') {
        return None;
    }
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .filter(|value| !value.is_empty())?;
    if let Some(rest) = authority.strip_prefix('[') {
        return rest.split(']').next().filter(|value| !value.is_empty());
    }
    authority
        .rsplit_once(':')
        .map_or(Some(authority), |(host, port)| {
            if port.chars().all(|character| character.is_ascii_digit()) {
                Some(host)
            } else {
                Some(authority)
            }
        })
        .filter(|value| !value.is_empty())
}

/// What: Build a pricing record from the host Pi `Model.cost` value.
///
/// Inputs:
/// - `provider`: Exact provider identifier reported by Pi.
/// - `model`: Exact model identifier reported by Pi.
/// - `cost`: The `Model.cost` JSON value exactly as Pi reported it.
/// - `subscription_routes`: Explicitly recognized subscription routes.
///
/// Output:
/// - The primary-source pricing record for this exact route.
///
/// Details:
/// - Pi reports USD-per-million-token rates under `input`/`output`. Legacy explicit
///   `input_cost_per_token`/`output_cost_per_token` names remain per-token and are scaled
///   independently rather than being confused with Pi's native fields.
/// - A missing or unusable field is an explicit error; it is never defaulted to zero, because
///   a silent zero would disable the cost cap.
///
/// # Errors
/// - Returns `PricingError::InvalidRate` when a rate field is absent, negative, non-finite,
///   or beyond the accepted maximum.
pub fn pricing_from_pi_model_cost(
    provider: &str,
    model: &str,
    cost: &Value,
    subscription_routes: &[(String, String)],
) -> Result<RoutePricing, PricingError> {
    let route = format!("{provider}/{model}");
    let input = read_pi_model_rate(cost, "input", "input_cost_per_token", &route)?;
    let output = read_pi_model_rate(cost, "output", "output_cost_per_token", &route)?;
    Ok(RoutePricing {
        provider: provider.to_string(),
        model: model.to_string(),
        rates: TokenRates {
            input_microusd_per_million: input,
            output_microusd_per_million: output,
        },
        source: PricingSource::PiModelCost,
        accounting: classify_accounting(provider, model, subscription_routes),
    })
}

/// What: Parse exact records from supplied `LiteLLM` structured cost-map bytes.
///
/// Inputs:
/// - `bytes`: The `LiteLLM` cost-map JSON document supplied by the caller.
/// - `subscription_routes`: Explicitly recognized subscription routes.
///
/// Output:
/// - One record per usable entry, keyed by its exact `litellm_provider` and model key.
///
/// Details:
/// - Entries without both per-token rates or without a provider field are skipped rather than
///   guessed, so an incomplete catalog can never produce a fabricated price.
/// - No network access occurs here; the caller supplies the bytes.
///
/// # Errors
/// - Returns `PricingError::MalformedCatalog` when the document is not a JSON object.
pub fn parse_litellm_catalog(
    bytes: &[u8],
    subscription_routes: &[(String, String)],
) -> Result<Vec<RoutePricing>, PricingError> {
    let document: Value =
        serde_json::from_slice(bytes).map_err(|error| PricingError::MalformedCatalog {
            catalog: "LiteLLM".to_string(),
            reason: error.to_string(),
        })?;
    let object = document
        .as_object()
        .ok_or_else(|| PricingError::MalformedCatalog {
            catalog: "LiteLLM".to_string(),
            reason: "the document root must be a JSON object of model entries".to_string(),
        })?;

    let mut records = Vec::new();
    for (model_key, entry) in object {
        let Some(provider) = entry.get("litellm_provider").and_then(Value::as_str) else {
            continue;
        };
        let Ok(input) = read_rate(entry, &["input_cost_per_token"], "LiteLLM", model_key) else {
            continue;
        };
        let Ok(output) = read_rate(entry, &["output_cost_per_token"], "LiteLLM", model_key) else {
            continue;
        };
        records.push(RoutePricing {
            provider: provider.to_string(),
            model: model_key.clone(),
            rates: TokenRates {
                input_microusd_per_million: input,
                output_microusd_per_million: output,
            },
            source: PricingSource::LiteLlmCatalog,
            accounting: classify_accounting(provider, model_key, subscription_routes),
        });
    }
    Ok(records)
}

/// What: Parse exact records from supplied `OpenRouter` models-endpoint bytes.
///
/// Inputs:
/// - `bytes`: The `OpenRouter` `/api/v1/models` JSON document supplied by the caller.
/// - `subscription_routes`: Explicitly recognized subscription routes.
///
/// Output:
/// - One record per usable entry, keyed by the exact `openrouter` provider and model `id`.
///
/// Details:
/// - `OpenRouter` reports per-token USD rates as decimal strings under `pricing.prompt` and
///   `pricing.completion`. Entries missing either field are skipped rather than guessed.
/// - No network access occurs here; the caller supplies the bytes.
///
/// # Errors
/// - Returns `PricingError::MalformedCatalog` when the document has no `data` array.
pub fn parse_openrouter_catalog(
    bytes: &[u8],
    subscription_routes: &[(String, String)],
) -> Result<Vec<RoutePricing>, PricingError> {
    let document: Value =
        serde_json::from_slice(bytes).map_err(|error| PricingError::MalformedCatalog {
            catalog: "OpenRouter".to_string(),
            reason: error.to_string(),
        })?;
    let entries = document
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| PricingError::MalformedCatalog {
            catalog: "OpenRouter".to_string(),
            reason: "the document must contain a 'data' array of model entries".to_string(),
        })?;

    let mut records = Vec::new();
    for entry in entries {
        let Some(model_id) = entry.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(pricing) = entry.get("pricing") else {
            continue;
        };
        let Ok(input) = read_rate(pricing, &["prompt"], "OpenRouter", model_id) else {
            continue;
        };
        let Ok(output) = read_rate(pricing, &["completion"], "OpenRouter", model_id) else {
            continue;
        };
        records.push(RoutePricing {
            provider: OPENROUTER_PROVIDER.to_string(),
            model: model_id.to_string(),
            rates: TokenRates {
                input_microusd_per_million: input,
                output_microusd_per_million: output,
            },
            source: PricingSource::OpenRouterCatalog,
            accounting: classify_accounting(OPENROUTER_PROVIDER, model_id, subscription_routes),
        });
    }
    Ok(records)
}

/// Exact provider identifier used for every OpenRouter-routed record.
pub const OPENROUTER_PROVIDER: &str = "openrouter";

/// Read one Pi native per-million rate or explicit legacy per-token rate.
fn read_pi_model_rate(
    container: &Value,
    native_field: &str,
    legacy_per_token_field: &str,
    route: &str,
) -> Result<u64, PricingError> {
    if let Some(value) = container.get(native_field) {
        let per_million = parse_decimal_rate(value).ok_or_else(|| PricingError::InvalidRate {
            catalog: "Pi model cost".to_string(),
            route: route.to_string(),
            reason: format!("the {native_field} rate is not a finite decimal number"),
        })?;
        return convert_per_million_usd(per_million).ok_or_else(|| PricingError::InvalidRate {
            catalog: "Pi model cost".to_string(),
            route: route.to_string(),
            reason: format!("the rate {per_million} is negative, non-finite, or implausibly large"),
        });
    }
    read_rate(container, &[legacy_per_token_field], "Pi model cost", route)
}

/// Parse one numeric or decimal-string rate without applying unit conversion.
fn parse_decimal_rate(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse::<f64>().ok(),
        _ => None,
    }
}

/// Convert Pi USD-per-million-token rates to micro-USD per million, rounding up.
fn convert_per_million_usd(per_million_usd: f64) -> Option<u64> {
    if !per_million_usd.is_finite() || per_million_usd < 0.0 {
        return None;
    }
    let scaled = (per_million_usd * PER_MILLION_USD_TO_MICROUSD).ceil();
    if scaled > MAX_MICROUSD_PER_MILLION {
        return None;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "value is verified finite, non-negative, and below MAX_MICROUSD_PER_MILLION"
    )]
    Some(scaled as u64)
}

/// Read one per-token USD rate field and convert it to micro-USD per million tokens.
fn read_rate(
    container: &Value,
    field_names: &[&str],
    catalog: &str,
    route: &str,
) -> Result<u64, PricingError> {
    let value = field_names
        .iter()
        .find_map(|name| container.get(*name))
        .ok_or_else(|| PricingError::InvalidRate {
            catalog: catalog.to_string(),
            route: route.to_string(),
            reason: format!("no rate field among {field_names:?} is present"),
        })?;
    let per_token = parse_decimal_rate(value).ok_or_else(|| PricingError::InvalidRate {
        catalog: catalog.to_string(),
        route: route.to_string(),
        reason: "the rate is not a finite decimal number".to_string(),
    })?;
    convert_per_token_usd(per_token).ok_or_else(|| PricingError::InvalidRate {
        catalog: catalog.to_string(),
        route: route.to_string(),
        reason: format!("the rate {per_token} is negative, non-finite, or implausibly large"),
    })
}

/// Convert a per-token USD rate into micro-USD per million tokens, rounding up.
fn convert_per_token_usd(per_token_usd: f64) -> Option<u64> {
    if !per_token_usd.is_finite() || per_token_usd < 0.0 {
        return None;
    }
    let scaled = (per_token_usd * PER_TOKEN_USD_TO_MICROUSD_PER_MILLION).ceil();
    if scaled > MAX_MICROUSD_PER_MILLION {
        return None;
    }
    // The finite, non-negative, and upper-bound checks above make this cast exact-enough and
    // never negative; rounding up keeps the reservation conservative.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "value is verified finite, non-negative, and below MAX_MICROUSD_PER_MILLION"
    )]
    Some(scaled as u64)
}

/// What: Resolve the tokens charged for one scan, preferring reported usage.
///
/// Inputs:
/// - `usage`: Accumulated RPC byte and reported-token accounting.
///
/// Output:
/// - Reported tokens when trustworthy, otherwise the conservative byte-based estimate.
///
/// Details:
/// - Delegates to [`UsageAccounting`] so the approved
///   `ceil(rpc_bytes / 2) + 8,000` formula has exactly one definition in the codebase.
#[must_use]
pub const fn conservative_tokens(usage: UsageAccounting) -> u64 {
    usage.effective_tokens()
}

/// What: Reserve the worst-case micro-USD cost for one scan before it starts.
///
/// Inputs:
/// - `pricing`: The exact route record for the selected model.
/// - `usage`: Worst-case usage accounting for the reservation.
///
/// Output:
/// - Micro-USD to reserve, charging every token at the higher of the two rates.
///
/// Details:
/// - The token split is unknown before the scan runs, so the whole reservation is charged at
///   the more expensive rate. Under-reserving would let a scan exceed the approved cap.
#[must_use]
pub const fn reserve_worst_case_microusd(pricing: &RoutePricing, usage: UsageAccounting) -> u64 {
    let tokens = conservative_tokens(usage);
    let worst_rate =
        if pricing.rates.input_microusd_per_million > pricing.rates.output_microusd_per_million {
            pricing.rates.input_microusd_per_million
        } else {
            pricing.rates.output_microusd_per_million
        };
    match pricing.accounting {
        PricingAccounting::SubscriptionBacked => 0,
        PricingAccounting::Metered => worst_rate.saturating_mul(tokens).div_ceil(1_000_000),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EndpointClass, PricingAccounting, PricingCatalog, PricingError, PricingSource, TokenRates,
        classify_endpoint, classify_freshness, parse_litellm_catalog, parse_openrouter_catalog,
        pricing_from_pi_model_cost, reserve_worst_case_microusd,
    };
    use crate::logic::pi_scan::result::UsageAccounting;

    #[test]
    fn pi_model_cost_is_the_primary_exact_source() {
        let cost = serde_json::json!({ "input": 3.0, "output": 15.0 });
        let record = pricing_from_pi_model_cost("anthropic", "claude-x", &cost, &[])
            .expect("valid cost record");
        assert_eq!(record.source, PricingSource::PiModelCost);
        assert_eq!(record.rates.input_microusd_per_million, 3_000_000);
        assert_eq!(record.rates.output_microusd_per_million, 15_000_000);
    }

    #[test]
    fn lookup_is_exact_and_never_substitutes() {
        let records = parse_litellm_catalog(
            br#"{"claude-x":{"litellm_provider":"anthropic","input_cost_per_token":3e-6,"output_cost_per_token":1.5e-5}}"#,
            &[],
        )
        .expect("catalog parses");
        let catalog = PricingCatalog::new(records, 0);
        assert!(catalog.lookup_exact("anthropic", "claude-x").is_ok());
        for (provider, model) in [
            ("anthropic", "claude-x-2"),
            ("anthropic", "claude"),
            ("Anthropic", "claude-x"),
            ("anthropic", "CLAUDE-X"),
        ] {
            assert!(
                matches!(
                    catalog.lookup_exact(provider, model),
                    Err(PricingError::RouteNotFound { .. })
                ),
                "{provider}/{model} must not fuzzily match"
            );
        }
    }

    #[test]
    fn openrouter_string_rates_parse_exactly() {
        let records = parse_openrouter_catalog(
            br#"{"data":[{"id":"vendor/model","pricing":{"prompt":"0.000003","completion":"0.000015"}}]}"#,
            &[],
        )
        .expect("catalog parses");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider, "openrouter");
        assert_eq!(records[0].rates.output_microusd_per_million, 15_000_000);
    }

    #[test]
    fn subscription_routes_are_zero_dollar_but_labelled() {
        let routes = vec![("vendor".to_string(), "sub-model".to_string())];
        let cost = serde_json::json!({ "input": 3.0, "output": 15.0 });
        let record = pricing_from_pi_model_cost("vendor", "sub-model", &cost, &routes)
            .expect("valid cost record");
        assert_eq!(record.accounting, PricingAccounting::SubscriptionBacked);
        assert_eq!(
            reserve_worst_case_microusd(
                &record,
                UsageAccounting {
                    rpc_bytes: 1_000_000,
                    reported_tokens: None,
                }
            ),
            0
        );
        assert_eq!(
            record.accounting.label(),
            "Subscription-backed (not free API usage)"
        );
    }

    #[test]
    fn endpoints_classify_conservatively() {
        assert_eq!(
            classify_endpoint("http://127.0.0.1:8080"),
            EndpointClass::Local
        );
        assert_eq!(
            classify_endpoint("unix:///run/model.sock"),
            EndpointClass::Local
        );
        assert_eq!(
            classify_endpoint("http://192.168.1.5:1234/v1"),
            EndpointClass::PrivateNetwork
        );
        assert_eq!(
            classify_endpoint("http://[fd00::1]/v1"),
            EndpointClass::PrivateNetwork
        );
        assert_eq!(
            classify_endpoint("https://localhost.my-vendor.example/v1"),
            EndpointClass::Remote
        );
    }

    #[test]
    fn weekly_freshness_labels_stale_cached_pricing() {
        let week = 7 * 24 * 60 * 60;
        assert!(!classify_freshness(0, week).is_stale());
        assert!(classify_freshness(0, week + 1).is_stale());
    }

    #[test]
    fn cost_estimates_round_up() {
        let rates = TokenRates {
            input_microusd_per_million: 1,
            output_microusd_per_million: 0,
        };
        assert_eq!(rates.estimate_microusd(1, 0), 1);
    }
}
