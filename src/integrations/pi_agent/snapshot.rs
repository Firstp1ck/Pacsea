//! Private snapshot descriptor and exact-evidence assembly for one acquired package scan.
//!
//! This module turns finished acquisition artifacts into the four values the scan engine
//! needs: the private [`SnapshotRegistry`] descriptor, the bounded [`PackagePromptInput`],
//! the frozen [`ExpectedIdentity`], and the manifest-backed [`EvidenceIndex`].
//!
//! It performs no network, process, or acquisition work. It only assembles already
//! validated data, so the model-visible surface stays a pure function of what acquisition
//! actually materialized.
//!
//! Boundaries preserved here:
//!
//! - snapshot roots come from a Pacsea-owned descriptor and never appear in the prompt;
//! - the prompt receives only counts and bounded summaries, never manifests or file bodies;
//! - evidence content is indexed only for entries acquisition actually analyzed, so a
//!   citation for a manifest-only or binary entry cannot validate;
//! - in dry-run no root is published, which makes the descriptor unusable for a Pi launch.

use std::path::Path;

use super::restricted_tools::SnapshotRegistry;
use crate::logic::pi_scan::manifest::CanonicalManifest;
use crate::logic::pi_scan::prompt::{PackagePromptInput, SnapshotSummary};
use crate::logic::pi_scan::result::{EvidenceIndex, ExpectedIdentity};

/// Opaque snapshot id registered for the immutable recipe tree.
pub const RECIPE_SNAPSHOT_ID: &str = "recipe";

/// Opaque snapshot id registered for acquired upstream sources.
pub const SOURCE_SNAPSHOT_ID: &str = "source";

/// Maximum characters retained for one bounded prompt field.
const MAX_SUMMARY_CHARS: usize = 256;

/// Maximum entries retained in one bounded prompt list.
const MAX_SUMMARY_ITEMS: usize = 64;

/// What: Borrowed acquisition artifacts required to assemble the model-visible surface.
///
/// Inputs:
/// - Frozen identity, materialized snapshot roots, canonical manifests, analyzed entries,
///   and deterministic coverage notes.
///
/// Output:
/// - Consumed by [`assemble`].
///
/// Details:
/// - `publish_roots` is false for a dry-run preview. The registry is then left empty so the
///   preview can never be handed to a Pi process.
#[derive(Debug, Clone, Copy)]
pub struct SnapshotAssemblyInput<'a> {
    /// Scan identity recorded in the result.
    pub scan_id: &'a str,
    /// Canonical package base proven by the immutable recipe.
    pub package_base: &'a str,
    /// Package names the recipe declares for this base.
    pub package_names: &'a [String],
    /// Frozen immutable recipe commit.
    pub commit_oid: &'a str,
    /// Private recipe snapshot root.
    pub recipe_root: &'a Path,
    /// Private source snapshot root.
    pub source_root: &'a Path,
    /// Canonical recipe manifest.
    pub recipe_manifest: &'a CanonicalManifest,
    /// Canonical source manifest.
    pub source_manifest: &'a CanonicalManifest,
    /// Analyzed recipe text entries as path/content pairs.
    pub recipe_analyzed: &'a [(String, String)],
    /// Analyzed source text entries as path/content pairs.
    pub source_analyzed: &'a [(String, String)],
    /// Deterministic coverage limitations.
    pub coverage_notes: &'a [String],
    /// Whether snapshot roots may be published to the private descriptor.
    pub publish_roots: bool,
}

/// What: The assembled model-visible surface for one logical scan.
///
/// Inputs:
/// - Produced by [`assemble`].
///
/// Output:
/// - Passed to the scan engine together with the acquisition outcome.
///
/// Details:
/// - Every field is already bounded and identity-bound, so the engine performs no further
///   trimming and no further identity derivation.
#[derive(Debug)]
pub struct AssembledSnapshots {
    /// Private descriptor of immutable snapshot roots; empty in dry-run.
    pub registry: SnapshotRegistry,
    /// Bounded identity and coverage summary for the package prompt.
    pub prompt: PackagePromptInput,
    /// Frozen identity a model response must reproduce exactly.
    pub identity: ExpectedIdentity,
    /// Manifest-backed exact-evidence index.
    pub evidence: EvidenceIndex,
}

/// What: Assemble the descriptor, prompt input, frozen identity, and evidence index.
///
/// Inputs:
/// - `input`: Borrowed acquisition artifacts for exactly one package and commit.
///
/// Output:
/// - The assembled surface, or an actionable reason a snapshot root is unusable.
///
/// Details:
/// - Roots are registered only when `publish_roots` is set, and registration canonicalizes
///   each root so later containment checks compare fully resolved paths.
/// - Prompt lists and fields are bounded here so a package with very many names or notes
///   cannot produce an oversized prompt.
///
/// # Errors
/// - Returns `Err` when a published root does not resolve to an existing directory.
pub fn assemble(input: &SnapshotAssemblyInput<'_>) -> Result<AssembledSnapshots, String> {
    let mut registry = SnapshotRegistry::new();
    if input.publish_roots {
        registry
            .register(RECIPE_SNAPSHOT_ID, input.recipe_root)
            .map_err(|error| format!("the recipe snapshot root is unusable: {error}"))?;
        registry
            .register(SOURCE_SNAPSHOT_ID, input.source_root)
            .map_err(|error| format!("the source snapshot root is unusable: {error}"))?;
    }

    let prompt = PackagePromptInput {
        scan_id: input.scan_id.to_string(),
        package_base: input.package_base.to_string(),
        package_names: bounded_list(input.package_names),
        commit_oid: input.commit_oid.to_string(),
        snapshots: vec![
            summary(
                RECIPE_SNAPSHOT_ID,
                "immutable AUR recipe tree",
                input.recipe_manifest,
            ),
            summary(
                SOURCE_SNAPSHOT_ID,
                "declared upstream sources",
                input.source_manifest,
            ),
        ],
        coverage_notes: bounded_list(input.coverage_notes),
    };

    let identity = ExpectedIdentity {
        scan_id: input.scan_id.to_string(),
        package_base: input.package_base.to_string(),
        commit_oid: input.commit_oid.to_string(),
    };

    let mut evidence = EvidenceIndex::new();
    index_analyzed(
        &mut evidence,
        RECIPE_SNAPSHOT_ID,
        input.recipe_manifest,
        input.recipe_analyzed,
    );
    index_analyzed(
        &mut evidence,
        SOURCE_SNAPSHOT_ID,
        input.source_manifest,
        input.source_analyzed,
    );

    Ok(AssembledSnapshots {
        registry,
        prompt,
        identity,
        evidence,
    })
}

/// Build one bounded snapshot summary from a canonical manifest.
fn summary(id: &str, origin: &str, manifest: &CanonicalManifest) -> SnapshotSummary {
    SnapshotSummary {
        id: id.to_string(),
        origin: origin.to_string(),
        file_count: manifest.entries.len() as u64,
        total_bytes: manifest
            .entries
            .iter()
            .fold(0_u64, |total, entry| total.saturating_add(entry.size_bytes)),
    }
}

/// Bound one prompt list by item count and per-item length.
fn bounded_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .take(MAX_SUMMARY_ITEMS)
        .map(|value| {
            let sanitized: String = value
                .chars()
                .filter(|character| !character.is_control())
                .take(MAX_SUMMARY_CHARS)
                .collect();
            sanitized
        })
        .filter(|value| !value.is_empty())
        .collect()
}

/// Index only analyzed entries that the canonical manifest actually contains.
fn index_analyzed(
    evidence: &mut EvidenceIndex,
    snapshot: &str,
    manifest: &CanonicalManifest,
    analyzed: &[(String, String)],
) {
    for (path, content) in analyzed {
        if manifest.find_entry(snapshot, path).is_some() {
            evidence.insert(snapshot, path, content);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RECIPE_SNAPSHOT_ID, SOURCE_SNAPSHOT_ID, SnapshotAssemblyInput, assemble};
    use crate::logic::pi_scan::manifest::{CanonicalManifest, ManifestEntry};

    /// Build a one-entry manifest for assembly tests.
    fn manifest(category: &str, path: &str, content: &str) -> CanonicalManifest {
        use sha2::{Digest as _, Sha256};
        use std::fmt::Write as _;

        let digest = Sha256::digest(content.as_bytes());
        let hex = digest.iter().fold(String::new(), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        });
        CanonicalManifest::new(vec![
            ManifestEntry::new(category, path, content.len() as u64, hex, false, false)
                .expect("valid manifest entry"),
        ])
    }

    #[test]
    fn dry_run_publishes_no_snapshot_root() {
        let recipe = manifest(RECIPE_SNAPSHOT_ID, "PKGBUILD", "pkgname=demo");
        let source = manifest(SOURCE_SNAPSHOT_ID, "demo.tar/file", "body");
        let names = vec!["demo".to_string()];
        let notes: Vec<String> = Vec::new();
        let analyzed = vec![("PKGBUILD".to_string(), "pkgname=demo".to_string())];
        let empty: Vec<(String, String)> = Vec::new();
        let input = SnapshotAssemblyInput {
            scan_id: "scan-1",
            package_base: "demo",
            package_names: &names,
            commit_oid: &"a".repeat(40),
            recipe_root: std::path::Path::new("/nonexistent-recipe"),
            source_root: std::path::Path::new("/nonexistent-source"),
            recipe_manifest: &recipe,
            source_manifest: &source,
            recipe_analyzed: &analyzed,
            source_analyzed: &empty,
            coverage_notes: &notes,
            publish_roots: false,
        };
        let assembled = assemble(&input).expect("dry-run assembly needs no root");
        assert!(assembled.registry.root(RECIPE_SNAPSHOT_ID).is_err());
        assert_eq!(assembled.identity.package_base, "demo");
        assert_eq!(
            assembled.evidence.content(RECIPE_SNAPSHOT_ID, "PKGBUILD"),
            Some("pkgname=demo")
        );
    }

    #[test]
    fn evidence_is_limited_to_manifested_entries() {
        let recipe = manifest(RECIPE_SNAPSHOT_ID, "PKGBUILD", "pkgname=demo");
        let source = manifest(SOURCE_SNAPSHOT_ID, "demo.tar/file", "body");
        let names = vec!["demo".to_string()];
        let notes: Vec<String> = Vec::new();
        let fabricated = vec![("not-in-manifest".to_string(), "invented".to_string())];
        let empty: Vec<(String, String)> = Vec::new();
        let input = SnapshotAssemblyInput {
            scan_id: "scan-1",
            package_base: "demo",
            package_names: &names,
            commit_oid: &"b".repeat(40),
            recipe_root: std::path::Path::new("/nonexistent-recipe"),
            source_root: std::path::Path::new("/nonexistent-source"),
            recipe_manifest: &recipe,
            source_manifest: &source,
            recipe_analyzed: &fabricated,
            source_analyzed: &empty,
            coverage_notes: &notes,
            publish_roots: false,
        };
        let assembled = assemble(&input).expect("assembly");
        assert_eq!(
            assembled
                .evidence
                .content(RECIPE_SNAPSHOT_ID, "not-in-manifest"),
            None
        );
    }
}
