//! Deterministic static security detectors and evidence fingerprinting for Pi scanning.

use crate::logic::pi_scan::identity::PackageBase;
use crate::logic::pi_scan::manifest::CanonicalManifest;
use sha2::{Digest, Sha256};

/// Helper to format byte slice as lowercase hexadecimal string.
fn format_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// What: Compute a deterministic evidence fingerprint for a finding.
///
/// Inputs:
/// - `detector_id`: Unique identifier of the detector.
/// - `detector_version`: Version number of the detector.
/// - `package_base`: Package base name string.
/// - `snapshot_category`: Snapshot category ("recipe" or "source").
/// - `relative_path`: Relative file path inside snapshot.
/// - `line_hash_or_content`: Line content or hash of evidence line.
/// - `matched_text`: Matched evidence substring.
///
/// Output:
/// - Lowercase 64-character SHA-256 hexadecimal evidence fingerprint.
///
/// Details:
/// - The fingerprint is immutable across scans when evidence is identical.
/// - Used for deduplicating findings and matching user benign verdicts.
#[must_use]
pub fn calculate_evidence_fingerprint(
    detector_id: &str,
    detector_version: u32,
    package_base: &str,
    snapshot_category: &str,
    relative_path: &str,
    line_hash_or_content: &str,
    matched_text: &str,
) -> String {
    let mut hasher = Sha256::new();
    let formatted = format!(
        "{detector_id}:{detector_version}:{package_base}:{snapshot_category}:{relative_path}:{line_hash_or_content}:{matched_text}"
    );
    hasher.update(formatted.as_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    format_hex(&digest)
}

/// What: Advisory security or quality finding produced by a deterministic static detector.
///
/// Inputs:
/// - Finding attributes including severity, title, path, evidence snippet, line number, fingerprint, rationale, and recommendation.
///
/// Output:
/// - Struct representing a deterministic detector finding.
///
/// Details:
/// - Deterministic findings are attributed to their static detector ID and cannot be overridden or suppressed by LLM model output.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeterministicFinding {
    /// Detector identifier (e.g. "curl-pipe-bash").
    pub detector_id: String,
    /// Detector version.
    pub detector_version: u32,
    /// Severity level ("critical", "high", "medium", "low").
    pub severity: String,
    /// Human-readable title of finding.
    pub title: String,
    /// Snapshot category ("recipe" or "source").
    pub snapshot_category: String,
    /// Relative path inside snapshot root.
    pub relative_path: String,
    /// Exact evidence text matched in file.
    pub evidence_text: String,
    /// Line number in file (1-indexed), if applicable.
    pub line_number: Option<usize>,
    /// Deterministic evidence fingerprint.
    pub evidence_fingerprint: String,
    /// Explanation of potential security risk.
    pub rationale: String,
    /// Recommended remediation steps.
    pub recommendation: String,
}

/// What: Run static deterministic detectors over a canonical manifest and file content provider.
///
/// Inputs:
/// - `package_base`: Package base being scanned.
/// - `manifest`: `CanonicalManifest` covering snapshot files.
/// - `file_content_provider`: Closure returning string content for `(category, relative_path)`.
///
/// Output:
/// - Vector of `DeterministicFinding` objects.
///
/// Details:
/// - Evaluates static detector rules on recipe and source files.
/// - Identifies patterns such as curl-pipe-bash, sudo usage, unencrypted HTTP fetches, and root destdir writes.
pub fn run_deterministic_detectors<F>(
    package_base: &PackageBase,
    manifest: &CanonicalManifest,
    file_content_provider: F,
) -> Vec<DeterministicFinding>
where
    F: Fn(&str, &str) -> Option<String>,
{
    let mut findings = Vec::new();

    for entry in &manifest.entries {
        if entry.is_binary {
            continue;
        }

        let Some(content) = file_content_provider(&entry.snapshot_category, &entry.relative_path)
        else {
            continue;
        };

        inspect_file_content(
            package_base,
            &entry.snapshot_category,
            &entry.relative_path,
            &content,
            &mut findings,
        );
    }

    findings
}

/// What: Inspect string content of a single file against static detector rules.
///
/// Inputs:
/// - `package_base`: Owning package base.
/// - `category`: Snapshot category ("recipe" or "source").
/// - `path`: Relative path.
/// - `content`: File string content.
/// - `findings`: Mutable vector to collect findings.
///
/// Output: None.
///
/// Details:
/// - Scans line-by-line for curl-pipe-bash, sudo usage, unencrypted HTTP, and root writes.
fn inspect_file_content(
    package_base: &PackageBase,
    category: &str,
    path: &str,
    content: &str,
    findings: &mut Vec<DeterministicFinding>,
) {
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        let lower = line.to_ascii_lowercase();

        check_curl_pipe_bash(
            package_base,
            category,
            path,
            line,
            &lower,
            line_num,
            findings,
        );
        check_sudo_usage(
            package_base,
            category,
            path,
            line,
            &lower,
            line_num,
            findings,
        );
        check_insecure_http(
            package_base,
            category,
            path,
            line,
            &lower,
            line_num,
            findings,
        );
        check_root_destdir(
            package_base,
            category,
            path,
            line,
            &lower,
            line_num,
            findings,
        );
    }
}

/// What: Check a line for curl/wget piped directly into bash or shell execution.
fn check_curl_pipe_bash(
    package_base: &PackageBase,
    category: &str,
    path: &str,
    line: &str,
    lower: &str,
    line_num: usize,
    findings: &mut Vec<DeterministicFinding>,
) {
    let has_fetch = lower.contains("curl ") || lower.contains("wget ");
    let has_pipe_sh = lower.contains("| bash")
        || lower.contains("| sh")
        || lower.contains("| zsh")
        || lower.contains("|python");

    if has_fetch && has_pipe_sh {
        let detector_id = "curl-pipe-bash";
        let detector_version = 1;
        let fingerprint = calculate_evidence_fingerprint(
            detector_id,
            detector_version,
            package_base.as_str(),
            category,
            path,
            line.trim(),
            line.trim(),
        );

        findings.push(DeterministicFinding {
            detector_id: detector_id.to_string(),
            detector_version,
            severity: "high".to_string(),
            title: "Remote network payload piped directly into shell interpreter".to_string(),
            snapshot_category: category.to_string(),
            relative_path: path.to_string(),
            evidence_text: line.trim().to_string(),
            line_number: Some(line_num),
            evidence_fingerprint: fingerprint,
            rationale: "Piping remote network downloads directly into a shell interpreter executes unverified code blindly.".to_string(),
            recommendation: "Download the script to a file, verify checksums or signatures, and inspect contents before execution.".to_string(),
        });
    }
}

/// What: Check a line for privilege escalation via sudo in build scripts.
fn check_sudo_usage(
    package_base: &PackageBase,
    category: &str,
    path: &str,
    line: &str,
    lower: &str,
    line_num: usize,
    findings: &mut Vec<DeterministicFinding>,
) {
    if lower.contains("sudo ") || lower.contains("sudo\t") {
        let detector_id = "sudo-in-build";
        let detector_version = 1;
        let fingerprint = calculate_evidence_fingerprint(
            detector_id,
            detector_version,
            package_base.as_str(),
            category,
            path,
            line.trim(),
            line.trim(),
        );

        findings.push(DeterministicFinding {
            detector_id: detector_id.to_string(),
            detector_version,
            severity: "high".to_string(),
            title: "Privilege escalation command (sudo) found in build recipe or script".to_string(),
            snapshot_category: category.to_string(),
            relative_path: path.to_string(),
            evidence_text: line.trim().to_string(),
            line_number: Some(line_num),
            evidence_fingerprint: fingerprint,
            rationale: "Build scripts and PKGBUILDs should run unprivileged under makepkg/fakeroot. Invoking sudo indicates dangerous elevation.".to_string(),
            recommendation: "Remove sudo calls from build instructions and use fakeroot/pkgdir staging instead.".to_string(),
        });
    }
}

/// What: Check a line for insecure unencrypted HTTP downloads or disabled TLS verification.
fn check_insecure_http(
    package_base: &PackageBase,
    category: &str,
    path: &str,
    line: &str,
    lower: &str,
    line_num: usize,
    findings: &mut Vec<DeterministicFinding>,
) {
    let has_http = lower.contains("http://")
        && !lower.contains("http://localhost")
        && !lower.contains("http://127.0.0.1");
    let has_insecure_curl =
        lower.contains("curl ") && (lower.contains(" -k") || lower.contains("--insecure"));
    let has_insecure_wget = lower.contains("wget ") && lower.contains("--no-check-certificate");

    if has_http || has_insecure_curl || has_insecure_wget {
        let detector_id = "insecure-http-download";
        let detector_version = 1;
        let fingerprint = calculate_evidence_fingerprint(
            detector_id,
            detector_version,
            package_base.as_str(),
            category,
            path,
            line.trim(),
            line.trim(),
        );

        findings.push(DeterministicFinding {
            detector_id: detector_id.to_string(),
            detector_version,
            severity: "medium".to_string(),
            title: "Unencrypted HTTP download or disabled TLS certificate verification".to_string(),
            snapshot_category: category.to_string(),
            relative_path: path.to_string(),
            evidence_text: line.trim().to_string(),
            line_number: Some(line_num),
            evidence_fingerprint: fingerprint,
            rationale: "Unencrypted HTTP or disabled TLS verification exposes source downloads to man-in-the-middle tampering.".to_string(),
            recommendation: "Use secure HTTPS URLs with valid TLS certificate verification.".to_string(),
        });
    }
}

/// What: Check a line for dangerous file writes targeting root filesystem paths outside $pkgdir.
fn check_root_destdir(
    package_base: &PackageBase,
    category: &str,
    path: &str,
    line: &str,
    lower: &str,
    line_num: usize,
    findings: &mut Vec<DeterministicFinding>,
) {
    let touches_root = lower.contains("rm -rf /usr")
        || lower.contains("rm -rf /etc")
        || lower.contains("cp ") && lower.contains(" /etc/") && !lower.contains("$pkgdir")
        || lower.contains("cp ") && lower.contains(" /usr/") && !lower.contains("$pkgdir");

    if touches_root {
        let detector_id = "root-destdir-write";
        let detector_version = 1;
        let fingerprint = calculate_evidence_fingerprint(
            detector_id,
            detector_version,
            package_base.as_str(),
            category,
            path,
            line.trim(),
            line.trim(),
        );

        findings.push(DeterministicFinding {
            detector_id: detector_id.to_string(),
            detector_version,
            severity: "critical".to_string(),
            title: "Direct write or modification to host root filesystem during build".to_string(),
            snapshot_category: category.to_string(),
            relative_path: path.to_string(),
            evidence_text: line.trim().to_string(),
            line_number: Some(line_num),
            evidence_fingerprint: fingerprint,
            rationale: "Package installation steps must stage files under $pkgdir rather than directly modifying host directories.".to_string(),
            recommendation: "Ensure all install operations use '$pkgdir' or '${pkgdir}' as the destination root.".to_string(),
        });
    }
}
