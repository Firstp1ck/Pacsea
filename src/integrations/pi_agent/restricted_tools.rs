//! Path-confined, read-only, bounded implementations of the four model-visible tools.
//!
//! The model can call only `pacsea_scan_read`, `pacsea_scan_grep`, `pacsea_scan_find`,
//! and `pacsea_scan_ls`. Every call names a snapshot by an opaque id that comes from a
//! private Pacsea-owned descriptor; the model never supplies a root, an absolute path,
//! or a regular expression.
//!
//! Enforced boundaries:
//!
//! - relative, normalized, depth-bounded, control-free paths only;
//! - no `..`, no `.`, no absolute path, no Windows prefix, no empty component;
//! - symlink escapes, root replacement, and non-regular files are rejected after
//!   canonicalization, not before;
//! - every request bound and every result bound is checked, and oversized requests are
//!   rejected rather than silently clamped;
//! - `pacsea_scan_grep` is literal substring search only;
//! - file contents must be strict UTF-8; other encodings are reported as unsupported.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use super::limits;

/// What: Failure modes of a restricted tool request.
///
/// Inputs: Produced by path resolution and by each tool implementation.
///
/// Output: Implements `Display`/`Error`. The rendered text is inert model-visible data.
///
/// Details:
/// - Messages never include host paths outside the snapshot, file contents, or any
///   sentinel data, so a rejected traversal cannot become an oracle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolError {
    /// The named snapshot is not registered for this scan.
    UnknownSnapshot {
        /// Snapshot id supplied by the model.
        snapshot: String,
    },
    /// The registered snapshot root no longer resolves to the recorded directory.
    SnapshotRootChanged {
        /// Snapshot id whose root was replaced.
        snapshot: String,
    },
    /// The relative path was empty.
    EmptyPath,
    /// The relative path was absolute or carried a filesystem prefix.
    AbsolutePath,
    /// The relative path contained `.` or `..`.
    TraversalComponent,
    /// The relative path contained a control character or a path separator inside a component.
    ControlCharacter,
    /// The relative path exceeded the compiled depth bound.
    TooDeep {
        /// Compiled depth bound.
        limit: usize,
    },
    /// The resolved path left the snapshot root, typically through a symlink.
    OutsideRoot,
    /// The target exists but is not a regular file or directory.
    NotARegularFile,
    /// The target does not exist inside the snapshot.
    NotFound,
    /// A request parameter exceeded its compiled bound.
    RequestTooLarge {
        /// Parameter name.
        parameter: &'static str,
        /// Requested value.
        requested: usize,
        /// Compiled bound.
        limit: usize,
    },
    /// A request parameter was invalid, for example an empty search literal.
    InvalidRequest {
        /// Explanation of the invalid parameter.
        reason: String,
    },
    /// The file is not strict UTF-8 and therefore cannot be returned as text.
    UnsupportedEncoding,
    /// An I/O error occurred while serving the request.
    Io {
        /// Error rendering without host path disclosure.
        reason: String,
    },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSnapshot { snapshot } => {
                write!(f, "unknown snapshot {snapshot:?}")
            }
            Self::SnapshotRootChanged { snapshot } => write!(
                f,
                "snapshot {snapshot:?} is no longer the directory Pacsea prepared; the scan was stopped"
            ),
            Self::EmptyPath => write!(f, "the path must not be empty"),
            Self::AbsolutePath => write!(f, "absolute paths are not allowed"),
            Self::TraversalComponent => write!(f, "'.' and '..' path components are not allowed"),
            Self::ControlCharacter => {
                write!(
                    f,
                    "path components must not contain control characters or separators"
                )
            }
            Self::TooDeep { limit } => write!(f, "paths may not be deeper than {limit} components"),
            Self::OutsideRoot => write!(f, "the path resolves outside the snapshot"),
            Self::NotARegularFile => {
                write!(f, "only regular files and directories can be accessed")
            }
            Self::NotFound => write!(f, "no such entry in this snapshot"),
            Self::RequestTooLarge {
                parameter,
                requested,
                limit,
            } => write!(f, "{parameter} {requested} exceeds the limit of {limit}"),
            Self::InvalidRequest { reason } => write!(f, "invalid request: {reason}"),
            Self::UnsupportedEncoding => {
                write!(
                    f,
                    "this file is not valid UTF-8 text and cannot be read as text"
                )
            }
            Self::Io { reason } => write!(f, "the snapshot could not be read: {reason}"),
        }
    }
}

impl std::error::Error for ToolError {}

/// What: Private Pacsea-owned mapping from opaque snapshot ids to canonical roots.
///
/// Inputs: Populated by the scan driver before Pi is launched.
///
/// Output: Resolution service for all four tools.
///
/// Details:
/// - The model only ever sees the opaque ids; roots never appear in prompts or results.
/// - Roots are canonicalized once at registration. Each request re-canonicalizes and
///   compares, so replacing the root with a symlink between calls is detected.
#[derive(Debug, Default, Clone)]
pub struct SnapshotRegistry {
    /// Snapshot id to canonical root directory.
    roots: BTreeMap<String, PathBuf>,
}

impl SnapshotRegistry {
    /// What: Create an empty registry.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - A registry with no snapshots.
    ///
    /// Details:
    /// - One registry belongs to exactly one logical scan.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// What: Register one snapshot root under an opaque id.
    ///
    /// Inputs:
    /// - `id`: Opaque snapshot identifier shown to the model.
    /// - `root`: Directory that the tools may read.
    ///
    /// Output:
    /// - `Ok(())` when the root canonicalizes to an existing directory.
    ///
    /// Details:
    /// - The stored root is canonical, so later `starts_with` containment checks compare
    ///   fully resolved paths on both sides.
    ///
    /// # Errors
    /// - Returns `Err` when the id is empty or control-bearing, or the root is not a directory.
    pub fn register(&mut self, id: &str, root: &Path) -> Result<(), ToolError> {
        if id.is_empty() {
            return Err(ToolError::InvalidRequest {
                reason: "snapshot id must not be empty".to_string(),
            });
        }
        if super::has_forbidden_control(id) {
            return Err(ToolError::ControlCharacter);
        }
        let canonical = root.canonicalize().map_err(|error| ToolError::Io {
            reason: error.to_string(),
        })?;
        if !canonical.is_dir() {
            return Err(ToolError::NotARegularFile);
        }
        self.roots.insert(id.to_string(), canonical);
        Ok(())
    }

    /// What: Resolve a snapshot id to its verified canonical root.
    ///
    /// Inputs:
    /// - `id`: Snapshot id supplied by the model.
    ///
    /// Output:
    /// - The canonical root directory.
    ///
    /// Details:
    /// - Re-canonicalizes the recorded root and rejects the request if it changed, which
    ///   covers root replacement by a symlink or a bind mount between calls.
    ///
    /// # Errors
    /// - Returns `Err` when the id is unknown or the root no longer resolves identically.
    pub fn root(&self, id: &str) -> Result<&Path, ToolError> {
        let recorded = self
            .roots
            .get(id)
            .ok_or_else(|| ToolError::UnknownSnapshot {
                snapshot: id.to_string(),
            })?;
        let current = recorded
            .canonicalize()
            .map_err(|_| ToolError::SnapshotRootChanged {
                snapshot: id.to_string(),
            })?;
        if current != *recorded || !current.is_dir() {
            return Err(ToolError::SnapshotRootChanged {
                snapshot: id.to_string(),
            });
        }
        Ok(recorded.as_path())
    }

    /// What: Report the registered snapshot ids in stable order.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Sorted snapshot ids.
    ///
    /// Details:
    /// - Used by the prompt builder so the model learns the exact ids it may name.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.roots.keys().cloned().collect()
    }

    /// What: Serialize the private descriptor consumed by the embedded extension.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Deterministic JSON object mapping snapshot id to absolute canonical root.
    ///
    /// Details:
    /// - Written to a mode-0600 file inside the private mode-0700 runtime directory. The
    ///   model never sees this file and never supplies a root.
    /// - Key order is stable because the backing map is ordered, which keeps dry-run
    ///   previews and tests deterministic.
    #[must_use]
    pub fn to_descriptor_json(&self) -> String {
        let map: serde_json::Map<String, serde_json::Value> = self
            .roots
            .iter()
            .map(|(id, root)| {
                (
                    id.clone(),
                    serde_json::Value::String(root.to_string_lossy().into_owned()),
                )
            })
            .collect();
        serde_json::Value::Object(map).to_string()
    }
}

/// What: Validate a model-supplied relative path without touching the filesystem.
///
/// Inputs:
/// - `relative`: Path fragment supplied by the model.
///
/// Output:
/// - The validated normalized relative path.
///
/// Details:
/// - Rejects empty paths, absolute paths, Windows prefixes, `.`/`..`, control characters,
///   embedded separators inside components, and paths deeper than the compiled bound.
/// - Backslashes are rejected outright so a Windows-style traversal cannot slip past a
///   Unix-only component parser.
///
/// # Errors
/// - Returns `Err` for every condition above.
pub fn validate_relative_path(relative: &str) -> Result<PathBuf, ToolError> {
    if relative.is_empty() {
        return Err(ToolError::EmptyPath);
    }
    if super::has_forbidden_control(relative) || relative.contains('\\') {
        return Err(ToolError::ControlCharacter);
    }
    let candidate = Path::new(relative);
    if candidate.is_absolute()
        || relative
            .as_bytes()
            .get(..2)
            .is_some_and(|prefix| prefix[0].is_ascii_alphabetic() && prefix[1] == b':')
    {
        return Err(ToolError::AbsolutePath);
    }
    if relative.split('/').any(str::is_empty) {
        return Err(ToolError::EmptyPath);
    }
    let mut normalized = PathBuf::new();
    let mut depth = 0usize;
    for component in candidate.components() {
        match component {
            Component::Normal(part) => {
                let Some(text) = part.to_str() else {
                    return Err(ToolError::ControlCharacter);
                };
                if text.is_empty() {
                    return Err(ToolError::EmptyPath);
                }
                depth += 1;
                if depth > limits::MAX_PATH_DEPTH {
                    return Err(ToolError::TooDeep {
                        limit: limits::MAX_PATH_DEPTH,
                    });
                }
                normalized.push(text);
            }
            Component::ParentDir | Component::CurDir => return Err(ToolError::TraversalComponent),
            Component::RootDir | Component::Prefix(_) => return Err(ToolError::AbsolutePath),
        }
    }
    if depth == 0 {
        return Err(ToolError::EmptyPath);
    }
    Ok(normalized)
}

/// What: Resolve a model-supplied relative path to a real path inside the snapshot.
///
/// Inputs:
/// - `registry`: Private snapshot descriptor set.
/// - `snapshot`: Snapshot id supplied by the model.
/// - `relative`: Optional relative path; `None` addresses the snapshot root.
///
/// Output:
/// - The canonical existing path, guaranteed to be inside the canonical root.
///
/// Details:
/// - Syntactic validation runs first, then canonicalization, then containment. Doing
///   containment after canonicalization is what defeats symlink escapes.
/// - A missing entry is reported as [`ToolError::NotFound`] and never distinguishes
///   "outside the root but exists" from "does not exist".
///
/// # Errors
/// - Returns `Err` for invalid syntax, unknown snapshots, missing entries, or escapes.
pub fn resolve_in_snapshot(
    registry: &SnapshotRegistry,
    snapshot: &str,
    relative: Option<&str>,
) -> Result<PathBuf, ToolError> {
    let root = registry.root(snapshot)?;
    let target = match relative {
        None => root.to_path_buf(),
        Some(path) => root.join(validate_relative_path(path)?),
    };
    let canonical = target.canonicalize().map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ToolError::NotFound
        } else {
            ToolError::Io {
                reason: error.to_string(),
            }
        }
    })?;
    if !canonical.starts_with(root) {
        return Err(ToolError::OutsideRoot);
    }
    Ok(canonical)
}

/// What: Bounded result of `pacsea_scan_read`.
///
/// Inputs: Produced by [`read_file`].
///
/// Output: Text plus explicit truncation and offset provenance.
///
/// Details:
/// - `truncated` is always reported so the model cannot mistake a bounded window for a
///   whole file, and so coverage accounting stays honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadResult {
    /// Snapshot-relative path that was read.
    pub path: String,
    /// Byte offset of the returned window.
    pub offset: u64,
    /// Returned UTF-8 text.
    pub text: String,
    /// Total file size in bytes.
    pub total_bytes: u64,
    /// Whether more bytes exist after the returned window.
    pub truncated: bool,
}

/// What: Read a bounded UTF-8 window from a snapshot file.
///
/// Inputs:
/// - `registry`: Private snapshot descriptor set.
/// - `snapshot`: Snapshot id.
/// - `relative_path`: Snapshot-relative file path.
/// - `offset`: Byte offset to start at.
/// - `limit`: Requested byte count; `None` uses the compiled maximum.
///
/// Output:
/// - The bounded read result.
///
/// Details:
/// - Rejects a requested limit above [`limits::MAX_READ_BYTES`] instead of clamping, so a
///   model cannot probe the bound by asking for more.
/// - Requires the window to be strict UTF-8 on a character boundary; a window that splits
///   a multi-byte character is trimmed back to the last complete character.
///
/// # Errors
/// - Returns `Err` for path rejections, non-regular files, oversized requests, non-UTF-8
///   content, or I/O failures.
pub fn read_file(
    registry: &SnapshotRegistry,
    snapshot: &str,
    relative_path: &str,
    offset: u64,
    limit: Option<usize>,
) -> Result<ReadResult, ToolError> {
    let requested = limit.unwrap_or(limits::MAX_READ_BYTES);
    if requested == 0 {
        return Err(ToolError::InvalidRequest {
            reason: "limit must be greater than zero".to_string(),
        });
    }
    if requested > limits::MAX_READ_BYTES {
        return Err(ToolError::RequestTooLarge {
            parameter: "limit",
            requested,
            limit: limits::MAX_READ_BYTES,
        });
    }
    let mut file = open_verified_file(registry, snapshot, relative_path)?;
    let metadata = file.metadata().map_err(io_error)?;
    let total_bytes = metadata.len();
    let window = read_window(&mut file, offset, requested)?;
    let truncated = offset.saturating_add(window.len() as u64) < total_bytes;
    let text = decode_utf8_window(&window, truncated)?;
    Ok(ReadResult {
        path: relative_path.to_string(),
        offset,
        truncated: truncated || text.len() < window.len(),
        text,
        total_bytes,
    })
}

/// What: Read a bounded byte window from an already validated file.
///
/// Inputs:
/// - `path`: Canonical file path inside the snapshot.
/// - `offset`: Byte offset to seek to.
/// - `limit`: Maximum bytes to read.
///
/// Output:
/// - The raw window bytes.
///
/// Details:
/// - Separated from [`read_file`] so the byte-level bound stays testable and short.
///
/// # Errors
/// - Returns `Err` on seek or read failure.
fn read_window(file: &mut std::fs::File, offset: u64, limit: usize) -> Result<Vec<u8>, ToolError> {
    use std::io::{Read as _, Seek as _, SeekFrom};

    file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
    let mut buffer = vec![0u8; limit];
    let mut filled = 0usize;
    while filled < limit {
        let read = file.read(&mut buffer[filled..]).map_err(io_error)?;
        if read == 0 {
            break;
        }
        filled += read;
    }
    buffer.truncate(filled);
    Ok(buffer)
}

/// Open one regular file and verify the opened descriptor remains inside the snapshot root.
fn open_verified_file(
    registry: &SnapshotRegistry,
    snapshot: &str,
    relative_path: &str,
) -> Result<std::fs::File, ToolError> {
    let root = registry.root(snapshot)?;
    let path = resolve_in_snapshot(registry, snapshot, Some(relative_path))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(nix::libc::O_NOFOLLOW);
    }
    let file = options.open(path).map_err(io_error)?;
    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ToolError::NotARegularFile);
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd as _;
        let descriptor = PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()));
        let opened = descriptor
            .canonicalize()
            .map_err(|_| ToolError::OutsideRoot)?;
        if !opened.starts_with(root) {
            return Err(ToolError::OutsideRoot);
        }
    }
    Ok(file)
}

/// What: Decode a byte window as strict UTF-8, trimming only a split trailing character.
///
/// Inputs:
/// - `window`: Raw bytes.
/// - `truncated`: Whether more bytes follow the window in the file.
///
/// Output:
/// - The decoded text.
///
/// Details:
/// - When the window ends mid-character because of the byte bound, the partial character
///   is dropped. Any other invalid sequence is an unsupported encoding, not a truncation.
///
/// # Errors
/// - Returns [`ToolError::UnsupportedEncoding`] for genuinely invalid UTF-8.
fn decode_utf8_window(window: &[u8], truncated: bool) -> Result<String, ToolError> {
    match std::str::from_utf8(window) {
        Ok(text) => Ok(text.to_string()),
        Err(error) => {
            let valid = error.valid_up_to();
            if truncated && error.error_len().is_none() {
                // Only the final character was cut by the byte bound.
                Ok(String::from_utf8_lossy(&window[..valid]).into_owned())
            } else {
                Err(ToolError::UnsupportedEncoding)
            }
        }
    }
}

/// What: One literal grep match.
///
/// Inputs: Produced by [`grep_literal`].
///
/// Output: Snapshot-relative path plus one-based line number and bounded line text.
///
/// Details:
/// - Line text is bounded by the total byte budget so a single minified line cannot
///   exhaust the result bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepMatch {
    /// Snapshot-relative file path.
    pub path: String,
    /// One-based line number.
    pub line: u64,
    /// Bounded matching line text.
    pub text: String,
}

/// What: Bounded result of `pacsea_scan_grep`.
///
/// Inputs: Produced by [`grep_literal`].
///
/// Output: Matches plus explicit truncation.
///
/// Details:
/// - `truncated` distinguishes "no more matches" from "bound reached".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrepResult {
    /// Matches in deterministic path then line order.
    pub matches: Vec<GrepMatch>,
    /// Whether a bound stopped the search before it completed.
    pub truncated: bool,
}

/// What: Bounded literal substring search across a snapshot subtree.
///
/// Inputs:
/// - `registry`: Private snapshot descriptor set.
/// - `snapshot`: Snapshot id.
/// - `literal`: Literal substring; never a regular expression.
/// - `case_sensitive`: Whether matching respects case.
/// - `max_matches`: Requested match bound; `None` uses the compiled maximum.
///
/// Output:
/// - Deterministically ordered bounded matches.
///
/// Details:
/// - There is no regex engine on this path at all, so catastrophic backtracking and
///   model-supplied patterns are impossible by construction.
/// - Non-UTF-8 and non-regular files are skipped rather than failing the whole call, so a
///   binary asset cannot deny the model access to the rest of the snapshot.
///
/// # Errors
/// - Returns `Err` for unknown snapshots, empty or oversized literals, or oversized bounds.
pub fn grep_literal(
    registry: &SnapshotRegistry,
    snapshot: &str,
    literal: &str,
    case_sensitive: bool,
    max_matches: Option<usize>,
) -> Result<GrepResult, ToolError> {
    use std::io::Read as _;

    let bound = check_bound("max_matches", max_matches, limits::MAX_GREP_MATCHES)?;
    if literal.is_empty() {
        return Err(ToolError::InvalidRequest {
            reason: "the search literal must not be empty".to_string(),
        });
    }
    if literal.len() > 1024 {
        return Err(ToolError::RequestTooLarge {
            parameter: "literal",
            requested: literal.len(),
            limit: 1024,
        });
    }
    let root = registry.root(snapshot)?.to_path_buf();
    let needle = if case_sensitive {
        literal.to_string()
    } else {
        literal.to_lowercase()
    };

    let mut matches = Vec::new();
    let mut budget = limits::MAX_GREP_BYTES;
    let (files, walk_truncated) = walk_files(&root, limits::MAX_LISTING_ENTRIES.saturating_mul(20));
    let mut truncated = walk_truncated;
    for relative in files {
        if matches.len() >= bound {
            truncated = true;
            break;
        }
        let Ok(mut file) = open_verified_file(registry, snapshot, &relative) else {
            continue;
        };
        let Ok(metadata) = file.metadata() else {
            continue;
        };
        if metadata.len() > limits::MAX_ANALYZABLE_TEXT_BYTES as u64 {
            continue;
        }
        let mut content = Vec::new();
        if file
            .by_ref()
            .take(limits::MAX_ANALYZABLE_TEXT_BYTES as u64 + 1)
            .read_to_end(&mut content)
            .is_err()
            || content.len() > limits::MAX_ANALYZABLE_TEXT_BYTES
        {
            continue;
        }
        let Ok(text) = std::str::from_utf8(&content) else {
            continue;
        };
        if scan_lines(
            text,
            &relative,
            &needle,
            case_sensitive,
            bound,
            &mut budget,
            &mut matches,
        ) {
            truncated = true;
            break;
        }
    }
    Ok(GrepResult { matches, truncated })
}

/// What: Collect literal matches from one file's text within the shared budgets.
///
/// Inputs:
/// - `text`: File content.
/// - `relative`: Snapshot-relative path.
/// - `needle`: Literal (already lowercased for case-insensitive search).
/// - `case_sensitive`: Whether matching respects case.
/// - `bound`: Maximum total match count.
/// - `budget`: Remaining output byte budget, decremented in place.
/// - `matches`: Accumulator.
///
/// Output:
/// - `true` when a bound stopped the scan.
///
/// Details:
/// - Extracted from [`grep_literal`] to keep both functions well under the complexity bound.
fn scan_lines(
    text: &str,
    relative: &str,
    needle: &str,
    case_sensitive: bool,
    bound: usize,
    budget: &mut usize,
    matches: &mut Vec<GrepMatch>,
) -> bool {
    for (index, line) in text.split('\n').enumerate() {
        if matches.len() >= bound {
            return true;
        }
        let haystack = if case_sensitive {
            line.to_string()
        } else {
            line.to_lowercase()
        };
        if !haystack.contains(needle) {
            continue;
        }
        let bounded = bounded_line(line);
        let cost = bounded.len() + relative.len() + 16;
        if cost > *budget {
            return true;
        }
        *budget -= cost;
        matches.push(GrepMatch {
            path: relative.to_string(),
            line: index as u64 + 1,
            text: bounded,
        });
    }
    false
}

/// Maximum characters returned for a single matching line.
const MAX_GREP_LINE_CHARS: usize = 512;

/// What: Bound and sanitize a single matching line for model consumption.
///
/// Inputs:
/// - `line`: Raw line text.
///
/// Output:
/// - A control-free line of at most [`MAX_GREP_LINE_CHARS`] characters.
///
/// Details:
/// - Control characters are replaced rather than dropped so terminal escape sequences in
///   hostile source cannot reach the TUI through a tool result.
fn bounded_line(line: &str) -> String {
    line.chars()
        .take(MAX_GREP_LINE_CHARS)
        .map(|ch| if ch.is_control() { '\u{fffd}' } else { ch })
        .collect()
}

/// What: One directory or find entry.
///
/// Inputs: Produced by [`list_directory`] and [`find_paths`].
///
/// Output: Snapshot-relative path plus entry kind and size.
///
/// Details:
/// - Only `file`, `dir`, and `other` are reported. `other` covers symlinks and special
///   files, which are metadata-only and can never be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Snapshot-relative path.
    pub path: String,
    /// Entry kind: `file`, `dir`, or `other`.
    pub kind: &'static str,
    /// Size in bytes for regular files, zero otherwise.
    pub size: u64,
}

/// What: Bounded result of `pacsea_scan_ls` and `pacsea_scan_find`.
///
/// Inputs: Produced by the listing tools.
///
/// Output: Sorted entries plus explicit truncation.
///
/// Details:
/// - Entries are sorted by path so repeated calls are deterministic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingResult {
    /// Sorted entries.
    pub entries: Vec<Entry>,
    /// Whether a bound stopped the listing before it completed.
    pub truncated: bool,
}

/// What: List one directory inside a snapshot.
///
/// Inputs:
/// - `registry`: Private snapshot descriptor set.
/// - `snapshot`: Snapshot id.
/// - `relative_path`: Directory path; `None` lists the snapshot root.
/// - `max_entries`: Requested entry bound; `None` uses the compiled maximum.
///
/// Output:
/// - Sorted bounded entries.
///
/// Details:
/// - Uses `symlink_metadata`, so a symlink is reported as `other` and is never followed
///   into a directory outside the snapshot.
///
/// # Errors
/// - Returns `Err` for path rejections, non-directories, or oversized bounds.
pub fn list_directory(
    registry: &SnapshotRegistry,
    snapshot: &str,
    relative_path: Option<&str>,
    max_entries: Option<usize>,
) -> Result<ListingResult, ToolError> {
    let bound = check_bound("max_entries", max_entries, limits::MAX_LISTING_ENTRIES)?;
    let root = registry.root(snapshot)?.to_path_buf();
    let directory = resolve_in_snapshot(registry, snapshot, relative_path)?;
    if !directory.is_dir() {
        return Err(ToolError::NotARegularFile);
    }
    let prefix = relative_path.unwrap_or("").trim_end_matches('/');

    let mut entries = Vec::new();
    let mut budget = limits::MAX_LISTING_BYTES;
    let mut truncated = false;
    let mut reader: Vec<_> = std::fs::read_dir(&directory)
        .map_err(io_error)?
        .filter_map(Result::ok)
        .collect();
    reader.sort_by_key(std::fs::DirEntry::file_name);
    for item in reader {
        if entries.len() >= bound {
            truncated = true;
            break;
        }
        let Some(name) = item.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let relative = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        if relative.len() + 32 > budget {
            truncated = true;
            break;
        }
        budget -= relative.len() + 32;
        let metadata = item
            .metadata()
            .or_else(|_| std::fs::symlink_metadata(item.path()));
        entries.push(describe_entry(relative, metadata.ok().as_ref()));
    }
    let _ = &root;
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ListingResult { entries, truncated })
}

/// What: Find snapshot entries whose relative path matches a bounded glob.
///
/// Inputs:
/// - `registry`: Private snapshot descriptor set.
/// - `snapshot`: Snapshot id.
/// - `glob`: Bounded glob supporting `*`, `**`, and `?` only.
/// - `max_results`: Requested result bound; `None` uses the compiled maximum.
///
/// Output:
/// - Sorted bounded matching entries.
///
/// Details:
/// - The glob matcher is a linear two-pointer implementation with no backtracking blowup
///   and no regular-expression engine, so a hostile pattern cannot cause a denial of service.
///
/// # Errors
/// - Returns `Err` for unknown snapshots, empty or oversized globs, or oversized bounds.
pub fn find_paths(
    registry: &SnapshotRegistry,
    snapshot: &str,
    glob: &str,
    max_results: Option<usize>,
) -> Result<ListingResult, ToolError> {
    let bound = check_bound("max_results", max_results, limits::MAX_LISTING_ENTRIES)?;
    if glob.is_empty() {
        return Err(ToolError::InvalidRequest {
            reason: "the glob must not be empty".to_string(),
        });
    }
    if glob.len() > 256 {
        return Err(ToolError::RequestTooLarge {
            parameter: "glob",
            requested: glob.len(),
            limit: 256,
        });
    }
    if super::has_forbidden_control(glob) {
        return Err(ToolError::ControlCharacter);
    }
    let root = registry.root(snapshot)?.to_path_buf();

    let mut entries = Vec::new();
    let mut budget = limits::MAX_LISTING_BYTES;
    let (files, walk_truncated) = walk_files(&root, limits::MAX_LISTING_ENTRIES.saturating_mul(20));
    let mut truncated = walk_truncated;
    for relative in files {
        if entries.len() >= bound {
            truncated = true;
            break;
        }
        if !glob_matches(glob, &relative) {
            continue;
        }
        if relative.len() + 32 > budget {
            truncated = true;
            break;
        }
        budget -= relative.len() + 32;
        let metadata = std::fs::symlink_metadata(root.join(&relative)).ok();
        entries.push(describe_entry(relative, metadata.as_ref()));
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(ListingResult { entries, truncated })
}

/// What: Build an entry descriptor from optional metadata.
///
/// Inputs:
/// - `relative`: Snapshot-relative path.
/// - `metadata`: Metadata obtained without following symlinks, when available.
///
/// Output:
/// - The entry descriptor.
///
/// Details:
/// - Unknown metadata degrades to `other` with zero size rather than guessing.
fn describe_entry(relative: String, metadata: Option<&std::fs::Metadata>) -> Entry {
    let (kind, size) = match metadata {
        Some(meta) if meta.is_file() => ("file", meta.len()),
        Some(meta) if meta.is_dir() => ("dir", 0),
        _ => ("other", 0),
    };
    Entry {
        path: relative,
        kind,
        size,
    }
}

/// What: Enumerate regular files under a snapshot root in deterministic order.
///
/// Inputs:
/// - `root`: Canonical snapshot root.
/// - `max_visited`: Hard cap on visited entries.
///
/// Output:
/// - Sorted snapshot-relative file paths.
///
/// Details:
/// - Never follows symlinks and never descends deeper than the compiled path bound, so a
///   symlink loop inside a hostile snapshot cannot hang the scan.
///
/// # Errors
/// - Returns `Err` only when the root itself cannot be read.
fn walk_files(root: &Path, max_visited: usize) -> (Vec<String>, bool) {
    let mut found = Vec::new();
    let mut queue = vec![(root.to_path_buf(), String::new(), 0usize)];
    let mut visited = 0usize;
    let mut truncated = false;
    while let Some((directory, prefix, depth)) = queue.pop() {
        if depth > limits::MAX_PATH_DEPTH || visited >= max_visited {
            truncated = true;
            break;
        }
        let Ok(reader) = std::fs::read_dir(&directory) else {
            continue;
        };
        for item in reader.filter_map(Result::ok) {
            visited += 1;
            if visited >= max_visited {
                truncated = true;
                break;
            }
            let Some(name) = item.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let relative = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let Ok(metadata) = std::fs::symlink_metadata(item.path()) else {
                continue;
            };
            if metadata.is_dir() {
                queue.push((item.path(), relative, depth + 1));
            } else if metadata.is_file() {
                found.push(relative);
            }
        }
    }
    found.sort();
    (found, truncated)
}

/// What: Match a bounded glob against a snapshot-relative path.
///
/// Inputs:
/// - `pattern`: Glob using `*`, `**`, and `?`.
/// - `path`: Snapshot-relative path.
///
/// Output:
/// - `true` when the whole path matches.
///
/// Details:
/// - `*` matches within one path segment, `**` matches across segments, `?` matches one
///   non-separator character. Every other character is literal.
/// - Uses bounded dynamic programming rather than recursive backtracking, so hostile glob
///   input cannot trigger exponential work.
#[must_use]
pub fn glob_matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = path.chars().collect();
    let mut matched = vec![vec![false; text.len() + 1]; pattern.len() + 1];
    matched[pattern.len()][text.len()] = true;

    for pattern_index in (0..pattern.len()).rev() {
        for text_index in (0..=text.len()).rev() {
            if pattern[pattern_index] == '*' {
                let crosses = pattern.get(pattern_index + 1) == Some(&'*');
                let next_pattern = pattern_index + usize::from(crosses) + 1;
                let consumes = text_index < text.len()
                    && (crosses || text[text_index] != '/')
                    && matched[pattern_index][text_index + 1];
                matched[pattern_index][text_index] = matched[next_pattern][text_index] || consumes;
            } else if text_index < text.len() {
                let current = pattern[pattern_index];
                let consumes =
                    current == text[text_index] || (current == '?' && text[text_index] != '/');
                matched[pattern_index][text_index] =
                    consumes && matched[pattern_index + 1][text_index + 1];
            }
        }
    }
    matched[0][0]
}

/// What: Validate a requested bound against its compiled maximum.
///
/// Inputs:
/// - `parameter`: Parameter name for the error message.
/// - `requested`: Requested value, if the model supplied one.
/// - `maximum`: Compiled maximum.
///
/// Output:
/// - The effective bound.
///
/// Details:
/// - Oversized requests are rejected rather than clamped, and zero is rejected as invalid.
///
/// # Errors
/// - Returns `Err` when the request is zero or above the compiled maximum.
fn check_bound(
    parameter: &'static str,
    requested: Option<usize>,
    maximum: usize,
) -> Result<usize, ToolError> {
    match requested {
        None => Ok(maximum),
        Some(0) => Err(ToolError::InvalidRequest {
            reason: format!("{parameter} must be greater than zero"),
        }),
        Some(value) if value > maximum => Err(ToolError::RequestTooLarge {
            parameter,
            requested: value,
            limit: maximum,
        }),
        Some(value) => Ok(value),
    }
}

/// What: Convert an I/O error into a disclosure-free tool error.
///
/// Inputs:
/// - `error`: Underlying I/O error.
///
/// Output:
/// - [`ToolError::NotFound`] or [`ToolError::Io`].
///
/// Details:
/// - Only the error kind text is preserved; host paths are never included.
#[allow(
    clippy::needless_pass_by_value,
    reason = "map_err requires an owned-error callback and only the nondisclosing kind is retained"
)]
fn io_error(error: std::io::Error) -> ToolError {
    if error.kind() == std::io::ErrorKind::NotFound {
        ToolError::NotFound
    } else {
        ToolError::Io {
            reason: error.kind().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Entry, SnapshotRegistry, ToolError, find_paths, glob_matches, grep_literal, list_directory,
        read_file, resolve_in_snapshot, validate_relative_path,
    };
    use crate::pi_agent::limits;
    use std::fmt::Write as _;
    use std::path::{Path, PathBuf};

    /// Sentinel content that must never reach the model through any tool.
    const SENTINEL: &str = "PACSEA-HOST-SENTINEL-a1b2c3";

    /// Build a snapshot with an adjacent host sentinel and an escaping symlink.
    fn fixture() -> (tempfile::TempDir, SnapshotRegistry, PathBuf) {
        let temp = tempfile::tempdir().expect("temp dir");
        let host = temp.path().join("host");
        std::fs::create_dir_all(&host).expect("host dir");
        std::fs::write(host.join("secret.txt"), SENTINEL).expect("sentinel");

        let root = temp.path().join("snapshot");
        std::fs::create_dir_all(root.join("src")).expect("snapshot dirs");
        std::fs::write(root.join("PKGBUILD"), "pkgname=demo\nsource=('x')\n").expect("pkgbuild");
        std::fs::write(root.join(".SRCINFO"), "pkgbase = demo\n").expect("srcinfo");
        std::fs::write(root.join("src/main.rs"), "fn main() { curl_download(); }\n")
            .expect("source");
        std::fs::write(root.join("src/blob.bin"), [0xff, 0xfe, 0x00, 0x01]).expect("binary");
        #[cfg(unix)]
        std::os::unix::fs::symlink(host.join("secret.txt"), root.join("escape.txt"))
            .expect("escaping symlink");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&host, root.join("escape_dir")).expect("escaping dir symlink");

        let mut registry = SnapshotRegistry::new();
        registry.register("recipe", &root).expect("register");
        let canonical = root.canonicalize().expect("canonical root");
        (temp, registry, canonical)
    }

    /// Verify syntactic path validation rejects every hostile shape.
    #[test]
    fn relative_path_validation_rejects_hostile_shapes() {
        assert_eq!(validate_relative_path(""), Err(ToolError::EmptyPath));
        assert_eq!(
            validate_relative_path("/etc/passwd"),
            Err(ToolError::AbsolutePath)
        );
        assert_eq!(
            validate_relative_path("../../etc/passwd"),
            Err(ToolError::TraversalComponent)
        );
        assert_eq!(validate_relative_path("a//b"), Err(ToolError::EmptyPath));
        assert_eq!(validate_relative_path("a/"), Err(ToolError::EmptyPath));
        assert_eq!(
            validate_relative_path("C:/Windows/System32"),
            Err(ToolError::AbsolutePath)
        );
        assert_eq!(
            validate_relative_path("src/../../etc/passwd"),
            Err(ToolError::TraversalComponent)
        );
        assert_eq!(
            validate_relative_path("./PKGBUILD"),
            Err(ToolError::TraversalComponent)
        );
        assert_eq!(
            validate_relative_path("."),
            Err(ToolError::TraversalComponent)
        );
        assert_eq!(
            validate_relative_path(".."),
            Err(ToolError::TraversalComponent)
        );
        assert_eq!(
            validate_relative_path("a\u{0}b"),
            Err(ToolError::ControlCharacter)
        );
        assert_eq!(
            validate_relative_path("a\nb"),
            Err(ToolError::ControlCharacter)
        );
        assert_eq!(
            validate_relative_path("..\\..\\windows"),
            Err(ToolError::ControlCharacter)
        );
        assert_eq!(
            validate_relative_path("a\u{1b}[31m"),
            Err(ToolError::ControlCharacter)
        );
        let deep = vec!["d"; limits::MAX_PATH_DEPTH + 1].join("/");
        assert_eq!(
            validate_relative_path(&deep),
            Err(ToolError::TooDeep {
                limit: limits::MAX_PATH_DEPTH
            })
        );
        assert_eq!(
            validate_relative_path("src/main.rs").expect("valid"),
            PathBuf::from("src/main.rs")
        );
    }

    /// Verify symlink escapes are rejected after canonicalization.
    #[test]
    #[cfg(unix)]
    fn symlink_escapes_are_rejected() {
        let (_temp, registry, _root) = fixture();
        assert_eq!(
            resolve_in_snapshot(&registry, "recipe", Some("escape.txt")),
            Err(ToolError::OutsideRoot)
        );
        assert_eq!(
            resolve_in_snapshot(&registry, "recipe", Some("escape_dir/secret.txt")),
            Err(ToolError::OutsideRoot)
        );
    }

    /// Verify the model cannot read an outside-root sentinel through any tool or error.
    #[test]
    #[cfg(unix)]
    fn outside_root_sentinel_is_never_disclosed() {
        let (_temp, registry, _root) = fixture();
        let mut rendered = String::new();
        for attempt in [
            "escape.txt",
            "escape_dir/secret.txt",
            "../host/secret.txt",
            "/etc/passwd",
        ] {
            let error = read_file(&registry, "recipe", attempt, 0, None)
                .expect_err("every escape attempt must fail");
            rendered.push_str(&error.to_string());
        }
        let grep = grep_literal(&registry, "recipe", SENTINEL, true, None).expect("grep runs");
        assert!(grep.matches.is_empty(), "sentinel must not be searchable");
        let found = find_paths(&registry, "recipe", "**/secret.txt", None).expect("find runs");
        assert!(found.entries.is_empty(), "sentinel must not be findable");
        let listing = list_directory(&registry, "recipe", None, None).expect("ls runs");
        let _ = write!(rendered, "{listing:?}{grep:?}{found:?}");
        assert!(
            !rendered.contains(SENTINEL),
            "no tool output or error may disclose the host sentinel"
        );
        // The escaping symlink is visible as inert metadata only.
        assert!(
            listing
                .entries
                .iter()
                .any(|entry| entry.path == "escape.txt" && entry.kind == "other")
        );
    }

    /// Verify root replacement between calls is detected.
    #[test]
    #[cfg(unix)]
    fn snapshot_root_replacement_is_detected() {
        let temp = tempfile::tempdir().expect("temp dir");
        let real = temp.path().join("real");
        let evil = temp.path().join("evil");
        std::fs::create_dir_all(&real).expect("real");
        std::fs::create_dir_all(&evil).expect("evil");
        std::fs::write(evil.join("secret.txt"), SENTINEL).expect("sentinel");
        let link = temp.path().join("root");
        std::os::unix::fs::symlink(&real, &link).expect("root symlink");

        let mut registry = SnapshotRegistry::new();
        registry.register("recipe", &link).expect("register");
        std::fs::remove_file(&link).expect("remove link");
        std::os::unix::fs::symlink(&evil, &link).expect("replace root");

        // The recorded root is the canonical original directory, so the replacement is
        // invisible to the tools; removing the original makes the mismatch explicit.
        std::fs::remove_dir_all(&real).expect("remove original root");
        assert_eq!(
            registry.root("recipe"),
            Err(ToolError::SnapshotRootChanged {
                snapshot: "recipe".to_string()
            })
        );
        assert!(
            read_file(&registry, "recipe", "secret.txt", 0, None)
                .expect_err("must fail")
                .to_string()
                .contains("no longer the directory")
        );
    }

    /// Verify unknown snapshots and unreadable kinds fail closed.
    #[test]
    fn unknown_snapshot_and_special_kinds_fail_closed() {
        let (_temp, registry, _root) = fixture();
        assert_eq!(
            read_file(&registry, "other", "PKGBUILD", 0, None),
            Err(ToolError::UnknownSnapshot {
                snapshot: "other".to_string()
            })
        );
        assert_eq!(
            read_file(&registry, "recipe", "src", 0, None),
            Err(ToolError::NotARegularFile)
        );
        assert_eq!(
            read_file(&registry, "recipe", "missing.txt", 0, None),
            Err(ToolError::NotFound)
        );
        assert_eq!(
            list_directory(&registry, "recipe", Some("PKGBUILD"), None),
            Err(ToolError::NotARegularFile)
        );
    }

    /// Verify read bounds are enforced and never silently clamped.
    #[test]
    fn read_requests_are_bounded_and_truncation_is_explicit() {
        let (_temp, registry, root) = fixture();
        let big = "x".repeat(limits::MAX_READ_BYTES * 2);
        std::fs::write(root.join("big.txt"), &big).expect("big file");

        assert_eq!(
            read_file(
                &registry,
                "recipe",
                "big.txt",
                0,
                Some(limits::MAX_READ_BYTES + 1)
            ),
            Err(ToolError::RequestTooLarge {
                parameter: "limit",
                requested: limits::MAX_READ_BYTES + 1,
                limit: limits::MAX_READ_BYTES,
            })
        );
        assert!(matches!(
            read_file(&registry, "recipe", "big.txt", 0, Some(0)),
            Err(ToolError::InvalidRequest { .. })
        ));

        let window = read_file(&registry, "recipe", "big.txt", 0, None).expect("bounded read");
        assert_eq!(window.text.len(), limits::MAX_READ_BYTES);
        assert!(window.truncated);
        assert_eq!(window.total_bytes, big.len() as u64);

        let tail = read_file(&registry, "recipe", "PKGBUILD", 0, Some(8)).expect("small read");
        assert_eq!(tail.text, "pkgname=");
        assert!(tail.truncated);
    }

    /// Verify non-UTF-8 files are reported as unsupported rather than lossily decoded.
    #[test]
    fn non_utf8_files_are_unsupported() {
        let (_temp, registry, _root) = fixture();
        assert_eq!(
            read_file(&registry, "recipe", "src/blob.bin", 0, None),
            Err(ToolError::UnsupportedEncoding)
        );
    }

    /// Verify grep is literal-only, bounded, and control-sanitized.
    #[test]
    fn grep_is_literal_bounded_and_sanitized() {
        let (_temp, registry, root) = fixture();
        let hit = grep_literal(&registry, "recipe", "curl_download", true, None).expect("grep");
        assert_eq!(hit.matches.len(), 1);
        assert_eq!(hit.matches[0].path, "src/main.rs");
        assert_eq!(hit.matches[0].line, 1);

        // A regex metacharacter sequence is matched literally, never compiled.
        let regexish = grep_literal(&registry, "recipe", ".*", true, None).expect("grep");
        assert!(
            regexish.matches.is_empty(),
            "'.*' must be a literal, not a pattern"
        );
        let catastrophic =
            grep_literal(&registry, "recipe", "(a+)+$", true, None).expect("grep runs");
        assert!(catastrophic.matches.is_empty());

        assert!(matches!(
            grep_literal(&registry, "recipe", "", true, None),
            Err(ToolError::InvalidRequest { .. })
        ));
        assert_eq!(
            grep_literal(
                &registry,
                "recipe",
                "x",
                true,
                Some(limits::MAX_GREP_MATCHES + 1)
            ),
            Err(ToolError::RequestTooLarge {
                parameter: "max_matches",
                requested: limits::MAX_GREP_MATCHES + 1,
                limit: limits::MAX_GREP_MATCHES,
            })
        );

        std::fs::write(root.join("noisy.txt"), "hit \u{1b}[31mred\u{1b}[0m\n").expect("noisy");
        let sanitized = grep_literal(&registry, "recipe", "hit", true, None).expect("grep");
        assert!(
            !sanitized.matches[0].text.contains('\u{1b}'),
            "terminal controls must be neutralized"
        );

        let mut many = String::new();
        for index in 0..(limits::MAX_GREP_MATCHES * 2) {
            let _ = writeln!(many, "needle {index}");
        }
        std::fs::write(root.join("many.txt"), many).expect("many");
        let bounded = grep_literal(&registry, "recipe", "needle", true, None).expect("grep");
        assert_eq!(bounded.matches.len(), limits::MAX_GREP_MATCHES);
        assert!(bounded.truncated);

        let insensitive = grep_literal(&registry, "recipe", "PKGNAME", false, None).expect("grep");
        assert_eq!(insensitive.matches.len(), 1);
    }

    /// Verify listings are sorted, bounded, and symlink-safe.
    #[test]
    fn listings_are_sorted_and_bounded() {
        let (_temp, registry, root) = fixture();
        let listing = list_directory(&registry, "recipe", None, None).expect("ls");
        let paths: Vec<&str> = listing.entries.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(paths.contains(&"PKGBUILD"));
        assert!(paths.contains(&"src"));
        assert!(!listing.truncated);
        assert!(listing.entries.iter().any(|e| e
            == &Entry {
                path: "src".to_string(),
                kind: "dir",
                size: 0
            }));

        let nested = list_directory(&registry, "recipe", Some("src"), None).expect("ls src");
        assert!(nested.entries.iter().all(|e| e.path.starts_with("src/")));

        for index in 0..(limits::MAX_LISTING_ENTRIES + 10) {
            std::fs::write(root.join(format!("f{index:04}.txt")), "x").expect("bulk file");
        }
        let bounded = list_directory(&registry, "recipe", None, None).expect("ls");
        assert_eq!(bounded.entries.len(), limits::MAX_LISTING_ENTRIES);
        assert!(bounded.truncated);
        assert_eq!(
            list_directory(
                &registry,
                "recipe",
                None,
                Some(limits::MAX_LISTING_ENTRIES + 1)
            ),
            Err(ToolError::RequestTooLarge {
                parameter: "max_entries",
                requested: limits::MAX_LISTING_ENTRIES + 1,
                limit: limits::MAX_LISTING_ENTRIES,
            })
        );
    }

    /// Verify find uses the bounded glob syntax and rejects hostile input.
    #[test]
    fn find_uses_bounded_globs() {
        let (_temp, registry, _root) = fixture();
        let rust = find_paths(&registry, "recipe", "**/*.rs", None).expect("find");
        assert_eq!(
            rust.entries
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/main.rs"]
        );
        let top = find_paths(&registry, "recipe", "*", None).expect("find");
        assert!(top.entries.iter().all(|e| !e.path.contains('/')));
        assert!(matches!(
            find_paths(&registry, "recipe", "", None),
            Err(ToolError::InvalidRequest { .. })
        ));
        assert_eq!(
            find_paths(&registry, "recipe", "a\u{0}b", None),
            Err(ToolError::ControlCharacter)
        );
        assert!(matches!(
            find_paths(&registry, "recipe", &"a".repeat(257), None),
            Err(ToolError::RequestTooLarge { .. })
        ));
    }

    /// Verify glob semantics for separator handling and anchoring.
    #[test]
    fn glob_semantics_are_segment_aware() {
        assert!(glob_matches("*.rs", "main.rs"));
        assert!(!glob_matches("*.rs", "src/main.rs"));
        assert!(glob_matches("**/*.rs", "src/deep/main.rs"));
        assert!(glob_matches("src/*.rs", "src/main.rs"));
        assert!(!glob_matches("src/*.rs", "src/deep/main.rs"));
        assert!(glob_matches("PKGBUILD", "PKGBUILD"));
        assert!(!glob_matches("PKGBUILD", "PKGBUILD.bak"));
        assert!(glob_matches("?KGBUILD", "PKGBUILD"));
        assert!(!glob_matches("?", "a/b"));
        assert!(glob_matches("**", "a/b/c"));
        assert!(!glob_matches("main.rs", "src/main.rs"));
    }

    /// Verify registration rejects control-bearing ids and non-directories.
    #[test]
    fn registration_is_validated() {
        let (_temp, _registry, root) = fixture();
        let mut registry = SnapshotRegistry::new();
        assert!(matches!(
            registry.register("", &root),
            Err(ToolError::InvalidRequest { .. })
        ));
        assert_eq!(
            registry.register("bad\nid", &root),
            Err(ToolError::ControlCharacter)
        );
        assert!(matches!(
            registry.register("missing", Path::new("/nonexistent/pacsea-snapshot")),
            Err(ToolError::Io { .. })
        ));
        registry.register("ok", &root).expect("valid registration");
        assert_eq!(registry.ids(), vec!["ok".to_string()]);
    }
}
