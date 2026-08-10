//! Adversarial WS1 `.SRCINFO`, source policy, integrity, and archive inspection tests.

use blake2::{Blake2b512, Digest as BlakeDigest};
use pacsea::logic::pi_scan::head_source::{
    SourceLocator, classify_source_locator, is_public_ip, validate_public_addresses,
    validate_redirect_chain,
};
use pacsea::logic::pi_scan::recipe::{DeclaredChecksum, parse_srcinfo};
use pacsea::logic::pi_scan::source::{
    AcquisitionStatus, ArchiveFormat, ArchiveLimits, ChecksumAlgorithm, SignatureStatus,
    evaluate_integrity, inspect_source,
};
use sha2::{Digest as ShaDigest, Sha256, Sha384, Sha512};
use std::io::{Cursor, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Format bytes as lowercase hexadecimal for checksum fixtures.
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
                .expect("append deterministic tar entry");
        }
        builder.finish().expect("finish deterministic tar");
    }
    output
}

/// Compress bytes with gzip for standalone and tar-stream coverage.
fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("gzip input");
    encoder.finish().expect("gzip output")
}

/// Compress bytes with bzip2 for standalone and tar-stream coverage.
fn bzip2_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::default());
    encoder.write_all(bytes).expect("bzip2 input");
    encoder.finish().expect("bzip2 output")
}

/// Compress bytes with XZ using the test-only pure-Rust encoder feature.
fn xz_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = lzma_rust2::XzWriter::new(Vec::new(), lzma_rust2::XzOptions::default())
        .expect("XZ encoder");
    encoder.write_all(bytes).expect("XZ input");
    encoder.finish().expect("XZ output")
}

/// Compress bytes with Zstandard for standalone and tar-stream coverage.
fn zstd_bytes(bytes: &[u8]) -> Vec<u8> {
    zstd::stream::encode_all(bytes, 1).expect("zstd output")
}

/// Build a ZIP containing one Stored and one Deflate regular file.
fn zip_bytes() -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("stored.txt", stored)
        .expect("stored entry");
    writer.write_all(b"stored").expect("stored bytes");
    writer
        .start_file("nested/deflated.txt", deflated)
        .expect("deflated entry");
    writer.write_all(b"deflated").expect("deflated bytes");
    writer.finish().expect("finish ZIP").into_inner()
}

/// Build a tar entry with a raw hostile path that safe builder APIs refuse to create.
fn raw_tar_with_path(path: &[u8], content: &[u8], entry_type: u8) -> Vec<u8> {
    let mut header = [0_u8; 512];
    header[..path.len()].copy_from_slice(path);
    write_tar_octal(&mut header[100..108], 0o644);
    write_tar_octal(&mut header[108..116], 0);
    write_tar_octal(&mut header[116..124], 0);
    write_tar_octal(&mut header[124..136], content.len() as u64);
    write_tar_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = entry_type;
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum: u64 = header.iter().map(|byte| u64::from(*byte)).sum();
    write_tar_octal(&mut header[148..156], checksum);
    let mut output = header.to_vec();
    output.extend_from_slice(content);
    let padding = (512 - content.len() % 512) % 512;
    output.resize(output.len() + padding + 1024, 0);
    output
}

/// Write a tar-compatible zero-padded octal field with trailing NUL.
fn write_tar_octal(field: &mut [u8], value: u64) {
    let text = format!("{:0width$o}\0", value, width = field.len() - 1);
    field.copy_from_slice(text.as_bytes());
}

/// What: Verify strict `.SRCINFO` identity, architecture-local positional checksum, key, and noextract binding.
///
/// Inputs:
/// - Valid split-package metadata plus malformed/misaligned alternatives.
///
/// Output:
/// - Exact bound source records or deterministic parser failures.
///
/// Details:
/// - No PKGBUILD expression or source code is evaluated by this test.
#[test]
fn srcinfo_strictly_binds_build_relevant_arrays() {
    let document = "pkgbase = demo\n\
                    pkgname = demo\n\
                    pkgname = demo-docs\n\
                    source = renamed.tar.gz::https://example.com/upstream.tar.gz\n\
                    source_x86_64 = git+https://example.com/repo.git#commit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n\
                    sha256sums = 0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n\
                    sha512sums_x86_64 = SKIP\n\
                    validpgpkeys = 0123456789abcdef0123456789abcdef01234567\n\
                    noextract = renamed.tar.gz\n";
    let parsed = parse_srcinfo(document).expect("strict .SRCINFO");
    assert_eq!(parsed.package_base.as_str(), "demo");
    assert_eq!(parsed.package_names.len(), 2);
    assert_eq!(parsed.sources.len(), 2);
    assert_eq!(parsed.sources[0].effective_name, "renamed.tar.gz");
    assert!(parsed.sources[0].no_extract);
    assert_eq!(
        parsed.sources[0].checksums[0].algorithm,
        ChecksumAlgorithm::Sha256
    );
    assert_eq!(parsed.sources[1].architecture.as_deref(), Some("x86_64"));
    assert_eq!(
        parsed.valid_pgp_keys[0],
        "0123456789ABCDEF0123456789ABCDEF01234567"
    );

    let mismatched = "pkgbase = demo\npkgname = demo\nsource = https://example.com/a\nsource = https://example.com/b\nsha256sums = SKIP\n";
    assert!(parse_srcinfo(mismatched).is_err());
    assert!(parse_srcinfo("pkgbase demo\npkgname = demo\n").is_err());
    assert!(parse_srcinfo("pkgbase = demo\npkgbase = other\npkgname = demo\n").is_err());
    assert!(
        parse_srcinfo(
            "pkgbase = demo\npkgname = demo\nsource = ../escape::https://example.com/a\n"
        )
        .is_err()
    );
}

/// What: Verify standard tab-indented `.SRCINFO` metadata is accepted.
///
/// Inputs:
/// - A valid `.SRCINFO` document using makepkg's tab indentation.
///
/// Output:
/// - The document parses into the expected package identity.
///
/// Details:
/// - AUR-generated `.SRCINFO` files conventionally indent fields with tabs.
#[test]
fn srcinfo_accepts_standard_tab_indentation() {
    let document = "pkgbase = demo\n\tpkgdesc = Demo package\n\tpkgname = demo\n";
    let parsed = parse_srcinfo(document).expect("tab-indented .SRCINFO should parse");
    assert_eq!(parsed.package_base.as_str(), "demo");
    assert_eq!(parsed.package_names[0].as_str(), "demo");
}

/// What: Verify immutable transport, redirect, userinfo, and explicit public-IP policy.
///
/// Inputs:
/// - HTTPS/static, pinned/mutable Git, hostile schemes, redirect chains, and special-use IPs.
///
/// Output:
/// - Only policy-complete static or fully pinned Git identities and public addresses are accepted.
///
/// Details:
/// - Helpers consume explicit values and never perform DNS or network access.
#[test]
fn source_policy_is_https_immutable_and_public_only() {
    assert!(matches!(
        classify_source_locator("https://example.com/source.tar.gz"),
        SourceLocator::StaticHttps { .. }
    ));
    assert!(matches!(
        classify_source_locator(
            "git+https://example.com/repo.git#commit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ),
        SourceLocator::GitHttps { .. }
    ));
    assert!(matches!(
        classify_source_locator("git+https://example.com/repo.git#tag=v1"),
        SourceLocator::MutableGitHttps { .. }
    ));
    for mutable_or_unsafe in [
        "http://example.com/source.tar.gz",
        "https://user:pass@example.com/source.tar.gz",
        "git+https://example.com/repo.git#commit=abc123",
        "git://example.com/repo.git#commit=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(matches!(
            classify_source_locator(mutable_or_unsafe),
            SourceLocator::Incomplete { .. }
        ));
    }
    assert!(
        validate_redirect_chain(&["https://example.com/a", "https://cdn.example.com/b"]).is_ok()
    );
    assert!(validate_redirect_chain(&["https://example.com/a", "http://example.com/b"]).is_err());
    let too_many: Vec<String> = (0..7)
        .map(|index| format!("https://example.com/{index}"))
        .collect();
    assert!(validate_redirect_chain(&too_many).is_err());

    for private in [
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)),
        IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
        IpAddr::V6(Ipv6Addr::LOCALHOST),
        "fc00::1".parse().expect("ULA"),
        "2001:db8::1".parse().expect("documentation"),
    ] {
        assert!(!is_public_ip(private));
    }
    let public = [
        IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
        "2606:4700:4700::1111".parse().expect("public IPv6"),
    ];
    assert!(validate_public_addresses(&public).is_ok());
    assert!(validate_public_addresses(&[public[0], IpAddr::V4(Ipv4Addr::LOCALHOST)]).is_err());
}

/// What: Verify strong checksum alignment and missing/SKIP/weak/signature status semantics.
///
/// Inputs:
/// - SHA-256/384/512/BLAKE2 matches, mismatch, missing, SKIP, weak-only, and signature states.
///
/// Output:
/// - Exact complete, incomplete, or failed integrity statuses.
///
/// Details:
/// - Digest computation is in-process and does not use GPG, shell, helpers, or network.
#[test]
fn checksum_policy_reports_complete_incomplete_and_failed() {
    let bytes = b"immutable source bytes";
    let strong = [
        (ChecksumAlgorithm::Sha256, hex(&Sha256::digest(bytes))),
        (ChecksumAlgorithm::Sha384, hex(&Sha384::digest(bytes))),
        (ChecksumAlgorithm::Sha512, hex(&Sha512::digest(bytes))),
        (
            ChecksumAlgorithm::Blake2b512,
            hex(&Blake2b512::digest(bytes)),
        ),
    ];
    for (algorithm, value) in &strong {
        let report = evaluate_integrity(
            bytes,
            &[DeclaredChecksum {
                algorithm: *algorithm,
                value: value.clone(),
            }],
            SignatureStatus::NotRequired,
        );
        assert_eq!(report.status, AcquisitionStatus::Complete);
    }
    let required_signature_unavailable = evaluate_integrity(
        bytes,
        &[DeclaredChecksum {
            algorithm: strong[0].0,
            value: strong[0].1.clone(),
        }],
        SignatureStatus::Unavailable,
    );
    assert_eq!(
        required_signature_unavailable.status,
        AcquisitionStatus::Incomplete
    );
    let mismatch = evaluate_integrity(
        bytes,
        &[DeclaredChecksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "0".repeat(64),
        }],
        SignatureStatus::NotRequired,
    );
    assert_eq!(mismatch.status, AcquisitionStatus::Failed);
    assert_eq!(
        evaluate_integrity(bytes, &[], SignatureStatus::NotRequired).status,
        AcquisitionStatus::Incomplete
    );
    assert_eq!(
        evaluate_integrity(
            bytes,
            &[DeclaredChecksum {
                algorithm: ChecksumAlgorithm::Sha256,
                value: "SKIP".to_string(),
            }],
            SignatureStatus::NotRequired,
        )
        .status,
        AcquisitionStatus::Incomplete
    );
    let weak = [DeclaredChecksum {
        algorithm: ChecksumAlgorithm::Sha1,
        value: "0".repeat(40),
    }];
    assert_eq!(
        evaluate_integrity(bytes, &weak, SignatureStatus::NotRequired).status,
        AcquisitionStatus::Incomplete
    );
    assert_eq!(
        evaluate_integrity(bytes, &weak, SignatureStatus::Verified).status,
        AcquisitionStatus::Complete
    );
    assert_eq!(
        evaluate_integrity(bytes, &weak, SignatureStatus::Failed).status,
        AcquisitionStatus::Failed
    );
}

/// What: Verify every supported raw/compressed/tar/ZIP format produces canonical byte-hashed manifests.
///
/// Inputs:
/// - In-memory raw, tar, gzip, bzip2, XZ, Zstandard, and ZIP Stored/Deflate fixtures.
///
/// Output:
/// - Complete reports with expected entry names and canonical manifest hashes.
///
/// Details:
/// - Tests never materialize archive entries or invoke broad unpack helpers.
#[test]
fn supported_formats_are_inspected_entry_by_entry() {
    let limits = ArchiveLimits::default();
    let raw = b"hello source";
    for (name, bytes, format) in [
        ("plain.txt", raw.to_vec(), ArchiveFormat::Raw),
        ("plain.txt.gz", gzip_bytes(raw), ArchiveFormat::Gzip),
        ("plain.txt.bz2", bzip2_bytes(raw), ArchiveFormat::Bzip2),
        ("plain.txt.xz", xz_bytes(raw), ArchiveFormat::Xz),
        ("plain.txt.zst", zstd_bytes(raw), ArchiveFormat::Zstd),
    ] {
        let report = inspect_source(name, &bytes, format, limits);
        assert_eq!(report.status, AcquisitionStatus::Complete, "{format:?}");
        assert_eq!(report.manifest.len(), 1);
    }

    let tar = tar_bytes(&[("a.txt", b"alpha"), ("nested/b.txt", b"beta")]);
    for (bytes, format) in [
        (tar.clone(), ArchiveFormat::Tar),
        (gzip_bytes(&tar), ArchiveFormat::TarGzip),
        (bzip2_bytes(&tar), ArchiveFormat::TarBzip2),
        (xz_bytes(&tar), ArchiveFormat::TarXz),
        (zstd_bytes(&tar), ArchiveFormat::TarZstd),
    ] {
        let report = inspect_source("source.tar", &bytes, format, limits);
        assert_eq!(report.status, AcquisitionStatus::Complete, "{format:?}");
        assert_eq!(report.manifest.len(), 2);
        assert!(
            report
                .manifest
                .find_entry("source", "nested/b.txt")
                .is_some()
        );
    }

    let zip = zip_bytes();
    let report = inspect_source("source.zip", &zip, ArchiveFormat::Zip, limits);
    assert_eq!(report.status, AcquisitionStatus::Complete);
    assert_eq!(report.manifest.len(), 2);
    assert_eq!(
        report.manifest.entries[0].relative_path,
        "nested/deflated.txt"
    );
    assert_eq!(report.manifest.entries[1].relative_path, "stored.txt");
}

/// What: Verify unsafe tar paths, links, duplicate paths, and file/directory conflicts fail closed.
///
/// Inputs:
/// - Handcrafted traversal/link tar and deterministic duplicate/conflict archives.
///
/// Output:
/// - Incomplete results with no unsafe entry materialization.
///
/// Details:
/// - Structural policy rejection is distinct from corrupt-container failure.
#[test]
fn archive_paths_links_duplicates_and_conflicts_are_rejected() {
    let traversal = raw_tar_with_path(b"../escape", b"secret", b'0');
    let traversal_report = inspect_source(
        "bad.tar",
        &traversal,
        ArchiveFormat::Tar,
        ArchiveLimits::default(),
    );
    assert_eq!(traversal_report.status, AcquisitionStatus::Incomplete);
    assert!(traversal_report.manifest.is_empty());

    let link = raw_tar_with_path(b"link", b"", b'2');
    assert_eq!(
        inspect_source(
            "link.tar",
            &link,
            ArchiveFormat::Tar,
            ArchiveLimits::default()
        )
        .status,
        AcquisitionStatus::Incomplete
    );
    let duplicate = tar_bytes(&[("same", b"one"), ("same", b"two")]);
    assert_eq!(
        inspect_source(
            "duplicate.tar",
            &duplicate,
            ArchiveFormat::Tar,
            ArchiveLimits::default()
        )
        .status,
        AcquisitionStatus::Incomplete
    );
    let conflict = tar_bytes(&[("parent", b"file"), ("parent/child", b"child")]);
    assert_eq!(
        inspect_source(
            "conflict.tar",
            &conflict,
            ArchiveFormat::Tar,
            ArchiveLimits::default()
        )
        .status,
        AcquisitionStatus::Incomplete
    );
}

/// What: Verify entry, aggregate, ratio, depth, and compiled-limit violations are explicit.
///
/// Inputs:
/// - Lowered valid limits and one attempted above-maximum configuration.
///
/// Output:
/// - Incomplete data-limit reports and failed invalid-configuration report.
///
/// Details:
/// - Lower limits exercise boundaries without allocating production maxima.
#[test]
fn archive_hard_limits_cannot_be_raised_or_silently_truncated() {
    let tar = tar_bytes(&[("deep/path/file.txt", b"0123456789")]);
    let small_entry = ArchiveLimits {
        entry_bytes: 4,
        ..ArchiveLimits::default()
    };
    assert_eq!(
        inspect_source("entry.tar", &tar, ArchiveFormat::Tar, small_entry).status,
        AcquisitionStatus::Incomplete
    );
    let shallow = ArchiveLimits {
        path_depth: 2,
        ..ArchiveLimits::default()
    };
    assert_eq!(
        inspect_source("depth.tar", &tar, ArchiveFormat::Tar, shallow).status,
        AcquisitionStatus::Incomplete
    );
    let one_entry = ArchiveLimits {
        entries: 1,
        ..ArchiveLimits::default()
    };
    let two = tar_bytes(&[("a", b"a"), ("b", b"b")]);
    assert_eq!(
        inspect_source("count.tar", &two, ArchiveFormat::Tar, one_entry).status,
        AcquisitionStatus::Incomplete
    );
    let ratio = ArchiveLimits {
        expansion_ratio: 1,
        ..ArchiveLimits::default()
    };
    let compressed = gzip_bytes(&vec![b'x'; 4096]);
    assert_eq!(
        inspect_source("ratio.gz", &compressed, ArchiveFormat::Gzip, ratio).status,
        AcquisitionStatus::Incomplete
    );
    let raised = ArchiveLimits {
        entries: 10_001,
        ..ArchiveLimits::default()
    };
    assert_eq!(
        inspect_source("raw", b"x", ArchiveFormat::Raw, raised).status,
        AcquisitionStatus::Failed
    );
}

/// What: Verify malformed containers are failed rather than represented as partial success.
///
/// Inputs:
/// - Invalid tar, gzip, bzip2, XZ, Zstandard, and ZIP bytes.
///
/// Output:
/// - Explicit failed outcomes for every corrupt format.
///
/// Details:
/// - Corruption is not downgraded to an unsupported/incomplete policy state.
#[test]
fn corrupt_containers_are_failed() {
    for format in [
        ArchiveFormat::Tar,
        ArchiveFormat::Gzip,
        ArchiveFormat::Bzip2,
        ArchiveFormat::Xz,
        ArchiveFormat::Zstd,
        ArchiveFormat::TarGzip,
        ArchiveFormat::TarBzip2,
        ArchiveFormat::TarXz,
        ArchiveFormat::TarZstd,
        ArchiveFormat::Zip,
    ] {
        let report = inspect_source(
            "corrupt.archive",
            b"not a valid container",
            format,
            ArchiveLimits::default(),
        );
        assert_eq!(report.status, AcquisitionStatus::Failed, "{format:?}");
    }
}
