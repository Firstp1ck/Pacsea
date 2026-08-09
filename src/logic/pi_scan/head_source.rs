//! Pure URL, redirect, immutable VCS identity, and public-address policy helpers.

use crate::logic::pi_scan::identity::CommitOid;
use reqwest::Url;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Maximum HTTPS redirects accepted by source acquisition policy.
pub const MAX_SOURCE_REDIRECTS: usize = 5;

/// What: A validated source locator with complete or explicitly incomplete identity.
///
/// Inputs:
/// - Static HTTPS URL or `git+https` URL.
///
/// Output:
/// - Transport-specific immutable policy representation.
///
/// Details:
/// - Git locators are complete only with an exact `#commit=<40-hex-oid>` fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceLocator {
    /// Static source reachable through HTTPS.
    StaticHttps {
        /// Canonical parsed URL string.
        url: String,
    },
    /// Git repository reachable through HTTPS at one immutable commit.
    GitHttps {
        /// Canonical repository URL without a fragment.
        repository_url: String,
        /// Full immutable Git commit identity.
        commit_oid: CommitOid,
    },
    /// Git repository reachable through HTTPS at a mutable ref resolved for advisory scanning.
    MutableGitHttps {
        /// Exact source declaration retained for provenance.
        declaration: String,
        /// Canonical repository URL without a fragment.
        repository_url: String,
        /// Fully qualified branch/tag ref or `HEAD`.
        reference: String,
    },
    /// Syntactically valid but unsupported source.
    Incomplete {
        /// Exact source declaration.
        declaration: String,
        /// Reason complete acquisition is impossible.
        reason: String,
    },
}

/// What: URL, redirect, DNS-address, or immutable-source policy failure.
///
/// Inputs:
/// - Explicit URL or IP inputs supplied by an external acquisition adapter.
///
/// Output:
/// - Inert validation error; no resolver or network operation is performed.
///
/// Details:
/// - Separates pure validation from any future executor and prevents accidental ambient networking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourcePolicyError {
    /// Actionable policy violation.
    pub reason: String,
}

impl fmt::Display for SourcePolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Source policy rejected input: {}", self.reason)
    }
}

impl std::error::Error for SourcePolicyError {}

/// What: Classify one `.SRCINFO` locator without performing network or Git operations.
///
/// Inputs:
/// - `declaration`: Locator text after any `name::` prefix is removed.
///
/// Output:
/// - Static HTTPS, immutable Git HTTPS, or explicit incomplete policy state.
///
/// Details:
/// - Unsupported schemes and mutable Git refs are represented as incomplete rather than fabricated.
#[must_use]
pub fn classify_source_locator(declaration: &str) -> SourceLocator {
    if let Some(git_url) = declaration.strip_prefix("git+") {
        return classify_git_locator(declaration, git_url);
    }
    match validate_https_url(declaration, false) {
        Ok(url) => SourceLocator::StaticHttps { url },
        Err(error) => SourceLocator::Incomplete {
            declaration: declaration.to_string(),
            reason: error.reason,
        },
    }
}

/// Classify a Git HTTPS locator and require one full immutable commit fragment.
fn classify_git_locator(original: &str, git_url: &str) -> SourceLocator {
    let Ok(mut parsed) = parse_https_url(git_url) else {
        return SourceLocator::Incomplete {
            declaration: original.to_string(),
            reason: "git source must use git+https with no userinfo".to_string(),
        };
    };
    let Some(fragment) = parsed.fragment().map(str::to_string) else {
        parsed.set_fragment(None);
        return SourceLocator::MutableGitHttps {
            declaration: original.to_string(),
            repository_url: parsed.to_string(),
            reference: "HEAD".to_string(),
        };
    };
    if let Some((kind, value)) = fragment.split_once('=')
        && matches!(kind, "branch" | "tag")
        && valid_mutable_git_ref(value)
    {
        parsed.set_fragment(None);
        let namespace = if kind == "branch" { "heads" } else { "tags" };
        return SourceLocator::MutableGitHttps {
            declaration: original.to_string(),
            repository_url: parsed.to_string(),
            reference: format!("refs/{namespace}/{value}"),
        };
    }
    let Some(oid_text) = fragment.strip_prefix("commit=") else {
        return SourceLocator::Incomplete {
            declaration: original.to_string(),
            reason: "git source fragment must be branch=<ref>, tag=<ref>, or commit=<full-oid>"
                .to_string(),
        };
    };
    let Ok(commit_oid) = CommitOid::new(oid_text) else {
        return SourceLocator::Incomplete {
            declaration: original.to_string(),
            reason: "git commit identity must be a full 40-hex OID".to_string(),
        };
    };
    parsed.set_fragment(None);
    SourceLocator::GitHttps {
        repository_url: parsed.to_string(),
        commit_oid,
    }
}

/// Validate a branch/tag name conservatively without invoking Git.
fn valid_mutable_git_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && !value.starts_with('-')
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.ends_with('.')
        && !std::path::Path::new(value)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("lock"))
        && !value.contains("..")
        && !value.contains("@{")
        && !value.contains("//")
        && !value.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '~' | '^' | ':' | '?' | '*' | '[' | '\\')
        })
}

/// What: Validate one source URL as HTTPS with no credentials or forbidden fragment.
///
/// Inputs:
/// - `input`: Explicit URL string.
/// - `allow_fragment`: Whether a caller-owned fragment grammar will be checked separately.
///
/// Output:
/// - Canonical URL serialization.
///
/// Details:
/// - Performs no DNS lookup and permits no scheme downgrade or URL userinfo.
///
/// # Errors
/// Returns `SourcePolicyError` for malformed, non-HTTPS, credential-bearing, or fragment-bearing input.
pub fn validate_https_url(input: &str, allow_fragment: bool) -> Result<String, SourcePolicyError> {
    let parsed = parse_https_url(input)?;
    if parsed.fragment().is_some() && !allow_fragment {
        return Err(policy_error(
            "URL fragments are forbidden for static sources",
        ));
    }
    Ok(parsed.to_string())
}

/// Parse common HTTPS policy shared by initial and redirect URLs.
fn parse_https_url(input: &str) -> Result<Url, SourcePolicyError> {
    if input.chars().any(char::is_control) {
        return Err(policy_error("URL contains a control character"));
    }
    let parsed =
        Url::parse(input).map_err(|error| policy_error(format!("malformed URL: {error}")))?;
    if parsed.scheme() != "https" {
        return Err(policy_error("only HTTPS URLs are allowed"));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(policy_error("URL userinfo or credentials are forbidden"));
    }
    if parsed.host_str().is_none() {
        return Err(policy_error("URL requires a host"));
    }
    Ok(parsed)
}

/// What: Validate a complete explicit redirect chain before any response body is accepted.
///
/// Inputs:
/// - `urls`: Initial URL followed by each redirect destination.
///
/// Output:
/// - Canonical URL chain with at most five redirects.
///
/// Details:
/// - Every hop independently requires HTTPS, no userinfo, and no fragments.
///
/// # Errors
/// Returns `SourcePolicyError` for an empty, oversized, or invalid chain.
pub fn validate_redirect_chain(urls: &[impl AsRef<str>]) -> Result<Vec<String>, SourcePolicyError> {
    if urls.is_empty() {
        return Err(policy_error("redirect chain cannot be empty"));
    }
    if urls.len() - 1 > MAX_SOURCE_REDIRECTS {
        return Err(policy_error(format!(
            "redirect chain exceeds {MAX_SOURCE_REDIRECTS} redirects"
        )));
    }
    urls.iter()
        .map(|url| validate_https_url(url.as_ref(), false))
        .collect()
}

/// What: Validate explicit DNS answers supplied by a network adapter.
///
/// Inputs:
/// - `addresses`: Every address returned for the destination host.
///
/// Output:
/// - Success only when the set is non-empty and every address is public Internet space.
///
/// Details:
/// - The check must be repeated for every redirect destination and connection attempt.
///
/// # Errors
/// Returns `SourcePolicyError` for empty answers or any non-public address.
pub fn validate_public_addresses(addresses: &[IpAddr]) -> Result<(), SourcePolicyError> {
    if addresses.is_empty() {
        return Err(policy_error("DNS answer set is empty"));
    }
    if let Some(address) = addresses.iter().find(|address| !is_public_ip(**address)) {
        return Err(policy_error(format!(
            "destination address {address} is not public Internet space"
        )));
    }
    Ok(())
}

/// What: Determine whether an explicit address is eligible for public-Internet acquisition.
///
/// Inputs:
/// - `address`: IPv4 or IPv6 address supplied directly by a resolver adapter.
///
/// Output:
/// - `true` only for addresses outside compiled special-use and non-routable ranges.
///
/// Details:
/// - IPv4-mapped IPv6 addresses are evaluated using the IPv4 policy.
#[must_use]
pub fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map_or_else(|| is_public_ipv6(address), is_public_ipv4),
    }
}

/// Apply the compiled IPv4 special-use denylist.
fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let value = u32::from(address);
    !IPV4_NON_PUBLIC_RANGES
        .iter()
        .any(|(network, prefix)| prefix_matches_u32(value, *network, *prefix))
}

/// Apply the compiled IPv6 special-use denylist.
fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let value = u128::from(address);
    !IPV6_NON_PUBLIC_RANGES
        .iter()
        .any(|(network, prefix)| prefix_matches_u128(value, *network, *prefix))
}

/// Compare one IPv4 value against a CIDR prefix.
const fn prefix_matches_u32(value: u32, network: u32, prefix: u8) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    };
    value & mask == network & mask
}

/// Compare one IPv6 value against a CIDR prefix.
const fn prefix_matches_u128(value: u128, network: u128, prefix: u8) -> bool {
    let mask = if prefix == 0 {
        0
    } else {
        u128::MAX << (128 - prefix)
    };
    value & mask == network & mask
}

/// Build a compact source-policy error.
fn policy_error(reason: impl Into<String>) -> SourcePolicyError {
    SourcePolicyError {
        reason: reason.into(),
    }
}

/// Compiled IPv4 special-use ranges forbidden as acquisition destinations.
const IPV4_NON_PUBLIC_RANGES: &[(u32, u8)] = &[
    (0x0000_0000, 8),
    (0x0a00_0000, 8),
    (0x6440_0000, 10),
    (0x7f00_0000, 8),
    (0xa9fe_0000, 16),
    (0xac10_0000, 12),
    (0xc000_0000, 24),
    (0xc000_0200, 24),
    (0xc0a8_0000, 16),
    (0xc612_0000, 15),
    (0xc633_6400, 24),
    (0xcb00_7100, 24),
    (0xe000_0000, 4),
    (0xf000_0000, 4),
];

/// Compiled IPv6 special-use ranges forbidden as acquisition destinations.
const IPV6_NON_PUBLIC_RANGES: &[(u128, u8)] = &[
    (0, 8),
    (0x0064_ff9b_0000_0000_0000_0000_0000_0000, 96),
    (0x0064_ff9b_0001_0000_0000_0000_0000_0000, 48),
    (0x0100_0000_0000_0000_0000_0000_0000_0000, 64),
    (0x2001_0000_0000_0000_0000_0000_0000_0000, 23),
    (0x2001_0db8_0000_0000_0000_0000_0000_0000, 32),
    (0x3fff_0000_0000_0000_0000_0000_0000_0000, 20),
    (0xfc00_0000_0000_0000_0000_0000_0000_0000, 7),
    (0xfe80_0000_0000_0000_0000_0000_0000_0000, 10),
    (0xff00_0000_0000_0000_0000_0000_0000_0000, 8),
];
