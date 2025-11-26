//! Unit tests for preflight summary computation.

use super::*;
use crate::state::types::Source;
use std::collections::HashMap;
use std::os::unix::process::ExitStatusExt;
use std::sync::Mutex;

type MockCommandKey = (String, Vec<String>);
type MockCommandResult = Result<String, CommandError>;
type MockResponseMap = HashMap<MockCommandKey, MockCommandResult>;

#[derive(Default)]
struct MockRunner {
    responses: Mutex<MockResponseMap>,
}

impl MockRunner {
    #[allow(clippy::missing_const_for_fn)]
    fn with(responses: MockResponseMap) -> Self {
        Self {
            responses: Mutex::new(responses),
        }
    }
}

impl CommandRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, CommandError> {
        let key = (
            program.to_string(),
            args.iter().map(ToString::to_string).collect::<Vec<_>>(),
        );
        let mut guard = self.responses.lock().expect("poisoned responses mutex");
        guard.remove(&key).unwrap_or_else(|| {
            Err(CommandError::Failed {
                program: program.to_string(),
                args: args.iter().map(ToString::to_string).collect(),
                status: std::process::ExitStatus::from_raw(1),
            })
        })
    }
}

#[test]
/// What: Ensure core package major bumps elevate risk and populate notes.
///
/// Inputs:
/// - Single core package (`systemd`) transitioning from `1.0.0` to `2.0.0`.
///
/// Output:
/// - Risk score escalates to the "High" bucket with appropriate notes and chip totals.
fn summary_identifies_core_major_bump() {
    let mut responses = HashMap::new();
    responses.insert(
        ("pacman".into(), vec!["-Q".into(), "systemd".into()]),
        Ok("systemd 1.0.0\n".to_string()),
    );
    responses.insert(
        ("pacman".into(), vec!["-Qi".into(), "systemd".into()]),
        Ok("Name            : systemd\nInstalled Size  : 4.00 MiB\n".to_string()),
    );
    responses.insert(
        ("pacman".into(), vec!["-Si".into(), "extra/systemd".into()]),
        Ok("Repository      : extra\nName            : systemd\nVersion         : 2.0.0\nDownload Size   : 2.00 MiB\nInstalled Size  : 5.00 MiB\n".to_string()),
    );

    let runner = MockRunner::with(responses);
    let item = PackageItem {
        name: "systemd".into(),
        version: "2.0.0".into(),
        description: "system init".into(),
        source: Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

    let outcome = compute_preflight_summary_with_runner(&[item], PreflightAction::Install, &runner);

    assert_eq!(outcome.summary.package_count, 1);
    assert_eq!(outcome.summary.aur_count, 0);
    assert_eq!(outcome.summary.risk_score, 5);
    assert_eq!(outcome.summary.risk_level, RiskLevel::High);
    assert!(
        outcome
            .summary
            .major_bump_packages
            .iter()
            .any(|name| name == "systemd")
    );
    assert!(
        outcome
            .summary
            .core_system_updates
            .iter()
            .any(|name| name == "systemd")
    );
    assert_eq!(
        outcome.summary.download_bytes,
        2 * 1024 * 1024,
        "Download bytes should match pacman -Si output"
    );
    assert_eq!(
        outcome.summary.install_delta_bytes,
        i64::from(5 * 1024 * 1024) - i64::from(4 * 1024 * 1024),
        "Install delta should reflect target minus current size"
    );
    assert!(
        outcome
            .summary
            .summary_warnings
            .iter()
            .any(|reason| reason.contains("Core/system"))
    );
    assert_eq!(outcome.header.risk_score, 5);
    assert_eq!(outcome.header.package_count, 1);
    assert_eq!(
        outcome.summary.packages[0].install_delta_bytes,
        Some(i64::from(5 * 1024 * 1024) - i64::from(4 * 1024 * 1024))
    );
}

#[test]
/// What: Confirm AUR-only transactions contribute to risk heuristics even without metadata.
///
/// Inputs:
/// - Single AUR package with no pacman metadata responses configured.
///
/// Output:
/// - Risk score increments by the AUR heuristic and remains within the "Medium" bucket.
fn summary_handles_aur_without_metadata() {
    let runner = MockRunner::default();
    let item = PackageItem {
        name: "my-aur-tool".into(),
        version: "1.4.0".into(),
        description: "AUR utility".into(),
        source: Source::Aur,
        popularity: Some(42.0),
        out_of_date: None,
        orphaned: false,
    };

    let outcome = compute_preflight_summary_with_runner(&[item], PreflightAction::Install, &runner);

    assert_eq!(outcome.summary.package_count, 1);
    assert_eq!(outcome.summary.aur_count, 1);
    assert_eq!(outcome.summary.risk_score, 2);
    assert_eq!(outcome.summary.risk_level, RiskLevel::Medium);
    assert!(
        outcome
            .summary
            .risk_reasons
            .iter()
            .any(|reason| reason.contains("AUR"))
    );
    assert_eq!(outcome.header.aur_count, 1);
}
