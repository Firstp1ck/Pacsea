//! Strict, non-executing `.SRCINFO` parsing and recipe/source metadata binding.

use crate::logic::pi_scan::identity::{PackageBase, PackageName};
use crate::logic::pi_scan::source::ChecksumAlgorithm;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Maximum accepted `.SRCINFO` byte length.
pub const MAX_SRCINFO_BYTES: usize = 1024 * 1024;
/// Maximum accepted `.SRCINFO` line count.
pub const MAX_SRCINFO_LINES: usize = 20_000;

/// What: A checksum declaration bound to one source entry.
///
/// Inputs:
/// - Algorithm and exact `.SRCINFO` value.
///
/// Output:
/// - Immutable checksum metadata for integrity policy evaluation.
///
/// Details:
/// - Values are not interpreted while parsing, except that empty values are rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredChecksum {
    /// Declared checksum algorithm.
    pub algorithm: ChecksumAlgorithm,
    /// Trimmed checksum text, including a possible `SKIP` marker.
    pub value: String,
}

/// What: One `.SRCINFO` source with its positional checksum and `noextract` bindings.
///
/// Inputs:
/// - Source declaration, optional architecture suffix, effective filename, and parallel metadata.
///
/// Output:
/// - Deterministic source policy input independent of PKGBUILD execution.
///
/// Details:
/// - Checksums are bound by algorithm, architecture suffix, and declaration position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeSource {
    /// Exact source locator after trimming.
    pub value: String,
    /// Optional architecture suffix from `source_<arch>`.
    pub architecture: Option<String>,
    /// Effective local filename, honoring `name::locator` syntax.
    pub effective_name: String,
    /// Checksums positionally bound to this source.
    pub checksums: Vec<DeclaredChecksum>,
    /// Whether the effective filename appears in `noextract`.
    pub no_extract: bool,
    /// Effective filename covered by this detached `.sig`/`.asc`, when paired exactly.
    pub detached_signature_for: Option<String>,
}

/// What: Strictly parsed build-relevant `.SRCINFO` metadata.
///
/// Inputs:
/// - A bounded UTF-8 `.SRCINFO` document.
///
/// Output:
/// - Validated package identity and bound source integrity metadata.
///
/// Details:
/// - Unrelated well-formed keys are ignored; malformed syntax and ambiguous relevant bindings fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrcInfo {
    /// Exactly one declared package base.
    pub package_base: PackageBase,
    /// Sorted, deduplicated package names belonging to the base.
    pub package_names: Vec<PackageName>,
    /// Sources in original declaration order.
    pub sources: Vec<RecipeSource>,
    /// Sorted, deduplicated uppercase full `OpenPGP` fingerprints.
    pub valid_pgp_keys: Vec<String>,
    /// Sorted, deduplicated `noextract` effective filenames.
    pub no_extract: Vec<String>,
}

/// What: Strict `.SRCINFO` parse or metadata-binding failure.
///
/// Inputs:
/// - Malformed document syntax or invalid build-relevant metadata.
///
/// Output:
/// - Line-aware inert error suitable for an explicit failed acquisition result.
///
/// Details:
/// - The parser never evaluates shell, variable, or PKGBUILD syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrcInfoError {
    /// One-based line number, or zero for document-level limits and omissions.
    pub line: usize,
    /// Actionable reason the document cannot be trusted.
    pub reason: String,
}

impl fmt::Display for SrcInfoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(formatter, "Invalid .SRCINFO: {}", self.reason)
        } else {
            write!(
                formatter,
                "Invalid .SRCINFO at line {}: {}",
                self.line, self.reason
            )
        }
    }
}

impl std::error::Error for SrcInfoError {}

/// Raw source declaration retained until positional arrays are validated.
#[derive(Debug)]
struct RawSource {
    /// Optional architecture suffix.
    architecture: Option<String>,
    /// Exact declaration value.
    value: String,
    /// Effective local filename.
    effective_name: String,
}

/// Mutable parser state for build-relevant fields.
#[derive(Debug, Default)]
struct ParserState {
    /// Exactly one package base candidate.
    package_base: Option<PackageBase>,
    /// Package names with duplicate detection.
    package_names: BTreeSet<PackageName>,
    /// Sources in declaration order.
    sources: Vec<RawSource>,
    /// Checksum arrays keyed by algorithm and architecture.
    checksums: BTreeMap<(ChecksumAlgorithm, Option<String>), Vec<String>>,
    /// Valid signing fingerprints.
    valid_pgp_keys: BTreeSet<String>,
    /// Effective names excluded from makepkg extraction.
    no_extract: BTreeSet<String>,
}

/// What: Parse `.SRCINFO` without running PKGBUILD, makepkg, shell, or helpers.
///
/// Inputs:
/// - `input`: UTF-8 `.SRCINFO` text bounded by compiled byte and line maxima.
///
/// Output:
/// - A strictly bound `SrcInfo` document.
///
/// Details:
/// - Relevant array keys are architecture-aware and checksum arrays must align exactly with sources.
///
/// # Errors
/// Returns `SrcInfoError` for malformed syntax, limits, invalid identity, or ambiguous arrays.
pub fn parse_srcinfo(input: &str) -> Result<SrcInfo, SrcInfoError> {
    if input.len() > MAX_SRCINFO_BYTES {
        return Err(document_error(format!(
            "document exceeds {MAX_SRCINFO_BYTES} bytes"
        )));
    }
    let mut state = ParserState::default();
    for (index, raw_line) in input.lines().enumerate() {
        if index >= MAX_SRCINFO_LINES {
            return Err(document_error(format!(
                "document exceeds {MAX_SRCINFO_LINES} lines"
            )));
        }
        parse_line(&mut state, raw_line, index + 1)?;
    }
    finish_parse(state)
}

/// Parse one syntactically complete `.SRCINFO` line.
fn parse_line(state: &mut ParserState, raw_line: &str, line: usize) -> Result<(), SrcInfoError> {
    let trimmed = raw_line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return Ok(());
    }
    if raw_line.contains('\0') || raw_line.chars().any(char::is_control) {
        return Err(line_error(line, "control character is forbidden"));
    }
    let (raw_key, raw_value) = trimmed
        .split_once('=')
        .ok_or_else(|| line_error(line, "expected `key = value`"))?;
    let key = raw_key.trim();
    let value = raw_value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(line_error(line, "key and value must both be non-empty"));
    }
    if !key.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        return Err(line_error(line, "key contains unsupported characters"));
    }
    apply_field(state, key, value, line)
}

/// Bind one relevant field while leaving unrelated valid metadata inert.
fn apply_field(
    state: &mut ParserState,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), SrcInfoError> {
    match key {
        "pkgbase" => bind_package_base(state, value, line),
        "pkgname" => bind_package_name(state, value, line),
        "validpgpkeys" => bind_valid_pgp_key(state, value, line),
        "noextract" => bind_no_extract(state, value, line),
        _ => {
            if let ArrayKey::Matched(architecture) = array_architecture(key, "source")? {
                bind_source(state, architecture, value, line)
            } else if let Some((algorithm, architecture)) = checksum_key(key)? {
                state
                    .checksums
                    .entry((algorithm, architecture))
                    .or_default()
                    .push(value.to_string());
                Ok(())
            } else {
                Ok(())
            }
        }
    }
}

/// Bind the single package-base scalar.
fn bind_package_base(
    state: &mut ParserState,
    value: &str,
    line: usize,
) -> Result<(), SrcInfoError> {
    if state.package_base.is_some() {
        return Err(line_error(line, "pkgbase must be declared exactly once"));
    }
    state.package_base = Some(
        PackageBase::new(value)
            .map_err(|error| line_error(line, format!("invalid pkgbase: {error}")))?,
    );
    Ok(())
}

/// Bind and deduplicate one package name.
fn bind_package_name(
    state: &mut ParserState,
    value: &str,
    line: usize,
) -> Result<(), SrcInfoError> {
    let package_name = PackageName::new(value)
        .map_err(|error| line_error(line, format!("invalid pkgname: {error}")))?;
    if !state.package_names.insert(package_name) {
        return Err(line_error(line, "duplicate pkgname declaration"));
    }
    Ok(())
}

/// Validate and bind a full `OpenPGP` fingerprint.
fn bind_valid_pgp_key(
    state: &mut ParserState,
    value: &str,
    line: usize,
) -> Result<(), SrcInfoError> {
    if !matches!(value.len(), 40 | 64)
        || !value.chars().all(|character| character.is_ascii_hexdigit())
    {
        return Err(line_error(
            line,
            "validpgpkeys requires a full 40- or 64-hex fingerprint",
        ));
    }
    state.valid_pgp_keys.insert(value.to_ascii_uppercase());
    Ok(())
}

/// Validate and bind one `noextract` effective filename.
fn bind_no_extract(state: &mut ParserState, value: &str, line: usize) -> Result<(), SrcInfoError> {
    validate_effective_name(value, line)?;
    if !state.no_extract.insert(value.to_string()) {
        return Err(line_error(line, "duplicate noextract declaration"));
    }
    Ok(())
}

/// Parse and bind one source declaration.
fn bind_source(
    state: &mut ParserState,
    architecture: Option<String>,
    value: &str,
    line: usize,
) -> Result<(), SrcInfoError> {
    let effective_name = effective_source_name(value, line)?;
    state.sources.push(RawSource {
        architecture,
        value: value.to_string(),
        effective_name,
    });
    Ok(())
}

/// Match state for base or architecture-suffixed array keys.
enum ArrayKey {
    /// The key is unrelated to the requested array base.
    NotMatched,
    /// The key matched, with an optional architecture suffix.
    Matched(Option<String>),
}

/// Recognize a base or architecture-suffixed array key.
fn array_architecture(key: &str, base: &str) -> Result<ArrayKey, SrcInfoError> {
    if key == base {
        return Ok(ArrayKey::Matched(None));
    }
    let Some(suffix) = key.strip_prefix(&format!("{base}_")) else {
        return Ok(ArrayKey::NotMatched);
    };
    if suffix.is_empty()
        || !suffix.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(document_error(format!(
            "invalid architecture suffix in key `{key}`"
        )));
    }
    Ok(ArrayKey::Matched(Some(suffix.to_string())))
}

/// Recognize supported checksum array keys and architecture suffixes.
fn checksum_key(key: &str) -> Result<Option<(ChecksumAlgorithm, Option<String>)>, SrcInfoError> {
    for (name, algorithm) in ChecksumAlgorithm::SRCINFO_KEYS {
        if let ArrayKey::Matched(architecture) = array_architecture(key, name)? {
            return Ok(Some((algorithm, architecture)));
        }
    }
    Ok(None)
}

/// Derive a safe effective filename from makepkg's optional `name::locator` syntax.
fn effective_source_name(value: &str, line: usize) -> Result<String, SrcInfoError> {
    if let Some((name, locator)) = value.split_once("::") {
        validate_effective_name(name, line)?;
        if locator.is_empty() {
            return Err(line_error(line, "source locator after `::` is empty"));
        }
        return Ok(name.to_string());
    }
    let locator = value.strip_prefix("git+").unwrap_or(value);
    let path = locator.split(['?', '#']).next().unwrap_or(locator);
    let name = path.rsplit('/').next().unwrap_or(path);
    validate_effective_name(name, line)?;
    Ok(name.to_string())
}

/// Validate that a source's local effective name cannot introduce a path.
fn validate_effective_name(value: &str, line: usize) -> Result<(), SrcInfoError> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.contains(['/', '\\'])
        || value.chars().any(char::is_control)
    {
        return Err(line_error(
            line,
            "source effective name must be a safe basename",
        ));
    }
    Ok(())
}

/// Complete identity checks and bind every checksum array by architecture and position.
fn finish_parse(state: ParserState) -> Result<SrcInfo, SrcInfoError> {
    let package_base = state
        .package_base
        .ok_or_else(|| document_error("missing pkgbase"))?;
    if state.package_names.is_empty() {
        return Err(document_error("missing pkgname"));
    }
    let mut architecture_positions: BTreeMap<Option<String>, usize> = BTreeMap::new();
    let mut sources = Vec::with_capacity(state.sources.len());
    for raw_source in state.sources {
        let position = architecture_positions
            .entry(raw_source.architecture.clone())
            .or_default();
        let checksums = bind_source_checksums(
            &state.checksums,
            raw_source.architecture.as_ref(),
            *position,
        );
        *position += 1;
        sources.push(RecipeSource {
            no_extract: state.no_extract.contains(&raw_source.effective_name),
            value: raw_source.value,
            architecture: raw_source.architecture,
            effective_name: raw_source.effective_name,
            checksums,
            detached_signature_for: None,
        });
    }
    validate_checksum_lengths(&state.checksums, &architecture_positions)?;
    validate_unique_source_names(&sources)?;
    bind_detached_signatures(&mut sources);
    Ok(SrcInfo {
        package_base,
        package_names: state.package_names.into_iter().collect(),
        sources,
        valid_pgp_keys: state.valid_pgp_keys.into_iter().collect(),
        no_extract: state.no_extract.into_iter().collect(),
    })
}

/// Collect checksums at one architecture-local source position.
fn bind_source_checksums(
    checksums: &BTreeMap<(ChecksumAlgorithm, Option<String>), Vec<String>>,
    architecture: Option<&String>,
    position: usize,
) -> Vec<DeclaredChecksum> {
    ChecksumAlgorithm::ALL
        .iter()
        .filter_map(|algorithm| {
            checksums
                .get(&(*algorithm, architecture.cloned()))
                .and_then(|values| values.get(position))
                .map(|value| DeclaredChecksum {
                    algorithm: *algorithm,
                    value: value.clone(),
                })
        })
        .collect()
}

/// Require each present checksum array to cover every matching source exactly.
fn validate_checksum_lengths(
    checksums: &BTreeMap<(ChecksumAlgorithm, Option<String>), Vec<String>>,
    source_counts: &BTreeMap<Option<String>, usize>,
) -> Result<(), SrcInfoError> {
    for ((algorithm, architecture), values) in checksums {
        let expected = source_counts.get(architecture).copied().unwrap_or_default();
        if values.len() != expected {
            return Err(document_error(format!(
                "{} count {} does not match source count {expected} for architecture {}",
                algorithm.srcinfo_key(),
                values.len(),
                architecture.as_deref().unwrap_or("any")
            )));
        }
    }
    Ok(())
}

/// Pair detached signature basenames with the exact source declaration they cover.
fn bind_detached_signatures(sources: &mut [RecipeSource]) {
    let names: BTreeSet<String> = sources
        .iter()
        .map(|source| source.effective_name.clone())
        .collect();
    for source in sources {
        let covered = source
            .effective_name
            .strip_suffix(".sig")
            .or_else(|| source.effective_name.strip_suffix(".asc"));
        if let Some(covered) = covered
            && names.contains(covered)
        {
            source.detached_signature_for = Some(covered.to_string());
        }
    }
}

/// Reject ambiguous effective names that would collide in a snapshot.
fn validate_unique_source_names(sources: &[RecipeSource]) -> Result<(), SrcInfoError> {
    let mut names = BTreeSet::new();
    for source in sources {
        if !names.insert(source.effective_name.as_str()) {
            return Err(document_error(format!(
                "duplicate source effective name `{}`",
                source.effective_name
            )));
        }
    }
    Ok(())
}

/// Construct a line-specific parser error.
fn line_error(line: usize, reason: impl Into<String>) -> SrcInfoError {
    SrcInfoError {
        line,
        reason: reason.into(),
    }
}

/// Construct a document-level parser error.
fn document_error(reason: impl Into<String>) -> SrcInfoError {
    line_error(0, reason)
}
