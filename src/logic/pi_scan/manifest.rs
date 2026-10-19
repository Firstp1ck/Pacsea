//! Canonical manifest entries, ordering, entry hashing, and path normalization for Pi scanning.

use sha2::{Digest, Sha256};
use std::fmt;

/// What: Errors that occur during manifest path normalization or manifest construction.
///
/// Inputs:
/// - Path or input details causing the failure.
///
/// Output:
/// - Manifest validation error.
///
/// Details:
/// - Provides explicit errors for path traversal, absolute paths, invalid characters, and empty paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// Path violates normalization or security rules.
    InvalidPath {
        /// Raw path candidate.
        path: String,
        /// Reason for failure.
        reason: String,
    },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { path, reason } => {
                write!(f, "Invalid manifest relative path '{path}': {reason}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Helper to format byte slice as lowercase hexadecimal string.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What: Normalize and validate a relative file path for inclusion in a canonical manifest.
///
/// Inputs:
/// - `raw_path`: String slice candidate path.
///
/// Output:
/// - `Ok(String)` containing normalized Unix relative path (slashes `/`), or `Err(ManifestError)`.
///
/// Details:
/// - Rejects absolute paths (`/foo` or drive letters `C:\`), parent traversal `..`, NUL/control bytes, backslashes `\`, and empty segments (`//`).
/// - Strips redundant leading `./` if present.
///
/// # Errors
/// Returns `ManifestError::InvalidPath` if path is absolute, contains traversal or invalid characters.
pub fn normalize_manifest_path(raw_path: &str) -> Result<String, ManifestError> {
    if raw_path.is_empty() {
        return Err(ManifestError::InvalidPath {
            path: raw_path.to_string(),
            reason: "path cannot be empty".to_string(),
        });
    }

    if raw_path.contains('\\') {
        return Err(ManifestError::InvalidPath {
            path: raw_path.to_string(),
            reason: "path contains forbidden Windows backslashes '\\'".to_string(),
        });
    }

    for c in raw_path.chars() {
        if c.is_control() {
            return Err(ManifestError::InvalidPath {
                path: raw_path.to_string(),
                reason: "path contains control characters".to_string(),
            });
        }
    }

    let trimmed = raw_path.strip_prefix("./").unwrap_or(raw_path);

    if trimmed.starts_with('/') {
        return Err(ManifestError::InvalidPath {
            path: raw_path.to_string(),
            reason: "path cannot be an absolute path starting with '/'".to_string(),
        });
    }

    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        let first = trimmed.as_bytes()[0];
        if first.is_ascii_alphabetic() {
            return Err(ManifestError::InvalidPath {
                path: raw_path.to_string(),
                reason: "path cannot begin with a drive letter".to_string(),
            });
        }
    }

    let parts: Vec<&str> = trimmed.split('/').collect();
    let mut normalized_parts = Vec::new();

    for part in parts {
        if part.is_empty() {
            return Err(ManifestError::InvalidPath {
                path: raw_path.to_string(),
                reason: "path contains empty directory segment '//'".to_string(),
            });
        }
        if part == "." {
            continue;
        }
        if part == ".." {
            return Err(ManifestError::InvalidPath {
                path: raw_path.to_string(),
                reason: "path contains forbidden parent directory traversal '..'".to_string(),
            });
        }
        normalized_parts.push(part);
    }

    if normalized_parts.is_empty() {
        return Err(ManifestError::InvalidPath {
            path: raw_path.to_string(),
            reason: "path resolved to empty location".to_string(),
        });
    }

    Ok(normalized_parts.join("/"))
}

/// What: Single canonical entry in a scan snapshot manifest.
///
/// Inputs:
/// - Snapshot category ("recipe" or "source"), normalized relative path, byte size, SHA-256 hex, executable bit, and binary flag.
///
/// Output:
/// - Struct representing a file entry in the snapshot manifest.
///
/// Details:
/// - Path must be normalized with `normalize_manifest_path`. SHA-256 hex must be 64 characters.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ManifestEntry {
    /// Snapshot category ("recipe" or "source").
    pub snapshot_category: String,
    /// Normalized relative path inside snapshot root.
    pub relative_path: String,
    /// Uncompressed file size in bytes.
    pub size_bytes: u64,
    /// Lowercase 64-character SHA-256 hexadecimal digest of file content.
    pub sha256_hex: String,
    /// True if file permissions include executable mode bit.
    pub is_executable: bool,
    /// True if non-UTF8 or non-text binary bytes were detected.
    pub is_binary: bool,
}

impl ManifestEntry {
    /// What: Create a new `ManifestEntry` after validating path and digest.
    ///
    /// Inputs:
    /// - `snapshot_category`: Category string ("recipe" or "source").
    /// - `raw_path`: Relative path candidate string.
    /// - `size_bytes`: File byte count.
    /// - `sha256_hex`: 64-char SHA-256 hex string.
    /// - `is_executable`: Executable permission bit flag.
    /// - `is_binary`: Non-text content flag.
    ///
    /// Output:
    /// - `Ok(ManifestEntry)` if path and digest are valid, `Err(ManifestError)` otherwise.
    ///
    /// Details:
    /// - Normalizes `raw_path` and verifies `sha256_hex` is 64 hexadecimal characters.
    ///
    /// # Errors
    /// Returns `ManifestError::InvalidPath` if path or SHA-256 digest fails validation.
    pub fn new(
        snapshot_category: impl Into<String>,
        raw_path: &str,
        size_bytes: u64,
        sha256_hex: impl AsRef<str>,
        is_executable: bool,
        is_binary: bool,
    ) -> Result<Self, ManifestError> {
        let relative_path = normalize_manifest_path(raw_path)?;
        let sha_str = sha256_hex.as_ref().to_ascii_lowercase();

        if sha_str.len() != 64 || !sha_str.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ManifestError::InvalidPath {
                path: relative_path,
                reason: format!(
                    "SHA-256 hex digest must be 64 hexadecimal characters, got {}",
                    sha_str.len()
                ),
            });
        }

        Ok(Self {
            snapshot_category: snapshot_category.into(),
            relative_path,
            size_bytes,
            sha256_hex: sha_str,
            is_executable,
            is_binary,
        })
    }
}

/// What: Ordered, hashable manifest representing the complete inventory of a scan snapshot.
///
/// Inputs:
/// - Vector of `ManifestEntry` instances.
///
/// Output:
/// - Struct containing canonically sorted entries and methods to compute manifest hashes.
///
/// Details:
/// - Entries are automatically sorted by `(snapshot_category, relative_path)`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CanonicalManifest {
    /// Canonically sorted entries.
    pub entries: Vec<ManifestEntry>,
}

impl CanonicalManifest {
    /// What: Create a new `CanonicalManifest` from entries, sorting them canonically.
    ///
    /// Inputs:
    /// - `entries`: Vector of `ManifestEntry`.
    ///
    /// Output:
    /// - `CanonicalManifest` with entries sorted by `(snapshot_category, relative_path)`.
    ///
    /// Details:
    /// - Canonical sorting ensures deterministic manifest hashing regardless of filesystem traversal order.
    #[must_use]
    pub fn new(mut entries: Vec<ManifestEntry>) -> Self {
        entries.sort_by(|a, b| {
            a.snapshot_category
                .cmp(&b.snapshot_category)
                .then_with(|| a.relative_path.cmp(&b.relative_path))
        });
        Self { entries }
    }

    /// What: Calculate the canonical SHA-256 hash of this manifest.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Lowercase 64-character SHA-256 hexadecimal hash string.
    ///
    /// Details:
    /// - Hashes line-formatted entry fields: `"{category}\t{path}\t{size}\t{sha256}\t{exec}\t{bin}\n"`.
    #[must_use]
    pub fn calculate_manifest_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for entry in &self.entries {
            let line = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                entry.snapshot_category,
                entry.relative_path,
                entry.size_bytes,
                entry.sha256_hex,
                entry.is_executable,
                entry.is_binary
            );
            hasher.update(line.as_bytes());
        }
        let digest: [u8; 32] = hasher.finalize().into();
        format_hex(&digest)
    }

    /// What: Search for an entry by snapshot category and relative path.
    ///
    /// Inputs:
    /// - `category`: Snapshot category string ("recipe" or "source").
    /// - `relative_path`: Relative path string.
    ///
    /// Output:
    /// - `Option<&ManifestEntry>`.
    ///
    /// Details:
    /// - Performs linear or binary search over sorted entries.
    #[must_use]
    pub fn find_entry(&self, category: &str, relative_path: &str) -> Option<&ManifestEntry> {
        self.entries
            .iter()
            .find(|e| e.snapshot_category == category && e.relative_path == relative_path)
    }

    /// What: Check if the manifest contains zero entries.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `bool`.
    ///
    /// Details:
    /// - Returns true if entries vector is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// What: Get the count of entries in the manifest.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `usize`.
    ///
    /// Details:
    /// - Returns the length of the entries vector.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }
}
