//! Accepted baseline, observed cursor, OID-keyed backlog ledger, and versioned persistence with atomic quarantine.

use crate::logic::pi_scan::identity::{CommitOid, PackageBase};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// What: Classification of commit changes regarding build relevance.
///
/// Inputs:
/// - List of changed relative file paths in a commit.
///
/// Output:
/// - `BuildRelevant`, `ObservedNoRecipeDelta`, or `Uncertain`.
///
/// Details:
/// - Used to determine whether a commit requires a paid/model scan or can be ledgered without AI analysis.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum CommitBuildRelevance {
    /// Commit modifies PKGBUILD, .SRCINFO, patches, install scripts, or source files.
    BuildRelevant,
    /// Commit modifies only non-build files (e.g., .gitignore, README, CI configs).
    ObservedNoRecipeDelta,
    /// Commit contains no changed file list or ambiguous modifications.
    Uncertain,
}

/// What: Classify a commit's changed files into build relevance categories.
///
/// Inputs:
/// - `changed_files`: Slice of changed relative file paths.
///
/// Output:
/// - `CommitBuildRelevance` enum variant.
///
/// Details:
/// - If any file is build-relevant (PKGBUILD, .SRCINFO, *.install, *.patch, *.diff, *.sh, *.src.tar.*, etc.), returns `BuildRelevant`.
/// - If `changed_files` is non-empty and all files are non-build assets (.gitignore, README*, LICENSE*, .github/*, CI*), returns `ObservedNoRecipeDelta`.
/// - If empty or undetermined, returns `Uncertain`.
pub fn classify_commit_delta(changed_files: &[impl AsRef<str>]) -> CommitBuildRelevance {
    if changed_files.is_empty() {
        return CommitBuildRelevance::Uncertain;
    }

    let mut all_non_build = true;
    let mut any_build_relevant = false;

    for file in changed_files {
        let path = file.as_ref();
        let lower = path.to_ascii_lowercase();
        let filename = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();

        let extension = Path::new(&lower).extension().and_then(|ext| ext.to_str());
        let is_patch = extension == Some("patch");
        let is_diff = extension == Some("diff");
        let is_sh = extension == Some("sh");

        if filename == "pkgbuild"
            || filename == ".srcinfo"
            || lower.ends_with(".install")
            || is_patch
            || is_diff
            || is_sh
            || lower.ends_with(".src.tar.gz")
            || lower.ends_with(".src.tar.xz")
            || lower.ends_with(".src.tar.zst")
        {
            any_build_relevant = true;
            all_non_build = false;
            break;
        }

        let is_non_build = filename == ".gitignore"
            || filename.starts_with("readme")
            || filename.starts_with("license")
            || filename.starts_with("changelog")
            || filename.starts_with("news")
            || lower.starts_with(".github/")
            || lower.starts_with("ci/");

        if !is_non_build {
            all_non_build = false;
        }
    }

    if any_build_relevant {
        CommitBuildRelevance::BuildRelevant
    } else if all_non_build {
        CommitBuildRelevance::ObservedNoRecipeDelta
    } else {
        CommitBuildRelevance::Uncertain
    }
}

/// What: Accepted comparison baseline entry for a package base.
///
/// Inputs:
/// - Accepted commit OID, timestamp, evidence fingerprint, and optional notes.
///
/// Output:
/// - Struct representing an accepted baseline state record.
///
/// Details:
/// - Established only after an explicit complete scan or user baseline action.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AcceptedBaselineEntry {
    /// Package base for this baseline.
    pub package_base: PackageBase,
    /// Git commit OID accepted as baseline.
    pub accepted_commit_oid: CommitOid,
    /// Unix timestamp when accepted.
    pub accepted_at_unix_ts: u64,
    /// Evidence fingerprint bound to this baseline.
    pub evidence_fingerprint: String,
    /// Optional user or system notes.
    pub notes: Option<String>,
}

/// What: Container for all accepted package baselines.
///
/// Inputs:
/// - Schema version and map of package base strings to `AcceptedBaselineEntry`.
///
/// Output:
/// - Persisted baseline state object.
///
/// Details:
/// - Schema version 1. Managed independently from cursor and queue state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AcceptedBaselineState {
    /// Schema version (1).
    pub schema_version: u32,
    /// Map of package base name string to accepted baseline entry.
    pub entries: BTreeMap<String, AcceptedBaselineEntry>,
}

impl Default for AcceptedBaselineState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            entries: BTreeMap::new(),
        }
    }
}

/// What: Last observed cursor entry for a package base.
///
/// Inputs:
/// - Package base, last observed commit OID, and observation timestamp.
///
/// Output:
/// - Struct representing observed HEAD cursor position.
///
/// Details:
/// - Updated whenever an official AUR repository observation cycle observes a HEAD commit.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservedCursorEntry {
    /// Package base.
    pub package_base: PackageBase,
    /// Last observed HEAD commit OID.
    pub last_observed_commit_oid: CommitOid,
    /// Unix timestamp of observation.
    pub observed_at_unix_ts: u64,
}

/// What: Container for all observed package base HEAD cursors.
///
/// Inputs:
/// - Schema version and map of package base strings to `ObservedCursorEntry`.
///
/// Output:
/// - Persisted cursor state object.
///
/// Details:
/// - Schema version 1. Managed independently from baseline and queue state.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ObservedCursorState {
    /// Schema version (1).
    pub schema_version: u32,
    /// Map of package base name string to observed cursor entry.
    pub entries: BTreeMap<String, ObservedCursorEntry>,
}

impl Default for ObservedCursorState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            entries: BTreeMap::new(),
        }
    }
}

/// What: Single commit entry in the backlog ledger.
///
/// Inputs:
/// - Commit OID, package base, observation timestamp, and build relevance classification.
///
/// Output:
/// - Ledger record struct.
///
/// Details:
/// - OID-keyed record tracking observed commits in strict topological/chronological order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LedgerCommitEntry {
    /// Git commit OID.
    pub commit_oid: CommitOid,
    /// Owning package base.
    pub package_base: PackageBase,
    /// Unix timestamp when commit was observed.
    pub observed_at_unix_ts: u64,
    /// Build relevance classification.
    pub relevance: CommitBuildRelevance,
}

/// What: Container for the backlog ledger and scan queue.
///
/// Inputs:
/// - Schema version and queue of `LedgerCommitEntry`.
///
/// Output:
/// - Backlog ledger state object.
///
/// Details:
/// - Schema version 1. Preserves oldest-first insertion order with no coalescing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BacklogLedgerState {
    /// Schema version (1).
    pub schema_version: u32,
    /// Queue of commits in oldest-first order.
    pub queue: Vec<LedgerCommitEntry>,
}

impl Default for BacklogLedgerState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            queue: Vec::new(),
        }
    }
}

impl BacklogLedgerState {
    /// What: Push a new commit entry to the backlog queue.
    ///
    /// Inputs:
    /// - `entry`: `LedgerCommitEntry` to insert.
    ///
    /// Output: None.
    ///
    /// Details:
    /// - Appends entry to preserve oldest-first ordering without coalescing.
    pub fn push_entry(&mut self, entry: LedgerCommitEntry) {
        self.queue.push(entry);
    }

    /// What: Push multiple commit entries preserving oldest-first order.
    ///
    /// Inputs:
    /// - `entries`: Vector of `LedgerCommitEntry` sorted oldest-first.
    ///
    /// Output: None.
    ///
    /// Details:
    /// - Appends all entries without coalescing or discarding intermediate commits.
    pub fn push_oldest_first(&mut self, entries: Vec<LedgerCommitEntry>) {
        for entry in entries {
            self.queue.push(entry);
        }
    }

    /// What: Pop the oldest pending commit entry from the queue.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `Some(LedgerCommitEntry)` if queue is non-empty, `None` otherwise.
    ///
    /// Details:
    /// - Removes and returns the first (oldest) entry.
    pub fn pop_oldest(&mut self) -> Option<LedgerCommitEntry> {
        if self.queue.is_empty() {
            None
        } else {
            Some(self.queue.remove(0))
        }
    }
}

/// What: Errors that occur during state persistence loading, saving, or quarantine.
///
/// Inputs:
/// - Path, reason, or I/O failure details.
///
/// Output:
/// - Structured persistence error.
///
/// Details:
/// - Distinguishes missing file, corrupt file, unsupported newer schema version, I/O errors, and quarantine errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistenceError {
    /// Target file does not exist.
    Missing {
        /// Target file path.
        path: String,
    },
    /// State file content is corrupt or malformed JSON.
    Corrupt {
        /// Target file path.
        path: String,
        /// Reason for parsing failure.
        reason: String,
        /// Path to quarantined artifact if quarantine succeeded.
        quarantined_to: Option<String>,
    },
    /// State file uses an unsupported newer schema version.
    UnsupportedNewerVersion {
        /// Target file path.
        path: String,
        /// Observed schema version in file.
        observed: u32,
        /// Maximum schema version supported by this build.
        max_supported: u32,
        /// Path to quarantined artifact if quarantine succeeded.
        quarantined_to: Option<String>,
    },
    /// File I/O failure.
    Io {
        /// Path affected.
        path: String,
        /// Detailed message.
        message: String,
    },
    /// Quarantine write failure.
    QuarantineFailed {
        /// Path affected.
        path: String,
        /// Detailed message.
        message: String,
    },
}

impl fmt::Display for PersistenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing { path } => write!(f, "State file missing: {path}"),
            Self::Corrupt {
                path,
                reason,
                quarantined_to,
            } => {
                if let Some(q) = quarantined_to {
                    write!(
                        f,
                        "State file '{path}' is corrupt ({reason}); quarantined to '{q}'"
                    )
                } else {
                    write!(f, "State file '{path}' is corrupt: {reason}")
                }
            }
            Self::UnsupportedNewerVersion {
                path,
                observed,
                max_supported,
                quarantined_to,
            } => {
                if let Some(q) = quarantined_to {
                    write!(
                        f,
                        "State file '{path}' version {observed} exceeds supported {max_supported}; quarantined to '{q}'"
                    )
                } else {
                    write!(
                        f,
                        "State file '{path}' version {observed} exceeds supported {max_supported}"
                    )
                }
            }
            Self::Io { path, message } => write!(f, "I/O failure at '{path}': {message}"),
            Self::QuarantineFailed { path, message } => {
                write!(f, "Failed to quarantine state file '{path}': {message}")
            }
        }
    }
}

impl std::error::Error for PersistenceError {}

/// Header struct used to inspect schema version before full deserialization.
#[derive(serde::Deserialize)]
struct SchemaVersionHeader {
    /// Version number field.
    schema_version: u32,
}

/// Helper to convert a byte slice into a lowercase hexadecimal string.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What: Perform atomic quarantine of a corrupt or unsupported state file.
///
/// Inputs:
/// - `source_path`: Path of the corrupt file.
/// - `file_bytes`: Raw byte contents of the corrupt file.
/// - `quarantine_dir`: Directory where quarantine artifact will be stored.
/// - `prefix`: Prefix string for quarantine filename (e.g. "baseline", "backlog").
///
/// Output:
/// - `Ok(PathBuf)` containing quarantine artifact path, or `Err(PersistenceError)`.
///
/// Details:
/// - Computes SHA-256 digest of `file_bytes`.
/// - Uses timestamp and SHA-256 to create atomic file `<prefix>-<timestamp>-<sha256>.json`.
/// - Enforces mode `0o700` for quarantine directory and `0o600` for quarantine file on Unix.
fn quarantine_corrupt_file(
    source_path: &Path,
    file_bytes: &[u8],
    quarantine_dir: &Path,
    prefix: &str,
) -> Result<PathBuf, PersistenceError> {
    create_private_dir_all(quarantine_dir).map_err(|e| PersistenceError::QuarantineFailed {
        path: source_path.display().to_string(),
        message: format!("Failed creating quarantine directory: {e}"),
    })?;

    let mut hasher = Sha256::new();
    hasher.update(file_bytes);
    let digest: [u8; 32] = hasher.finalize().into();
    let hash_hex = format_hex(&digest);

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());

    let filename = format!("{prefix}-{ts}-{hash_hex}.json");
    let dest_path = quarantine_dir.join(filename);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut f = match options.open(&dest_path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            return Ok(dest_path);
        }
        Err(e) => {
            return Err(PersistenceError::QuarantineFailed {
                path: source_path.display().to_string(),
                message: format!(
                    "Failed creating quarantine file {}: {e}",
                    dest_path.display()
                ),
            });
        }
    };

    f.write_all(file_bytes)
        .and_then(|()| f.sync_all())
        .map_err(|e| PersistenceError::QuarantineFailed {
            path: source_path.display().to_string(),
            message: format!("Failed writing quarantine content: {e}"),
        })?;
    if let Err(error) = fs::remove_file(source_path) {
        let _ = fs::remove_file(&dest_path);
        return Err(PersistenceError::QuarantineFailed {
            path: source_path.display().to_string(),
            message: format!("Failed moving original state into quarantine: {error}"),
        });
    }

    Ok(dest_path)
}

/// What: Create a private directory with mode `0o700` on Unix.
///
/// Inputs:
/// - `path`: Directory path to create.
///
/// Output:
/// - `io::Result<()>`.
///
/// Details:
/// - Recursively creates parent directories.
fn create_private_dir_all(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)?.permissions();
        perms.set_mode(0o700);
        fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// What: Load and decode a versioned state file with atomic quarantine for corrupt or newer state.
///
/// Inputs:
/// - `path`: Path to the state file on disk.
/// - `max_supported_version`: Maximum schema version supported by the caller.
/// - `quarantine_dir`: Directory path for quarantining corrupt or newer files.
/// - `prefix`: Artifact prefix used if quarantine occurs.
///
/// Output:
/// - `Ok(Some(T))` if file exists and decodes cleanly.
/// - `Ok(None)` if file does not exist (`Missing`).
/// - `Err(PersistenceError::Corrupt)` or `Err(PersistenceError::UnsupportedNewerVersion)` if malformed, with quarantine completed.
///
/// Details:
/// - Distinguishes missing file, corrupt data, unsupported newer schema version, and I/O failures.
/// - Never interprets corrupt or newer state as empty or clean.
///
/// # Errors
/// Returns `PersistenceError::Corrupt` or `PersistenceError::UnsupportedNewerVersion` if state file cannot be loaded cleanly.
pub fn load_versioned_state<T>(
    path: &Path,
    max_supported_version: u32,
    quarantine_dir: &Path,
    prefix: &str,
) -> Result<Option<T>, PersistenceError>
where
    T: serde::de::DeserializeOwned,
{
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(PersistenceError::Io {
                path: path.display().to_string(),
                message: e.to_string(),
            });
        }
    };

    let header: SchemaVersionHeader = match serde_json::from_slice(&bytes) {
        Ok(h) => h,
        Err(reason) => {
            let quarantined = quarantine_corrupt_file(path, &bytes, quarantine_dir, prefix)?;
            return Err(PersistenceError::Corrupt {
                path: path.display().to_string(),
                reason: reason.to_string(),
                quarantined_to: Some(quarantined.display().to_string()),
            });
        }
    };

    if header.schema_version > max_supported_version {
        let quarantined = quarantine_corrupt_file(path, &bytes, quarantine_dir, prefix)?;
        return Err(PersistenceError::UnsupportedNewerVersion {
            path: path.display().to_string(),
            observed: header.schema_version,
            max_supported: max_supported_version,
            quarantined_to: Some(quarantined.display().to_string()),
        });
    }

    let data: T = match serde_json::from_slice(&bytes) {
        Ok(val) => val,
        Err(reason) => {
            let quarantined = quarantine_corrupt_file(path, &bytes, quarantine_dir, prefix)?;
            return Err(PersistenceError::Corrupt {
                path: path.display().to_string(),
                reason: reason.to_string(),
                quarantined_to: Some(quarantined.display().to_string()),
            });
        }
    };

    Ok(Some(data))
}

/// What: Save a versioned state object atomically to disk with private file permissions.
///
/// Inputs:
/// - `path`: Target file path.
/// - `data`: Serializable state object.
///
/// Output:
/// - `Ok(())` on success, or `Err(PersistenceError)`.
///
/// Details:
/// - Writes to temporary file in the same parent directory using mode `0o600` on Unix.
/// - Performs atomic `fs::rename` over target path to prevent partial write corruption.
///
/// # Errors
/// Returns `PersistenceError::Io` or `PersistenceError::Corrupt` if write/serialization fails.
pub fn save_versioned_state_atomic<T>(path: &Path, data: &T) -> Result<(), PersistenceError>
where
    T: serde::Serialize,
{
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    create_private_dir_all(parent).map_err(|e| PersistenceError::Io {
        path: parent.display().to_string(),
        message: format!("Failed creating parent directory: {e}"),
    })?;

    let json_bytes = serde_json::to_vec_pretty(data).map_err(|e| PersistenceError::Corrupt {
        path: path.display().to_string(),
        reason: format!("Serialization failure: {e}"),
        quarantined_to: None,
    })?;

    let tmp_name = format!(
        ".tmp-{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("state")
    );
    let tmp_path = parent.join(tmp_name);

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(&tmp_path).map_err(|e| PersistenceError::Io {
        path: tmp_path.display().to_string(),
        message: format!("Failed opening temporary file for write: {e}"),
    })?;

    file.write_all(&json_bytes)
        .map_err(|e| PersistenceError::Io {
            path: tmp_path.display().to_string(),
            message: format!("Failed writing temporary file: {e}"),
        })?;

    file.sync_all().map_err(|e| PersistenceError::Io {
        path: tmp_path.display().to_string(),
        message: format!("Failed syncing temporary file: {e}"),
    })?;

    fs::rename(&tmp_path, path).map_err(|e| PersistenceError::Io {
        path: path.display().to_string(),
        message: format!("Failed renaming temporary file into target path: {e}"),
    })?;

    Ok(())
}
