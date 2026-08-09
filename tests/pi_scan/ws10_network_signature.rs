//! Deterministic WS10 production-adapter contract tests.
//!
//! No test contacts DNS, HTTP, AUR, keys.openpgp.org, or a real `GnuPG` executable.

use pacsea::logic::pi_scan::acquisition::{
    AcquisitionError, AcquisitionLimits, AcquisitionRequest, AddressResolver, AurRpcData,
    HttpFetcher, HttpRequest, HttpResponse, SignatureRequest, SignatureVerifier, acquire_package,
    download_static_source,
};
use pacsea::logic::pi_scan::identity::{CommitOid, PackageName};
use pacsea::logic::pi_scan::observer::{GitCommandRunner, GitInvocation, GitOutput, ObserverError};
use pacsea::logic::pi_scan::recipe::parse_srcinfo;
use pacsea::logic::pi_scan::signature::{
    GpgCommandRunner, GpgInvocation, GpgOutput, IsolatedSignatureVerifier, SigningKeyFetcher,
    key_retrieval_url,
};
use pacsea::logic::pi_scan::source::{AcquisitionStatus, SignatureStatus};
use sha2::{Digest as _, Sha256};
use std::collections::VecDeque;
use std::io::Cursor;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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

/// Build a deterministic regular-file tar fixture.
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
                .expect("append fixture");
        }
        builder.finish().expect("finish fixture");
    }
    output
}

/// Scripted HTTP transport recording the DNS-pinned address.
struct FakeHttp {
    /// Responses returned in order.
    responses: VecDeque<HttpResponse>,
    /// Requests observed in order.
    requests: Vec<HttpRequest>,
}

impl HttpFetcher for FakeHttp {
    fn fetch(&mut self, request: &HttpRequest) -> Result<HttpResponse, AcquisitionError> {
        self.requests.push(request.clone());
        self.responses
            .pop_front()
            .ok_or_else(|| AcquisitionError::Network {
                url: request.url.clone(),
                reason: "fake response exhausted".to_string(),
            })
    }
}

/// Resolver returning a scripted answer set for each hop.
struct FakeResolver {
    /// Address sets returned in order.
    answers: VecDeque<Vec<IpAddr>>,
}

impl AddressResolver for FakeResolver {
    fn resolve(&mut self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
        self.answers
            .pop_front()
            .ok_or_else(|| AcquisitionError::Network {
                url: "fake DNS".to_string(),
                reason: "fake answer exhausted".to_string(),
            })
    }
}

/// Resolver that consumes part of the caller's absolute deadline before HTTP begins.
struct DelayedResolver;

impl AddressResolver for DelayedResolver {
    fn resolve(&mut self, _host: &str, _port: u16) -> Result<Vec<IpAddr>, AcquisitionError> {
        std::thread::sleep(Duration::from_millis(50));
        Ok(vec![IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))])
    }
}

#[test]
fn ws10_dns_time_is_subtracted_before_http_timeout() {
    let mut resolver = DelayedResolver;
    let mut http = FakeHttp {
        responses: VecDeque::from(vec![HttpResponse {
            status: 200,
            location: None,
            body: b"bounded".to_vec(),
        }]),
        requests: Vec::new(),
    };
    download_static_source(
        &mut http,
        &mut resolver,
        "https://example.com/source",
        1024,
        Duration::from_millis(200),
    )
    .expect("bounded download");
    assert!(http.requests[0].timeout < Duration::from_millis(175));
}

#[test]
fn ws10_redirect_hops_are_revalidated_pinned_and_recorded() {
    let first = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let second = IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1));
    let mut resolver = FakeResolver {
        answers: VecDeque::from(vec![vec![first], vec![second]]),
    };
    let mut http = FakeHttp {
        responses: VecDeque::from(vec![
            HttpResponse {
                status: 302,
                location: Some("/final".to_string()),
                body: Vec::new(),
            },
            HttpResponse {
                status: 200,
                location: None,
                body: b"bounded".to_vec(),
            },
        ]),
        requests: Vec::new(),
    };

    let downloaded = download_static_source(
        &mut http,
        &mut resolver,
        "https://example.com/start",
        1024,
        Duration::from_secs(1),
    )
    .expect("bounded download");

    assert_eq!(downloaded.bytes, b"bounded");
    assert_eq!(
        downloaded.redirect_chain,
        ["https://example.com/start", "https://example.com/final"]
    );
    assert_eq!(http.requests[0].pinned_address, first);
    assert_eq!(http.requests[1].pinned_address, second);
    assert_eq!(downloaded.address_provenance[0].resolved_addresses, [first]);
    assert_eq!(downloaded.address_provenance[1].pinned_address, second);
}

#[test]
fn ws10_mixed_public_private_dns_is_rejected_before_contact() {
    let mut resolver = FakeResolver {
        answers: VecDeque::from(vec![vec![
            IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34)),
            IpAddr::V4(Ipv4Addr::LOCALHOST),
        ]]),
    };
    let mut http = FakeHttp {
        responses: VecDeque::new(),
        requests: Vec::new(),
    };
    let error = download_static_source(
        &mut http,
        &mut resolver,
        "https://example.com/source",
        1024,
        Duration::from_secs(1),
    )
    .expect_err("mixed answer must fail");
    assert!(matches!(error, AcquisitionError::Network { .. }));
    assert!(http.requests.is_empty());
}

#[test]
fn ws10_nat64_embedded_private_destination_is_rejected_before_contact() {
    let mut resolver = FakeResolver {
        answers: VecDeque::from(vec![vec![IpAddr::V6(
            "64:ff9b::7f00:1"
                .parse::<Ipv6Addr>()
                .expect("NAT64 address"),
        )]]),
    };
    let mut http = FakeHttp {
        responses: VecDeque::new(),
        requests: Vec::new(),
    };
    let error = download_static_source(
        &mut http,
        &mut resolver,
        "https://example.com/source",
        1024,
        Duration::from_secs(1),
    )
    .expect_err("NAT64 private embedding must fail");
    assert!(matches!(error, AcquisitionError::Network { .. }));
    assert!(http.requests.is_empty());
}

#[test]
fn ws10_full_fingerprint_url_is_exact_and_rejects_key_ids() {
    let fingerprint = "0123456789ABCDEF0123456789ABCDEF01234567";
    assert_eq!(
        key_retrieval_url(fingerprint).expect("full fingerprint"),
        format!("https://keys.openpgp.org/vks/v1/by-fingerprint/{fingerprint}")
    );
    assert!(key_retrieval_url("DEADBEEF").is_err());
    assert!(key_retrieval_url(&format!("{fingerprint}/search")).is_err());
}

/// Fake exact-key fetcher retaining only requested URLs.
struct FakeKeyFetcher {
    /// Key body returned for every URL.
    body: Vec<u8>,
    /// Requested exact URLs.
    seen: Arc<Mutex<Vec<String>>>,
}

impl SigningKeyFetcher for FakeKeyFetcher {
    fn fetch_key(&mut self, url: &str) -> Result<Vec<u8>, AcquisitionError> {
        self.seen.lock().expect("URL lock").push(url.to_string());
        Ok(self.body.clone())
    }
}

/// Fake `GnuPG` runner checking private artifacts and returning exact status records.
struct FakeGpgRunner {
    /// Declared full fingerprint.
    fingerprint: String,
    /// Captured invocations.
    seen: Arc<Mutex<Vec<GpgInvocation>>>,
}

impl GpgCommandRunner for FakeGpgRunner {
    fn run(&mut self, invocation: &GpgInvocation) -> Result<GpgOutput, String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                std::fs::metadata(&invocation.home)
                    .map_err(|error| error.to_string())?
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        let argv = invocation.argv_strings();
        self.seen
            .lock()
            .map_err(|_| "invocation lock poisoned".to_string())?
            .push(invocation.clone());
        if argv.iter().any(|value| value == "--import") {
            assert!(argv.iter().any(|value| value == "--no-options"));
            assert!(argv.iter().any(|value| value == "--no-autostart"));
            let key_path = PathBuf::from(argv.last().ok_or("missing key path")?);
            assert_private_file(&key_path)?;
            Ok(GpgOutput {
                success: true,
                status: format!("[GNUPG:] IMPORT_OK 1 {}\n", self.fingerprint).into_bytes(),
            })
        } else {
            assert_eq!(argv.len(), 8);
            assert_private_file(Path::new(&argv[6]))?;
            assert_private_file(Path::new(&argv[7]))?;
            Ok(GpgOutput {
                success: true,
                status: format!(
                    "[GNUPG:] VALIDSIG {} 2026-01-01 0 4 0 1 10 00 {}\n",
                    self.fingerprint, self.fingerprint
                )
                .into_bytes(),
            })
        }
    }
}

/// Assert one fake-observed materialized artifact is a private regular file.
fn assert_private_file(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_file() {
        return Err("artifact is not a regular file".to_string());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if metadata.permissions().mode() & 0o777 != 0o600 {
            return Err("artifact mode is not 0600".to_string());
        }
    }
    Ok(())
}

#[test]
fn ws10_isolated_verifier_binds_import_and_validsig_fingerprints() {
    let temp = tempfile::tempdir().expect("workspace parent");
    let fingerprint = "0123456789ABCDEF0123456789ABCDEF01234567".to_string();
    let urls = Arc::new(Mutex::new(Vec::new()));
    let invocations = Arc::new(Mutex::new(Vec::new()));
    let mut verifier = IsolatedSignatureVerifier::with_seams(
        temp.path().to_path_buf(),
        Some(PathBuf::from("/fake/gpg")),
        Some(PathBuf::from("/fake/gpgv")),
        Box::new(FakeKeyFetcher {
            body: b"fake public key".to_vec(),
            seen: Arc::clone(&urls),
        }),
        Box::new(FakeGpgRunner {
            fingerprint: fingerprint.clone(),
            seen: Arc::clone(&invocations),
        }),
        Duration::from_secs(1),
    );
    let status = verifier.verify(&SignatureRequest {
        data: b"covered bytes",
        signature: b"detached signature",
        fingerprints: std::slice::from_ref(&fingerprint),
    });

    assert_eq!(status, SignatureStatus::Verified);
    assert_eq!(urls.lock().expect("URL lock").len(), 1);
    assert_eq!(invocations.lock().expect("invocation lock").len(), 2);
    assert_eq!(
        std::fs::read_dir(temp.path())
            .expect("clean parent")
            .count(),
        0,
        "one-use key bodies and homes must be removed"
    );
}

#[test]
fn ws10_bad_imported_fingerprint_fails_and_missing_tools_are_unavailable() {
    let temp = tempfile::tempdir().expect("workspace parent");
    let declared = "0123456789ABCDEF0123456789ABCDEF01234567".to_string();
    let imported = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
    let mut wrong = IsolatedSignatureVerifier::with_seams(
        temp.path().to_path_buf(),
        Some(PathBuf::from("/fake/gpg")),
        Some(PathBuf::from("/fake/gpgv")),
        Box::new(FakeKeyFetcher {
            body: b"wrong key".to_vec(),
            seen: Arc::new(Mutex::new(Vec::new())),
        }),
        Box::new(FakeGpgRunner {
            fingerprint: imported,
            seen: Arc::new(Mutex::new(Vec::new())),
        }),
        Duration::from_secs(1),
    );
    let request = SignatureRequest {
        data: b"covered bytes",
        signature: b"detached signature",
        fingerprints: std::slice::from_ref(&declared),
    };
    assert_eq!(wrong.verify(&request), SignatureStatus::Failed);

    let mut unavailable = IsolatedSignatureVerifier::with_seams(
        temp.path().to_path_buf(),
        None,
        None,
        Box::new(FakeKeyFetcher {
            body: Vec::new(),
            seen: Arc::new(Mutex::new(Vec::new())),
        }),
        Box::new(FakeGpgRunner {
            fingerprint: declared.clone(),
            seen: Arc::new(Mutex::new(Vec::new())),
        }),
        Duration::from_secs(1),
    );
    assert_eq!(unavailable.verify(&request), SignatureStatus::Unavailable);
}

/// Fake Git runner returning a frozen recipe archive.
struct FakeGit {
    /// Recipe archive returned by `git archive`.
    recipe: Vec<u8>,
}

impl GitCommandRunner for FakeGit {
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError> {
        let archive = invocation
            .argv_strings()
            .iter()
            .any(|argument| argument == "archive");
        Ok(GitOutput {
            success: true,
            stdout: if archive {
                self.recipe.clone()
            } else {
                Vec::new()
            },
            stderr: String::new(),
        })
    }
}

/// Shared recording of covered-source and detached-signature byte pairs.
type RecordedBindings = Arc<Mutex<Vec<(Vec<u8>, Vec<u8>)>>>;

/// Recording verifier proving which source/signature bytes were paired.
struct RecordingVerifier {
    /// Requests reduced to owned byte pairs.
    seen: RecordedBindings,
}

impl SignatureVerifier for RecordingVerifier {
    fn verify(&mut self, request: &SignatureRequest<'_>) -> SignatureStatus {
        self.seen
            .lock()
            .expect("record lock")
            .push((request.data.to_vec(), request.signature.to_vec()));
        SignatureStatus::Verified
    }
}

#[test]
fn ws10_srcinfo_signature_declaration_makes_covered_source_verification_mandatory() {
    let temp = tempfile::tempdir().expect("workspace");
    let payload = b"covered payload".to_vec();
    let signature = b"detached signature".to_vec();
    let fingerprint = "0123456789ABCDEF0123456789ABCDEF01234567";
    let srcinfo = format!(
        "pkgbase = demo\npkgname = demo\nsource = https://example.com/demo.bin\nsource = https://example.com/demo.bin.sig\nsha256sums = {}\nsha256sums = SKIP\nvalidpgpkeys = {fingerprint}\n",
        hex(&Sha256::digest(&payload))
    );
    let parsed = parse_srcinfo(&srcinfo).expect("paired metadata");
    assert_eq!(
        parsed.sources[1].detached_signature_for.as_deref(),
        Some("demo.bin")
    );
    let recipe = tar_bytes(&[(".SRCINFO", srcinfo.as_bytes())]);
    let mut git = FakeGit { recipe };
    let mut http = FakeHttp {
        responses: VecDeque::from(vec![
            HttpResponse {
                status: 200,
                location: None,
                body: signature.clone(),
            },
            HttpResponse {
                status: 200,
                location: None,
                body: payload.clone(),
            },
        ]),
        requests: Vec::new(),
    };
    let public = IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34));
    let mut resolver = FakeResolver {
        answers: VecDeque::from(vec![vec![public], vec![public]]),
    };
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut verifier = RecordingVerifier {
        seen: Arc::clone(&seen),
    };
    let outcome = acquire_package(
        &AcquisitionRequest {
            scan_id: "ws10-binding".to_string(),
            package_name: PackageName::new("demo").expect("package"),
            commit_oid: CommitOid::new("a".repeat(40)).expect("commit"),
            rpc: AurRpcData::from_pairs(&[("demo", "demo")]),
            limits: AcquisitionLimits::default(),
            dry_run: false,
        },
        temp.path(),
        Path::new("/usr/bin/git"),
        &mut http,
        &mut resolver,
        &mut git,
        &mut verifier,
    )
    .expect("acquisition");

    assert_eq!(outcome.status, AcquisitionStatus::Complete);
    assert_eq!(
        seen.lock().expect("record lock").as_slice(),
        &[(payload, signature)]
    );
    assert_eq!(
        outcome.provenance.sources[0].signature,
        SignatureStatus::Verified
    );
    assert_eq!(
        outcome.provenance.sources[1].status,
        AcquisitionStatus::Complete
    );
}
