//! Bounded logical Pi scan orchestration over an injectable strict RPC transport.

use std::fmt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use super::client::{PiRpcClient, RpcTransport, TransportError};
use super::protocol::{CommandCorrelator, decode_record, encode_command};
use super::restricted_tools::SnapshotRegistry;
use super::session::{FailureDecision, ModelChoice, ScanAttemptController, SessionAction};
use crate::logic::pi_scan::prompt::{
    PROMPT_VERSION, PackagePromptInput, SCHEMA_VERSION, build_correction_prompt,
    build_package_prompt, build_system_prompt,
};
use crate::logic::pi_scan::result::{
    AttributedResult, EvidenceIndex, ExpectedIdentity, MergedScanResult, ModelAttemptRecord,
    ScanProvenance, UsageAccounting, merge_results, validate_response,
};

/// Maximum configured model attempt duration.
const MAX_ATTEMPT_TIMEOUT: Duration = Duration::from_mins(5);
/// Maximum configured logical scan duration.
const MAX_LOGICAL_TIMEOUT: Duration = Duration::from_mins(12);

/// What: Borrowed immutable inputs for one logical scan.
///
/// Inputs: Frozen prompt identity, evidence, confirmed models, deadlines, and cancellation.
///
/// Output: Consumed by [`execute_with_transport`].
///
/// Details:
/// - Snapshot roots are intentionally absent; only the production launcher receives them.
/// - The engine never executes package code, follows source instructions, or fetches URLs.
#[derive(Clone, Copy)]
pub struct ScanExecutionInput<'a> {
    /// Bounded package prompt summaries.
    pub prompt: &'a PackagePromptInput,
    /// Frozen response identity.
    pub identity: &'a ExpectedIdentity,
    /// Manifest-backed exact evidence.
    pub evidence: &'a EvidenceIndex,
    /// Explicitly confirmed primary and fallback models.
    pub models: &'a [ModelChoice],
    /// Explicit Pi thinking level applied after every model selection.
    pub thinking: &'a str,
    /// Verified Pi version string.
    pub pi_version: &'a str,
    /// Per-model-attempt wall deadline.
    pub model_attempt_timeout: Duration,
    /// Whole logical-scan wall deadline.
    pub logical_timeout: Duration,
    /// Sticky cancellation flag owned by the caller.
    pub cancelled: &'a AtomicBool,
}

/// What: Production launch inputs plus immutable logical scan data.
///
/// Inputs: Absolute Pi executable, private workspace parent, snapshots, and scan input.
///
/// Output: Consumed by [`execute_scan`].
///
/// Details:
/// - Acquisition has already completed; roots must be immutable snapshots registered by WS1.
#[derive(Clone, Copy)]
pub struct ProductionScanRequest<'a> {
    /// Parent directory for a private ephemeral session workspace.
    pub workspace_parent: &'a Path,
    /// Resolved absolute Pi executable.
    pub executable: &'a Path,
    /// Private descriptor source for immutable snapshot roots.
    pub snapshots: &'a SnapshotRegistry,
    /// Logical scan input.
    pub input: ScanExecutionInput<'a>,
}

/// What: Validated merged result and non-sensitive execution provenance.
///
/// Inputs: Produced only after strict identity/evidence validation.
///
/// Output: Domain-facing completion value for WS3 integration.
///
/// Details:
/// - Contains no raw prompt, source body, thinking, invalid output, or original response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanExecutionOutput {
    /// Deterministically merged attributed findings.
    pub result: MergedScanResult,
    /// Pi/model/tool/schema/usage provenance.
    pub provenance: ScanProvenance,
}

/// What: Fail-closed logical execution failure.
///
/// Inputs: Configuration, transport, RPC, timeout, cancellation, or validation policy.
///
/// Output: Actionable error without raw hostile content.
///
/// Details:
/// - Validation failures are reduced to Pacsea-generated descriptions and are never persisted.
#[derive(Debug)]
pub enum ExecutionError {
    /// Input bounds or identities are invalid.
    InvalidInput(String),
    /// Strict transport or correlated RPC failed.
    Transport(TransportError),
    /// A correlated Pi response reported failure or malformed data.
    Rpc(String),
    /// A per-attempt tool call or tool-result byte budget was exceeded.
    AttemptBudgetExceeded(&'static str),
    /// The current model attempt deadline elapsed.
    AttemptTimeout,
    /// The whole logical deadline elapsed.
    LogicalTimeout,
    /// Sticky user cancellation stopped all later work.
    Cancelled,
    /// Every correction and confirmed fallback was exhausted.
    AttemptsExhausted,
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(reason) => write!(formatter, "invalid Pi scan input: {reason}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Rpc(reason) => write!(formatter, "Pi RPC contract failed: {reason}"),
            Self::AttemptBudgetExceeded(resource) => write!(
                formatter,
                "Pi model attempt exceeded its bounded {resource} allowance"
            ),
            Self::AttemptTimeout => write!(formatter, "Pi model attempt exceeded its deadline"),
            Self::LogicalTimeout => write!(formatter, "Pi logical scan exceeded its deadline"),
            Self::Cancelled => write!(formatter, "Pi logical scan was cancelled"),
            Self::AttemptsExhausted => write!(
                formatter,
                "Pi did not return a valid identity- and evidence-bound result after the confirmed attempts"
            ),
        }
    }
}

impl std::error::Error for ExecutionError {}

impl From<TransportError> for ExecutionError {
    fn from(value: TransportError) -> Self {
        Self::Transport(value)
    }
}

/// What: Launch production Pi and execute one bounded logical scan.
///
/// Inputs:
/// - `request`: Absolute executable, private workspace parent, immutable roots, and scan data.
///
/// Output:
/// - Strictly validated merged output and provenance.
///
/// Details:
/// - Preparation materializes the descriptor and verified embedded extension before direct-argv
///   launch. The transport always reaps and removes private workspaces before return.
///
/// # Errors
/// - Returns a fail-closed launch, RPC, timeout, cancellation, or validation error.
pub fn execute_scan(
    request: ProductionScanRequest<'_>,
) -> Result<ScanExecutionOutput, ExecutionError> {
    let mut client = PiRpcClient::launch(
        request.workspace_parent,
        request.executable,
        request.snapshots,
    )?;
    execute_with_transport(&mut client, request.input)
}

/// What: Execute one logical scan over an injectable raw RPC transport.
///
/// Inputs:
/// - `transport`: Production child or deterministic fake LF-JSONL pipe.
/// - `input`: Frozen scan data, bounds, confirmed models, and cancellation.
///
/// Output:
/// - Strictly validated merged result and provenance.
///
/// Details:
/// - Always calls bounded reap. Cancellation instead uses correlated abort/group reap and is
///   sticky, suppressing correction and fallback.
///
/// # Errors
/// - Returns the first fail-closed execution error; teardown errors replace success only.
pub fn execute_with_transport(
    transport: &mut dyn RpcTransport,
    input: ScanExecutionInput<'_>,
) -> Result<ScanExecutionOutput, ExecutionError> {
    let mut correlator = CommandCorrelator::new();
    let mut run = execute_inner(transport, &input, &mut correlator);
    let cancelled =
        input.cancelled.load(Ordering::SeqCst) || matches!(run, Err(ExecutionError::Cancelled));
    if cancelled && run.is_ok() {
        run = Err(ExecutionError::Cancelled);
    }
    let teardown = if cancelled {
        transport.abort_and_reap(&mut correlator)
    } else {
        transport.reap()
    };
    match (run, teardown) {
        (Ok(output), Ok(())) => Ok(output),
        (Ok(_), Err(error)) => Err(ExecutionError::Transport(error)),
        (Err(error), _) => Err(error),
    }
}

/// Execute preflight, attempt policy, validation, merge, and provenance.
fn execute_inner(
    transport: &mut dyn RpcTransport,
    input: &ScanExecutionInput<'_>,
    correlator: &mut CommandCorrelator,
) -> Result<ScanExecutionOutput, ExecutionError> {
    validate_input(input)?;
    let logical_deadline = Instant::now() + input.logical_timeout;
    let package_prompt = build_package_prompt(input.prompt)
        .map_err(|error| ExecutionError::InvalidInput(error.to_string()))?;
    let full_prompt = format!("{}\n{}", build_system_prompt(), package_prompt);
    let mut controller = ScanAttemptController::new(input.models.to_vec())
        .map_err(|error| ExecutionError::InvalidInput(error.to_string()))?;
    preflight(transport, input, correlator, logical_deadline)?;
    let _ = controller
        .begin()
        .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
    run_attempts(
        transport,
        input,
        correlator,
        &mut controller,
        &full_prompt,
        logical_deadline,
    )
}

/// Validate caller-controlled durations and frozen identity agreement.
fn validate_input(input: &ScanExecutionInput<'_>) -> Result<(), ExecutionError> {
    if input.model_attempt_timeout.is_zero() || input.model_attempt_timeout > MAX_ATTEMPT_TIMEOUT {
        return Err(ExecutionError::InvalidInput(
            "model attempt timeout must be within 1ns..=300s".to_string(),
        ));
    }
    if input.logical_timeout.is_zero() || input.logical_timeout > MAX_LOGICAL_TIMEOUT {
        return Err(ExecutionError::InvalidInput(
            "logical timeout must be within 1ns..=720s".to_string(),
        ));
    }
    if input.prompt.scan_id != input.identity.scan_id
        || input.prompt.package_base != input.identity.package_base
        || input.prompt.commit_oid != input.identity.commit_oid
    {
        return Err(ExecutionError::InvalidInput(
            "package prompt identity does not match the frozen response identity".to_string(),
        ));
    }
    if input.pi_version.is_empty() || super::has_forbidden_control(input.pi_version) {
        return Err(ExecutionError::InvalidInput(
            "verified Pi version is empty or control-bearing".to_string(),
        ));
    }
    Ok(())
}

/// Correlate required state/model/retry commands before any model prompt.
fn preflight(
    transport: &mut dyn RpcTransport,
    input: &ScanExecutionInput<'_>,
    correlator: &mut CommandCorrelator,
    deadline: Instant,
) -> Result<(), ExecutionError> {
    let state = rpc_call(
        transport,
        correlator,
        "get_state",
        &Map::new(),
        deadline,
        deadline,
        input,
    )?;
    require_data_object(&state, "get_state")?;
    let models = rpc_call(
        transport,
        correlator,
        "get_available_models",
        &Map::new(),
        deadline,
        deadline,
        input,
    )?;
    validate_available_models(&models, input.models)?;
    set_model(transport, correlator, &input.models[0], deadline, input)?;
    set_thinking_level(transport, correlator, input.thinking, deadline, input)?;
    let mut retry = Map::new();
    retry.insert("enabled".to_string(), Value::Bool(true));
    rpc_call(
        transport,
        correlator,
        "set_auto_retry",
        &retry,
        deadline,
        deadline,
        input,
    )?;
    Ok(())
}

/// Apply one explicit supported Pi thinking level after model selection.
fn set_thinking_level(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    thinking: &str,
    deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<(), ExecutionError> {
    if !matches!(
        thinking,
        "off" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
    ) {
        return Err(ExecutionError::InvalidInput(format!(
            "unsupported Pi thinking level {thinking:?}"
        )));
    }
    let mut fields = Map::new();
    fields.insert("level".to_string(), Value::String(thinking.to_string()));
    rpc_call(
        transport,
        correlator,
        "set_thinking_level",
        &fields,
        deadline,
        deadline,
        input,
    )?;
    Ok(())
}

/// Run bounded full/correction/fallback attempts until one validates.
fn run_attempts(
    transport: &mut dyn RpcTransport,
    input: &ScanExecutionInput<'_>,
    correlator: &mut CommandCorrelator,
    controller: &mut ScanAttemptController,
    full_prompt: &str,
    logical_deadline: Instant,
) -> Result<ScanExecutionOutput, ExecutionError> {
    let mut records = Vec::new();
    let mut attributed = Vec::new();
    let mut previous_tokens = 0u64;
    loop {
        check_cancelled(input)?;
        let attempt_started = transport.bytes_exchanged();
        let attempt_deadline = bounded_deadline(logical_deadline, input.model_attempt_timeout)?;
        let outcome = run_one_model(
            transport,
            input,
            correlator,
            controller,
            full_prompt,
            attempt_deadline,
            logical_deadline,
        )?;
        let cumulative = session_tokens(
            transport,
            correlator,
            attempt_deadline,
            logical_deadline,
            input,
        )?;
        let reported = cumulative.map(|total| total.saturating_sub(previous_tokens));
        if let Some(total) = cumulative {
            previous_tokens = total;
        }
        let usage = UsageAccounting {
            rpc_bytes: transport.bytes_exchanged().saturating_sub(attempt_started),
            reported_tokens: reported,
        };
        let choice = controller.current_model().clone();
        records.push(ModelAttemptRecord {
            provider: choice.provider.clone(),
            model: choice.model.clone(),
            validated: outcome.result.is_some(),
            corrected: outcome.corrected,
            usage,
        });
        if let Some(result) = outcome.result {
            attributed.push(AttributedResult {
                provider: choice.provider,
                model: choice.model,
                result,
            });
            controller
                .validated()
                .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
            return Ok(build_output(transport, input, records, &attributed));
        }
        match controller
            .validation_failed()
            .map_err(|error| ExecutionError::Rpc(error.to_string()))?
        {
            FailureDecision::Action(SessionAction::SelectModel { .. }) => {
                let choice = controller.current_model().clone();
                set_model(transport, correlator, &choice, logical_deadline, input)?;
                set_thinking_level(
                    transport,
                    correlator,
                    input.thinking,
                    logical_deadline,
                    input,
                )?;
                controller
                    .model_selected()
                    .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
            }
            FailureDecision::Exhausted => return Err(ExecutionError::AttemptsExhausted),
            FailureDecision::Suppressed => return Err(ExecutionError::Cancelled),
            FailureDecision::Action(_) => {
                return Err(ExecutionError::Rpc(
                    "attempt controller returned an out-of-order action".to_string(),
                ));
            }
        }
    }
}

/// Per-model event budgets enforced while waiting for full settlement.
#[derive(Default)]
struct AttemptEventBudget {
    /// Observed tool starts across the full response and its one correction.
    tool_calls: u32,
    /// Conservative wire bytes carrying tool-result events.
    tool_result_bytes: usize,
}

impl AttemptEventBudget {
    /// Observe one bounded Pi event and reject an exhausted attempt budget.
    fn observe(
        &mut self,
        object: &Map<String, Value>,
        wire_bytes: usize,
    ) -> Result<(), ExecutionError> {
        match object.get("type").and_then(Value::as_str) {
            Some("tool_execution_start") => {
                self.tool_calls = self.tool_calls.saturating_add(1);
                if self.tool_calls > super::limits::MAX_TOOL_CALLS_PER_ATTEMPT {
                    return Err(ExecutionError::AttemptBudgetExceeded("tool calls"));
                }
            }
            Some("tool_execution_update" | "tool_execution_end") => {
                self.tool_result_bytes = self.tool_result_bytes.saturating_add(wire_bytes);
                if self.tool_result_bytes > super::limits::MAX_RPC_RECORD_BYTES {
                    return Err(ExecutionError::AttemptBudgetExceeded("tool-result bytes"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Result of one model's full response and optional correction.
struct ModelOutcome {
    /// Validated response, when successful.
    result: Option<crate::logic::pi_scan::result::ValidatedScanResult>,
    /// Whether correction was sent.
    corrected: bool,
}

/// Run one full prompt and at most one correction for the current model.
fn run_one_model(
    transport: &mut dyn RpcTransport,
    input: &ScanExecutionInput<'_>,
    correlator: &mut CommandCorrelator,
    controller: &mut ScanAttemptController,
    full_prompt: &str,
    deadline: Instant,
    logical_deadline: Instant,
) -> Result<ModelOutcome, ExecutionError> {
    let mut budget = AttemptEventBudget::default();
    let raw = prompt_and_read(
        transport,
        correlator,
        full_prompt,
        deadline,
        logical_deadline,
        input,
        &mut budget,
    )?;
    match validate_response(&raw, input.identity, input.evidence) {
        Ok(result) => Ok(ModelOutcome {
            result: Some(result),
            corrected: false,
        }),
        Err(error) => {
            let decision = controller
                .validation_failed()
                .map_err(|session| ExecutionError::Rpc(session.to_string()))?;
            if !matches!(
                decision,
                FailureDecision::Action(SessionAction::SendCorrection { .. })
            ) {
                return Err(ExecutionError::Rpc(
                    "correction controller returned an out-of-order action".to_string(),
                ));
            }
            let correction = build_correction_prompt(&error.to_string())
                .map_err(|prompt| ExecutionError::Rpc(prompt.to_string()))?;
            let raw = prompt_and_read(
                transport,
                correlator,
                &correction,
                deadline,
                logical_deadline,
                input,
                &mut budget,
            )?;
            Ok(ModelOutcome {
                result: validate_response(&raw, input.identity, input.evidence).ok(),
                corrected: true,
            })
        }
    }
}

/// Send one prompt, wait for full settlement, and fetch exactly one final assistant text.
fn prompt_and_read(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    prompt: &str,
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
    budget: &mut AttemptEventBudget,
) -> Result<String, ExecutionError> {
    let mut fields = Map::new();
    fields.insert("message".to_string(), Value::String(prompt.to_string()));
    rpc_call(
        transport,
        correlator,
        "prompt",
        &fields,
        deadline,
        logical_deadline,
        input,
    )?;
    wait_for_settled(
        transport,
        correlator,
        deadline,
        logical_deadline,
        input,
        budget,
    )?;
    let response = rpc_call(
        transport,
        correlator,
        "get_last_assistant_text",
        &Map::new(),
        deadline,
        logical_deadline,
        input,
    )?;
    response
        .pointer("/data/text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ExecutionError::Rpc("get_last_assistant_text returned no text".to_string()))
}

/// Send one correlated command and wait for its successful response.
fn rpc_call(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    command: &str,
    fields: &Map<String, Value>,
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<Value, ExecutionError> {
    check_deadline(deadline, logical_deadline, input)?;
    let id = correlator
        .issue(command)
        .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
    let record = encode_command(&id, command, fields)
        .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
    transport.write_record(&record)?;
    loop {
        let (object, _) = read_object(transport, deadline, logical_deadline, input)?;
        if CommandCorrelator::is_event(&object) {
            continue;
        }
        let settled = correlator
            .settle(&object)
            .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
        if settled != command
            || object.get("command").and_then(Value::as_str) != Some(command)
            || object.get("type").and_then(Value::as_str) != Some("response")
        {
            return Err(ExecutionError::Rpc(
                "a correlated response named the wrong command".to_string(),
            ));
        }
        if object.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(ExecutionError::Rpc(format!(
                "the correlated {command} command failed"
            )));
        }
        return Ok(Value::Object(object));
    }
}

/// Wait until the model has no retry, compaction, or queued continuation remaining.
fn wait_for_settled(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
    budget: &mut AttemptEventBudget,
) -> Result<(), ExecutionError> {
    loop {
        let (object, wire_bytes) = read_object(transport, deadline, logical_deadline, input)?;
        budget.observe(&object, wire_bytes)?;
        if object.get("type").and_then(Value::as_str) == Some("agent_settled")
            && CommandCorrelator::is_event(&object)
        {
            return Ok(());
        }
        if !CommandCorrelator::is_event(&object) {
            correlator
                .settle(&object)
                .map_err(|error| ExecutionError::Rpc(error.to_string()))?;
            return Err(ExecutionError::Rpc(
                "unexpected correlated response while waiting for agent_settled".to_string(),
            ));
        }
    }
}

/// Read and strictly decode one RPC object under cancellation and deadlines.
fn read_object(
    transport: &mut dyn RpcTransport,
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<(Map<String, Value>, usize), ExecutionError> {
    check_deadline(deadline, logical_deadline, input)?;
    match transport.read_record(deadline, input.cancelled) {
        Ok(record) => {
            let wire_bytes = record.len().saturating_add(1);
            let object =
                decode_record(&record).map_err(|error| ExecutionError::Rpc(error.to_string()))?;
            Ok((object, wire_bytes))
        }
        Err(TransportError::Cancelled) => Err(ExecutionError::Cancelled),
        Err(TransportError::Timeout) => {
            if Instant::now() >= deadline {
                check_deadline(deadline, logical_deadline, input)?;
            }
            Err(ExecutionError::AttemptTimeout)
        }
        Err(error) => Err(ExecutionError::Transport(error)),
    }
}

/// Validate all confirmed model choices are advertised exactly.
fn validate_available_models(
    response: &Value,
    choices: &[ModelChoice],
) -> Result<(), ExecutionError> {
    let models = response
        .pointer("/data/models")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ExecutionError::Rpc("get_available_models returned no model list".to_string())
        })?;
    for choice in choices {
        let available = models.iter().any(|model| {
            model.get("provider").and_then(Value::as_str) == Some(&choice.provider)
                && model.get("id").and_then(Value::as_str) == Some(&choice.model)
        });
        if !available {
            return Err(ExecutionError::InvalidInput(format!(
                "confirmed model {}/{} is not available in this Pi session",
                choice.provider, choice.model
            )));
        }
    }
    Ok(())
}

/// Select one exact confirmed provider/model and wait for correlation settlement.
fn set_model(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    choice: &ModelChoice,
    deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<(), ExecutionError> {
    let mut fields = Map::new();
    fields.insert(
        "provider".to_string(),
        Value::String(choice.provider.clone()),
    );
    fields.insert("modelId".to_string(), Value::String(choice.model.clone()));
    rpc_call(
        transport,
        correlator,
        "set_model",
        &fields,
        deadline,
        deadline,
        input,
    )?;
    Ok(())
}

/// Read cumulative session token usage, accepting absence as conservative fallback.
fn session_tokens(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<Option<u64>, ExecutionError> {
    let response = rpc_call(
        transport,
        correlator,
        "get_session_stats",
        &Map::new(),
        deadline,
        logical_deadline,
        input,
    )?;
    Ok(response
        .pointer("/data/tokens/total")
        .and_then(Value::as_u64))
}

/// Require a command response to carry object-shaped data.
fn require_data_object(response: &Value, command: &str) -> Result<(), ExecutionError> {
    if response.get("data").is_some_and(Value::is_object) {
        Ok(())
    } else {
        Err(ExecutionError::Rpc(format!(
            "{command} returned malformed data"
        )))
    }
}

/// Compute an attempt deadline bounded by the logical deadline.
fn bounded_deadline(logical: Instant, attempt: Duration) -> Result<Instant, ExecutionError> {
    let now = Instant::now();
    if now >= logical {
        return Err(ExecutionError::LogicalTimeout);
    }
    Ok((now + attempt).min(logical))
}

/// Fail promptly on sticky cancellation.
fn check_cancelled(input: &ScanExecutionInput<'_>) -> Result<(), ExecutionError> {
    if input.cancelled.load(Ordering::SeqCst) {
        Err(ExecutionError::Cancelled)
    } else {
        Ok(())
    }
}

/// Distinguish attempt and logical timeout at the current deadline.
fn check_deadline(
    deadline: Instant,
    logical_deadline: Instant,
    input: &ScanExecutionInput<'_>,
) -> Result<(), ExecutionError> {
    check_cancelled(input)?;
    let now = Instant::now();
    if now < deadline {
        return Ok(());
    }
    if now >= logical_deadline {
        Err(ExecutionError::LogicalTimeout)
    } else {
        Err(ExecutionError::AttemptTimeout)
    }
}

/// Assemble the only persistable execution output from validated typed values.
fn build_output(
    transport: &dyn RpcTransport,
    input: &ScanExecutionInput<'_>,
    records: Vec<ModelAttemptRecord>,
    attributed: &[AttributedResult],
) -> ScanExecutionOutput {
    let metadata = transport.metadata();
    ScanExecutionOutput {
        result: merge_results(input.identity, attributed),
        provenance: ScanProvenance {
            pi_version: input.pi_version.to_string(),
            extension_sha256: metadata.extension_sha256,
            prompt_version: PROMPT_VERSION.to_string(),
            schema_version: SCHEMA_VERSION.to_string(),
            tool_contract_version: metadata.tool_contract_version,
            attempts: records,
        },
    }
}
