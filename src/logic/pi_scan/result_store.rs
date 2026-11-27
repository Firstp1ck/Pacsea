//! Private, atomic, versioned storage for validated Pi scan results.
//!
//! Storage invariants enforced here:
//!
//! - only validated, canonical, typed data is persisted: identity, coverage, merged findings,
//!   provenance, and manifests. No raw prompt, source body, thinking trace, invalid response,
//!   or original assistant text ever reaches disk;
//! - every path is confined under the caller-supplied results root and is rejected if it
//!   escapes, traverses, or is absolute;
//! - directories are mode 0700 and files mode 0600 on Unix, and every write is atomic;
//! - loading distinguishes missing, corrupt, unsupported-newer, and I/O failure, and moves
//!   corrupt or newer artifacts into quarantine instead of treating them as empty;
//! - retention keeps the newest detailed result and the accepted baseline result when they
//!   differ, applies the 30-day rule only to other superseded results, runs only after a
//!   successful load and atomic commit, and never deletes a quarantine artifact.

use crate::logic::pi_scan::identity::{CommitOid, PackageBase};
use crate::logic::pi_scan::manifest::CanonicalManifest;
use crate::logic::pi_scan::result::{
    Coverage, ExpectedIdentity, FindingAssessment, MergedFinding, MergedScanResult, ScanProvenance,
    Severity,
};
use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Component, Path, PathBuf};

/// Schema version of one persisted scan result document.
pub const RESULT_SCHEMA_VERSION: u32 = 1;

/// Directory name holding versioned per-package result documents.
pub const RESULTS_DIR_NAME: &str = "results-v1";

/// Directory name holding quarantined artifacts, which are never deleted automatically.
pub const QUARANTINE_DIR_NAME: &str = "quarantine";

/// Default retention window applied to superseded result documents.
pub const DEFAULT_RETENTION_DAYS: u64 = 30;

/// Maximum detailed result documents loaded into one startup projection.
pub const MAX_RESTORED_RESULTS: usize = 5_000;

/// Maximum accepted scan identifier length.
pub const MAX_SCAN_ID_LENGTH: usize = 64;

/// Maximum accepted size of one persisted result document.
pub const MAX_RESULT_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

/// Object keys that must never appear anywhere in a persisted result document.
///
/// These are exactly the categories the plan forbids persisting. The check is by exact key
/// name so a legitimate field such as `prompt_version` is not confused with a raw prompt.
pub const FORBIDDEN_RAW_FIELDS: [&str; 10] = [
    "prompt",
    "raw",
    "raw_response",
    "raw_output",
    "response",
    "assistant_text",
    "thinking",
    "reasoning",
    "source_body",
    "file_content",
];

/// What: Result storage failure with actionable guidance.
///
/// Inputs:
/// - Produced while validating a path, writing, loading, quarantining, or pruning.
///
/// Output:
/// - A message naming the affected artifact and the user's next step.
///
/// Details:
/// - Constructing an error never mutates durable state and never deletes anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultStoreError {
    /// A supplied identity would escape the results root or is otherwise unusable as a path.
    UnsafePath {
        /// Rejected component, shown verbatim.
        component: String,
        /// Reason the component was rejected.
        reason: String,
    },
    /// The stored document does not exist.
    Missing {
        /// Affected path.
        path: String,
    },
    /// The stored document is malformed and was quarantined.
    Corrupt {
        /// Affected path.
        path: String,
        /// Reason decoding failed.
        reason: String,
        /// Quarantine artifact path, when quarantine succeeded.
        quarantined_to: Option<String>,
    },
    /// The stored document uses a newer schema version and was quarantined.
    UnsupportedNewerVersion {
        /// Affected path.
        path: String,
        /// Version observed in the document.
        observed: u32,
        /// Maximum version this build supports.
        max_supported: u32,
        /// Quarantine artifact path, when quarantine succeeded.
        quarantined_to: Option<String>,
    },
    /// A filesystem operation failed.
    Io {
        /// Affected path.
        path: String,
        /// Underlying message.
        message: String,
    },
    /// Quarantine could not be completed, so the original was left untouched.
    QuarantineFailed {
        /// Affected path.
        path: String,
        /// Underlying message.
        message: String,
    },
}

impl fmt::Display for ResultStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsafePath { component, reason } => write!(
                formatter,
                "the scan result identity '{component}' is unsafe: {reason}. Nothing was written; \
                 retry the scan or reset this package's scan results"
            ),
            Self::Missing { path } => write!(
                formatter,
                "no stored scan result exists at {path}. Run a new scan for this package"
            ),
            Self::Corrupt {
                path,
                reason,
                quarantined_to,
            } => match quarantined_to {
                Some(target) => write!(
                    formatter,
                    "the stored scan result {path} is unreadable ({reason}) and was moved to \
                     {target}. It was not treated as an empty or clean result; re-run the scan"
                ),
                None => write!(
                    formatter,
                    "the stored scan result {path} is unreadable: {reason}. It was left in place; \
                     re-run the scan or reset this package's scan results"
                ),
            },
            Self::UnsupportedNewerVersion {
                path,
                observed,
                max_supported,
                quarantined_to,
            } => match quarantined_to {
                Some(target) => write!(
                    formatter,
                    "the stored scan result {path} uses schema version {observed}, newer than the \
                     supported {max_supported}, and was moved to {target}. Update Pacsea to read it"
                ),
                None => write!(
                    formatter,
                    "the stored scan result {path} uses schema version {observed}, newer than the \
                     supported {max_supported}. Update Pacsea to read it"
                ),
            },
            Self::Io { path, message } => write!(
                formatter,
                "a filesystem operation on {path} failed: {message}. Check that the Pacsea \
                 configuration directory exists and is writable"
            ),
            Self::QuarantineFailed { path, message } => write!(
                formatter,
                "the damaged scan result {path} could not be quarantined: {message}. The original \
                 was left untouched and scan results stay unavailable until this is resolved"
            ),
        }
    }
}

impl std::error::Error for ResultStoreError {}

/// What: One model's assessment of a stored finding.
///
/// Inputs: Converted from a validated merged finding assessment.
///
/// Output: Canonical persisted attribution.
///
/// Details:
/// - Model wording is retained verbatim only because it already passed strict validation,
///   including control-character and length checks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredAssessment {
    /// Exact provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// Severity this model reported.
    pub severity: String,
    /// Title this model reported.
    pub title: String,
    /// Rationale this model reported.
    pub rationale: String,
    /// Recommendation this model reported.
    pub recommendation: String,
}

/// What: One canonical stored finding.
///
/// Inputs: Converted from a validated merged finding.
///
/// Output: Canonical persisted finding.
///
/// Details:
/// - `evidence` is the verbatim text that validation already proved occurs in the cited
///   manifest entry, not an arbitrary source body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredFinding {
    /// Exact-evidence fingerprint.
    pub fingerprint: String,
    /// Highest reported severity.
    pub severity: String,
    /// Cited snapshot identifier.
    pub snapshot: String,
    /// Cited snapshot-relative path.
    pub path: String,
    /// Verbatim proven evidence text.
    pub evidence: String,
    /// Per-model assessments in stable order.
    pub assessments: Vec<StoredAssessment>,
    /// Whether models disagreed about severity.
    pub disagreement: bool,
}

/// What: One stored model attempt record.
///
/// Inputs: Converted from a validated attempt record.
///
/// Output: Canonical persisted provenance detail.
///
/// Details:
/// - Failed and corrected attempts are stored too, so cost and coverage stay auditable.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredAttempt {
    /// Exact provider identifier.
    pub provider: String,
    /// Exact model identifier.
    pub model: String,
    /// Whether this attempt produced a validated result.
    pub validated: bool,
    /// Whether the single bounded correction was used.
    pub corrected: bool,
    /// Total RPC bytes exchanged for this attempt.
    pub rpc_bytes: u64,
    /// Tokens charged for this attempt after conservative fallback.
    pub effective_tokens: u64,
}

/// What: Stored provenance binding a result to its exact execution context.
///
/// Inputs: Converted from validated scan provenance.
///
/// Output: Canonical persisted provenance.
///
/// Details:
/// - This is what makes a stale or drifted result detectable after Pi, the prompt, the schema,
///   or the tool contract changes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredProvenance {
    /// Verified Pi version string.
    pub pi_version: String,
    /// Verified extension asset SHA-256.
    pub extension_sha256: String,
    /// Prompt contract version used.
    pub prompt_version: String,
    /// Result schema contract version used.
    pub result_schema_version: String,
    /// Tool contract version used.
    pub tool_contract_version: String,
    /// Every model attempt, in attempt order.
    pub attempts: Vec<StoredAttempt>,
}

/// What: One stored snapshot manifest bound to the result.
///
/// Inputs: Converted from a canonical manifest.
///
/// Output: Canonical persisted manifest binding.
///
/// Details:
/// - Manifests carry only path, size, digest, and classification metadata. They never carry
///   file contents, so persisting them cannot leak hostile source bodies.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredManifest {
    /// Canonical SHA-256 of the manifest.
    pub manifest_hash: String,
    /// Canonically sorted manifest.
    pub manifest: CanonicalManifest,
}

/// What: One fully validated, canonical, persisted scan result document.
///
/// Inputs: Built by [`StoredScanResult::from_validated`].
///
/// Output: The exact JSON document written under `results-v1`.
///
/// Details:
/// - Every field here is derived from data that already passed strict validation. There is
///   deliberately no field for a raw prompt, raw response, thinking trace, or source body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredScanResult {
    /// Document schema version.
    pub schema_version: u32,
    /// Scan identity.
    pub scan_id: String,
    /// Canonical package base.
    pub package_base: String,
    /// Full immutable recipe commit OID.
    pub commit_oid: String,
    /// Official AUR HEAD frozen when the scan target was created.
    #[serde(default)]
    pub observed_head_oid: String,
    /// Coverage claim after merging.
    pub coverage: String,
    /// Approved completion wording.
    pub completion_wording: String,
    /// Sorted union of limitation notes.
    pub limitations: Vec<String>,
    /// Merged findings sorted by fingerprint.
    pub findings: Vec<StoredFinding>,
    /// Execution provenance.
    pub provenance: StoredProvenance,
    /// Bound snapshot manifests.
    pub manifests: Vec<StoredManifest>,
    /// Unix second the document was produced.
    pub stored_at_unix: u64,
    /// Whether this result is the currently accepted observation baseline.
    pub accepted_baseline: bool,
    /// Whether exact post-execution HEAD or mutable-source rechecks changed.
    #[serde(default)]
    pub stale: bool,
    /// Mutable Git refs resolved during advisory acquisition.
    #[serde(default)]
    pub mutable_sources: Vec<crate::logic::pi_scan::acquisition::MutableSourceIdentity>,
}

impl StoredScanResult {
    /// Reconstruct the canonical typed result after validating persisted enum and identity fields.
    ///
    /// # Errors
    /// - Returns a semantic corruption reason for invalid identity, enum, path, or control text.
    pub fn to_merged_result(&self) -> Result<MergedScanResult, String> {
        if self.schema_version != RESULT_SCHEMA_VERSION {
            return Err(format!(
                "stored result schema {} does not equal supported schema {RESULT_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        validate_scan_id(&self.scan_id).map_err(|error| error.to_string())?;
        PackageBase::new(self.package_base.clone()).map_err(|error| error.to_string())?;
        CommitOid::new(self.commit_oid.clone()).map_err(|error| error.to_string())?;
        if !self.observed_head_oid.is_empty() {
            CommitOid::new(self.observed_head_oid.clone()).map_err(|error| error.to_string())?;
        }
        let coverage = parse_stored_coverage(&self.coverage)
            .ok_or_else(|| format!("stored result has unknown coverage {:?}", self.coverage))?;
        let findings = self
            .findings
            .iter()
            .map(stored_finding_to_merged)
            .collect::<Result<Vec<_>, _>>()?;
        if self
            .limitations
            .iter()
            .any(|text| crate::pi_agent::has_forbidden_control(text))
        {
            return Err("stored result limitation contains a terminal control".to_string());
        }
        Ok(MergedScanResult {
            identity: ExpectedIdentity {
                scan_id: self.scan_id.clone(),
                package_base: self.package_base.clone(),
                commit_oid: self.commit_oid.clone(),
            },
            coverage,
            limitations: self.limitations.clone(),
            findings,
        })
    }

    /// What: Build a canonical stored document from validated typed data only.
    ///
    /// Inputs:
    /// - `scan_id`: Scan identifier, validated as a safe single path segment.
    /// - `merged`: The validated merged multi-model result.
    /// - `provenance`: The validated execution provenance.
    /// - `manifests`: Canonical snapshot manifests bound to this result.
    /// - `stored_at_unix`: Unix second the document is produced.
    /// - `accepted_baseline`: Whether this result is the accepted baseline.
    ///
    /// Output:
    /// - The canonical document ready for atomic persistence.
    ///
    /// Details:
    /// - The identity is taken from the merged result, which validation already proved matches
    ///   the frozen scan identity, so a stale response cannot be stored against another scan.
    /// - Nothing raw is accepted as input, so nothing raw can be written.
    ///
    /// # Errors
    /// - Returns `ResultStoreError::UnsafePath` when the scan id or package base is not a safe
    ///   single path segment.
    pub fn from_validated(
        scan_id: &str,
        merged: &MergedScanResult,
        provenance: &ScanProvenance,
        manifests: &[CanonicalManifest],
        stored_at_unix: u64,
        accepted_baseline: bool,
    ) -> Result<Self, ResultStoreError> {
        Self::from_validated_with_staleness(
            scan_id,
            merged,
            provenance,
            manifests,
            stored_at_unix,
            accepted_baseline,
            merged.identity.commit_oid.as_str(),
            false,
        )
    }

    /// What: Build a canonical stored document with exact stale-recheck state.
    ///
    /// Inputs:
    /// - `scan_id`: Safe scan identifier.
    /// - `merged`: Strictly validated merged result.
    /// - `provenance`: Validated Pi/model/tool provenance.
    /// - `manifests`: Canonical bound manifests.
    /// - `stored_at_unix`: Result timestamp.
    /// - `accepted_baseline`: Accepted baseline marker.
    /// - `observed_head_oid`: Official AUR HEAD frozen with the target.
    /// - `stale`: Outcome of exact post-execution identity rechecks.
    ///
    /// Output:
    /// - Canonical private result-store document.
    ///
    /// Details:
    /// - Staleness is supplied by the orchestration owner after WS6 and is never inferred
    ///   from model output or version equality.
    ///
    /// # Errors
    /// - Returns `ResultStoreError::UnsafePath` for unsafe identity components.
    #[allow(clippy::too_many_arguments)]
    pub fn from_validated_with_staleness(
        scan_id: &str,
        merged: &MergedScanResult,
        provenance: &ScanProvenance,
        manifests: &[CanonicalManifest],
        stored_at_unix: u64,
        accepted_baseline: bool,
        observed_head_oid: &str,
        stale: bool,
    ) -> Result<Self, ResultStoreError> {
        validate_scan_id(scan_id)?;
        validate_package_base(&merged.identity.package_base)?;
        CommitOid::new(observed_head_oid).map_err(|error| ResultStoreError::UnsafePath {
            component: observed_head_oid.to_string(),
            reason: error.to_string(),
        })?;
        Ok(Self {
            schema_version: RESULT_SCHEMA_VERSION,
            scan_id: scan_id.to_string(),
            package_base: merged.identity.package_base.clone(),
            commit_oid: merged.identity.commit_oid.clone(),
            observed_head_oid: observed_head_oid.to_string(),
            coverage: merged.coverage.as_str().to_string(),
            completion_wording: merged.completion_wording(),
            limitations: merged.limitations.clone(),
            findings: merged
                .findings
                .iter()
                .map(|finding| StoredFinding {
                    fingerprint: finding.fingerprint.clone(),
                    severity: finding.severity.as_str().to_string(),
                    snapshot: finding.snapshot.clone(),
                    path: finding.path.clone(),
                    evidence: finding.evidence.clone(),
                    assessments: finding
                        .assessments
                        .iter()
                        .map(|assessment| StoredAssessment {
                            provider: assessment.provider.clone(),
                            model: assessment.model.clone(),
                            severity: assessment.severity.as_str().to_string(),
                            title: assessment.title.clone(),
                            rationale: assessment.rationale.clone(),
                            recommendation: assessment.recommendation.clone(),
                        })
                        .collect(),
                    disagreement: finding.disagreement,
                })
                .collect(),
            provenance: StoredProvenance {
                pi_version: provenance.pi_version.clone(),
                extension_sha256: provenance.extension_sha256.clone(),
                prompt_version: provenance.prompt_version.clone(),
                result_schema_version: provenance.schema_version.clone(),
                tool_contract_version: provenance.tool_contract_version.clone(),
                attempts: provenance
                    .attempts
                    .iter()
                    .map(|attempt| StoredAttempt {
                        provider: attempt.provider.clone(),
                        model: attempt.model.clone(),
                        validated: attempt.validated,
                        corrected: attempt.corrected,
                        rpc_bytes: attempt.usage.rpc_bytes,
                        effective_tokens: attempt.usage.effective_tokens(),
                    })
                    .collect(),
            },
            manifests: manifests
                .iter()
                .map(|manifest| StoredManifest {
                    manifest_hash: manifest.calculate_manifest_hash(),
                    manifest: manifest.clone(),
                })
                .collect(),
            stored_at_unix,
            accepted_baseline,
            stale,
            mutable_sources: Vec::new(),
        })
    }
}

/// What: Confirm a serialized document contains no forbidden raw field anywhere.
///
/// Inputs:
/// - `document`: The serialized result document bytes.
///
/// Output:
/// - The forbidden key that was found, or `None` when the document is clean.
///
/// Details:
/// - Checks object keys by exact name at every nesting depth. This is a defence-in-depth
///   assertion over the type system, which already has no field for raw data.
#[must_use]
pub fn find_forbidden_raw_field(document: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(document).ok()?;
    let mut found = None;
    walk_for_forbidden(&value, &mut found);
    found
}

/// Recursively search a JSON value for a forbidden object key.
fn walk_for_forbidden(value: &serde_json::Value, found: &mut Option<String>) {
    if found.is_some() {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if FORBIDDEN_RAW_FIELDS.contains(&key.as_str()) {
                    *found = Some(key.clone());
                    return;
                }
                walk_for_forbidden(child, found);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                walk_for_forbidden(child, found);
            }
        }
        _ => {}
    }
}

/// Parse canonical or schema-v1 debug-form coverage text.
fn parse_stored_coverage(value: &str) -> Option<Coverage> {
    match value {
        "complete" | "Complete" => Some(Coverage::Complete),
        "incomplete" | "Incomplete" => Some(Coverage::Incomplete),
        _ => None,
    }
}

/// Parse canonical or schema-v1 debug-form severity text.
fn parse_stored_severity(value: &str) -> Option<Severity> {
    Severity::parse(value).or(match value {
        "Info" => Some(Severity::Info),
        "Low" => Some(Severity::Low),
        "Medium" => Some(Severity::Medium),
        "High" => Some(Severity::High),
        "Critical" => Some(Severity::Critical),
        _ => None,
    })
}

/// Convert one semantically validated stored finding back to its canonical typed form.
fn stored_finding_to_merged(finding: &StoredFinding) -> Result<MergedFinding, String> {
    let severity = parse_stored_severity(&finding.severity)
        .ok_or_else(|| format!("stored finding has unknown severity {:?}", finding.severity))?;
    crate::logic::pi_scan::manifest::normalize_manifest_path(&finding.path)
        .map_err(|error| error.to_string())?;
    if finding.fingerprint.is_empty()
        || [
            finding.snapshot.as_str(),
            finding.path.as_str(),
            finding.evidence.as_str(),
        ]
        .iter()
        .any(|text| crate::pi_agent::has_forbidden_control(text))
    {
        return Err("stored finding has an empty fingerprint or terminal control".to_string());
    }
    let assessments = finding
        .assessments
        .iter()
        .map(|assessment| {
            let severity = parse_stored_severity(&assessment.severity).ok_or_else(|| {
                format!(
                    "stored assessment has unknown severity {:?}",
                    assessment.severity
                )
            })?;
            if [
                assessment.provider.as_str(),
                assessment.model.as_str(),
                assessment.title.as_str(),
                assessment.rationale.as_str(),
                assessment.recommendation.as_str(),
            ]
            .iter()
            .any(|text| crate::pi_agent::has_forbidden_control(text))
            {
                return Err("stored assessment contains a terminal control".to_string());
            }
            Ok(FindingAssessment {
                provider: assessment.provider.clone(),
                model: assessment.model.clone(),
                severity,
                title: assessment.title.clone(),
                rationale: assessment.rationale.clone(),
                recommendation: assessment.recommendation.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(MergedFinding {
        fingerprint: finding.fingerprint.clone(),
        severity,
        snapshot: finding.snapshot.clone(),
        path: finding.path.clone(),
        evidence: finding.evidence.clone(),
        assessments,
        disagreement: finding.disagreement,
    })
}

/// What: Resolve the confined on-disk path of one stored result document.
///
/// Inputs:
/// - `results_root`: The `pi_scan/results-v1` root directory.
/// - `package_base`: Canonical package base directory name.
/// - `scan_id`: Scan identifier used as the document file stem.
///
/// Output:
/// - `<results_root>/<package_base>/<scan_id>.json`.
///
/// Details:
/// - Both identity components are validated as safe single segments before being joined, and
///   the joined path is re-checked to confirm it stays under the root. Absolute paths,
///   traversal, separators, and control characters are all rejected.
///
/// # Errors
/// - Returns `ResultStoreError::UnsafePath` when either component is unsafe or the resulting
///   path escapes the root.
pub fn result_path(
    results_root: &Path,
    package_base: &str,
    scan_id: &str,
) -> Result<PathBuf, ResultStoreError> {
    validate_package_base(package_base)?;
    validate_scan_id(scan_id)?;
    let path = results_root
        .join(package_base)
        .join(format!("{scan_id}.json"));
    confirm_confined(results_root, &path)?;
    Ok(path)
}

/// Validate a package base as a safe single path segment.
fn validate_package_base(package_base: &str) -> Result<(), ResultStoreError> {
    PackageBase::new(package_base).map_err(|error| ResultStoreError::UnsafePath {
        component: package_base.to_string(),
        reason: error.to_string(),
    })?;
    validate_segment(package_base)
}

/// Validate a scan identifier as a safe single path segment.
fn validate_scan_id(scan_id: &str) -> Result<(), ResultStoreError> {
    if scan_id.is_empty() || scan_id.len() > MAX_SCAN_ID_LENGTH {
        return Err(ResultStoreError::UnsafePath {
            component: scan_id.to_string(),
            reason: format!("length must be between 1 and {MAX_SCAN_ID_LENGTH} characters"),
        });
    }
    let allowed = scan_id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.'));
    if !allowed {
        return Err(ResultStoreError::UnsafePath {
            component: scan_id.to_string(),
            reason: "only ASCII letters, digits, '-', '_', and '.' are allowed".to_string(),
        });
    }
    validate_segment(scan_id)
}

/// Reject any value that is not exactly one safe, non-traversing path segment.
fn validate_segment(value: &str) -> Result<(), ResultStoreError> {
    if value == "." || value == ".." || value.contains("..") {
        return Err(ResultStoreError::UnsafePath {
            component: value.to_string(),
            reason: "path traversal is not permitted".to_string(),
        });
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(ResultStoreError::UnsafePath {
            component: value.to_string(),
            reason: "path separators and NUL bytes are not permitted".to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(ResultStoreError::UnsafePath {
            component: value.to_string(),
            reason: "control characters are not permitted".to_string(),
        });
    }
    Ok(())
}

/// Confirm a joined path stays lexically under the results root.
fn confirm_confined(results_root: &Path, candidate: &Path) -> Result<(), ResultStoreError> {
    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ResultStoreError::UnsafePath {
            component: candidate.display().to_string(),
            reason: "the resolved path contains a parent-directory component".to_string(),
        });
    }
    if !candidate.starts_with(results_root) {
        return Err(ResultStoreError::UnsafePath {
            component: candidate.display().to_string(),
            reason: "the resolved path escapes the scan results root".to_string(),
        });
    }
    Ok(())
}

/// What: Proof that a result document was loaded and committed successfully.
///
/// Inputs: Returned only by [`save_result_atomic`] and a successful [`load_result`].
///
/// Output: Required argument of [`cleanup_expired_results`].
///
/// Details:
/// - This token exists so retention cleanup is structurally impossible before a successful
///   load and atomic commit. It cannot be constructed outside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommitReceipt {
    /// Unix second the successful commit or load completed.
    committed_at_unix: u64,
}

impl CommitReceipt {
    /// Return the Unix second of the successful commit or load.
    #[must_use]
    pub const fn committed_at_unix(self) -> u64 {
        self.committed_at_unix
    }
}

/// What: Persist one validated result document atomically with private permissions.
///
/// Inputs:
/// - `results_root`: The `pi_scan/results-v1` root directory.
/// - `document`: The canonical validated document.
///
/// Output:
/// - A [`CommitReceipt`] proving the atomic commit succeeded.
///
/// Details:
/// - Directories are created mode 0700 and the document is written through a mode-0600
///   temporary file that is renamed over the target, so a reader never observes a partial
///   document and no other user can read it.
/// - The serialized document is re-checked for forbidden raw fields before it is written.
///
/// # Errors
/// - Returns `ResultStoreError` when the identity is unsafe, serialization fails, the
///   document is oversized, a forbidden field is present, or a filesystem operation fails.
pub fn save_result_atomic(
    results_root: &Path,
    document: &StoredScanResult,
) -> Result<CommitReceipt, ResultStoreError> {
    let path = result_path(results_root, &document.package_base, &document.scan_id)?;
    let bytes = serde_json::to_vec_pretty(document).map_err(|error| ResultStoreError::Io {
        path: path.display().to_string(),
        message: format!("could not serialize the scan result: {error}"),
    })?;
    if bytes.len() > MAX_RESULT_DOCUMENT_BYTES {
        return Err(ResultStoreError::Io {
            path: path.display().to_string(),
            message: format!(
                "the serialized result is {} bytes, above the {MAX_RESULT_DOCUMENT_BYTES}-byte limit",
                bytes.len()
            ),
        });
    }
    if let Some(field) = find_forbidden_raw_field(&bytes) {
        return Err(ResultStoreError::UnsafePath {
            component: field,
            reason: "raw prompt, source, thinking, or response data must never be persisted"
                .to_string(),
        });
    }

    let parent = path.parent().unwrap_or(results_root);
    create_private_dir_all(parent)?;
    let tmp_path = parent.join(format!(".tmp-{}.json", document.scan_id));
    write_private_file(&tmp_path, &bytes)?;
    fs::rename(&tmp_path, &path).map_err(|error| {
        let _ = fs::remove_file(&tmp_path);
        ResultStoreError::Io {
            path: path.display().to_string(),
            message: format!("could not atomically commit the scan result: {error}"),
        }
    })?;

    Ok(CommitReceipt {
        committed_at_unix: document.stored_at_unix,
    })
}

/// What: Load one stored result document, quarantining corrupt or newer artifacts.
///
/// Inputs:
/// - `results_root`: The `pi_scan/results-v1` root directory.
/// - `quarantine_dir`: The private quarantine directory.
/// - `package_base`: Canonical package base.
/// - `scan_id`: Scan identifier.
/// - `now_unix`: Current Unix second, recorded on the returned receipt.
///
/// Output:
/// - The decoded document and a [`CommitReceipt`] proving a successful load.
///
/// Details:
/// - Missing, corrupt, unsupported-newer, and I/O failures are distinct. Corrupt and newer
///   documents are moved into quarantine and are never interpreted as empty or clean.
/// - A failed quarantine leaves the original untouched and reports the artifact unavailable.
///
/// # Errors
/// - Returns the corresponding `ResultStoreError` variant for every failure above.
pub fn load_result(
    results_root: &Path,
    quarantine_dir: &Path,
    package_base: &str,
    scan_id: &str,
    now_unix: u64,
) -> Result<(StoredScanResult, CommitReceipt), ResultStoreError> {
    let path = result_path(results_root, package_base, scan_id)?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ResultStoreError::Missing {
                path: path.display().to_string(),
            });
        }
        Err(error) => {
            return Err(ResultStoreError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            });
        }
    };

    let header: SchemaHeader = match serde_json::from_slice(&bytes) {
        Ok(header) => header,
        Err(error) => {
            return Err(quarantine_document(
                &path,
                &bytes,
                quarantine_dir,
                now_unix,
                |target| ResultStoreError::Corrupt {
                    path: path.display().to_string(),
                    reason: error.to_string(),
                    quarantined_to: target,
                },
            ));
        }
    };
    if header.schema_version > RESULT_SCHEMA_VERSION {
        return Err(quarantine_document(
            &path,
            &bytes,
            quarantine_dir,
            now_unix,
            |target| ResultStoreError::UnsupportedNewerVersion {
                path: path.display().to_string(),
                observed: header.schema_version,
                max_supported: RESULT_SCHEMA_VERSION,
                quarantined_to: target,
            },
        ));
    }

    match serde_json::from_slice::<StoredScanResult>(&bytes) {
        Ok(document) => match document.to_merged_result() {
            Ok(_) => Ok((
                document,
                CommitReceipt {
                    committed_at_unix: now_unix,
                },
            )),
            Err(reason) => Err(quarantine_document(
                &path,
                &bytes,
                quarantine_dir,
                now_unix,
                |target| ResultStoreError::Corrupt {
                    path: path.display().to_string(),
                    reason,
                    quarantined_to: target,
                },
            )),
        },
        Err(error) => Err(quarantine_document(
            &path,
            &bytes,
            quarantine_dir,
            now_unix,
            |target| ResultStoreError::Corrupt {
                path: path.display().to_string(),
                reason: error.to_string(),
                quarantined_to: target,
            },
        )),
    }
}

/// Batch of semantically validated stored results plus quarantined-document warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredResultBatch {
    /// Validated documents sorted oldest-to-newest.
    pub documents: Vec<StoredScanResult>,
    /// Actionable warnings for documents quarantined or otherwise unavailable.
    pub warnings: Vec<String>,
}

/// Load every confined stored result, quarantining invalid documents individually.
///
/// # Errors
/// - Returns an I/O error when the results root itself cannot be enumerated.
pub fn load_all_results(
    results_root: &Path,
    quarantine_dir: &Path,
    now_unix: u64,
) -> Result<StoredResultBatch, ResultStoreError> {
    if !results_root.exists() {
        return Ok(StoredResultBatch {
            documents: Vec::new(),
            warnings: Vec::new(),
        });
    }
    let package_dirs = fs::read_dir(results_root).map_err(|error| ResultStoreError::Io {
        path: results_root.display().to_string(),
        message: error.to_string(),
    })?;
    let mut documents = Vec::new();
    let mut warnings = Vec::new();
    'packages: for package_entry in package_dirs {
        let package_entry = package_entry.map_err(|error| ResultStoreError::Io {
            path: results_root.display().to_string(),
            message: error.to_string(),
        })?;
        let package_path = package_entry.path();
        let file_type = package_entry
            .file_type()
            .map_err(|error| ResultStoreError::Io {
                path: package_path.display().to_string(),
                message: error.to_string(),
            })?;
        if !file_type.is_dir() {
            continue;
        }
        let package_base = package_entry.file_name().to_string_lossy().into_owned();
        if validate_package_base(&package_base).is_err() {
            continue;
        }
        let entries = fs::read_dir(&package_path).map_err(|error| ResultStoreError::Io {
            path: package_path.display().to_string(),
            message: error.to_string(),
        })?;
        for entry in entries {
            if documents.len() >= MAX_RESTORED_RESULTS {
                warnings.push(format!(
                    "only the first {MAX_RESTORED_RESULTS} stored Pi scan results were restored"
                ));
                break 'packages;
            }
            let entry = entry.map_err(|error| ResultStoreError::Io {
                path: package_path.display().to_string(),
                message: error.to_string(),
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| ResultStoreError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            })?;
            if !file_type.is_file()
                || path.extension().and_then(|value| value.to_str()) != Some("json")
            {
                continue;
            }
            let Some(scan_id) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            match load_result(
                results_root,
                quarantine_dir,
                &package_base,
                scan_id,
                now_unix,
            ) {
                Ok((document, _)) => documents.push(document),
                Err(error) => warnings.push(error.to_string()),
            }
        }
    }
    documents.sort_by_key(|document| document.stored_at_unix);
    Ok(StoredResultBatch {
        documents,
        warnings,
    })
}

/// Minimal header used to inspect the schema version before full decoding.
#[derive(serde::Deserialize)]
struct SchemaHeader {
    /// Document schema version.
    schema_version: u32,
}

/// Move a damaged document into quarantine and build the caller's error.
fn quarantine_document(
    path: &Path,
    bytes: &[u8],
    quarantine_dir: &Path,
    now_unix: u64,
    build: impl FnOnce(Option<String>) -> ResultStoreError,
) -> ResultStoreError {
    match move_into_quarantine(path, bytes, quarantine_dir, now_unix) {
        Ok(target) => build(Some(target.display().to_string())),
        Err(error) => error,
    }
}

/// Copy a damaged document into quarantine, then remove the original.
fn move_into_quarantine(
    source: &Path,
    bytes: &[u8],
    quarantine_dir: &Path,
    now_unix: u64,
) -> Result<PathBuf, ResultStoreError> {
    create_private_dir_all(quarantine_dir).map_err(|error| ResultStoreError::QuarantineFailed {
        path: source.display().to_string(),
        message: error.to_string(),
    })?;
    let digest = sha256_hex(bytes);
    let target = quarantine_dir.join(format!("result-{now_unix}-{digest}.json"));
    if !target.exists() {
        write_private_file(&target, bytes).map_err(|error| ResultStoreError::QuarantineFailed {
            path: source.display().to_string(),
            message: error.to_string(),
        })?;
    }
    fs::remove_file(source).map_err(|error| ResultStoreError::QuarantineFailed {
        path: source.display().to_string(),
        message: format!("the quarantine copy was written but the original remains: {error}"),
    })?;
    Ok(target)
}

/// Compute the lowercase hexadecimal SHA-256 of a byte slice.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Create a directory tree and restrict it to mode 0700 on Unix.
fn create_private_dir_all(path: &Path) -> Result<(), ResultStoreError> {
    fs::create_dir_all(path).map_err(|error| ResultStoreError::Io {
        path: path.display().to_string(),
        message: format!("could not create the directory: {error}"),
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = fs::metadata(path)
            .map_err(|error| ResultStoreError::Io {
                path: path.display().to_string(),
                message: error.to_string(),
            })?
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).map_err(|error| ResultStoreError::Io {
            path: path.display().to_string(),
            message: format!("could not restrict directory permissions: {error}"),
        })?;
    }
    Ok(())
}

/// Write a file with mode 0600 on Unix and flush it to disk.
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), ResultStoreError> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| ResultStoreError::Io {
        path: path.display().to_string(),
        message: format!("could not open the file for writing: {error}"),
    })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| ResultStoreError::Io {
            path: path.display().to_string(),
            message: format!("could not write the file: {error}"),
        })
}

/// What: Minimal retention input describing one stored result document.
///
/// Inputs: Collected by the caller from loaded documents.
///
/// Output: Input to [`plan_retention`].
///
/// Details:
/// - Only identity and age are needed, so retention planning stays pure and testable without
///   reading any file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredResultSummary {
    /// Scan identifier.
    pub scan_id: String,
    /// Unix second the document was produced.
    pub stored_at_unix: u64,
    /// Whether this document is the currently accepted baseline.
    pub accepted_baseline: bool,
}

/// What: Retention decision for one package base.
///
/// Inputs: Produced by [`plan_retention`].
///
/// Output: Input to [`cleanup_expired_results`].
///
/// Details:
/// - `keep` always contains the newest detailed result and, when different, the accepted
///   baseline result.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetentionPlan {
    /// Scan identifiers that must be retained.
    pub keep: Vec<String>,
    /// Scan identifiers eligible for deletion.
    pub delete: Vec<String>,
}

/// What: Plan retention for one package base's stored results.
///
/// Inputs:
/// - `summaries`: Every stored document for the package base.
/// - `now_unix`: Current Unix second.
/// - `retention_days`: Retention window applied to superseded documents.
///
/// Output:
/// - The retained and deletable scan identifiers.
///
/// Details:
/// - The newest detailed result is always kept. The accepted baseline result is also kept when
///   it is a different document.
/// - Every other document is deleted only once it is older than the retention window, so a
///   recently superseded result remains available for comparison.
#[must_use]
pub fn plan_retention(
    summaries: &[StoredResultSummary],
    now_unix: u64,
    retention_days: u64,
) -> RetentionPlan {
    let mut keep: BTreeSet<String> = BTreeSet::new();
    if let Some(newest) = summaries
        .iter()
        .max_by_key(|summary| summary.stored_at_unix)
    {
        keep.insert(newest.scan_id.clone());
    }
    if let Some(current_baseline) = summaries
        .iter()
        .filter(|summary| summary.accepted_baseline)
        .max_by_key(|summary| summary.stored_at_unix)
    {
        keep.insert(current_baseline.scan_id.clone());
    }

    let window = retention_days.saturating_mul(24 * 60 * 60);
    let mut delete = Vec::new();
    for summary in summaries {
        if keep.contains(&summary.scan_id) {
            continue;
        }
        let age = now_unix.saturating_sub(summary.stored_at_unix);
        if age > window {
            delete.push(summary.scan_id.clone());
        }
    }
    delete.sort();

    RetentionPlan {
        keep: keep.into_iter().collect(),
        delete,
    }
}

/// What: Delete superseded result documents after a proven successful load and commit.
///
/// Inputs:
/// - `results_root`: The `pi_scan/results-v1` root directory.
/// - `package_base`: Canonical package base.
/// - `plan`: The retention plan produced by [`plan_retention`].
/// - `receipt`: Proof that a load and atomic commit already succeeded.
///
/// Output:
/// - The paths actually removed.
///
/// Details:
/// - Requiring a [`CommitReceipt`] makes it structurally impossible to prune results before a
///   successful load and commit, which is what protects a user from a corrupt-state wipe.
/// - Quarantine lives outside the results root and is never referenced here, so a quarantine
///   artifact can never be deleted by cleanup.
/// - A document named in `plan.keep` is never removed even if it also appears in `delete`.
///
/// # Errors
/// - Returns `ResultStoreError` when a path is unsafe or a removal fails for a reason other
///   than the file already being absent.
pub fn cleanup_expired_results(
    results_root: &Path,
    package_base: &str,
    plan: &RetentionPlan,
    receipt: &CommitReceipt,
) -> Result<Vec<PathBuf>, ResultStoreError> {
    let _ = receipt.committed_at_unix();
    let mut removed = Vec::new();
    for scan_id in &plan.delete {
        if plan.keep.iter().any(|kept| kept == scan_id) {
            continue;
        }
        let path = result_path(results_root, package_base, scan_id)?;
        match fs::remove_file(&path) {
            Ok(()) => removed.push(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ResultStoreError::Io {
                    path: path.display().to_string(),
                    message: format!("could not remove the superseded result: {error}"),
                });
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::{
        RESULT_SCHEMA_VERSION, ResultStoreError, StoredResultSummary, find_forbidden_raw_field,
        plan_retention, result_path,
    };
    use std::path::Path;

    #[test]
    fn paths_stay_confined_to_the_results_root() {
        let root = Path::new("/tmp/pacsea/pi_scan/results-v1");
        let ok = result_path(root, "yay", "scan-1").expect("safe path");
        assert!(ok.starts_with(root));
        for (base, id) in [
            ("..", "scan"),
            ("yay", ".."),
            ("yay", "a/b"),
            ("yay", "a\0b"),
        ] {
            assert!(
                matches!(
                    result_path(root, base, id),
                    Err(ResultStoreError::UnsafePath { .. })
                ),
                "{base}/{id} must be rejected"
            );
        }
    }

    #[test]
    fn forbidden_raw_fields_are_detected_at_any_depth() {
        let nested = br#"{"schema_version":1,"a":{"b":[{"thinking":"x"}]}}"#;
        assert_eq!(
            find_forbidden_raw_field(nested).as_deref(),
            Some("thinking")
        );
        let clean = br#"{"schema_version":1,"provenance":{"prompt_version":"v1"}}"#;
        assert!(find_forbidden_raw_field(clean).is_none());
    }

    #[test]
    fn retention_keeps_newest_and_accepted_baseline() {
        let day = 24 * 60 * 60;
        let summaries = vec![
            StoredResultSummary {
                scan_id: "newest".to_string(),
                stored_at_unix: 100 * day,
                accepted_baseline: false,
            },
            StoredResultSummary {
                scan_id: "baseline".to_string(),
                stored_at_unix: 10 * day,
                accepted_baseline: true,
            },
            StoredResultSummary {
                scan_id: "old".to_string(),
                stored_at_unix: 10 * day,
                accepted_baseline: false,
            },
            StoredResultSummary {
                scan_id: "recent".to_string(),
                stored_at_unix: 99 * day,
                accepted_baseline: false,
            },
        ];
        let plan = plan_retention(&summaries, 100 * day, 30);
        assert!(plan.keep.contains(&"newest".to_string()));
        assert!(plan.keep.contains(&"baseline".to_string()));
        assert_eq!(plan.delete, vec!["old".to_string()]);
    }

    #[test]
    fn schema_version_is_one() {
        assert_eq!(RESULT_SCHEMA_VERSION, 1);
    }
}
