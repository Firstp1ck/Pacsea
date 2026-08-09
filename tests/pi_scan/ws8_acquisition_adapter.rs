//! Adversarial WS8 acquisition-adapter tests driven entirely by deterministic fakes.
//!
//! No test here performs real network, Git, `GnuPG`, or Pi work. Every external effect is
//! injected through the adapter's seams, so the whole acquisition flow — identity proof,
//! recipe materialization, redirect policy, integrity policy, archive materialization,
//! cleanup, and dry-run isolation — is exercised offline and deterministically.

use pacsea::logic::pi_scan::acquisition::{
    AcquisitionError, AcquisitionLimits, AcquisitionRequest, AddressResolver, AurRpcData,
    HttpFetcher, HttpRequest, HttpResponse, MAX_PACKAGE_BYTES, MAX_SOURCE_BYTES,
    RECIPE_SNAPSHOT_ID, SOURCE_SNAPSHOT_ID, SignatureRequest, SignatureVerifier,
    UnavailableSignatureVerifier, acquire_package, archive_tree_invocation,
    classify_archive_format, download_static_source, fetch_commit_invocation,
    init_repository_invocation, prove_srcinfo_membership, resolve_package_base,
};
use pacsea::logic::pi_scan::identity::{CommitOid, PackageBase, PackageName};
use pacsea::logic::pi_scan::observer::{GitCommandRunner, GitInvocation, GitOutput, ObserverError};
use pacsea::logic::pi_scan::recipe::parse_srcinfo;
use pacsea::logic::pi_scan::source::{AcquisitionStatus, ArchiveFormat, SignatureStatus};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr};
use std::path::Path;
use std::time::Duration;

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

/// Build a deterministic tar archive from regular-file path/content pairs.
fn tar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut output = Vec::new();
    {
        let mut builder = tar::Builder::new(&mut output);
        for (path, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder
                .append_data(&mut header, path, Cursor::new(*content))
                .expect("append tar entry");
        }
        builder.finish().expect("finish tar");
    }
    output
}

/// A `.SRCINFO` document declaring one static HTTPS source with a matching SHA-256.
fn srcinfo_with_source(digest: &str) -> String {
    format!(
        "pkgbase = demo\npkgname = demo\nsource = https://example.com/demo.tar\nsha256sums = {digest}\n"
    )
}

/// Scripted Git runner that answers by subcommand and records every invocation.
struct FakeGit {
    /// Responses keyed by the recognized subcommand.
    scripted: BTreeMap<String, GitOutput>,
    /// Every invocation observed, in order.
    seen: Vec<GitInvocation>,
}

impl FakeGit {
    /// Build a runner that succeeds for init/fetch and returns the given recipe archive.
    fn with_recipe(archive: Vec<u8>) -> Self {
        let mut scripted = BTreeMap::new();
        scripted.insert("init".to_string(), ok(Vec::new()));
        scripted.insert("fetch".to_string(), ok(Vec::new()));
        scripted.insert("archive".to_string(), ok(archive));
        Self {
            scripted,
            seen: Vec::new(),
        }
    }

    /// Return the argv of every recorded invocation.
    fn argvs(&self) -> Vec<Vec<String>> {
        self.seen
            .iter()
            .map(pacsea::logic::pi_scan::observer::GitInvocation::argv_strings)
            .collect()
    }
}

impl GitCommandRunner for FakeGit {
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError> {
        self.seen.push(invocation.clone());
        let argv = invocation.argv_strings();
        let subcommand = ["init", "fetch", "archive", "ls-remote"]
            .into_iter()
            .find(|candidate| argv.iter().any(|arg| arg == candidate))
            .unwrap_or("unknown");
        self.scripted
            .get(subcommand)
            .cloned()
            .ok_or_else(|| ObserverError::GitCommand {
                operation: subcommand.to_string(),
                reason: "no scripted response".to_string(),
            })
    }
}

/// Build a successful Git output.
const fn ok(stdout: Vec<u8>) -> GitOutput {
    GitOutput {
        success: true,
        stdout,
        stderr: String::new(),
    }
}

/// Scripted single-hop HTTP fetcher.
struct FakeHttp {
    /// Remaining scripted responses in order.
    scripted: VecDeque<HttpResponse>,
    /// URLs actually requested, in order.
    seen: Vec<String>,
}

impl FakeHttp {
    /// Build a fetcher returning one terminal 200 body.
    fn single(body: Vec<u8>) -> Self {
        Self {
            scripted: VecDeque::from(vec![HttpResponse {
                status: 200,
                location: None,
                body,
            }]),
            seen: Vec::new(),
        }
    }
}

impl HttpFetcher for FakeHttp {
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, AcquisitionError> {
        self.seen.push(request.url.clone());
        self.scripted
            .pop_front()
            .ok_or_else(|| AcquisitionError::Network {
                url: request.url.clone(),
                reason: "no scripted response".to_string(),
            })
    }
}

/// Resolver answering with one fixed public address.
struct PublicResolver;

impl AddressResolver for PublicResolver {
    fn resolve(&mut self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))])
    }
}

/// Resolver answering with a loopback address to exercise the address policy.
struct LoopbackResolver;

impl AddressResolver for LoopbackResolver {
    fn resolve(&mut self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
        Ok(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)])
    }
}

/// Verifier that always reports a failed signature.
struct FailingVerifier;

impl SignatureVerifier for FailingVerifier {
    fn verify(&mut self, _request: &SignatureRequest<'_>) -> SignatureStatus {
        SignatureStatus::Failed
    }
}

/// Build a standard acquisition request for the demo package.
fn request(scan_id: &str, dry_run: bool) -> AcquisitionRequest {
    AcquisitionRequest {
        scan_id: scan_id.to_string(),
        package_name: PackageName::new("demo").expect("valid name"),
        commit_oid: CommitOid::new("a".repeat(40)).expect("valid oid"),
        rpc: AurRpcData::from_pairs(&[("demo", "demo")]),
        limits: AcquisitionLimits::default(),
        dry_run,
    }
}

/// Build the recipe tar plus the matching HTTP body for a complete acquisition.
fn complete_fixture() -> (Vec<u8>, Vec<u8>) {
    let payload = tar_bytes(&[("inner.txt", b"hello source")]);
    let digest = hex(&Sha256::digest(&payload));
    let srcinfo = srcinfo_with_source(&digest);
    let recipe = tar_bytes(&[
        (".SRCINFO", srcinfo.as_bytes()),
        ("PKGBUILD", b"pkgname=demo\n"),
    ]);
    (recipe, payload)
}

#[test]
fn ws8_complete_acquisition_publishes_bound_snapshots_and_evidence() {
    let temp = tempfile::tempdir().expect("temp parent");
    let (recipe, payload) = complete_fixture();
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let outcome = acquire_package(
        &request("scan-complete", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition succeeds");

    assert_eq!(outcome.status, AcquisitionStatus::Complete);
    assert_eq!(outcome.identity.package_base, "demo");
    assert_eq!(outcome.identity.commit_oid, "a".repeat(40));
    assert!(
        outcome
            .snapshots
            .root(RECIPE_SNAPSHOT_ID)
            .expect("recipe root published")
            .is_dir()
    );
    assert!(
        outcome
            .snapshots
            .root(SOURCE_SNAPSHOT_ID)
            .expect("source root published")
            .is_dir()
    );
    assert!(
        outcome
            .recipe_manifest
            .find_entry("recipe", "PKGBUILD")
            .is_some(),
        "the recipe manifest must byte-hash every materialized entry"
    );
    assert!(
        outcome
            .source_manifest
            .find_entry("source", "demo.tar/inner.txt")
            .is_some(),
        "source entries are materialized under their declaring source"
    );
    assert_eq!(
        outcome.evidence.content(RECIPE_SNAPSHOT_ID, "PKGBUILD"),
        Some("pkgname=demo\n")
    );
    assert_eq!(
        outcome.provenance.downloaded_bytes,
        outcome.provenance.sources[0].bytes
    );
    assert!(!outcome.provenance.dry_run);
}

#[test]
fn ws8_git_invocations_are_direct_argv_without_hooks_submodules_or_credentials() {
    let temp = tempfile::tempdir().expect("temp parent");
    let (recipe, payload) = complete_fixture();
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let _outcome = acquire_package(
        &request("scan-argv", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition succeeds");

    let argvs = git.argvs();
    assert!(!argvs.is_empty(), "git must actually be invoked");
    for argv in &argvs {
        assert!(
            argv.contains(&"core.hooksPath=/dev/null".to_string()),
            "hooks must be disabled in every invocation"
        );
        assert!(
            argv.contains(&"credential.helper=".to_string()),
            "credential helpers must be disabled in every invocation"
        );
        assert!(
            argv.contains(&"http.proxy=".to_string()),
            "ambient proxies must be disabled in every invocation"
        );
        assert!(
            !argv
                .iter()
                .any(|arg| arg == "sh" || arg == "-c" && arg.len() > 2),
            "no shell form is ever constructed"
        );
    }
    let fetch = argvs
        .iter()
        .find(|argv| argv.contains(&"fetch".to_string()))
        .expect("a fetch invocation");
    assert!(fetch.contains(&"--no-recurse-submodules".to_string()));
    assert!(fetch.contains(&"--no-tags".to_string()));

    for environment in git
        .seen
        .iter()
        .map(|invocation| &invocation.fixed_environment)
    {
        assert!(
            environment
                .iter()
                .any(|(name, value)| name == "GIT_TERMINAL_PROMPT" && value == "0"),
            "interactive credential prompts must be disabled"
        );
    }
}

#[test]
fn ws8_membership_must_be_proven_by_the_immutable_srcinfo() {
    let srcinfo = parse_srcinfo("pkgbase = demo\npkgname = demo\n").expect("valid recipe");
    let base = PackageBase::new("demo").expect("valid base");
    let member = PackageName::new("demo").expect("valid name");
    let stranger = PackageName::new("other").expect("valid name");

    prove_srcinfo_membership(&srcinfo, &member, &base).expect("declared member");
    let error =
        prove_srcinfo_membership(&srcinfo, &stranger, &base).expect_err("stranger is rejected");
    assert!(matches!(error, AcquisitionError::MembershipUnproven { .. }));
}

#[test]
fn ws8_rpc_mapping_alone_cannot_bind_a_foreign_package() {
    let temp = tempfile::tempdir().expect("temp parent");
    let srcinfo = "pkgbase = demo\npkgname = demo\n";
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(Vec::new());

    let mut hostile = request("scan-hostile", false);
    hostile.package_name = PackageName::new("victim").expect("valid name");
    hostile.rpc = AurRpcData::from_pairs(&[("victim", "demo")]);

    let error = acquire_package(
        &hostile,
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect_err("membership must be proven by .SRCINFO, not by RPC data");
    assert!(matches!(error, AcquisitionError::MembershipUnproven { .. }));
}

#[test]
fn ws8_checksum_mismatch_fails_the_source() {
    let temp = tempfile::tempdir().expect("temp parent");
    let srcinfo = srcinfo_with_source(&"0".repeat(64));
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(b"unexpected bytes".to_vec());

    let outcome = acquire_package(
        &request("scan-mismatch", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition completes with an explicit failure");

    assert_eq!(outcome.status, AcquisitionStatus::Failed);
    assert_eq!(
        outcome.provenance.sources[0].status,
        AcquisitionStatus::Failed
    );
    assert!(
        outcome.source_manifest.is_empty(),
        "a mismatching source must never be materialized"
    );
}

#[test]
fn ws8_failed_signature_fails_the_source() {
    let temp = tempfile::tempdir().expect("temp parent");
    let payload = b"body".to_vec();
    let digest = hex(&Sha256::digest(&payload));
    let srcinfo = format!(
        "pkgbase = demo\npkgname = demo\nsource = https://example.com/demo.tar\nsha256sums = {digest}\nvalidpgpkeys = {}\n",
        "A".repeat(40)
    );
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let outcome = acquire_package(
        &request("scan-signature", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut FailingVerifier,
    )
    .expect("acquisition completes");

    // No detached signature body is declared for this source, so the isolated verifier is
    // not consulted and the strong checksum alone decides completeness.
    assert_eq!(
        outcome.provenance.sources[0].signature,
        SignatureStatus::NotRequired
    );
}

#[test]
fn ws8_missing_checksum_yields_incomplete_never_complete() {
    let temp = tempfile::tempdir().expect("temp parent");
    let srcinfo = "pkgbase = demo\npkgname = demo\nsource = https://example.com/demo.tar\n";
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(tar_bytes(&[("inner.txt", b"data")]));

    let outcome = acquire_package(
        &request("scan-nochecksum", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition completes");

    assert_eq!(outcome.status, AcquisitionStatus::Incomplete);
    assert!(
        outcome
            .coverage_notes
            .iter()
            .any(|note| note.contains("demo.tar")),
        "the coverage gap must be reported explicitly: {:?}",
        outcome.coverage_notes
    );
}

#[test]
fn ws8_unsupported_transports_are_incomplete_and_never_fetched() {
    let temp = tempfile::tempdir().expect("temp parent");
    let srcinfo = "pkgbase = demo\npkgname = demo\nsource = git://example.com/repo\n";
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp {
        scripted: VecDeque::new(),
        seen: Vec::new(),
    };

    let outcome = acquire_package(
        &request("scan-transport", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition completes");

    assert_eq!(outcome.status, AcquisitionStatus::Incomplete);
    assert!(
        http.seen.is_empty(),
        "an unsupported transport must never reach the network seam"
    );
}

#[test]
fn ws8_mutable_git_source_is_incomplete() {
    let temp = tempfile::tempdir().expect("temp parent");
    let srcinfo =
        "pkgbase = demo\npkgname = demo\nsource = git+https://example.com/repo.git#branch=main\n";
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    git.scripted.insert(
        "ls-remote".to_string(),
        ok(format!("{}\trefs/heads/main\n", "b".repeat(40)).into_bytes()),
    );
    let mut http = FakeHttp {
        scripted: VecDeque::new(),
        seen: Vec::new(),
    };

    let outcome = acquire_package(
        &request("scan-mutable", false),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition completes");

    assert_eq!(outcome.status, AcquisitionStatus::Incomplete);
    assert_eq!(outcome.mutable_sources.len(), 1);
    assert_eq!(outcome.mutable_sources[0].reference, "refs/heads/main");
    assert!(
        outcome.provenance.sources[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("mutable") || reason.contains("commit")),
        "the mutable identity must be named: {:?}",
        outcome.provenance.sources[0].reasons
    );
}

#[test]
fn ws8_dry_run_publishes_no_root_and_leaves_no_workspace() {
    let temp = tempfile::tempdir().expect("temp parent");
    let (recipe, payload) = complete_fixture();
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let outcome = acquire_package(
        &request("scan-dry", true),
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("dry-run acquisition completes");

    assert!(outcome.provenance.dry_run);
    assert!(
        outcome.snapshots.root(RECIPE_SNAPSHOT_ID).is_err(),
        "a dry-run preview must publish no snapshot root"
    );
    assert!(
        outcome.workspace_root().is_none(),
        "the dry-run workspace must already be cleaned"
    );
    let remaining: Vec<_> = std::fs::read_dir(temp.path())
        .expect("read parent")
        .filter_map(Result::ok)
        .collect();
    assert!(
        remaining.is_empty(),
        "dry-run must retain no repository, download, or snapshot"
    );
    assert!(
        !outcome.recipe_manifest.is_empty(),
        "a dry-run preview still reports what it would have analyzed"
    );
}

#[test]
fn ws8_workspace_is_always_cleaned_when_the_outcome_is_dropped() {
    let temp = tempfile::tempdir().expect("temp parent");
    let (recipe, payload) = complete_fixture();
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let workspace = {
        let outcome = acquire_package(
            &request("scan-cleanup", false),
            temp.path(),
            Path::new("/usr/bin/git"),
            &mut http,
            &mut PublicResolver,
            &mut git,
            &mut UnavailableSignatureVerifier,
        )
        .expect("acquisition succeeds");
        outcome
            .workspace_root()
            .expect("workspace exists while the outcome is alive")
            .to_path_buf()
    };
    assert!(
        !workspace.exists(),
        "dropping the outcome must remove the ephemeral workspace"
    );
}

#[test]
fn ws8_redirects_are_capped_at_five_hops() {
    let mut scripted = VecDeque::new();
    for index in 0..6 {
        scripted.push_back(HttpResponse {
            status: 302,
            location: Some(format!("https://example.com/hop{index}")),
            body: Vec::new(),
        });
    }
    let mut http = FakeHttp {
        scripted,
        seen: Vec::new(),
    };
    let error = download_static_source(
        &mut http,
        &mut PublicResolver,
        "https://example.com/start",
        MAX_SOURCE_BYTES,
        Duration::from_secs(5),
    )
    .expect_err("the redirect cap must stop the chain");
    assert!(matches!(error, AcquisitionError::Network { .. }));
    assert!(
        http.seen.len() <= 6,
        "at most the initial hop plus five redirects may be contacted, saw {}",
        http.seen.len()
    );
}

#[test]
fn ws8_non_public_destinations_are_rejected_before_any_request() {
    let mut http = FakeHttp::single(b"secret".to_vec());
    let error = download_static_source(
        &mut http,
        &mut LoopbackResolver,
        "https://internal.example.com/file",
        MAX_SOURCE_BYTES,
        Duration::from_secs(5),
    )
    .expect_err("loopback destinations must be refused");
    assert!(matches!(error, AcquisitionError::Network { .. }));
    assert!(
        http.seen.is_empty(),
        "the address check must run before the request is issued"
    );
}

#[test]
fn ws8_oversized_bodies_are_rejected_at_the_source_cap() {
    let mut http = FakeHttp::single(vec![0_u8; 4096]);
    let error = download_static_source(
        &mut http,
        &mut PublicResolver,
        "https://example.com/big",
        1024,
        Duration::from_secs(5),
    )
    .expect_err("a body above the cap must be refused");
    assert!(matches!(error, AcquisitionError::Network { .. }));
}

#[test]
fn ws8_package_budget_stops_further_downloads() {
    let temp = tempfile::tempdir().expect("temp parent");
    let payload = vec![b'x'; 4096];
    let digest = hex(&Sha256::digest(&payload));
    let srcinfo = format!(
        "pkgbase = demo\npkgname = demo\nsource = https://example.com/demo.bin\nsha256sums = {digest}\n"
    );
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit::with_recipe(recipe);
    let mut http = FakeHttp::single(payload);

    let mut tight = request("scan-budget", false);
    tight.limits = AcquisitionLimits {
        package_bytes: 512,
        ..AcquisitionLimits::default()
    };

    let outcome = acquire_package(
        &tight,
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect("acquisition completes");

    assert_eq!(outcome.status, AcquisitionStatus::Incomplete);
    assert!(
        outcome.provenance.sources[0]
            .reasons
            .iter()
            .any(|reason| reason.contains("budget")),
        "the exhausted package budget must be reported: {:?}",
        outcome.provenance.sources[0].reasons
    );
}

#[test]
fn ws8_git_invocation_builders_pin_the_frozen_commit() {
    let base = PackageBase::new("demo").expect("valid base");
    let url = pacsea::logic::pi_scan::identity::AurRepoUrl::for_package_base(&base);
    let commit = CommitOid::new("b".repeat(40)).expect("valid oid");
    let timeout = Duration::from_secs(30);

    let init =
        init_repository_invocation(OsStr::new("/usr/bin/git"), OsStr::new("/tmp/repo"), timeout);
    assert!(init.argv_strings().contains(&"--bare".to_string()));

    let fetch = fetch_commit_invocation(
        OsStr::new("/usr/bin/git"),
        OsStr::new("/tmp/repo"),
        &url,
        &commit,
        timeout,
    );
    let fetch_argv = fetch.argv_strings();
    assert!(fetch_argv.contains(&"b".repeat(40)));
    assert!(fetch_argv.contains(&"https://aur.archlinux.org/demo.git".to_string()));

    let archive = archive_tree_invocation(
        OsStr::new("/usr/bin/git"),
        OsStr::new("/tmp/repo"),
        &commit,
        timeout,
    );
    let archive_argv = archive.argv_strings();
    assert!(archive_argv.contains(&"--format=tar".to_string()));
    assert!(
        !archive_argv.iter().any(|arg| arg == "checkout"),
        "the recipe tree is exported, never checked out"
    );
}

#[test]
fn ws8_relative_git_executable_is_refused() {
    let temp = tempfile::tempdir().expect("temp parent");
    let mut git = FakeGit::with_recipe(Vec::new());
    let mut http = FakeHttp::single(Vec::new());

    let error = acquire_package(
        &request("scan-relative", false),
        temp.path(),
        Path::new("git"),
        &mut http,
        &mut PublicResolver,
        &mut git,
        &mut UnavailableSignatureVerifier,
    )
    .expect_err("a relative git path must be refused");
    assert!(matches!(error, AcquisitionError::Git { .. }));
}

#[test]
fn ws8_package_base_resolution_and_format_classification_are_strict() {
    let name = PackageName::new("demo-bin").expect("valid name");
    let rpc = AurRpcData::from_pairs(&[("demo-bin", "demo")]);
    assert_eq!(
        resolve_package_base(&name, &rpc)
            .expect("declared base")
            .as_str(),
        "demo"
    );
    assert!(resolve_package_base(&name, &AurRpcData::default()).is_err());

    assert_eq!(classify_archive_format("a.tar.gz"), ArchiveFormat::TarGzip);
    assert_eq!(
        classify_archive_format("a.tar.bz2"),
        ArchiveFormat::TarBzip2
    );
    assert_eq!(classify_archive_format("a.tar.xz"), ArchiveFormat::TarXz);
    assert_eq!(classify_archive_format("a.zip"), ArchiveFormat::Zip);
    assert_eq!(classify_archive_format("PKGBUILD"), ArchiveFormat::Raw);
}

#[test]
fn ws8_limits_never_exceed_compiled_maxima() {
    let clamped = AcquisitionLimits {
        source_bytes: MAX_SOURCE_BYTES * 10,
        package_bytes: MAX_PACKAGE_BYTES * 10,
        declared_sources: usize::MAX,
        ..AcquisitionLimits::default()
    }
    .clamped();
    assert_eq!(clamped.source_bytes, MAX_SOURCE_BYTES);
    assert_eq!(clamped.package_bytes, MAX_PACKAGE_BYTES);
    assert!(clamped.declared_sources <= 64);
}
