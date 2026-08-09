//! Immutable AUR recipe and declared-source acquisition over injectable process/network seams.
//!
//! This module owns the whole hostile-source acquisition boundary for one logical scan:
//!
//! - package-name to official package-base resolution proven by immutable `.SRCINFO`;
//! - a private ephemeral official AUR Git workspace driven by an absolute resolved Git
//!   executable and the shared isolation/environment/timeout runner from [`observer`];
//! - materialization of exactly one frozen recipe tree into a confined private snapshot;
//! - bounded acquisition of the static HTTPS and pinned `git+https` sources that the
//!   immutable `.SRCINFO` declares, with manual redirect handling and public-address
//!   validation at every hop;
//! - strong-checksum and exact-fingerprint signature verification through isolated seams.
//!
//! Security invariants enforced here:
//!
//! - never execute PKGBUILD, `prepare()`, `makepkg`, an AUR helper, or any shell; every
//!   process is direct `argv` and every Git invocation carries the isolation prefix;
//! - never follow `.gitmodules`, lockfiles, installers, scripts, or model-requested URLs;
//! - only `https` and pinned `git+https` transports; every other transport is `Incomplete`;
//! - at most [`MAX_SOURCE_REDIRECTS`] HTTPS redirects, no scheme downgrade, no URL
//!   userinfo, no ambient proxy, no custom CA, and no insecure TLS mode;
//! - hard per-source and per-package streaming byte caps;
//! - archives are inspected entry-by-entry in process and materialized only as normalized
//!   directories and regular files, never through a broad `unpack()` helper;
//! - every workspace, repository, and download is private, ephemeral, and always cleaned;
//! - dry-run performs the same read-only flow, persists nothing, and never launches Pi.
//!
//! Every external effect is an injectable seam ([`HttpFetcher`], [`AddressResolver`],
//! [`GitCommandRunner`], [`SignatureVerifier`]), so the whole flow is exercised by
//! deterministic fakes without real network, Git, or `GnuPG` access.

use crate::logic::pi_scan::head_source::{
    MAX_SOURCE_REDIRECTS, SourceLocator, classify_source_locator, validate_https_url,
    validate_public_addresses,
};
use crate::logic::pi_scan::identity::{
    AurRepoUrl, CommitOid, IdentityError, PackageBase, PackageName,
};
use crate::logic::pi_scan::manifest::{CanonicalManifest, ManifestEntry, normalize_manifest_path};
use crate::logic::pi_scan::observer::{
    GitCommandRunner, GitInvocation, GitOutput, ObserverError, git_argv,
};
use crate::logic::pi_scan::recipe::{RecipeSource, SrcInfo, SrcInfoError, parse_srcinfo};
use crate::logic::pi_scan::source::{
    AcquisitionStatus, ArchiveFormat, ArchiveLimits, InspectionReport, SignatureStatus,
    evaluate_integrity, inspect_source,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::io::Read;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Maximum compressed bytes accepted for one declared source.
pub const MAX_SOURCE_BYTES: u64 = 100 * 1024 * 1024;

/// Maximum aggregate downloaded bytes accepted for one package base and commit.
pub const MAX_PACKAGE_BYTES: u64 = 250 * 1024 * 1024;

/// Maximum bytes accepted for one immutable `.SRCINFO` transfer.
pub const MAX_SRCINFO_DOWNLOAD_BYTES: u64 = 10 * 1024 * 1024;

/// Maximum bytes accepted from one `git archive` recipe tree.
pub const MAX_RECIPE_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024;

/// Maximum declared sources acquired for one package base and commit.
pub const MAX_DECLARED_SOURCES: usize = 64;

/// Maximum bytes of one analyzed text entry retained for exact-evidence lookups.
pub const MAX_ANALYZED_TEXT_BYTES: usize = 16 * 1024 * 1024;

/// Maximum aggregate analyzed UTF-8 bytes retained per package acquisition.
pub const MAX_ANALYZED_PACKAGE_BYTES: u64 = 16 * 1024 * 1024;

/// Maximum coverage notes retained in the bounded prompt summary.
pub const MAX_COVERAGE_NOTES: usize = 32;

/// Default wall-clock deadline for one Git invocation performed during acquisition.
pub const DEFAULT_GIT_TIMEOUT: Duration = Duration::from_mins(1);

/// Default wall-clock deadline for one HTTPS request hop.
pub const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_mins(1);

/// Opaque snapshot id registered for the immutable recipe tree.
pub const RECIPE_SNAPSHOT_ID: &str = "recipe";

/// Opaque snapshot id registered for acquired upstream sources.
pub const SOURCE_SNAPSHOT_ID: &str = "source";

/// Manifest category used for recipe entries.
const RECIPE_CATEGORY: &str = "recipe";

/// Manifest category used for source entries.
const SOURCE_CATEGORY: &str = "source";

/// What: Acquisition failure that prevents producing any usable snapshot.
///
/// Inputs:
/// - Produced by seam failures, identity violations, or unsafe materialization input.
///
/// Output:
/// - An inert, user-facing message naming what failed and what the user can do next.
///
/// Details:
/// - Recoverable coverage limitations are not errors; they are recorded as `Incomplete`
///   reasons on the outcome so a partial scan can still be reported honestly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquisitionError {
    /// The package name could not be mapped to an official AUR package base.
    PackageBaseUnresolved {
        /// Package name that could not be resolved.
        package_name: String,
        /// Reason resolution failed.
        reason: String,
    },
    /// The immutable `.SRCINFO` did not prove the requested membership.
    MembershipUnproven {
        /// Package name that was expected in the recipe.
        package_name: String,
        /// Package base that was expected in the recipe.
        package_base: String,
    },
    /// The `.SRCINFO` document could not be parsed strictly.
    RecipeInvalid {
        /// Strict parser failure.
        source: SrcInfoError,
    },
    /// A Git invocation failed, timed out, or returned unusable output.
    Git {
        /// Underlying observer/process failure.
        source: ObserverError,
    },
    /// An HTTPS request violated transport, redirect, or address policy.
    Network {
        /// Requested URL, already validated as HTTPS without userinfo.
        url: String,
        /// Reason the request could not complete under policy.
        reason: String,
    },
    /// A private workspace, snapshot, or download path could not be prepared safely.
    Workspace {
        /// Path involved in the failure.
        path: PathBuf,
        /// Reason the path could not be used.
        reason: String,
    },
    /// An identity value crossing the boundary was invalid.
    Identity {
        /// Underlying identity validation failure.
        source: IdentityError,
    },
}

impl fmt::Display for AcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PackageBaseUnresolved {
                package_name,
                reason,
            } => write!(
                formatter,
                "'{package_name}' could not be mapped to an official AUR package base: \
                 {reason}. Refresh AUR metadata and retry, or exclude this package"
            ),
            Self::MembershipUnproven {
                package_name,
                package_base,
            } => write!(
                formatter,
                "the immutable .SRCINFO for '{package_base}' does not declare '{package_name}'. \
                 Nothing was scanned; verify the package really comes from this AUR base"
            ),
            Self::RecipeInvalid { source } => write!(
                formatter,
                "the immutable AUR recipe could not be parsed: {source}. Nothing was \
                 scanned; report this package base"
            ),
            Self::Git { source } => write!(formatter, "{source}"),
            Self::Network { url, reason } => write!(
                formatter,
                "the download of {url} was rejected: {reason}. Check network access and \
                 retry the scan"
            ),
            Self::Workspace { path, reason } => write!(
                formatter,
                "the private scan workspace at {} could not be prepared: {reason}. Check \
                 free space and permissions in your temporary directory",
                path.display()
            ),
            Self::Identity { source } => write!(
                formatter,
                "an unusable identity crossed the acquisition boundary: {source}. Nothing \
                 was recorded; retry the scan"
            ),
        }
    }
}

impl std::error::Error for AcquisitionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RecipeInvalid { source } => Some(source),
            Self::Git { source } => Some(source),
            Self::Identity { source } => Some(source),
            Self::PackageBaseUnresolved { .. }
            | Self::MembershipUnproven { .. }
            | Self::Network { .. }
            | Self::Workspace { .. } => None,
        }
    }
}

impl From<ObserverError> for AcquisitionError {
    fn from(source: ObserverError) -> Self {
        Self::Git { source }
    }
}

impl From<IdentityError> for AcquisitionError {
    fn from(source: IdentityError) -> Self {
        Self::Identity { source }
    }
}

/// What: One bounded HTTPS request hop issued by the adapter.
///
/// Inputs:
/// - Canonical HTTPS URL, hard byte cap, and wall-clock deadline.
///
/// Output:
/// - Consumed by a [`HttpFetcher`] implementation.
///
/// Details:
/// - Redirects are never followed by the fetcher. The adapter inspects every hop itself so
///   scheme, userinfo, and destination-address policy is re-checked before the next request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// Canonical HTTPS URL for exactly this hop.
    pub url: String,
    /// Validated address pinned for the actual connection.
    pub pinned_address: IpAddr,
    /// Hard cap on accepted response bytes for this hop.
    pub max_bytes: u64,
    /// Wall-clock deadline for this hop.
    pub timeout: Duration,
}

/// What: Captured result of one completed HTTPS request hop.
///
/// Inputs:
/// - Produced by a [`HttpFetcher`] implementation.
///
/// Output:
/// - Classified by the adapter before any byte is trusted.
///
/// Details:
/// - `body` must already be bounded by the request's `max_bytes`; the adapter re-checks it
///   so a faulty or hostile fetcher cannot exceed the cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP status code of this hop.
    pub status: u16,
    /// Exact `Location` header when the hop is a redirect.
    pub location: Option<String>,
    /// Response body bytes for a terminal 200 response.
    pub body: Vec<u8>,
}

/// What: Injectable bounded HTTPS transport seam.
///
/// Inputs:
/// - One fully-specified single-hop [`HttpRequest`].
///
/// Output:
/// - The captured [`HttpResponse`].
///
/// Details:
/// - Implementations must not follow redirects, must not inherit an ambient proxy, must not
///   install a custom certificate authority, and must never disable certificate validation.
///
/// # Errors
/// - Implementations return `Err` when the request cannot complete under those constraints.
pub trait HttpFetcher {
    /// Perform exactly one bounded HTTPS request hop.
    ///
    /// # Errors
    /// - Returns `Err` on connection, TLS, deadline, or byte-cap failure.
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, AcquisitionError>;
}

/// What: Injectable DNS seam used to validate every destination before it is contacted.
///
/// Inputs:
/// - Host and port of one hop.
///
/// Output:
/// - Every address the resolver returned for that host.
///
/// Details:
/// - The adapter rejects the hop when any returned address is outside public Internet
///   space, so a redirect into loopback, link-local, or RFC1918 space cannot be followed.
///
/// # Errors
/// - Implementations return `Err` when resolution fails.
pub trait AddressResolver {
    /// Resolve one host to every address the system would connect to.
    ///
    /// # Errors
    /// - Returns `Err` when the host cannot be resolved.
    fn resolve(&mut self, host: &str, port: u16) -> Result<Vec<IpAddr>, AcquisitionError>;

    /// Resolve one host under a caller-owned whole-operation deadline.
    ///
    /// # Errors
    /// - Returns `Err` when the host cannot be resolved within the supplied timeout.
    fn resolve_with_timeout(
        &mut self,
        host: &str,
        port: u16,
        _timeout: Duration,
    ) -> Result<Vec<IpAddr>, AcquisitionError> {
        self.resolve(host, port)
    }
}

/// What: One detached-signature verification request for an isolated verifier.
///
/// Inputs:
/// - Exact signed bytes, exact detached signature bytes, and the declared full fingerprints.
///
/// Output:
/// - Consumed by a [`SignatureVerifier`] implementation.
///
/// Details:
/// - Fingerprints come only from the immutable `.SRCINFO` `validpgpkeys` array, never from
///   model input, and are already normalized to uppercase full fingerprints by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureRequest<'a> {
    /// Exact bytes the signature must cover.
    pub data: &'a [u8],
    /// Exact detached signature bytes.
    pub signature: &'a [u8],
    /// Declared full uppercase fingerprints accepted for this package.
    pub fingerprints: &'a [String],
}

/// What: Injectable isolated exact-fingerprint signature verification seam.
///
/// Inputs:
/// - One [`SignatureRequest`].
///
/// Output:
/// - The explicit [`SignatureStatus`] the integrity policy consumes.
///
/// Details:
/// - Implementations must use an isolated `GnuPG` home and keyring with no ambient trustdb,
///   agent, keyserver, or configuration, and must accept only exact full-fingerprint keys.
/// - An implementation that cannot verify returns [`SignatureStatus::Unavailable`] rather
///   than guessing, which the policy turns into an explicit `Incomplete`.
pub trait SignatureVerifier {
    /// Verify one detached signature against exact declared fingerprints.
    fn verify(&mut self, request: &SignatureRequest<'_>) -> SignatureStatus;
}

/// What: A verifier that can never verify anything.
///
/// Inputs: None.
///
/// Output:
/// - Always [`SignatureStatus::Unavailable`] when verification is required.
///
/// Details:
/// - This is the fail-closed default. Using it makes every signature-required source
///   explicitly `Incomplete` instead of silently complete.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnavailableSignatureVerifier;

impl SignatureVerifier for UnavailableSignatureVerifier {
    fn verify(&mut self, _request: &SignatureRequest<'_>) -> SignatureStatus {
        SignatureStatus::Unavailable
    }
}

/// What: Bounded AUR RPC metadata supplied to package-base resolution.
///
/// Inputs:
/// - Either previously fetched RPC data or a bounded HTTPS RPC response.
///
/// Output:
/// - Consumed by [`resolve_package_base`].
///
/// Details:
/// - Only the mapping is used. No other RPC field influences identity, and the RPC answer
///   is never trusted alone: `.SRCINFO` membership must still prove the mapping.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AurRpcData {
    /// Package name to declared package base mapping.
    pub package_bases: BTreeMap<String, String>,
}

impl AurRpcData {
    /// What: Build RPC data from explicit name/base pairs.
    ///
    /// Inputs:
    /// - `pairs`: Package name and package base pairs.
    ///
    /// Output:
    /// - Bounded RPC data.
    ///
    /// Details:
    /// - Later duplicates of a name replace earlier ones, matching map semantics.
    #[must_use]
    pub fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        Self {
            package_bases: pairs
                .iter()
                .map(|(name, base)| ((*name).to_string(), (*base).to_string()))
                .collect(),
        }
    }
}

/// What: Effective acquisition bounds, never above the compiled maxima.
///
/// Inputs:
/// - Optional lowered operational limits from settings.
///
/// Output:
/// - Validated limits used by every download and archive inspection.
///
/// Details:
/// - `Default` selects every compiled maximum. Lowering is allowed; raising is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquisitionLimits {
    /// Maximum bytes accepted for one declared source.
    pub source_bytes: u64,
    /// Maximum aggregate bytes accepted for one package base and commit.
    pub package_bytes: u64,
    /// Maximum declared sources acquired for one package base and commit.
    pub declared_sources: usize,
    /// Wall-clock deadline for one Git invocation.
    pub git_timeout: Duration,
    /// Wall-clock deadline for one HTTPS hop.
    pub http_timeout: Duration,
    /// Archive inspection limits applied to every acquired source.
    pub archive: ArchiveLimits,
}

impl Default for AcquisitionLimits {
    fn default() -> Self {
        Self {
            source_bytes: MAX_SOURCE_BYTES,
            package_bytes: MAX_PACKAGE_BYTES,
            declared_sources: MAX_DECLARED_SOURCES,
            git_timeout: DEFAULT_GIT_TIMEOUT,
            http_timeout: DEFAULT_HTTP_TIMEOUT,
            archive: ArchiveLimits::default(),
        }
    }
}

impl AcquisitionLimits {
    /// What: Clamp every field down to its compiled maximum.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Limits guaranteed not to exceed compiled policy.
    ///
    /// Details:
    /// - Clamping down rather than rejecting keeps a misconfigured setting safe by
    ///   construction; the compiled bound always wins.
    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            source_bytes: self.source_bytes.clamp(1, MAX_SOURCE_BYTES),
            package_bytes: self.package_bytes.clamp(1, MAX_PACKAGE_BYTES),
            declared_sources: self.declared_sources.clamp(1, MAX_DECLARED_SOURCES),
            git_timeout: self.git_timeout.min(DEFAULT_GIT_TIMEOUT),
            http_timeout: self.http_timeout.min(DEFAULT_HTTP_TIMEOUT),
            archive: ArchiveLimits::validate(self.archive).unwrap_or_default(),
        }
    }
}

/// What: Frozen inputs for exactly one acquisition run.
///
/// Inputs:
/// - Scan identity, the requested package name, bounded RPC data, and the frozen commit.
///
/// Output:
/// - Consumed by [`acquire_package`].
///
/// Details:
/// - `commit_oid` is always a full immutable OID frozen by the observer. Acquisition never
///   resolves a branch, a tag, or `HEAD` itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionRequest {
    /// Scan identity recorded in the result.
    pub scan_id: String,
    /// Installed package name whose membership must be proven.
    pub package_name: PackageName,
    /// Frozen immutable recipe commit.
    pub commit_oid: CommitOid,
    /// Bounded AUR RPC metadata used only for the initial base mapping.
    pub rpc: AurRpcData,
    /// Effective bounds for this run.
    pub limits: AcquisitionLimits,
    /// Whether this run must persist nothing and never launch Pi.
    pub dry_run: bool,
}

/// What: One resolved and pinned HTTPS hop recorded in acquisition provenance.
///
/// Inputs:
/// - Produced after complete DNS-answer validation and before contact.
///
/// Output:
/// - Non-sensitive evidence of the hostname, accepted answer set, and selected address.
///
/// Details:
/// - Every address is public-policy validated; mixed answer sets are rejected, not recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressProvenance {
    /// HTTPS hostname retained for HTTP and TLS verification.
    pub host: String,
    /// Complete sorted DNS answer set validated for this hop.
    pub resolved_addresses: Vec<IpAddr>,
    /// Exact address pinned for the contacted hop.
    pub pinned_address: IpAddr,
}

/// What: Per-source acquisition provenance and outcome.
///
/// Inputs:
/// - Produced for every `.SRCINFO` source declaration, including unsupported ones.
///
/// Output:
/// - Coverage and provenance input for the result record.
///
/// Details:
/// - An unsupported or mutable declaration is recorded with `Incomplete` and an explicit
///   reason rather than being silently skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceOutcome {
    /// Exact `.SRCINFO` declaration.
    pub declaration: String,
    /// Effective local filename bound by the recipe parser.
    pub effective_name: String,
    /// Final status for this source.
    pub status: AcquisitionStatus,
    /// Ordered canonical URL chain actually contacted, including every redirect hop.
    pub redirect_chain: Vec<String>,
    /// DNS answer and connection-pin provenance corresponding to each contacted hop.
    pub address_provenance: Vec<AddressProvenance>,
    /// Bytes accepted for this source.
    pub bytes: u64,
    /// Signature policy result applied to this source.
    pub signature: SignatureStatus,
    /// Deterministic reasons explaining the status.
    pub reasons: Vec<String>,
}

/// What: Non-sensitive provenance describing one completed acquisition.
///
/// Inputs:
/// - Produced by [`acquire_package`].
///
/// Output:
/// - Persisted alongside the scan result.
///
/// Details:
/// - Contains identities, counts, and policy outcomes only; no source body, no key
///   material, and no host path outside the ephemeral workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionProvenance {
    /// Canonical official AUR repository URL that was observed.
    pub repository_url: String,
    /// Frozen immutable recipe commit.
    pub commit_oid: String,
    /// Per-source outcomes in declaration order.
    pub sources: Vec<SourceOutcome>,
    /// Aggregate accepted download bytes for this package and commit.
    pub downloaded_bytes: u64,
    /// Whether the run persisted nothing and launched no Pi process.
    pub dry_run: bool,
}

/// One mutable Git ref resolved to an exact advisory identity for later staleness checks.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MutableSourceIdentity {
    /// Exact `.SRCINFO` declaration.
    pub declaration: String,
    /// Canonical HTTPS repository URL without fragment.
    pub repository_url: String,
    /// Fully qualified ref or `HEAD`.
    pub reference: String,
    /// Exact OID observed during acquisition.
    pub resolved_oid: CommitOid,
}

/// What: Everything one logical scan needs from acquisition.
///
/// Inputs:
/// - Produced by [`acquire_package`].
///
/// Output:
/// - Snapshot roots, bounded prompt input, frozen identity, exact evidence, manifests, and
///   coverage/provenance.
///
/// Details:
/// - The outcome owns the ephemeral workspace. Dropping it removes every repository,
///   download, and snapshot, so no caller can leak acquisition state.
/// - In dry-run the workspace is cleaned before returning and `snapshots` is empty, so the
///   preview cannot be handed to Pi.
#[derive(Debug)]
pub struct AcquisitionOutcome {
    /// Overall acquisition status for this package and commit.
    pub status: AcquisitionStatus,
    /// Private descriptor of immutable snapshot roots, empty in dry-run.
    pub snapshots: crate::pi_agent::restricted_tools::SnapshotRegistry,
    /// Bounded identity and coverage summary for the package prompt.
    pub prompt: crate::logic::pi_scan::prompt::PackagePromptInput,
    /// Frozen identity a model response must reproduce exactly.
    pub identity: crate::logic::pi_scan::result::ExpectedIdentity,
    /// Manifest-backed exact-evidence index.
    pub evidence: crate::logic::pi_scan::result::EvidenceIndex,
    /// Canonical recipe manifest.
    pub recipe_manifest: CanonicalManifest,
    /// Canonical source manifest.
    pub source_manifest: CanonicalManifest,
    /// Strictly parsed immutable recipe metadata.
    pub srcinfo: SrcInfo,
    /// Deterministic coverage limitations, empty when acquisition is complete.
    pub coverage_notes: Vec<String>,
    /// Non-sensitive acquisition provenance.
    pub provenance: AcquisitionProvenance,
    /// Mutable Git refs resolved for advisory scanning and later continuation rechecks.
    pub mutable_sources: Vec<MutableSourceIdentity>,
    /// Ephemeral workspace guard; dropping it cleans every acquired artifact.
    workspace: EphemeralWorkspace,
}

impl AcquisitionOutcome {
    /// What: Borrow the private ephemeral workspace root while it still exists.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - The workspace root, or `None` once it has been cleaned.
    ///
    /// Details:
    /// - A dry-run outcome always reports `None` because its workspace is removed before
    ///   the outcome is returned. Callers use this only to assert cleanup, never to hand a
    ///   path to the model.
    #[must_use]
    pub fn workspace_root(&self) -> Option<&Path> {
        self.workspace.root.as_deref()
    }
}

/// What: Private ephemeral directory tree removed on drop.
///
/// Inputs:
/// - A parent directory and a unique run name.
///
/// Output:
/// - A mode-0700 directory that is always cleaned.
///
/// Details:
/// - Cleanup runs in `Drop` so an early return, a policy rejection, or a panic still
///   removes every repository, download, and snapshot.
#[derive(Debug)]
struct EphemeralWorkspace {
    /// Root of the private workspace, or `None` once it has been cleaned.
    root: Option<PathBuf>,
}

impl EphemeralWorkspace {
    /// Create a fresh private mode-0700 workspace that cannot reuse a planted directory.
    fn create(parent: &Path, name: &str) -> Result<Self, AcquisitionError> {
        let root = parent.join(name);
        std::fs::create_dir_all(parent).map_err(|error| AcquisitionError::Workspace {
            path: parent.to_path_buf(),
            reason: error.to_string(),
        })?;
        create_private_dir(&root)?;
        Ok(Self { root: Some(root) })
    }

    /// Borrow the workspace root while it still exists.
    fn root(&self) -> Result<&Path, AcquisitionError> {
        self.root
            .as_deref()
            .ok_or_else(|| AcquisitionError::Workspace {
                path: PathBuf::new(),
                reason: "the ephemeral workspace was already cleaned".to_string(),
            })
    }

    /// Remove the workspace immediately and mark it cleaned.
    fn clean(&mut self) {
        if let Some(root) = self.root.take() {
            drop(std::fs::remove_dir_all(root));
        }
    }
}

impl Drop for EphemeralWorkspace {
    fn drop(&mut self) {
        self.clean();
    }
}

/// Create one private mode-0700 directory, refusing to reuse an existing path or symlink.
fn create_private_dir(path: &Path) -> Result<(), AcquisitionError> {
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|error| AcquisitionError::Workspace {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
}

/// Create one private directory, tolerating a directory this run already created.
fn ensure_private_dir(path: &Path) -> Result<(), AcquisitionError> {
    if path.is_dir() {
        return Ok(());
    }
    create_private_dir(path)
}

/// Write one regular file privately, refusing to follow or replace an existing path.
fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), AcquisitionError> {
    use std::io::Write as _;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| AcquisitionError::Workspace {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| AcquisitionError::Workspace {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })
}

/// What: Map a package name to its official AUR package base from bounded RPC data.
///
/// Inputs:
/// - `package_name`: Installed package name to resolve.
/// - `rpc`: Bounded AUR RPC metadata.
///
/// Output:
/// - The declared package base.
///
/// Details:
/// - The RPC answer is only a hint. Callers must still prove `.SRCINFO` membership before
///   any acquired data is bound to this identity.
///
/// # Errors
/// - Returns `Err` when the name is absent from the RPC data or names an invalid base.
pub fn resolve_package_base(
    package_name: &PackageName,
    rpc: &AurRpcData,
) -> Result<PackageBase, AcquisitionError> {
    let declared = rpc
        .package_bases
        .get(package_name.as_str())
        .ok_or_else(|| AcquisitionError::PackageBaseUnresolved {
            package_name: package_name.as_str().to_string(),
            reason: "AUR metadata declares no package base for this name".to_string(),
        })?;
    PackageBase::new(declared).map_err(|error| AcquisitionError::PackageBaseUnresolved {
        package_name: package_name.as_str().to_string(),
        reason: error.to_string(),
    })
}

/// What: Prove that an immutable `.SRCINFO` really declares the requested identity.
///
/// Inputs:
/// - `srcinfo`: Strictly parsed immutable recipe.
/// - `package_name`: Installed package name that must be a member.
/// - `package_base`: Package base the recipe must declare.
///
/// Output:
/// - `Ok(())` only when both the base and the membership match exactly.
///
/// Details:
/// - This is the single authority for the mapping. A matching AUR RPC answer alone never
///   establishes membership.
///
/// # Errors
/// - Returns [`AcquisitionError::MembershipUnproven`] when either check fails.
pub fn prove_srcinfo_membership(
    srcinfo: &SrcInfo,
    package_name: &PackageName,
    package_base: &PackageBase,
) -> Result<(), AcquisitionError> {
    if &srcinfo.package_base != package_base || !srcinfo.package_names.contains(package_name) {
        return Err(AcquisitionError::MembershipUnproven {
            package_name: package_name.as_str().to_string(),
            package_base: package_base.as_str().to_string(),
        });
    }
    Ok(())
}

/// Build one isolated Git invocation with the shared isolation prefix and environment.
fn git_invocation(executable: &OsStr, args: &[&OsStr], timeout: Duration) -> GitInvocation {
    use crate::logic::pi_scan::observer::{GIT_FIXED_ENVIRONMENT, GIT_PASSTHROUGH_ENVIRONMENT};

    GitInvocation {
        executable: executable.to_os_string(),
        argv: git_argv(args),
        passthrough_environment: GIT_PASSTHROUGH_ENVIRONMENT
            .iter()
            .map(|name| (*name).to_string())
            .collect(),
        fixed_environment: GIT_FIXED_ENVIRONMENT
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect(),
        timeout,
    }
}

/// What: Build the invocation initializing a private bare repository for one package base.
///
/// Inputs:
/// - `executable`: Resolved absolute Git executable.
/// - `repository_dir`: Private ephemeral repository directory.
/// - `timeout`: Wall-clock deadline for this invocation.
///
/// Output:
/// - A complete [`GitInvocation`] for `init --bare --quiet`.
///
/// Details:
/// - A bare repository has no worktree, so no checkout, filter, or hook can ever run.
#[must_use]
pub fn init_repository_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let args: [&OsStr; 4] = [
        OsStr::new("init"),
        OsStr::new("--bare"),
        OsStr::new("--quiet"),
        dir.as_os_str(),
    ];
    git_invocation(executable, &args, timeout)
}

/// What: Build the invocation fetching exactly one immutable commit.
///
/// Inputs:
/// - `executable`: Resolved absolute Git executable.
/// - `repository_dir`: Private ephemeral repository directory.
/// - `repo_url`: Canonical official AUR repository URL.
/// - `commit`: Frozen full commit OID to fetch.
/// - `timeout`: Wall-clock deadline for this invocation.
///
/// Output:
/// - A complete [`GitInvocation`] for a submodule-free, tag-free `fetch`.
///
/// Details:
/// - `--no-recurse-submodules` and `--no-tags` keep the fetch to exactly the requested
///   history, and the isolation prefix already disables hooks, credentials, and proxies.
/// - The commit is passed as its own argv element, so no shell quoting is involved.
#[must_use]
pub fn fetch_commit_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    repo_url: &AurRepoUrl,
    commit: &CommitOid,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let url = OsString::from(repo_url.as_str());
    let oid = OsString::from(commit.as_str());
    let args: [&OsStr; 9] = [
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("fetch"),
        OsStr::new("--no-recurse-submodules"),
        OsStr::new("--no-tags"),
        OsStr::new("--quiet"),
        OsStr::new("--"),
        url.as_os_str(),
        oid.as_os_str(),
    ];
    git_invocation(executable, &args, timeout)
}

/// Build one DNS-pinned immutable upstream Git fetch invocation.
fn fetch_pinned_source_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    repository_url: &str,
    commit: &CommitOid,
    address: &AddressProvenance,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let url = OsString::from(repository_url);
    let oid = OsString::from(commit.as_str());
    let port = reqwest::Url::parse(repository_url)
        .ok()
        .and_then(|parsed| parsed.port_or_known_default())
        .unwrap_or(443);
    let pinned = match address.pinned_address {
        IpAddr::V4(value) => format!("{}:{port}:{value}", address.host),
        IpAddr::V6(value) => format!("{}:{port}:[{value}]", address.host),
    };
    let pinned = OsString::from(format!("http.curloptResolve={pinned}"));
    let args: [&OsStr; 11] = [
        OsStr::new("-c"),
        pinned.as_os_str(),
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("fetch"),
        OsStr::new("--no-recurse-submodules"),
        OsStr::new("--no-tags"),
        OsStr::new("--quiet"),
        OsStr::new("--"),
        url.as_os_str(),
        oid.as_os_str(),
    ];
    git_invocation(executable, &args, timeout)
}

/// Build one DNS-pinned `ls-remote` invocation for a mutable advisory Git ref.
fn resolve_mutable_source_invocation(
    executable: &OsStr,
    repository_url: &str,
    reference: &str,
    address: &AddressProvenance,
    timeout: Duration,
) -> GitInvocation {
    let url = OsString::from(repository_url);
    let reference = OsString::from(reference);
    let port = reqwest::Url::parse(repository_url)
        .ok()
        .and_then(|parsed| parsed.port_or_known_default())
        .unwrap_or(443);
    let pinned = match address.pinned_address {
        IpAddr::V4(value) => format!("{}:{port}:{value}", address.host),
        IpAddr::V6(value) => format!("{}:{port}:[{value}]", address.host),
    };
    let pinned = OsString::from(format!("http.curloptResolve={pinned}"));
    let args: [&OsStr; 7] = [
        OsStr::new("-c"),
        pinned.as_os_str(),
        OsStr::new("ls-remote"),
        OsStr::new("--exit-code"),
        OsStr::new("--"),
        url.as_os_str(),
        reference.as_os_str(),
    ];
    git_invocation(executable, &args, timeout)
}

/// What: Build the invocation exporting one frozen recipe tree as an uncompressed tar.
///
/// Inputs:
/// - `executable`: Resolved absolute Git executable.
/// - `repository_dir`: Private ephemeral repository directory.
/// - `commit`: Frozen full commit OID to export.
/// - `timeout`: Wall-clock deadline for this invocation.
///
/// Output:
/// - A complete [`GitInvocation`] for `archive --format=tar`.
///
/// Details:
/// - `git archive` streams the tree without creating a worktree, so no checkout filter,
///   `.gitattributes` textconv, hook, or submodule traversal is involved.
#[must_use]
pub fn archive_tree_invocation(
    executable: &OsStr,
    repository_dir: &OsStr,
    commit: &CommitOid,
    timeout: Duration,
) -> GitInvocation {
    let dir = repository_dir.to_os_string();
    let oid = OsString::from(commit.as_str());
    let args: [&OsStr; 6] = [
        OsStr::new("-C"),
        dir.as_os_str(),
        OsStr::new("archive"),
        OsStr::new("--format=tar"),
        OsStr::new("--"),
        oid.as_os_str(),
    ];
    git_invocation(executable, &args, timeout)
}

/// Run one Git invocation and require success plus a bounded stdout.
fn run_git(
    runner: &mut dyn GitCommandRunner,
    invocation: &GitInvocation,
    https_proxy: Option<&str>,
    operation: &str,
    limit: u64,
) -> Result<Vec<u8>, AcquisitionError> {
    let mut invocation = invocation.clone();
    if let Some(proxy) = https_proxy {
        invocation.set_https_proxy(proxy);
    }
    let output: GitOutput = runner.run(&invocation)?;
    if !output.success {
        let reason = if output.stderr.trim().is_empty() {
            "the command exited with a non-zero status".to_string()
        } else {
            output.stderr.trim().to_string()
        };
        return Err(ObserverError::GitCommand {
            operation: operation.to_string(),
            reason,
        }
        .into());
    }
    let observed = output.stdout.len();
    if observed as u64 > limit {
        return Err(ObserverError::OutputTooLarge {
            operation: operation.to_string(),
            observed,
            limit: usize::try_from(limit).unwrap_or(usize::MAX),
        }
        .into());
    }
    Ok(output.stdout)
}

/// What: One entry materialized from an archive into a private snapshot.
///
/// Inputs:
/// - Produced by [`materialize_tar`] and [`materialize_zip`].
///
/// Output:
/// - Manifest and evidence input.
///
/// Details:
/// - Only normalized directories and regular files are ever produced. Links, devices,
///   FIFOs, sockets, and unknown entry types make the snapshot incomplete instead.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MaterializedEntry {
    /// Normalized snapshot-relative path.
    path: String,
    /// Exact entry bytes.
    bytes: Vec<u8>,
    /// Whether the entry carries an executable mode bit.
    executable: bool,
}

/// Iterate a tar archive in process and collect only normalized regular files.
fn materialize_tar(
    bytes: &[u8],
    limits: ArchiveLimits,
) -> Result<Vec<MaterializedEntry>, AcquisitionError> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(bytes));
    let entries = archive
        .entries()
        .map_err(|error| AcquisitionError::Network {
            url: "git archive".to_string(),
            reason: format!("invalid tar stream: {error}"),
        })?;
    let mut collected = Vec::new();
    let mut total: u64 = 0;
    for entry_result in entries {
        let mut entry = entry_result.map_err(|error| AcquisitionError::Network {
            url: "git archive".to_string(),
            reason: format!("invalid tar entry: {error}"),
        })?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        if !entry_type.is_file() {
            continue;
        }
        if collected.len() >= limits.entries {
            break;
        }
        let path_bytes = entry.path_bytes();
        let Ok(raw_path) = std::str::from_utf8(&path_bytes) else {
            continue;
        };
        let Ok(path) = normalize_manifest_path(raw_path) else {
            continue;
        };
        if path.split('/').count() > limits.path_depth {
            continue;
        }
        let mut body = Vec::new();
        let mut limited = (&mut entry).take(limits.entry_bytes.saturating_add(1));
        limited
            .read_to_end(&mut body)
            .map_err(|error| AcquisitionError::Network {
                url: "git archive".to_string(),
                reason: format!("tar entry read failed: {error}"),
            })?;
        if body.len() as u64 > limits.entry_bytes {
            break;
        }
        total = total.saturating_add(body.len() as u64);
        if total > limits.expanded_bytes {
            break;
        }
        let executable = entry.header().mode().unwrap_or_default() & 0o111 != 0;
        collected.push(MaterializedEntry {
            path,
            bytes: body,
            executable,
        });
    }
    Ok(collected)
}

/// Iterate a ZIP archive in process and collect only Stored/Deflate regular files.
fn materialize_zip(
    bytes: &[u8],
    limits: ArchiveLimits,
) -> Result<Vec<MaterializedEntry>, AcquisitionError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|error| {
        AcquisitionError::Network {
            url: "zip source".to_string(),
            reason: format!("invalid ZIP archive: {error}"),
        }
    })?;
    let mut collected = Vec::new();
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        if collected.len() >= limits.entries {
            break;
        }
        let Ok(mut entry) = archive.by_index(index) else {
            break;
        };
        if entry.is_dir() || entry.is_symlink() || !entry.is_file() || entry.encrypted() {
            continue;
        }
        if !matches!(
            entry.compression(),
            zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
        ) {
            continue;
        }
        let Ok(raw_name) = std::str::from_utf8(entry.name_raw()) else {
            continue;
        };
        let Ok(path) = normalize_manifest_path(raw_name) else {
            continue;
        };
        if path.split('/').count() > limits.path_depth {
            continue;
        }
        let mut body = Vec::new();
        let mut limited = (&mut entry).take(limits.entry_bytes.saturating_add(1));
        if limited.read_to_end(&mut body).is_err() {
            break;
        }
        if body.len() as u64 > limits.entry_bytes {
            break;
        }
        total = total.saturating_add(body.len() as u64);
        if total > limits.expanded_bytes {
            break;
        }
        let executable = entry.unix_mode().is_some_and(|mode| mode & 0o111 != 0);
        collected.push(MaterializedEntry {
            path,
            bytes: body,
            executable,
        });
    }
    Ok(collected)
}

/// Write already-validated entries under a confined snapshot root and build the manifest.
fn write_entries(
    root: &Path,
    category: &str,
    entries: &[MaterializedEntry],
) -> Result<Vec<ManifestEntry>, AcquisitionError> {
    use sha2::{Digest as _, Sha256};

    let mut manifest_entries = Vec::with_capacity(entries.len());
    for entry in entries {
        let target = confined_join(root, &entry.path)?;
        if let Some(parent) = target.parent() {
            create_directory_chain(root, parent)?;
        }
        write_private_file(&target, &entry.bytes)?;
        let digest = hex(&Sha256::digest(&entry.bytes));
        let manifest_entry = ManifestEntry::new(
            category,
            &entry.path,
            entry.bytes.len() as u64,
            digest,
            entry.executable,
            is_binary(&entry.bytes),
        )
        .map_err(|error| AcquisitionError::Workspace {
            path: target.clone(),
            reason: error.to_string(),
        })?;
        manifest_entries.push(manifest_entry);
    }
    Ok(manifest_entries)
}

/// Join a normalized relative path under a root and re-verify containment.
fn confined_join(root: &Path, relative: &str) -> Result<PathBuf, AcquisitionError> {
    let normalized =
        normalize_manifest_path(relative).map_err(|error| AcquisitionError::Workspace {
            path: root.to_path_buf(),
            reason: error.to_string(),
        })?;
    let mut target = root.to_path_buf();
    for component in normalized.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(AcquisitionError::Workspace {
                path: root.to_path_buf(),
                reason: "snapshot path component is not a plain name".to_string(),
            });
        }
        target.push(component);
    }
    if !target.starts_with(root) {
        return Err(AcquisitionError::Workspace {
            path: target,
            reason: "snapshot path escapes its root".to_string(),
        });
    }
    Ok(target)
}

/// Create every missing private directory between a root and a target directory.
fn create_directory_chain(root: &Path, target: &Path) -> Result<(), AcquisitionError> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| AcquisitionError::Workspace {
            path: target.to_path_buf(),
            reason: "snapshot directory escapes its root".to_string(),
        })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        ensure_private_dir(&current)?;
    }
    Ok(())
}

/// Format bytes as lowercase hexadecimal.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    )
}

/// Classify bytes conservatively for manifest text coverage.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
}

/// What: Classify one effective source filename into a supported container format.
///
/// Inputs:
/// - `name`: Effective local filename bound by the recipe parser.
///
/// Output:
/// - The explicit supported format, defaulting to a raw single file.
///
/// Details:
/// - Classification is filename-based on purpose: content sniffing would let a hostile
///   archive choose its own decoder.
#[must_use]
pub fn classify_archive_format(name: &str) -> ArchiveFormat {
    let lower = name.to_ascii_lowercase();
    for (suffix, format) in [
        (".tar.gz", ArchiveFormat::TarGzip),
        (".tgz", ArchiveFormat::TarGzip),
        (".tar.bz2", ArchiveFormat::TarBzip2),
        (".tbz2", ArchiveFormat::TarBzip2),
        (".tar.xz", ArchiveFormat::TarXz),
        (".txz", ArchiveFormat::TarXz),
        (".tar.zst", ArchiveFormat::TarZstd),
        (".tzst", ArchiveFormat::TarZstd),
        (".tar", ArchiveFormat::Tar),
        (".zip", ArchiveFormat::Zip),
        (".gz", ArchiveFormat::Gzip),
        (".bz2", ArchiveFormat::Bzip2),
        (".xz", ArchiveFormat::Xz),
        (".zst", ArchiveFormat::Zstd),
    ] {
        if lower.ends_with(suffix) {
            return format;
        }
    }
    ArchiveFormat::Raw
}

/// What: Fully bounded HTTPS download result with redirect and address provenance.
///
/// Inputs:
/// - Produced by [`download_static_source`].
///
/// Output:
/// - Accepted bytes and the exact contacted network path.
///
/// Details:
/// - URL and address vectors have one element per contacted hop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadedSource {
    /// Accepted terminal response bytes.
    pub bytes: Vec<u8>,
    /// Ordered canonical contacted URLs.
    pub redirect_chain: Vec<String>,
    /// Validated answer sets and pins for every contacted URL.
    pub address_provenance: Vec<AddressProvenance>,
}

/// What: Download one static HTTPS source with manual bounded redirect handling.
///
/// Inputs:
/// - `http`: Bounded transport seam.
/// - `resolver`: DNS seam used before every hop.
/// - `url`: Initial canonical HTTPS URL.
/// - `max_bytes`: Hard byte cap for this source.
/// - `timeout`: Whole-download wall-clock deadline shared across every redirect hop.
///
/// Output:
/// - The accepted bytes plus the exact ordered URL chain that was contacted.
///
/// Details:
/// - Every hop independently requires HTTPS, no userinfo, no fragment, and a fully public
///   destination address set. At most [`MAX_SOURCE_REDIRECTS`] redirects are followed.
/// - A redirect without a `Location`, to a non-HTTPS scheme, or beyond the cap is rejected
///   rather than retried.
///
/// # Errors
/// - Returns [`AcquisitionError::Network`] for any policy, status, or byte-cap violation.
pub fn download_static_source(
    http: &mut dyn HttpFetcher,
    resolver: &mut dyn AddressResolver,
    url: &str,
    max_bytes: u64,
    timeout: Duration,
) -> Result<DownloadedSource, AcquisitionError> {
    let mut current =
        validate_https_url(url, false).map_err(|error| AcquisitionError::Network {
            url: url.to_string(),
            reason: error.reason,
        })?;
    let mut chain = Vec::new();
    let mut provenance = Vec::new();
    let started_at = Instant::now();
    for _ in 0..=MAX_SOURCE_REDIRECTS {
        let dns_timeout = remaining_download_time(timeout, started_at, &current)?;
        let address = verify_public_destination(resolver, &current, dns_timeout)?;
        let request_timeout = remaining_download_time(timeout, started_at, &current)?;
        chain.push(current.clone());
        let response = http.fetch(&HttpRequest {
            url: current.clone(),
            pinned_address: address.pinned_address,
            max_bytes,
            timeout: request_timeout,
        })?;
        provenance.push(address);
        match response.status {
            200 => {
                if response.body.len() as u64 > max_bytes {
                    return Err(AcquisitionError::Network {
                        url: current,
                        reason: format!("response exceeds the {max_bytes}-byte source limit"),
                    });
                }
                return Ok(DownloadedSource {
                    bytes: response.body,
                    redirect_chain: chain,
                    address_provenance: provenance,
                });
            }
            301 | 302 | 303 | 307 | 308 => {
                current = next_redirect_target(&current, response.location.as_deref())?;
            }
            status => {
                return Err(AcquisitionError::Network {
                    url: current,
                    reason: format!("unexpected HTTP status {status}"),
                });
            }
        }
    }
    Err(AcquisitionError::Network {
        url: current,
        reason: format!("more than {MAX_SOURCE_REDIRECTS} redirects were requested"),
    })
}

/// Return the remaining wall-clock allowance for one complete redirected download.
fn remaining_download_time(
    timeout: Duration,
    started_at: Instant,
    url: &str,
) -> Result<Duration, AcquisitionError> {
    timeout
        .checked_sub(started_at.elapsed())
        .filter(|remaining| !remaining.is_zero())
        .ok_or_else(|| AcquisitionError::Network {
            url: url.to_string(),
            reason: "the whole download deadline was exhausted".to_string(),
        })
}

/// Validate one redirect target under the same strict transport policy as the first hop.
fn next_redirect_target(current: &str, location: Option<&str>) -> Result<String, AcquisitionError> {
    let location = location.ok_or_else(|| AcquisitionError::Network {
        url: current.to_string(),
        reason: "redirect response carried no Location header".to_string(),
    })?;
    let base = reqwest::Url::parse(current).map_err(|error| AcquisitionError::Network {
        url: current.to_string(),
        reason: format!("redirect base URL is malformed: {error}"),
    })?;
    let joined = base
        .join(location)
        .map_err(|error| AcquisitionError::Network {
            url: current.to_string(),
            reason: format!("redirect Location is malformed: {error}"),
        })?;
    validate_https_url(joined.as_str(), false).map_err(|error| AcquisitionError::Network {
        url: current.to_string(),
        reason: format!("redirect rejected: {}", error.reason),
    })
}

/// Resolve one hop's host and require every returned address to be public Internet space.
fn verify_public_destination(
    resolver: &mut dyn AddressResolver,
    url: &str,
    timeout: Duration,
) -> Result<AddressProvenance, AcquisitionError> {
    let parsed = reqwest::Url::parse(url).map_err(|error| AcquisitionError::Network {
        url: url.to_string(),
        reason: format!("malformed URL: {error}"),
    })?;
    let host = parsed
        .host_str()
        .ok_or_else(|| AcquisitionError::Network {
            url: url.to_string(),
            reason: "URL requires a host".to_string(),
        })?
        .to_string();
    let port = parsed.port_or_known_default().unwrap_or(443);
    let mut addresses = resolver.resolve_with_timeout(&host, port, timeout)?;
    addresses.sort_unstable();
    addresses.dedup();
    validate_public_addresses(&addresses).map_err(|error| AcquisitionError::Network {
        url: url.to_string(),
        reason: error.reason,
    })?;
    let pinned_address = addresses[0];
    Ok(AddressProvenance {
        host,
        resolved_addresses: addresses,
        pinned_address,
    })
}

/// Accumulated per-package download accounting.
#[derive(Debug, Default)]
struct DownloadBudget {
    /// Bytes already accepted for this package base and commit.
    used: u64,
}

impl DownloadBudget {
    /// Return bytes still available before the package cap is reached.
    const fn remaining(&self, limit: u64) -> u64 {
        limit.saturating_sub(self.used)
    }

    /// Charge accepted bytes and report whether the package cap still holds.
    fn charge(&mut self, bytes: u64, limit: u64) -> Result<(), String> {
        let next = self.used.saturating_add(bytes);
        if next > limit {
            return Err(format!(
                "package download budget of {limit} bytes was exhausted"
            ));
        }
        self.used = next;
        Ok(())
    }
}

/// Borrowed seams shared by every acquisition step.
struct Seams<'a> {
    /// Resolved absolute Git executable shared by recipe and upstream acquisition.
    git_executable: &'a Path,
    /// Explicit validated HTTPS proxy applied to every Git transport invocation.
    https_proxy: Option<&'a str>,
    /// Bounded HTTPS transport.
    http: &'a mut dyn HttpFetcher,
    /// DNS answers validated at every hop.
    resolver: &'a mut dyn AddressResolver,
    /// Isolated direct-argv Git process seam.
    git: &'a mut dyn GitCommandRunner,
    /// Isolated exact-fingerprint signature verifier.
    verifier: &'a mut dyn SignatureVerifier,
}

/// Mutable per-source acquisition state shared across the source loop.
struct SourceContext<'a> {
    /// Private source snapshot root.
    root: &'a Path,
    /// Effective limits for this run.
    limits: AcquisitionLimits,
    /// Declared full fingerprints accepted for this package.
    fingerprints: &'a [String],
    /// Detached signature bodies keyed by the file they cover.
    signatures: &'a BTreeMap<String, Vec<u8>>,
    /// Sources for which a paired signature and validpgpkeys make verification mandatory.
    required_signatures: &'a BTreeSet<String>,
    /// Aggregate download accounting for this package and commit.
    budget: &'a mut DownloadBudget,
    /// Aggregate expanded regular-file bytes accepted across all declared sources.
    expanded_bytes: u64,
    /// Aggregate analyzed UTF-8 bytes retained across all declared sources.
    analyzed_bytes: u64,
    /// Manifest entries accumulated for the source snapshot.
    manifest: Vec<ManifestEntry>,
    /// Mutable Git refs resolved during advisory acquisition.
    mutable_sources: Vec<MutableSourceIdentity>,
}

/// What: Acquire the immutable recipe and every declared source for one frozen commit.
///
/// Inputs:
/// - `request`: Frozen scan identity, package name, commit, RPC data, limits, and dry-run.
/// - `parent`: Parent directory for the private ephemeral workspace.
/// - `executable`: Resolved absolute Git executable.
/// - `http`, `resolver`, `git`, `verifier`: Injectable acquisition seams.
///
/// Output:
/// - Snapshot roots, bounded prompt input, frozen identity, exact evidence, manifests,
///   coverage notes, and provenance.
///
/// Details:
/// - Package code is never executed. The recipe tree is exported with `git archive` and
///   materialized entry by entry; `PKGBUILD`, `prepare()`, `makepkg`, and helpers never run.
/// - Only `.SRCINFO` declarations are fetched. `.gitmodules`, lockfiles, installers, and
///   scripts are never followed.
/// - Any unsupported transport, mutable Git identity, missing strong checksum, or
///   unavailable required verification downgrades the outcome to `Incomplete`; a checksum
///   mismatch or a failed signature makes it `Failed`.
/// - In dry-run the same read-only flow runs, the workspace is cleaned before returning,
///   and no snapshot root is published, so Pi can never be launched against the preview.
///
/// # Errors
/// - Returns `Err` when identity cannot be proven, a seam fails hard, or the private
///   workspace cannot be prepared.
pub fn acquire_package(
    request: &AcquisitionRequest,
    parent: &Path,
    executable: &Path,
    http: &mut dyn HttpFetcher,
    resolver: &mut dyn AddressResolver,
    git: &mut dyn GitCommandRunner,
    verifier: &mut dyn SignatureVerifier,
) -> Result<AcquisitionOutcome, AcquisitionError> {
    acquire_package_inner(
        request, executable, parent, http, resolver, git, verifier, None,
    )
}

/// Acquire a package with one explicit credential-free HTTPS proxy for HTTP and Git.
///
/// # Errors
/// - Returns the same bounded acquisition errors as [`acquire_package`], plus proxy validation
///   failures before any external process or request starts.
#[allow(clippy::too_many_arguments)]
pub fn acquire_package_with_https_proxy(
    request: &AcquisitionRequest,
    parent: &Path,
    executable: &Path,
    http: &mut dyn HttpFetcher,
    resolver: &mut dyn AddressResolver,
    git: &mut dyn GitCommandRunner,
    verifier: &mut dyn SignatureVerifier,
    https_proxy: &str,
) -> Result<AcquisitionOutcome, AcquisitionError> {
    let canonical_proxy =
        validate_https_url(https_proxy, false).map_err(|error| AcquisitionError::Network {
            url: https_proxy.to_string(),
            reason: error.reason,
        })?;
    acquire_package_inner(
        request,
        executable,
        parent,
        http,
        resolver,
        git,
        verifier,
        Some(canonical_proxy.as_str()),
    )
}

/// Shared acquisition implementation with optional explicit Git proxy policy.
#[allow(clippy::too_many_arguments)]
fn acquire_package_inner(
    request: &AcquisitionRequest,
    executable: &Path,
    parent: &Path,
    http: &mut dyn HttpFetcher,
    resolver: &mut dyn AddressResolver,
    git: &mut dyn GitCommandRunner,
    verifier: &mut dyn SignatureVerifier,
    https_proxy: Option<&str>,
) -> Result<AcquisitionOutcome, AcquisitionError> {
    if !executable.is_absolute() {
        return Err(AcquisitionError::Git {
            source: ObserverError::GitUnavailable {
                reason: "the resolved git path is not absolute".to_string(),
            },
        });
    }
    let limits = request.limits.clamped();
    let package_base = resolve_package_base(&request.package_name, &request.rpc)?;
    let workspace = EphemeralWorkspace::create(
        parent,
        &format!("pacsea-scan-{}", sanitized_run_name(&request.scan_id)),
    )?;
    let mut seams = Seams {
        git_executable: executable,
        https_proxy,
        http,
        resolver,
        git,
        verifier,
    };
    let recipe = acquire_recipe(
        &mut seams,
        &workspace,
        executable,
        &package_base,
        &request.commit_oid,
        limits,
    )?;
    prove_srcinfo_membership(&recipe.srcinfo, &request.package_name, &package_base)?;

    let mut coverage = Vec::new();
    coverage.extend(recipe.notes.clone());
    let sources = acquire_sources(
        &mut seams,
        &workspace,
        &recipe.srcinfo,
        limits,
        &mut coverage,
    )?;

    finish_outcome(FinishInput {
        request,
        package_base: &package_base,
        recipe,
        sources,
        coverage,
        workspace,
    })
}

/// Derive a filesystem-safe unique run directory name from the scan id.
fn sanitized_run_name(scan_id: &str) -> String {
    let filtered: String = scan_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(48)
        .collect();
    if filtered.is_empty() {
        "run".to_string()
    } else {
        filtered
    }
}

/// Result of materializing one immutable recipe tree.
struct RecipeAcquisition {
    /// Private recipe snapshot root.
    root: PathBuf,
    /// Canonical recipe manifest.
    manifest: CanonicalManifest,
    /// Strictly parsed immutable recipe metadata.
    srcinfo: SrcInfo,
    /// Analyzed text entries retained for exact-evidence lookups.
    analyzed: Vec<(String, String)>,
    /// Coverage limitations observed while materializing the recipe.
    notes: Vec<String>,
}

/// Create the private repository, fetch exactly one commit, and materialize its tree.
fn acquire_recipe(
    seams: &mut Seams<'_>,
    workspace: &EphemeralWorkspace,
    executable: &Path,
    package_base: &PackageBase,
    commit: &CommitOid,
    limits: AcquisitionLimits,
) -> Result<RecipeAcquisition, AcquisitionError> {
    let root = workspace.root()?;
    let repository_dir = root.join("repo");
    create_private_dir(&repository_dir)?;
    let snapshot_root = root.join(RECIPE_SNAPSHOT_ID);
    create_private_dir(&snapshot_root)?;

    let executable_os = executable.as_os_str();
    let repository_os = repository_dir.as_os_str();
    let repo_url = AurRepoUrl::for_package_base(package_base);

    run_git(
        seams.git,
        &init_repository_invocation(executable_os, repository_os, limits.git_timeout),
        seams.https_proxy,
        "init",
        MAX_RECIPE_ARCHIVE_BYTES,
    )?;
    run_git(
        seams.git,
        &fetch_commit_invocation(
            executable_os,
            repository_os,
            &repo_url,
            commit,
            limits.git_timeout,
        ),
        seams.https_proxy,
        "fetch",
        MAX_RECIPE_ARCHIVE_BYTES,
    )?;
    let tar_bytes = run_git(
        seams.git,
        &archive_tree_invocation(executable_os, repository_os, commit, limits.git_timeout),
        seams.https_proxy,
        "archive",
        MAX_RECIPE_ARCHIVE_BYTES,
    )?;

    let inspection = inspect_source(
        "recipe-tree.tar",
        &tar_bytes,
        ArchiveFormat::Tar,
        limits.archive,
    );
    let notes = if inspection.status == AcquisitionStatus::Complete {
        Vec::new()
    } else {
        inspection
            .reasons
            .into_iter()
            .map(|reason| format!("recipe tree is incomplete: {reason}"))
            .collect()
    };
    let entries = materialize_tar(&tar_bytes, limits.archive)?;
    let manifest_entries = write_entries(&snapshot_root, RECIPE_CATEGORY, &entries)?;
    let srcinfo_bytes = entries
        .iter()
        .find(|entry| entry.path == ".SRCINFO")
        .map(|entry| entry.bytes.clone())
        .ok_or_else(|| AcquisitionError::RecipeInvalid {
            source: SrcInfoError {
                line: 0,
                reason: "the recipe tree contains no .SRCINFO".to_string(),
            },
        })?;
    let srcinfo_text =
        std::str::from_utf8(&srcinfo_bytes).map_err(|_| AcquisitionError::RecipeInvalid {
            source: SrcInfoError {
                line: 0,
                reason: ".SRCINFO is not valid UTF-8".to_string(),
            },
        })?;
    let srcinfo =
        parse_srcinfo(srcinfo_text).map_err(|source| AcquisitionError::RecipeInvalid { source })?;

    let mut analyzed_bytes = 0;
    let analyzed = analyzed_entries_bounded(&entries, &mut analyzed_bytes);
    Ok(RecipeAcquisition {
        root: snapshot_root,
        manifest: CanonicalManifest::new(manifest_entries),
        srcinfo,
        analyzed,
        notes,
    })
}

/// Retain UTF-8 evidence without exceeding per-entry or per-package memory bounds.
fn analyzed_entries_bounded(
    entries: &[MaterializedEntry],
    retained_bytes: &mut u64,
) -> Vec<(String, String)> {
    let mut analyzed = Vec::new();
    for entry in entries {
        if entry.bytes.len() > MAX_ANALYZED_TEXT_BYTES {
            continue;
        }
        let Some(next) = retained_bytes.checked_add(entry.bytes.len() as u64) else {
            break;
        };
        if next > MAX_ANALYZED_PACKAGE_BYTES {
            break;
        }
        let Ok(text) = std::str::from_utf8(&entry.bytes) else {
            continue;
        };
        *retained_bytes = next;
        analyzed.push((entry.path.clone(), text.to_string()));
    }
    analyzed
}

/// Result of acquiring every declared source for one commit.
struct SourceAcquisition {
    /// Private source snapshot root.
    root: PathBuf,
    /// Canonical source manifest.
    manifest: CanonicalManifest,
    /// Analyzed text entries retained for exact-evidence lookups.
    analyzed: Vec<(String, String)>,
    /// Per-source provenance in declaration order.
    outcomes: Vec<SourceOutcome>,
    /// Aggregate accepted download bytes.
    downloaded_bytes: u64,
    /// Mutable Git refs resolved for later staleness checks.
    mutable_sources: Vec<MutableSourceIdentity>,
}

/// Paired detached-signature downloads prepared before covered-source evaluation.
struct SignatureArtifacts {
    /// Signature body keyed by the effective name it covers.
    signatures: BTreeMap<String, Vec<u8>>,
    /// Effective names for which signature verification is mandatory.
    required: BTreeSet<String>,
    /// Signature declaration outcome keyed by its own effective name.
    outcomes: BTreeMap<String, SourceOutcome>,
}

/// Acquire every declared source, recording an explicit outcome for each declaration.
fn acquire_sources(
    seams: &mut Seams<'_>,
    workspace: &EphemeralWorkspace,
    srcinfo: &SrcInfo,
    limits: AcquisitionLimits,
    coverage: &mut Vec<String>,
) -> Result<SourceAcquisition, AcquisitionError> {
    let snapshot_root = workspace.root()?.join(SOURCE_SNAPSHOT_ID);
    create_private_dir(&snapshot_root)?;

    let mut budget = DownloadBudget::default();
    let mut artifacts = acquire_signature_artifacts(seams, srcinfo, limits, &mut budget);
    let mut context = SourceContext {
        root: &snapshot_root,
        limits,
        fingerprints: &srcinfo.valid_pgp_keys,
        signatures: &artifacts.signatures,
        required_signatures: &artifacts.required,
        budget: &mut budget,
        expanded_bytes: 0,
        analyzed_bytes: 0,
        manifest: Vec::new(),
        mutable_sources: Vec::new(),
    };
    let mut outcomes = Vec::new();
    let mut analyzed = Vec::new();

    for source in srcinfo.sources.iter().take(limits.declared_sources) {
        let (outcome, entries) =
            if source.detached_signature_for.is_some() && !srcinfo.valid_pgp_keys.is_empty() {
                (
                    artifacts
                        .outcomes
                        .remove(&source.effective_name)
                        .unwrap_or_else(|| {
                            incomplete_source(
                                source,
                                vec!["paired detached signature was not acquired".to_string()],
                            )
                        }),
                    Vec::new(),
                )
            } else {
                acquire_one_source(seams, &mut context, source)
            };
        if outcome.status != AcquisitionStatus::Complete {
            coverage.push(format!(
                "source `{}` is {:?}: {}",
                source.effective_name,
                outcome.status,
                outcome.reasons.join("; ")
            ));
        }
        analyzed.extend(analyzed_entries_bounded(
            &entries,
            &mut context.analyzed_bytes,
        ));
        outcomes.push(outcome);
    }
    if srcinfo.sources.len() > limits.declared_sources {
        coverage.push(format!(
            "only the first {} of {} declared sources were acquired",
            limits.declared_sources,
            srcinfo.sources.len()
        ));
    }

    let manifest = CanonicalManifest::new(std::mem::take(&mut context.manifest));
    let mutable_sources = std::mem::take(&mut context.mutable_sources);
    drop(context);
    Ok(SourceAcquisition {
        root: snapshot_root,
        manifest,
        analyzed,
        outcomes,
        downloaded_bytes: budget.used,
        mutable_sources,
    })
}

/// Fetch paired detached signatures before their covered sources are evaluated.
fn acquire_signature_artifacts(
    seams: &mut Seams<'_>,
    srcinfo: &SrcInfo,
    limits: AcquisitionLimits,
    budget: &mut DownloadBudget,
) -> SignatureArtifacts {
    let mut artifacts = SignatureArtifacts {
        signatures: BTreeMap::new(),
        required: BTreeSet::new(),
        outcomes: BTreeMap::new(),
    };
    if srcinfo.valid_pgp_keys.is_empty() {
        return artifacts;
    }
    for source in srcinfo.sources.iter().take(limits.declared_sources) {
        let Some(covered) = source.detached_signature_for.as_ref() else {
            continue;
        };
        artifacts.required.insert(covered.clone());
        let locator = source
            .value
            .split_once("::")
            .map_or(source.value.as_str(), |(_, locator)| locator);
        let SourceLocator::StaticHttps { url } = classify_source_locator(locator) else {
            artifacts.outcomes.insert(
                source.effective_name.clone(),
                incomplete_source(
                    source,
                    vec!["detached signature must use static HTTPS".to_string()],
                ),
            );
            continue;
        };
        let remaining = budget.remaining(limits.package_bytes);
        if remaining == 0 {
            artifacts.outcomes.insert(
                source.effective_name.clone(),
                incomplete_source(
                    source,
                    vec![format!(
                        "package download budget of {} bytes was exhausted",
                        limits.package_bytes
                    )],
                ),
            );
            continue;
        }
        let fetched = download_static_source(
            seams.http,
            seams.resolver,
            &url,
            limits.source_bytes.min(remaining),
            limits.http_timeout,
        );
        let Ok(downloaded) = fetched else {
            artifacts.outcomes.insert(
                source.effective_name.clone(),
                incomplete_source(
                    source,
                    vec!["paired detached signature download is unavailable".to_string()],
                ),
            );
            continue;
        };
        let DownloadedSource {
            bytes,
            redirect_chain,
            address_provenance,
        } = downloaded;
        if let Err(reason) = budget.charge(bytes.len() as u64, limits.package_bytes) {
            artifacts.outcomes.insert(
                source.effective_name.clone(),
                incomplete_source(source, vec![reason]),
            );
            continue;
        }
        artifacts.outcomes.insert(
            source.effective_name.clone(),
            SourceOutcome {
                declaration: source.value.clone(),
                effective_name: source.effective_name.clone(),
                status: AcquisitionStatus::Complete,
                redirect_chain,
                address_provenance,
                bytes: bytes.len() as u64,
                signature: SignatureStatus::NotRequired,
                reasons: vec![format!(
                    "detached signature is bound to `{covered}` and retained only for verification"
                )],
            },
        );
        artifacts.signatures.insert(covered.clone(), bytes);
    }
    artifacts
}

/// Acquire exactly one declared source and classify its outcome.
fn acquire_one_source(
    seams: &mut Seams<'_>,
    context: &mut SourceContext<'_>,
    source: &RecipeSource,
) -> (SourceOutcome, Vec<MaterializedEntry>) {
    let locator_text = source
        .value
        .split_once("::")
        .map_or(source.value.as_str(), |(_, locator)| locator);
    match classify_source_locator(locator_text) {
        SourceLocator::Incomplete {
            declaration,
            reason,
        } => (
            incomplete_source(
                source,
                vec![format!("{declaration} is unsupported: {reason}")],
            ),
            Vec::new(),
        ),
        SourceLocator::GitHttps {
            repository_url,
            commit_oid,
        } => acquire_git_source(seams, context, source, &repository_url, &commit_oid),
        SourceLocator::MutableGitHttps {
            declaration,
            repository_url,
            reference,
        } => acquire_mutable_git_source(
            seams,
            context,
            source,
            &declaration,
            &repository_url,
            &reference,
        ),
        SourceLocator::StaticHttps { url } => acquire_static_source(seams, context, source, &url),
    }
}

/// Resolve one mutable Git ref for advisory scanning, then acquire its exact observed OID.
fn acquire_mutable_git_source(
    seams: &mut Seams<'_>,
    context: &mut SourceContext<'_>,
    source: &RecipeSource,
    declaration: &str,
    repository_url: &str,
    reference: &str,
) -> (SourceOutcome, Vec<MaterializedEntry>) {
    let address =
        match verify_public_destination(seams.resolver, repository_url, context.limits.git_timeout)
        {
            Ok(address) => address,
            Err(error) => {
                return (
                    incomplete_source(source, vec![error.to_string()]),
                    Vec::new(),
                );
            }
        };
    let invocation = resolve_mutable_source_invocation(
        seams.git_executable.as_os_str(),
        repository_url,
        reference,
        &address,
        context.limits.git_timeout,
    );
    let output = match run_git(
        seams.git,
        &invocation,
        seams.https_proxy,
        "resolve mutable upstream ref",
        64 * 1024,
    ) {
        Ok(output) => output,
        Err(error) => {
            return (
                incomplete_source(source, vec![error.to_string()]),
                Vec::new(),
            );
        }
    };
    let resolved_oid = match parse_mutable_ref_oid(&output, reference) {
        Ok(oid) => oid,
        Err(reason) => return (incomplete_source(source, vec![reason]), Vec::new()),
    };
    context.mutable_sources.push(MutableSourceIdentity {
        declaration: declaration.to_string(),
        repository_url: repository_url.to_string(),
        reference: reference.to_string(),
        resolved_oid: resolved_oid.clone(),
    });
    let (mut outcome, entries) =
        acquire_git_source(seams, context, source, repository_url, &resolved_oid);
    outcome.status = worst_status(outcome.status, AcquisitionStatus::Incomplete);
    outcome.reasons.push(format!(
        "mutable Git ref {reference} resolved to {resolved_oid} for advisory analysis and must be rechecked before continuation"
    ));
    outcome.reasons.sort();
    outcome.reasons.dedup();
    (outcome, entries)
}

/// Parse exactly one full OID matching a requested `ls-remote` ref.
fn parse_mutable_ref_oid(output: &[u8], reference: &str) -> Result<CommitOid, String> {
    let text = std::str::from_utf8(output)
        .map_err(|_| "mutable Git ref resolution returned non-UTF-8 output".to_string())?;
    let mut matching = text.lines().filter_map(|line| {
        let (oid, observed_ref) = line.split_once(char::is_whitespace)?;
        (observed_ref.trim() == reference).then_some(oid)
    });
    let first = matching
        .next()
        .ok_or_else(|| format!("mutable Git ref {reference} did not resolve to an exact OID"))?;
    if matching.next().is_some() {
        return Err(format!(
            "mutable Git ref {reference} resolved ambiguously; advisory acquisition stopped"
        ));
    }
    CommitOid::new(first).map_err(|error| error.to_string())
}

/// Re-resolve one mutable advisory Git identity under the same DNS/Git policy.
///
/// # Errors
/// - Returns when destination validation, Git execution, or exact OID parsing fails.
pub fn mutable_source_identity_changed(
    identity: &MutableSourceIdentity,
    git_executable: &Path,
    resolver: &mut dyn AddressResolver,
    git: &mut dyn GitCommandRunner,
    https_proxy: Option<&str>,
    timeout: Duration,
) -> Result<bool, AcquisitionError> {
    let address = verify_public_destination(resolver, &identity.repository_url, timeout)?;
    let invocation = resolve_mutable_source_invocation(
        git_executable.as_os_str(),
        &identity.repository_url,
        &identity.reference,
        &address,
        timeout,
    );
    let output = run_git(
        git,
        &invocation,
        https_proxy,
        "re-resolve mutable upstream ref",
        64 * 1024,
    )?;
    let current = parse_mutable_ref_oid(&output, &identity.reference).map_err(|reason| {
        AcquisitionError::Network {
            url: identity.repository_url.clone(),
            reason,
        }
    })?;
    Ok(current != identity.resolved_oid)
}

/// Fetch, inspect, and materialize one DNS-pinned immutable upstream Git tree.
fn acquire_git_source(
    seams: &mut Seams<'_>,
    context: &mut SourceContext<'_>,
    source: &RecipeSource,
    repository_url: &str,
    commit: &CommitOid,
) -> (SourceOutcome, Vec<MaterializedEntry>) {
    let address =
        match verify_public_destination(seams.resolver, repository_url, context.limits.git_timeout)
        {
            Ok(address) => address,
            Err(error) => {
                return (
                    incomplete_source(source, vec![error.to_string()]),
                    Vec::new(),
                );
            }
        };
    let repository = context.root.parent().unwrap_or(context.root).join(format!(
        "upstream-git-{}",
        sanitized_run_name(&source.effective_name)
    ));
    let acquisition = (|| {
        create_private_dir(&repository)?;
        run_git(
            seams.git,
            &init_repository_invocation(
                seams.git_executable.as_os_str(),
                repository.as_os_str(),
                context.limits.git_timeout,
            ),
            seams.https_proxy,
            "upstream init",
            context.limits.source_bytes,
        )?;
        run_git(
            seams.git,
            &fetch_pinned_source_invocation(
                seams.git_executable.as_os_str(),
                repository.as_os_str(),
                repository_url,
                commit,
                &address,
                context.limits.git_timeout,
            ),
            seams.https_proxy,
            "upstream fetch",
            context.limits.source_bytes,
        )?;
        run_git(
            seams.git,
            &archive_tree_invocation(
                seams.git_executable.as_os_str(),
                repository.as_os_str(),
                commit,
                context.limits.git_timeout,
            ),
            seams.https_proxy,
            "upstream archive",
            context
                .limits
                .source_bytes
                .min(context.budget.remaining(context.limits.package_bytes)),
        )
    })();
    let _ = std::fs::remove_dir_all(&repository);
    let tar_bytes = match acquisition {
        Ok(bytes) => bytes,
        Err(error) => {
            return (
                incomplete_source(source, vec![error.to_string()]),
                Vec::new(),
            );
        }
    };
    if let Err(reason) = context
        .budget
        .charge(tar_bytes.len() as u64, context.limits.package_bytes)
    {
        return (incomplete_source(source, vec![reason]), Vec::new());
    }
    let report = inspect_source(
        &source.effective_name,
        &tar_bytes,
        ArchiveFormat::Tar,
        context.limits.archive,
    );
    let mut outcome = SourceOutcome {
        declaration: source.value.clone(),
        effective_name: source.effective_name.clone(),
        status: report.status,
        redirect_chain: vec![repository_url.to_string()],
        address_provenance: vec![address],
        bytes: tar_bytes.len() as u64,
        signature: SignatureStatus::NotRequired,
        reasons: report.reasons.clone(),
    };
    if report.status != AcquisitionStatus::Complete {
        return (outcome, Vec::new());
    }
    let expanded = context.expanded_bytes.saturating_add(report.expanded_bytes);
    if expanded > context.limits.archive.expanded_bytes {
        outcome.status = AcquisitionStatus::Incomplete;
        outcome.reasons.push(format!(
            "package aggregate expanded bytes exceed the {}-byte limit",
            context.limits.archive.expanded_bytes
        ));
        return (outcome, Vec::new());
    }
    context.expanded_bytes = expanded;
    match materialize_source_entries(context, source, &tar_bytes, ArchiveFormat::Tar) {
        Ok(entries) => (outcome, entries),
        Err(error) => {
            outcome.status = AcquisitionStatus::Incomplete;
            outcome.reasons.push(error.to_string());
            (outcome, Vec::new())
        }
    }
}

/// Download, verify, inspect, and materialize one static HTTPS source.
fn acquire_static_source(
    seams: &mut Seams<'_>,
    context: &mut SourceContext<'_>,
    source: &RecipeSource,
    url: &str,
) -> (SourceOutcome, Vec<MaterializedEntry>) {
    let remaining = context.budget.remaining(context.limits.package_bytes);
    if remaining == 0 {
        return (
            incomplete_source(
                source,
                vec![format!(
                    "package download budget of {} bytes was exhausted",
                    context.limits.package_bytes
                )],
            ),
            Vec::new(),
        );
    }
    let download = download_static_source(
        seams.http,
        seams.resolver,
        url,
        context.limits.source_bytes.min(remaining),
        context.limits.http_timeout,
    );
    let downloaded = match download {
        Ok(value) => value,
        Err(error) => {
            let reason = if remaining < context.limits.source_bytes {
                format!(
                    "package download budget of {} bytes prevented this source: {error}",
                    context.limits.package_bytes
                )
            } else {
                error.to_string()
            };
            return (incomplete_source(source, vec![reason]), Vec::new());
        }
    };
    let DownloadedSource {
        bytes,
        redirect_chain,
        address_provenance,
    } = downloaded;
    if let Err(reason) = context
        .budget
        .charge(bytes.len() as u64, context.limits.package_bytes)
    {
        return (incomplete_source(source, vec![reason]), Vec::new());
    }

    let signature_status = evaluate_signature(seams, context, source, &bytes);
    let integrity = evaluate_integrity(&bytes, &source.checksums, signature_status);
    let mut outcome = SourceOutcome {
        declaration: source.value.clone(),
        effective_name: source.effective_name.clone(),
        status: integrity.status,
        redirect_chain,
        address_provenance,
        bytes: bytes.len() as u64,
        signature: signature_status,
        reasons: integrity.reasons.clone(),
    };
    if integrity.status == AcquisitionStatus::Failed {
        return (outcome, Vec::new());
    }

    let format = classify_archive_format(&source.effective_name);
    let report: InspectionReport = inspect_source(
        &source.effective_name,
        &bytes,
        format,
        context.limits.archive,
    );
    if report.status != AcquisitionStatus::Complete {
        outcome.status = worst_status(outcome.status, report.status);
        outcome.reasons.extend(report.reasons);
        return (outcome, Vec::new());
    }
    let expanded = context.expanded_bytes.saturating_add(report.expanded_bytes);
    if expanded > context.limits.archive.expanded_bytes {
        outcome.status = worst_status(outcome.status, AcquisitionStatus::Incomplete);
        outcome.reasons.push(format!(
            "package aggregate expanded bytes exceed the {}-byte limit",
            context.limits.archive.expanded_bytes
        ));
        return (outcome, Vec::new());
    }
    context.expanded_bytes = expanded;

    match materialize_source_entries(context, source, &bytes, format) {
        Ok(entries) => (outcome, entries),
        Err(error) => {
            outcome.status = worst_status(outcome.status, AcquisitionStatus::Incomplete);
            outcome.reasons.push(error.to_string());
            (outcome, Vec::new())
        }
    }
}

/// Materialize one inspected source under its own confined subdirectory.
fn materialize_source_entries(
    context: &mut SourceContext<'_>,
    source: &RecipeSource,
    bytes: &[u8],
    format: ArchiveFormat,
) -> Result<Vec<MaterializedEntry>, AcquisitionError> {
    let entries = match format {
        ArchiveFormat::Raw => vec![MaterializedEntry {
            path: source.effective_name.clone(),
            bytes: bytes.to_vec(),
            executable: false,
        }],
        ArchiveFormat::Tar => materialize_tar(bytes, context.limits.archive)?,
        ArchiveFormat::Zip => materialize_zip(bytes, context.limits.archive)?,
        ArchiveFormat::TarGzip => materialize_tar(
            &decode_stream(flate2::read::MultiGzDecoder::new(bytes), context.limits)?,
            context.limits.archive,
        )?,
        ArchiveFormat::TarBzip2 => materialize_tar(
            &decode_stream(bzip2::read::MultiBzDecoder::new(bytes), context.limits)?,
            context.limits.archive,
        )?,
        ArchiveFormat::TarXz => materialize_tar(
            &decode_stream(lzma_rust2::XzReader::new(bytes, true), context.limits)?,
            context.limits.archive,
        )?,
        ArchiveFormat::TarZstd => {
            let decoder = zstd::stream::read::Decoder::new(bytes).map_err(|error| {
                AcquisitionError::Network {
                    url: source.effective_name.clone(),
                    reason: format!("invalid zstd stream: {error}"),
                }
            })?;
            materialize_tar(
                &decode_stream(decoder, context.limits)?,
                context.limits.archive,
            )?
        }
        ArchiveFormat::Gzip | ArchiveFormat::Bzip2 | ArchiveFormat::Xz | ArchiveFormat::Zstd => {
            let decoded = decode_standalone(bytes, format, context.limits, source)?;
            vec![MaterializedEntry {
                path: strip_compression_suffix(&source.effective_name).to_string(),
                bytes: decoded,
                executable: false,
            }]
        }
    };
    let prefixed: Vec<MaterializedEntry> = entries
        .into_iter()
        .map(|entry| MaterializedEntry {
            path: format!("{}/{}", source.effective_name, entry.path),
            ..entry
        })
        .collect();
    let manifest_entries = write_entries(context.root, SOURCE_CATEGORY, &prefixed)?;
    context.manifest.extend(manifest_entries);
    Ok(prefixed)
}

/// Decode one standalone compressed stream under the bounded inspection budget.
fn decode_standalone(
    bytes: &[u8],
    format: ArchiveFormat,
    limits: AcquisitionLimits,
    source: &RecipeSource,
) -> Result<Vec<u8>, AcquisitionError> {
    match format {
        ArchiveFormat::Gzip => decode_stream(flate2::read::MultiGzDecoder::new(bytes), limits),
        ArchiveFormat::Bzip2 => decode_stream(bzip2::read::MultiBzDecoder::new(bytes), limits),
        ArchiveFormat::Xz => decode_stream(lzma_rust2::XzReader::new(bytes, true), limits),
        ArchiveFormat::Zstd => {
            let decoder = zstd::stream::read::Decoder::new(bytes).map_err(|error| {
                AcquisitionError::Network {
                    url: source.effective_name.clone(),
                    reason: format!("invalid zstd stream: {error}"),
                }
            })?;
            decode_stream(decoder, limits)
        }
        ArchiveFormat::Raw
        | ArchiveFormat::Tar
        | ArchiveFormat::Zip
        | ArchiveFormat::TarGzip
        | ArchiveFormat::TarBzip2
        | ArchiveFormat::TarXz
        | ArchiveFormat::TarZstd => Ok(bytes.to_vec()),
    }
}

/// Read a decoder to completion under a hard expanded-byte ceiling.
fn decode_stream(
    mut reader: impl Read,
    limits: AcquisitionLimits,
) -> Result<Vec<u8>, AcquisitionError> {
    let ceiling = limits.archive.expanded_bytes;
    let mut output = Vec::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| AcquisitionError::Network {
                url: "compressed source".to_string(),
                reason: format!("decode failed: {error}"),
            })?;
        if read == 0 {
            return Ok(output);
        }
        if (output.len() as u64).saturating_add(read as u64) > ceiling {
            return Err(AcquisitionError::Network {
                url: "compressed source".to_string(),
                reason: "decoded stream exceeds the bounded inspection buffer".to_string(),
            });
        }
        output.extend_from_slice(&buffer[..read]);
    }
}

/// Strip exactly one supported standalone compression suffix.
fn strip_compression_suffix(name: &str) -> &str {
    for suffix in [".gz", ".bz2", ".xz", ".zst"] {
        if let Some(stripped) = name.strip_suffix(suffix)
            && !stripped.is_empty()
        {
            return stripped;
        }
    }
    name
}

/// Apply signature policy for one source through the isolated verifier seam.
fn evaluate_signature(
    seams: &mut Seams<'_>,
    context: &SourceContext<'_>,
    source: &RecipeSource,
    bytes: &[u8],
) -> SignatureStatus {
    if context.fingerprints.is_empty()
        || !context.required_signatures.contains(&source.effective_name)
    {
        return SignatureStatus::NotRequired;
    }
    let Some(signature) = context.signatures.get(&source.effective_name) else {
        return SignatureStatus::Unavailable;
    };
    seams.verifier.verify(&SignatureRequest {
        data: bytes,
        signature,
        fingerprints: context.fingerprints,
    })
}

/// Build an explicit incomplete source outcome without any acquired bytes.
fn incomplete_source(source: &RecipeSource, reasons: Vec<String>) -> SourceOutcome {
    SourceOutcome {
        declaration: source.value.clone(),
        effective_name: source.effective_name.clone(),
        status: AcquisitionStatus::Incomplete,
        redirect_chain: Vec::new(),
        address_provenance: Vec::new(),
        bytes: 0,
        signature: SignatureStatus::NotRequired,
        reasons,
    }
}

/// Combine two statuses, keeping the strictest one.
const fn worst_status(left: AcquisitionStatus, right: AcquisitionStatus) -> AcquisitionStatus {
    match (left, right) {
        (AcquisitionStatus::Failed, _) | (_, AcquisitionStatus::Failed) => {
            AcquisitionStatus::Failed
        }
        (AcquisitionStatus::Incomplete, _) | (_, AcquisitionStatus::Incomplete) => {
            AcquisitionStatus::Incomplete
        }
        _ => AcquisitionStatus::Complete,
    }
}

/// Borrowed inputs for assembling the final acquisition outcome.
struct FinishInput<'a> {
    /// Frozen acquisition request.
    request: &'a AcquisitionRequest,
    /// Proven official package base.
    package_base: &'a PackageBase,
    /// Materialized immutable recipe.
    recipe: RecipeAcquisition,
    /// Acquired declared sources.
    sources: SourceAcquisition,
    /// Coverage limitations gathered so far.
    coverage: Vec<String>,
    /// Ephemeral workspace guard transferred into the outcome.
    workspace: EphemeralWorkspace,
}

/// Assemble descriptors, prompt input, evidence, coverage, and provenance.
fn finish_outcome(input: FinishInput<'_>) -> Result<AcquisitionOutcome, AcquisitionError> {
    let FinishInput {
        request,
        package_base,
        recipe,
        sources,
        mut coverage,
        mut workspace,
    } = input;

    let mut status = sources
        .outcomes
        .iter()
        .fold(AcquisitionStatus::Complete, |accumulated, outcome| {
            worst_status(accumulated, outcome.status)
        });
    if !coverage.is_empty() {
        status = worst_status(status, AcquisitionStatus::Incomplete);
    }
    coverage.truncate(MAX_COVERAGE_NOTES);

    let assembly = crate::pi_agent::snapshot::SnapshotAssemblyInput {
        scan_id: &request.scan_id,
        package_base: package_base.as_str(),
        package_names: &recipe
            .srcinfo
            .package_names
            .iter()
            .map(|name| name.as_str().to_string())
            .collect::<Vec<String>>(),
        commit_oid: request.commit_oid.as_str(),
        recipe_root: &recipe.root,
        source_root: &sources.root,
        recipe_manifest: &recipe.manifest,
        source_manifest: &sources.manifest,
        recipe_analyzed: &recipe.analyzed,
        source_analyzed: &sources.analyzed,
        coverage_notes: &coverage,
        publish_roots: !request.dry_run,
    };
    let assembled = crate::pi_agent::snapshot::assemble(&assembly).map_err(|reason| {
        AcquisitionError::Workspace {
            path: recipe.root.clone(),
            reason,
        }
    })?;

    let provenance = AcquisitionProvenance {
        repository_url: AurRepoUrl::for_package_base(package_base)
            .as_str()
            .to_string(),
        commit_oid: request.commit_oid.as_str().to_string(),
        sources: sources.outcomes,
        downloaded_bytes: sources.downloaded_bytes,
        dry_run: request.dry_run,
    };

    if request.dry_run {
        workspace.clean();
    }

    Ok(AcquisitionOutcome {
        status,
        snapshots: assembled.registry,
        prompt: assembled.prompt,
        identity: assembled.identity,
        evidence: assembled.evidence,
        recipe_manifest: recipe.manifest,
        source_manifest: sources.manifest,
        srcinfo: recipe.srcinfo,
        coverage_notes: coverage,
        provenance,
        mutable_sources: sources.mutable_sources,
        workspace,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        AcquisitionError, AcquisitionLimits, AddressResolver, AurRpcData, HttpFetcher, HttpRequest,
        HttpResponse, MAX_PACKAGE_BYTES, MAX_SOURCE_BYTES, classify_archive_format,
        download_static_source, resolve_package_base, strip_compression_suffix, worst_status,
    };
    use crate::logic::pi_scan::identity::PackageName;
    use crate::logic::pi_scan::source::{AcquisitionStatus, ArchiveFormat};
    use std::collections::VecDeque;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Duration;

    /// Scripted fetcher returning canned single-hop responses.
    struct FakeHttp {
        /// Remaining scripted responses in order.
        scripted: VecDeque<HttpResponse>,
        /// URLs actually requested, in order.
        seen: Vec<String>,
    }

    impl HttpFetcher for FakeHttp {
        fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, AcquisitionError> {
            self.seen.push(request.url.clone());
            self.scripted
                .pop_front()
                .ok_or_else(|| AcquisitionError::Network {
                    url: request.url.clone(),
                    reason: "the test script ran out of responses".to_string(),
                })
        }
    }

    /// Resolver returning one fixed public address for every host.
    struct PublicResolver;

    impl AddressResolver for PublicResolver {
        fn resolve(&mut self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
            Ok(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))])
        }
    }

    /// Build a terminal 200 response.
    fn ok_body(body: &[u8]) -> HttpResponse {
        HttpResponse {
            status: 200,
            location: None,
            body: body.to_vec(),
        }
    }

    #[test]
    fn package_base_resolution_requires_declared_metadata() {
        let name = PackageName::new("yay-bin").expect("valid name");
        let rpc = AurRpcData::from_pairs(&[("yay-bin", "yay")]);
        let base = resolve_package_base(&name, &rpc).expect("declared base");
        assert_eq!(base.as_str(), "yay");
        assert!(resolve_package_base(&name, &AurRpcData::default()).is_err());
    }

    #[test]
    fn redirects_are_bounded_and_recorded() {
        let mut http = FakeHttp {
            scripted: VecDeque::from(vec![
                HttpResponse {
                    status: 302,
                    location: Some("https://cdn.example.com/file.tar.gz".to_string()),
                    body: Vec::new(),
                },
                ok_body(b"payload"),
            ]),
            seen: Vec::new(),
        };
        let downloaded = download_static_source(
            &mut http,
            &mut PublicResolver,
            "https://example.com/file.tar.gz",
            MAX_SOURCE_BYTES,
            Duration::from_secs(5),
        )
        .expect("bounded redirect");
        assert_eq!(downloaded.bytes, b"payload");
        assert_eq!(downloaded.redirect_chain.len(), 2);
        assert_eq!(downloaded.address_provenance.len(), 2);
        assert_eq!(
            downloaded.redirect_chain[1],
            "https://cdn.example.com/file.tar.gz"
        );
    }

    #[test]
    fn plain_http_redirect_targets_are_refused() {
        let mut http = FakeHttp {
            scripted: VecDeque::from(vec![HttpResponse {
                status: 302,
                location: Some("http://example.com/file".to_string()),
                body: Vec::new(),
            }]),
            seen: Vec::new(),
        };
        let error = download_static_source(
            &mut http,
            &mut PublicResolver,
            "https://example.com/file",
            MAX_SOURCE_BYTES,
            Duration::from_secs(5),
        )
        .expect_err("downgrade must be refused");
        assert!(matches!(error, AcquisitionError::Network { .. }));
    }

    #[test]
    fn archive_formats_are_classified_by_filename() {
        assert_eq!(
            classify_archive_format("src.tar.zst"),
            ArchiveFormat::TarZstd
        );
        assert_eq!(classify_archive_format("src.zip"), ArchiveFormat::Zip);
        assert_eq!(classify_archive_format("PKGBUILD"), ArchiveFormat::Raw);
    }

    #[test]
    fn compression_suffix_stripping_keeps_a_non_empty_name() {
        assert_eq!(strip_compression_suffix("notes.txt.gz"), "notes.txt");
        assert_eq!(strip_compression_suffix(".gz"), ".gz");
    }

    #[test]
    fn status_combination_keeps_the_strictest_outcome() {
        assert_eq!(
            worst_status(AcquisitionStatus::Complete, AcquisitionStatus::Incomplete),
            AcquisitionStatus::Incomplete
        );
        assert_eq!(
            worst_status(AcquisitionStatus::Incomplete, AcquisitionStatus::Failed),
            AcquisitionStatus::Failed
        );
    }

    #[test]
    fn limits_clamp_down_but_never_up() {
        let raised = AcquisitionLimits {
            source_bytes: MAX_SOURCE_BYTES * 4,
            package_bytes: MAX_PACKAGE_BYTES * 4,
            ..AcquisitionLimits::default()
        }
        .clamped();
        assert_eq!(raised.source_bytes, MAX_SOURCE_BYTES);
        assert_eq!(raised.package_bytes, MAX_PACKAGE_BYTES);
    }
}
