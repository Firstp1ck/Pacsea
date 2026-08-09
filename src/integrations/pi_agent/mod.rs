//! Optional host Pi coding-agent bridge for the advisory AUR scanner.
//!
//! This module owns the whole Pacsea side of the Pi boundary:
//!
//! - [`protocol`]: strict bounded LF-delimited JSONL framing plus command correlation.
//! - [`capabilities`]: fail-closed CLI/RPC capability probing.
//! - [`process`]: neutral direct-argv startup, trusted-asset verification, and
//!   process-group abort/kill/reap.
//! - [`restricted_tools`]: the four path-confined read-only tools exposed to the model.
//!
//! Security invariants enforced here (see `plans/planned/pi-agent-aur-scanner.md`):
//!
//! - direct argv only; never a shell, an AUR helper, or `makepkg`;
//! - a positive environment allowlist, never a denylist;
//! - no ambient resources, sessions, proxies, credentials, or provider keys;
//! - every process, RPC record, tool request, and tool result is bounded;
//! - anything missing, mismatched, or unparseable fails closed.

pub mod capabilities;
pub mod client;
pub mod process;
pub mod protocol;
pub mod restricted_tools;
pub mod scan_engine;
pub mod session;
pub mod snapshot;

use std::fmt;

/// Minimum supported Pi release. Capability probes remain authoritative above it.
pub const MINIMUM_PI_VERSION: PiVersion = PiVersion {
    major: 0,
    minor: 84,
    patch: 0,
};

/// Exact sorted restricted tool allowlist handed to Pi and expected back from it.
pub const RESTRICTED_TOOL_NAMES: [&str; 4] = [
    "pacsea_scan_find",
    "pacsea_scan_grep",
    "pacsea_scan_ls",
    "pacsea_scan_read",
];

/// Version tag for the embedded extension plus tool argument/result contract.
pub const TOOL_CONTRACT_VERSION: &str = "pacsea-scan-tools-1";

/// Compiled hard maxima from the plan's authoritative resource-bound table.
///
/// Settings may lower these values in later workstreams but may never raise them.
pub mod limits {
    /// Largest accepted single RPC record, derived from the 16 MiB tool-result bound.
    pub const MAX_RPC_RECORD_BYTES: usize = 16 * 1024 * 1024;
    /// Largest accepted final model JSON answer per attempt.
    pub const MAX_FINAL_JSON_BYTES: usize = 4 * 1024 * 1024;
    /// Maximum JSON container nesting accepted from Pi or the model.
    pub const MAX_JSON_DEPTH: usize = 32;
    /// Maximum bytes of one text file eligible for whole-file analysis.
    pub const MAX_ANALYZABLE_TEXT_BYTES: usize = 16 * 1024 * 1024;
    /// Maximum bytes returned by one `pacsea_scan_read` call.
    pub const MAX_READ_BYTES: usize = 64 * 1024;
    /// Maximum matches returned by one `pacsea_scan_grep` call.
    pub const MAX_GREP_MATCHES: usize = 200;
    /// Maximum bytes returned by one `pacsea_scan_grep` call.
    pub const MAX_GREP_BYTES: usize = 128 * 1024;
    /// Maximum entries returned by one `pacsea_scan_find` or `pacsea_scan_ls` call.
    pub const MAX_LISTING_ENTRIES: usize = 500;
    /// Maximum bytes returned by one `pacsea_scan_find` or `pacsea_scan_ls` call.
    pub const MAX_LISTING_BYTES: usize = 128 * 1024;
    /// Maximum path depth accepted inside a snapshot root.
    pub const MAX_PATH_DEPTH: usize = 16;
    /// Maximum tool calls accepted during one model attempt.
    pub const MAX_TOOL_CALLS_PER_ATTEMPT: u32 = 250;
    /// Maximum model attempts per logical scan.
    pub const MAX_MODEL_ATTEMPTS: u32 = 3;
    /// Maximum provider retry attempts per low-level request.
    pub const MAX_PROVIDER_RETRIES: u32 = 3;
    /// Grace period between an RPC abort and process-group termination.
    pub const ABORT_GRACE_SECONDS: u64 = 5;
    /// Total application shutdown abort/kill/reap deadline.
    pub const SHUTDOWN_DEADLINE_SECONDS: u64 = 10;
}

/// What: Three-component Pi semantic version used for the minimum-version gate.
///
/// Inputs: Parsed from `pi --version` by [`capabilities::parse_pi_version`].
///
/// Output: Comparable version value.
///
/// Details:
/// - Ordering is derived field-by-field, so `Ord` matches semantic precedence for
///   the release-only versions Pi emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PiVersion {
    /// Major component.
    pub major: u64,
    /// Minor component.
    pub minor: u64,
    /// Patch component.
    pub patch: u64,
}

impl fmt::Display for PiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// What: Render bytes as lowercase hexadecimal.
///
/// Inputs:
/// - `bytes`: Raw digest or identifier bytes.
///
/// Output:
/// - Lowercase hexadecimal string of exactly `2 * bytes.len()` characters.
///
/// Details:
/// - Used for asset hashes and evidence fingerprints so comparisons stay canonical.
#[must_use]
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Writing to a String is infallible; the result is discarded deliberately.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// What: Compute the SHA-256 digest of a byte slice.
///
/// Inputs:
/// - `bytes`: Content to hash.
///
/// Output:
/// - 32-byte digest.
///
/// Details:
/// - Centralized so extension-asset verification and evidence fingerprints cannot
///   drift onto different hash constructions.
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    use sha2::Digest as _;
    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, bytes);
    let digest = sha2::Digest::finalize(hasher);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

/// What: Report whether a string contains characters that must never cross the Pi boundary.
///
/// Inputs:
/// - `text`: Candidate identity, path, or payload fragment.
///
/// Output:
/// - `true` when the text contains C0/C1 controls, DEL, or Unicode line/paragraph separators.
///
/// Details:
/// - Tab, line feed, and carriage return are rejected together with every other control
///   character because scanner identities, paths, and evidence fields are single-line.
/// - `U+2028`/`U+2029` are rejected so no consumer can re-frame records on them.
#[must_use]
pub fn has_forbidden_control(text: &str) -> bool {
    text.chars()
        .any(|ch| ch.is_control() || ch == '\u{2028}' || ch == '\u{2029}')
}

/// What: Unified failure type for the Pi bridge.
///
/// Inputs: Produced by the capability, protocol, process, and tool layers.
///
/// Output: Implements `Display`/`Error` with actionable, user-facing wording.
///
/// Details:
/// - Every variant is fail-closed: the caller must treat any error as "scanner
///   unavailable" or "attempt rejected" and never as a passing result.
#[derive(Debug)]
pub enum PiAgentError {
    /// The Pi executable, a required flag, or a required RPC command is unusable.
    Capability(capabilities::CapabilityFailure),
    /// An RPC record violated the strict framing or JSON contract.
    Protocol(protocol::ProtocolError),
    /// Process startup, asset verification, or termination failed.
    Process(process::ProcessError),
    /// A restricted tool request was rejected or could not be served.
    Tool(restricted_tools::ToolError),
}

impl fmt::Display for PiAgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capability(inner) => write!(f, "{inner}"),
            Self::Protocol(inner) => write!(f, "{inner}"),
            Self::Process(inner) => write!(f, "{inner}"),
            Self::Tool(inner) => write!(f, "{inner}"),
        }
    }
}

impl std::error::Error for PiAgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capability(inner) => Some(inner),
            Self::Protocol(inner) => Some(inner),
            Self::Process(inner) => Some(inner),
            Self::Tool(inner) => Some(inner),
        }
    }
}

impl From<capabilities::CapabilityFailure> for PiAgentError {
    fn from(value: capabilities::CapabilityFailure) -> Self {
        Self::Capability(value)
    }
}

impl From<protocol::ProtocolError> for PiAgentError {
    fn from(value: protocol::ProtocolError) -> Self {
        Self::Protocol(value)
    }
}

impl From<process::ProcessError> for PiAgentError {
    fn from(value: process::ProcessError) -> Self {
        Self::Process(value)
    }
}

impl From<restricted_tools::ToolError> for PiAgentError {
    fn from(value: restricted_tools::ToolError) -> Self {
        Self::Tool(value)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MINIMUM_PI_VERSION, PiVersion, RESTRICTED_TOOL_NAMES, has_forbidden_control, sha256, to_hex,
    };

    /// Verify the restricted allowlist stays sorted so probe comparisons are canonical.
    #[test]
    fn restricted_tool_names_are_sorted_and_unique() {
        let mut sorted = RESTRICTED_TOOL_NAMES.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, RESTRICTED_TOOL_NAMES.to_vec());
    }

    /// Verify version ordering treats the minimum gate semantically.
    #[test]
    fn version_ordering_is_semantic() {
        let older = PiVersion {
            major: 0,
            minor: 83,
            patch: 9,
        };
        let newer = PiVersion {
            major: 0,
            minor: 84,
            patch: 1,
        };
        assert!(older < MINIMUM_PI_VERSION);
        assert!(newer > MINIMUM_PI_VERSION);
        assert_eq!(MINIMUM_PI_VERSION.to_string(), "0.84.0");
    }

    /// Verify the SHA-256 helper matches the published empty-input digest.
    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            to_hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// Verify control detection covers separators that could re-frame records.
    #[test]
    fn control_detection_covers_reframing_characters() {
        assert!(has_forbidden_control("a\nb"));
        assert!(has_forbidden_control("a\rb"));
        assert!(has_forbidden_control("a\tb"));
        assert!(has_forbidden_control("a\u{0}b"));
        assert!(has_forbidden_control("a\u{7f}b"));
        assert!(has_forbidden_control("a\u{1b}[31m"));
        assert!(has_forbidden_control("a\u{2028}b"));
        assert!(has_forbidden_control("a\u{2029}b"));
        assert!(!has_forbidden_control("pacsea/scan-1"));
    }
}
