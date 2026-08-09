//! Integrity evaluation and bounded, in-process source/archive inspection.

use crate::logic::pi_scan::manifest::{CanonicalManifest, ManifestEntry, normalize_manifest_path};
use crate::logic::pi_scan::recipe::DeclaredChecksum;
use blake2::{Blake2b512, Digest as BlakeDigest};
use sha2::{Digest as ShaDigest, Sha256, Sha384, Sha512};
use std::collections::BTreeSet;
use std::fmt;
use std::io::{Cursor, Read};

/// Maximum compressed bytes accepted for one source.
pub const MAX_COMPRESSED_SOURCE_BYTES: u64 = 100 * 1024 * 1024;
/// Maximum expanded regular-file bytes accepted for one source.
pub const MAX_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;
/// Maximum expanded bytes accepted for one regular archive entry.
pub const MAX_ENTRY_BYTES: u64 = MAX_EXPANDED_BYTES;
/// Maximum entries accepted in one archive.
pub const MAX_ARCHIVE_ENTRIES: usize = 10_000;
/// Maximum normalized archive path depth.
pub const MAX_ARCHIVE_PATH_DEPTH: usize = 16;
/// Maximum expanded-to-compressed byte ratio.
pub const MAX_EXPANSION_RATIO: u64 = 10;

/// What: Checksum algorithms represented by makepkg `.SRCINFO` arrays.
///
/// Inputs:
/// - A `.SRCINFO` checksum key and expected value.
///
/// Output:
/// - Strong or weak algorithm identity used by integrity policy.
///
/// Details:
/// - Only SHA-256/384/512 and BLAKE2b-512 can establish checksum completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChecksumAlgorithm {
    /// MD5 is recognized but is weak and never computed for completeness.
    Md5,
    /// SHA-1 is recognized but is weak and never computed for completeness.
    Sha1,
    /// SHA-256 strong digest.
    Sha256,
    /// SHA-384 strong digest.
    Sha384,
    /// SHA-512 strong digest.
    Sha512,
    /// BLAKE2b-512 strong digest used by makepkg `b2sums`.
    Blake2b512,
}

impl ChecksumAlgorithm {
    /// All supported `.SRCINFO` checksum algorithms in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Md5,
        Self::Sha1,
        Self::Sha256,
        Self::Sha384,
        Self::Sha512,
        Self::Blake2b512,
    ];
    /// Key/algorithm pairs recognized by the strict recipe parser.
    pub const SRCINFO_KEYS: [(&'static str, Self); 6] = [
        ("md5sums", Self::Md5),
        ("sha1sums", Self::Sha1),
        ("sha256sums", Self::Sha256),
        ("sha384sums", Self::Sha384),
        ("sha512sums", Self::Sha512),
        ("b2sums", Self::Blake2b512),
    ];

    /// What: Return the canonical `.SRCINFO` array key.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Static key name.
    ///
    /// Details:
    /// - Architecture suffixes are intentionally omitted.
    #[must_use]
    pub const fn srcinfo_key(self) -> &'static str {
        match self {
            Self::Md5 => "md5sums",
            Self::Sha1 => "sha1sums",
            Self::Sha256 => "sha256sums",
            Self::Sha384 => "sha384sums",
            Self::Sha512 => "sha512sums",
            Self::Blake2b512 => "b2sums",
        }
    }

    /// Return whether the algorithm can establish checksum completeness.
    const fn is_strong(self) -> bool {
        matches!(
            self,
            Self::Sha256 | Self::Sha384 | Self::Sha512 | Self::Blake2b512
        )
    }

    /// Return the required lowercase hexadecimal length.
    const fn hex_length(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
            Self::Sha384 => 96,
            Self::Sha512 | Self::Blake2b512 => 128,
        }
    }
}

/// What: Result status shared by integrity and archive inspection.
///
/// Inputs:
/// - Verified data, a coverage limitation, or a hard integrity/corruption failure.
///
/// Output:
/// - Explicit complete/incomplete/failed state.
///
/// Details:
/// - Incomplete is never silently promoted to complete by partial manifests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionStatus {
    /// Every applicable check completed under policy.
    Complete,
    /// Data was inspectable only with an explicit limitation.
    Incomplete,
    /// Integrity mismatch or malformed/corrupt input invalidated the data.
    Failed,
}

/// What: External detached-signature verification state supplied by a future isolated adapter.
///
/// Inputs:
/// - Whether signature policy applies and its verified result.
///
/// Output:
/// - Pure integrity-policy input.
///
/// Details:
/// - This module performs no GPG execution or key retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    /// No mandatory signature declaration applies.
    NotRequired,
    /// Mandatory signature and exact fingerprint verification succeeded externally.
    Verified,
    /// Mandatory verification could not be performed.
    Unavailable,
    /// Mandatory verification ran and failed.
    Failed,
}

/// What: Pure checksum/signature policy result for downloaded bytes.
///
/// Inputs:
/// - Declared checksums, exact bytes, and explicit signature status.
///
/// Output:
/// - Complete, incomplete, or failed with deterministic reasons.
///
/// Details:
/// - Includes the computed strong digests used for comparison.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntegrityReport {
    /// Final policy status.
    pub status: AcquisitionStatus,
    /// Deterministically ordered policy reasons.
    pub reasons: Vec<String>,
    /// Computed matching-candidate strong digests.
    pub computed: Vec<(ChecksumAlgorithm, String)>,
}

/// What: Evaluate aligned strong/weak/SKIP/missing checksum policy.
///
/// Inputs:
/// - `bytes`: Exact acquired bytes.
/// - `checksums`: Checksums positionally bound by the recipe parser.
/// - `signature_status`: Result from an isolated exact-fingerprint verifier, if required.
///
/// Output:
/// - Explicit integrity report.
///
/// Details:
/// - Any malformed or mismatching strong digest fails; verified signatures may cover missing/SKIP/weak-only declarations.
#[must_use]
pub fn evaluate_integrity(
    bytes: &[u8],
    checksums: &[DeclaredChecksum],
    signature_status: SignatureStatus,
) -> IntegrityReport {
    if signature_status == SignatureStatus::Failed {
        return integrity_report(
            AcquisitionStatus::Failed,
            "required signature verification failed",
        );
    }
    let strong: Vec<&DeclaredChecksum> = checksums
        .iter()
        .filter(|checksum| checksum.algorithm.is_strong() && checksum.value != "SKIP")
        .collect();
    let has_skip = checksums.iter().any(|checksum| checksum.value == "SKIP");
    let has_weak = checksums
        .iter()
        .any(|checksum| !checksum.algorithm.is_strong());
    let mut computed = Vec::new();
    for checksum in &strong {
        let expected = checksum.value.to_ascii_lowercase();
        if !valid_expected_digest(checksum.algorithm, &expected) {
            return IntegrityReport {
                status: AcquisitionStatus::Failed,
                reasons: vec![format!(
                    "{} declaration is not a canonical hexadecimal digest",
                    checksum.algorithm.srcinfo_key()
                )],
                computed,
            };
        }
        let actual = compute_digest(checksum.algorithm, bytes);
        computed.push((checksum.algorithm, actual.clone()));
        if actual != expected {
            return IntegrityReport {
                status: AcquisitionStatus::Failed,
                reasons: vec![format!("{} mismatch", checksum.algorithm.srcinfo_key())],
                computed,
            };
        }
    }
    if signature_status == SignatureStatus::Unavailable {
        return IntegrityReport {
            status: AcquisitionStatus::Incomplete,
            reasons: vec!["required signature verification is unavailable".to_string()],
            computed,
        };
    }
    if !strong.is_empty() {
        return IntegrityReport {
            status: AcquisitionStatus::Complete,
            reasons: vec!["at least one strong checksum matched".to_string()],
            computed,
        };
    }
    if signature_status == SignatureStatus::Verified {
        return IntegrityReport {
            status: AcquisitionStatus::Complete,
            reasons: vec![
                "required exact-fingerprint signature verification succeeded".to_string(),
            ],
            computed,
        };
    }
    let reason = if signature_status == SignatureStatus::Unavailable {
        "required signature verification is unavailable"
    } else if has_skip {
        "checksum policy contains only SKIP or non-strong declarations"
    } else if has_weak {
        "checksum policy contains only weak declarations"
    } else {
        "no checksum is declared"
    };
    IntegrityReport {
        status: AcquisitionStatus::Incomplete,
        reasons: vec![reason.to_string()],
        computed,
    }
}

/// Construct a one-reason integrity report.
fn integrity_report(status: AcquisitionStatus, reason: &str) -> IntegrityReport {
    IntegrityReport {
        status,
        reasons: vec![reason.to_string()],
        computed: Vec::new(),
    }
}

/// Validate one expected digest's canonical length and alphabet.
fn valid_expected_digest(algorithm: ChecksumAlgorithm, value: &str) -> bool {
    value.len() == algorithm.hex_length()
        && value.chars().all(|character| character.is_ascii_hexdigit())
}

/// Compute a strong digest selected by policy.
fn compute_digest(algorithm: ChecksumAlgorithm, bytes: &[u8]) -> String {
    match algorithm {
        ChecksumAlgorithm::Sha256 => format_hex(&Sha256::digest(bytes)),
        ChecksumAlgorithm::Sha384 => format_hex(&Sha384::digest(bytes)),
        ChecksumAlgorithm::Sha512 => format_hex(&Sha512::digest(bytes)),
        ChecksumAlgorithm::Blake2b512 => format_hex(&Blake2b512::digest(bytes)),
        ChecksumAlgorithm::Md5 | ChecksumAlgorithm::Sha1 => String::new(),
    }
}

/// Format bytes as lowercase hexadecimal.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

/// What: Supported source/archive container formats.
///
/// Inputs:
/// - Explicit media/filename classification from an acquisition adapter.
///
/// Output:
/// - Decoder and entry-iteration strategy.
///
/// Details:
/// - Compressed tar variants are explicit; standalone compressors produce one raw entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    /// Uncompressed single raw file.
    Raw,
    /// Uncompressed tar archive.
    Tar,
    /// Standalone gzip stream.
    Gzip,
    /// Standalone bzip2 stream.
    Bzip2,
    /// Standalone XZ stream.
    Xz,
    /// Standalone Zstandard stream.
    Zstd,
    /// Gzip-compressed tar archive.
    TarGzip,
    /// Bzip2-compressed tar archive.
    TarBzip2,
    /// XZ-compressed tar archive.
    TarXz,
    /// Zstandard-compressed tar archive.
    TarZstd,
    /// ZIP archive restricted to Stored and Deflate entries.
    Zip,
}

/// What: Effective archive limits bounded by compiled maxima.
///
/// Inputs:
/// - Optional lower operational limits.
///
/// Output:
/// - Validated inspection limits that can never exceed policy maxima.
///
/// Details:
/// - `Default` selects every compiled maximum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    /// Maximum compressed source bytes.
    pub compressed_bytes: u64,
    /// Maximum aggregate regular-file bytes.
    pub expanded_bytes: u64,
    /// Maximum bytes in one regular entry.
    pub entry_bytes: u64,
    /// Maximum archive entry count.
    pub entries: usize,
    /// Maximum normalized path depth.
    pub path_depth: usize,
    /// Maximum expanded/compressed ratio.
    pub expansion_ratio: u64,
}

impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            compressed_bytes: MAX_COMPRESSED_SOURCE_BYTES,
            expanded_bytes: MAX_EXPANDED_BYTES,
            entry_bytes: MAX_ENTRY_BYTES,
            entries: MAX_ARCHIVE_ENTRIES,
            path_depth: MAX_ARCHIVE_PATH_DEPTH,
            expansion_ratio: MAX_EXPANSION_RATIO,
        }
    }
}

impl ArchiveLimits {
    /// What: Validate lowered archive limits against compiled security maxima.
    ///
    /// Inputs:
    /// - `limits`: Candidate effective limits.
    ///
    /// Output:
    /// - The unchanged limits when every value is nonzero and no maximum is raised.
    ///
    /// Details:
    /// - Callers cannot weaken compiled resource bounds through configuration.
    ///
    /// # Errors
    /// Returns `InspectionError` when any field is zero or above its compiled maximum.
    pub fn validate(limits: Self) -> Result<Self, InspectionError> {
        let valid = limits.compressed_bytes > 0
            && limits.compressed_bytes <= MAX_COMPRESSED_SOURCE_BYTES
            && limits.expanded_bytes > 0
            && limits.expanded_bytes <= MAX_EXPANDED_BYTES
            && limits.entry_bytes > 0
            && limits.entry_bytes <= MAX_ENTRY_BYTES
            && limits.entries > 0
            && limits.entries <= MAX_ARCHIVE_ENTRIES
            && limits.path_depth > 0
            && limits.path_depth <= MAX_ARCHIVE_PATH_DEPTH
            && limits.expansion_ratio > 0
            && limits.expansion_ratio <= MAX_EXPANSION_RATIO;
        if valid {
            Ok(limits)
        } else {
            Err(InspectionError::new(
                AcquisitionStatus::Failed,
                "archive limits are zero or exceed compiled maxima",
            ))
        }
    }
}

/// What: Bounded inspection failure classified by acquisition semantics.
///
/// Inputs:
/// - Corruption, unsafe archive metadata, or a hard resource violation.
///
/// Output:
/// - Failed or incomplete state with inert reason text.
///
/// Details:
/// - No filesystem materialization occurs before or after this error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionError {
    /// Status assigned to the rejected input.
    pub status: AcquisitionStatus,
    /// Deterministic reason for rejection.
    pub reason: String,
}

impl InspectionError {
    /// Construct a classified inspection error.
    fn new(status: AcquisitionStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: reason.into(),
        }
    }
}

impl fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Archive inspection {:?}: {}",
            self.status, self.reason
        )
    }
}

impl std::error::Error for InspectionError {}

/// What: Canonical result of inspecting one source without materialization.
///
/// Inputs:
/// - Exact bytes, explicit format, source filename, and effective limits.
///
/// Output:
/// - Complete/incomplete/failed status, canonical manifest, counts, and reasons.
///
/// Details:
/// - Partial manifests are retained for diagnostics but never imply complete coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InspectionReport {
    /// Final archive coverage status.
    pub status: AcquisitionStatus,
    /// Canonical byte-hashed regular-file manifest.
    pub manifest: CanonicalManifest,
    /// Number of archive entries observed before completion or rejection.
    pub observed_entries: usize,
    /// Aggregate expanded regular-file bytes observed.
    pub expanded_bytes: u64,
    /// Explicit completion or rejection reasons.
    pub reasons: Vec<String>,
}

/// Mutable archive inspection accounting.
struct InspectionContext {
    /// Effective hard limits.
    limits: ArchiveLimits,
    /// Original compressed/source byte count used by ratio policy.
    compressed_bytes: u64,
    /// Number of archive entries observed.
    observed_entries: usize,
    /// Aggregate expanded regular-file bytes.
    expanded_bytes: u64,
    /// Actual normalized archive paths.
    seen_entries: BTreeSet<String>,
    /// Regular-file paths.
    files: BTreeSet<String>,
    /// Explicit and implicit directory paths.
    directories: BTreeSet<String>,
    /// Canonical manifest entries accumulated so far.
    manifest_entries: Vec<ManifestEntry>,
}

impl InspectionContext {
    /// Create accounting for one bounded source.
    const fn new(limits: ArchiveLimits, compressed_bytes: u64) -> Self {
        Self {
            limits,
            compressed_bytes,
            observed_entries: 0,
            expanded_bytes: 0,
            seen_entries: BTreeSet::new(),
            files: BTreeSet::new(),
            directories: BTreeSet::new(),
            manifest_entries: Vec::new(),
        }
    }

    /// Count and validate one normalized entry path before reading its body.
    fn register_path(&mut self, path: &str, is_directory: bool) -> Result<(), InspectionError> {
        self.observed_entries += 1;
        if self.observed_entries > self.limits.entries {
            return Err(incomplete("archive entry-count limit exceeded"));
        }
        if path.split('/').count() > self.limits.path_depth {
            return Err(incomplete("archive path-depth limit exceeded"));
        }
        if !self.seen_entries.insert(path.to_string()) {
            return Err(incomplete(format!("duplicate archive path `{path}`")));
        }
        for ancestor in path_ancestors(path) {
            if self.files.contains(ancestor) {
                return Err(incomplete(format!(
                    "archive path conflict: file `{ancestor}` is an ancestor"
                )));
            }
            self.directories.insert(ancestor.to_string());
        }
        if is_directory {
            if self.files.contains(path) {
                return Err(incomplete(format!("archive path conflict at `{path}`")));
            }
            self.directories.insert(path.to_string());
        } else {
            let prefix = format!("{path}/");
            if self.directories.contains(path)
                || self
                    .files
                    .iter()
                    .any(|existing| existing.starts_with(&prefix))
                || self
                    .directories
                    .iter()
                    .any(|existing| existing.starts_with(&prefix))
            {
                return Err(incomplete(format!("archive path conflict at `{path}`")));
            }
            self.files.insert(path.to_string());
        }
        Ok(())
    }

    /// Account expanded bytes and enforce per-entry, aggregate, and ratio limits.
    fn account_file_bytes(&mut self, entry_size: u64) -> Result<(), InspectionError> {
        if entry_size > self.limits.entry_bytes {
            return Err(incomplete("archive per-entry byte limit exceeded"));
        }
        self.expanded_bytes = self
            .expanded_bytes
            .checked_add(entry_size)
            .ok_or_else(|| incomplete("expanded byte accounting overflowed"))?;
        if self.expanded_bytes > self.limits.expanded_bytes {
            return Err(incomplete("archive expanded-byte limit exceeded"));
        }
        let ratio_limit = self
            .compressed_bytes
            .saturating_mul(self.limits.expansion_ratio);
        if self.expanded_bytes > ratio_limit {
            return Err(incomplete("archive expansion-ratio limit exceeded"));
        }
        Ok(())
    }

    /// Hash and append one fully read regular file.
    fn append_file(
        &mut self,
        path: &str,
        bytes: &[u8],
        executable: bool,
    ) -> Result<(), InspectionError> {
        self.account_file_bytes(bytes.len() as u64)?;
        let digest = format_hex(&Sha256::digest(bytes));
        let entry = ManifestEntry::new(
            "source",
            path,
            bytes.len() as u64,
            digest,
            executable,
            is_binary(bytes),
        )
        .map_err(|error| incomplete(error.to_string()))?;
        self.manifest_entries.push(entry);
        Ok(())
    }
}

/// What: Inspect an explicit source format entry-by-entry entirely in process.
///
/// Inputs:
/// - `source_name`: Safe effective source filename.
/// - `bytes`: Exact compressed or raw source bytes.
/// - `format`: Explicit supported format.
/// - `limits`: Lowered or default compiled limits.
///
/// Output:
/// - Canonical manifest and explicit completion status.
///
/// Details:
/// - Never calls broad unpack helpers, writes files, runs commands, or performs network access.
#[must_use]
pub fn inspect_source(
    source_name: &str,
    bytes: &[u8],
    format: ArchiveFormat,
    limits: ArchiveLimits,
) -> InspectionReport {
    let limits = match ArchiveLimits::validate(limits) {
        Ok(limits) => limits,
        Err(error) => return failed_report(error),
    };
    if bytes.len() as u64 > limits.compressed_bytes {
        return failed_report(incomplete("compressed source byte limit exceeded"));
    }
    let mut context = InspectionContext::new(limits, bytes.len() as u64);
    let outcome = inspect_format(&mut context, source_name, bytes, format);
    report_from_context(context, outcome)
}

/// Dispatch one explicit format to its narrow reader.
fn inspect_format(
    context: &mut InspectionContext,
    source_name: &str,
    bytes: &[u8],
    format: ArchiveFormat,
) -> Result<(), InspectionError> {
    match format {
        ArchiveFormat::Raw => inspect_raw(context, source_name, bytes),
        ArchiveFormat::Tar => inspect_tar(context, bytes),
        ArchiveFormat::Zip => inspect_zip(context, bytes),
        ArchiveFormat::Gzip => inspect_compressed_raw(
            context,
            source_name,
            flate2::read::MultiGzDecoder::new(bytes),
        ),
        ArchiveFormat::Bzip2 => inspect_compressed_raw(
            context,
            source_name,
            bzip2::read::MultiBzDecoder::new(bytes),
        ),
        ArchiveFormat::Xz => {
            inspect_compressed_raw(context, source_name, lzma_rust2::XzReader::new(bytes, true))
        }
        ArchiveFormat::Zstd => {
            let decoder = zstd::stream::read::Decoder::new(bytes)
                .map_err(|error| failed(format!("invalid zstd stream: {error}")))?;
            inspect_compressed_raw(context, source_name, decoder)
        }
        ArchiveFormat::TarGzip => {
            inspect_compressed_tar(context, flate2::read::MultiGzDecoder::new(bytes))
        }
        ArchiveFormat::TarBzip2 => {
            inspect_compressed_tar(context, bzip2::read::MultiBzDecoder::new(bytes))
        }
        ArchiveFormat::TarXz => {
            inspect_compressed_tar(context, lzma_rust2::XzReader::new(bytes, true))
        }
        ArchiveFormat::TarZstd => {
            let decoder = zstd::stream::read::Decoder::new(bytes)
                .map_err(|error| failed(format!("invalid zstd stream: {error}")))?;
            inspect_compressed_tar(context, decoder)
        }
    }
}

/// Inspect one raw file under its effective source name.
fn inspect_raw(
    context: &mut InspectionContext,
    source_name: &str,
    bytes: &[u8],
) -> Result<(), InspectionError> {
    let path = normalize_archive_path(source_name, false)?;
    context.register_path(&path, false)?;
    context.append_file(&path, bytes, false)
}

/// Decode a standalone compressed stream and inspect it as one raw file.
fn inspect_compressed_raw(
    context: &mut InspectionContext,
    source_name: &str,
    reader: impl Read,
) -> Result<(), InspectionError> {
    let decoded = read_decoder_bounded(context, reader)?;
    let output_name = strip_compression_suffix(source_name);
    inspect_raw(context, output_name, &decoded)
}

/// Decode a compressed tar stream under a bounded in-memory seam, then iterate headers.
fn inspect_compressed_tar(
    context: &mut InspectionContext,
    reader: impl Read,
) -> Result<(), InspectionError> {
    let decoded = read_decoder_bounded(context, reader)?;
    inspect_tar(context, &decoded)
}

/// Bound decoder output before passing it to a format-specific iterator.
fn read_decoder_bounded(
    context: &InspectionContext,
    mut reader: impl Read,
) -> Result<Vec<u8>, InspectionError> {
    let overhead = (context.limits.entries as u64)
        .saturating_mul(512)
        .saturating_add(1024);
    let expanded_limit = context.limits.expanded_bytes.saturating_add(overhead);
    let ratio_limit = context
        .compressed_bytes
        .saturating_mul(context.limits.expansion_ratio)
        .saturating_add(overhead);
    let output_limit = expanded_limit.min(ratio_limit);
    let mut output = Vec::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| failed(format!("compressed stream decode failed: {error}")))?;
        if read == 0 {
            break;
        }
        if (output.len() as u64).saturating_add(read as u64) > output_limit {
            return Err(incomplete(
                "decoded stream exceeds bounded inspection buffer",
            ));
        }
        output.extend_from_slice(&buffer[..read]);
    }
    Ok(output)
}

/// Iterate tar headers and regular-file bodies without calling `unpack`.
fn inspect_tar(context: &mut InspectionContext, bytes: &[u8]) -> Result<(), InspectionError> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| failed(format!("invalid tar archive: {error}")))?;
    for entry_result in entries {
        let mut entry =
            entry_result.map_err(|error| failed(format!("invalid tar entry: {error}")))?;
        let entry_type = entry.header().entry_type();
        let directory = entry_type.is_dir();
        let path_bytes = entry.path_bytes();
        let raw_path =
            std::str::from_utf8(&path_bytes).map_err(|_| incomplete("tar path is not UTF-8"))?;
        let path = normalize_archive_path(raw_path, directory)?;
        if directory {
            context.register_path(&path, true)?;
            continue;
        }
        context.register_path(&path, false)?;
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return Err(incomplete(format!(
                "archive link `{path}` is not materialized"
            )));
        }
        if !entry_type.is_file() {
            return Err(incomplete(format!(
                "archive special or unknown entry `{path}`"
            )));
        }
        let declared_size = entry
            .header()
            .size()
            .map_err(|error| failed(format!("invalid tar size for `{path}`: {error}")))?;
        let file_bytes = read_entry_bounded(&mut entry, declared_size, context.limits.entry_bytes)?;
        let mode = entry.header().mode().unwrap_or_default();
        context.append_file(&path, &file_bytes, mode & 0o111 != 0)?;
    }
    Ok(())
}

/// Iterate ZIP entries and permit only Stored/Deflate regular files and directories.
fn inspect_zip(context: &mut InspectionContext, bytes: &[u8]) -> Result<(), InspectionError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|error| failed(format!("invalid ZIP archive: {error}")))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| failed(format!("invalid ZIP entry {index}: {error}")))?;
        if entry.encrypted() {
            return Err(incomplete("encrypted ZIP entries are unsupported"));
        }
        if !matches!(
            entry.compression(),
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            return Err(incomplete(format!(
                "unsupported ZIP compression for `{}`",
                entry.name()
            )));
        }
        let raw_name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| incomplete("ZIP path is not UTF-8"))?;
        let directory = entry.is_dir();
        let path = normalize_archive_path(raw_name, directory)?;
        context.register_path(&path, directory)?;
        if entry.is_symlink() {
            return Err(incomplete(format!(
                "archive link `{path}` is not materialized"
            )));
        }
        if !directory && (!entry.is_file() || zip_mode_is_special(entry.unix_mode())) {
            return Err(incomplete(format!("archive special entry `{path}`")));
        }
        if directory {
            continue;
        }
        let declared_size = entry.size();
        let file_bytes = read_entry_bounded(&mut entry, declared_size, context.limits.entry_bytes)?;
        let executable = entry.unix_mode().is_some_and(|mode| mode & 0o111 != 0);
        context.append_file(&path, &file_bytes, executable)?;
    }
    Ok(())
}

/// Return whether ZIP Unix mode bits describe a special non-file entry.
fn zip_mode_is_special(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| {
        let file_type = mode & 0o170_000;
        file_type != 0 && file_type != 0o100_000
    })
}

/// Read one declared regular entry with a hard per-entry ceiling and exact-size check.
fn read_entry_bounded(
    reader: &mut impl Read,
    declared_size: u64,
    limit: u64,
) -> Result<Vec<u8>, InspectionError> {
    if declared_size > limit {
        return Err(incomplete("archive per-entry byte limit exceeded"));
    }
    let capacity = usize::try_from(declared_size)
        .map_err(|_| incomplete("archive entry does not fit address space"))?;
    let mut output = Vec::with_capacity(capacity);
    let mut limited = reader.take(limit.saturating_add(1));
    limited
        .read_to_end(&mut output)
        .map_err(|error| failed(format!("archive entry read failed: {error}")))?;
    if output.len() as u64 > limit {
        return Err(incomplete("archive per-entry byte limit exceeded"));
    }
    if output.len() as u64 != declared_size {
        return Err(failed("archive entry size does not match its header"));
    }
    Ok(output)
}

/// Normalize one UTF-8 archive path and apply directory trailing-slash rules.
fn normalize_archive_path(raw_path: &str, directory: bool) -> Result<String, InspectionError> {
    let candidate = if directory {
        raw_path.trim_end_matches('/')
    } else {
        raw_path
    };
    normalize_manifest_path(candidate).map_err(|error| incomplete(error.to_string()))
}

/// Return each non-empty ancestor of a normalized path.
fn path_ancestors(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(|(index, _)| &path[..index])
}

/// Strip exactly one supported standalone compression suffix.
fn strip_compression_suffix(source_name: &str) -> &str {
    for suffix in [".gz", ".bz2", ".xz", ".zst"] {
        if let Some(stripped) = source_name.strip_suffix(suffix)
            && !stripped.is_empty()
        {
            return stripped;
        }
    }
    source_name
}

/// Classify bytes conservatively for manifest text coverage.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
}

/// Convert inspection state and a classified outcome into a stable report.
fn report_from_context(
    context: InspectionContext,
    outcome: Result<(), InspectionError>,
) -> InspectionReport {
    let (status, reasons) = match outcome {
        Ok(()) => (
            AcquisitionStatus::Complete,
            vec!["all entries inspected and byte-hashed".to_string()],
        ),
        Err(error) => (error.status, vec![error.reason]),
    };
    InspectionReport {
        status,
        manifest: CanonicalManifest::new(context.manifest_entries),
        observed_entries: context.observed_entries,
        expanded_bytes: context.expanded_bytes,
        reasons,
    }
}

/// Build a report when inspection cannot initialize.
fn failed_report(error: InspectionError) -> InspectionReport {
    InspectionReport {
        status: error.status,
        manifest: CanonicalManifest::new(Vec::new()),
        observed_entries: 0,
        expanded_bytes: 0,
        reasons: vec![error.reason],
    }
}

/// Construct an incomplete inspection rejection.
fn incomplete(reason: impl Into<String>) -> InspectionError {
    InspectionError::new(AcquisitionStatus::Incomplete, reason)
}

/// Construct a failed corruption/decoder rejection.
fn failed(reason: impl Into<String>) -> InspectionError {
    InspectionError::new(AcquisitionStatus::Failed, reason)
}
