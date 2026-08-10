//! Injected deterministic coverage for the guided setup no-model Pi probe.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{
    PiSetupProbeBackend, PiSetupProbeError, PiSetupProbeRequest, SETUP_PROBE_RPC_COMMANDS,
    probe_pi_setup, probe_pi_setup_with_backend, resolve_setup_probe_binary,
    run_pi_cli_information_with_timeout,
};
use crate::pi_agent::RESTRICTED_TOOL_NAMES;
use crate::pi_agent::capabilities::REQUIRED_PI_FLAGS;
use crate::pi_agent::client::{RpcTransport, TransportError, TransportMetadata};
use crate::pi_agent::process::{EMBEDDED_EXTENSION_SHA256, PACSEA_EXTENSION_COMMAND, pi_argv};
use crate::pi_agent::protocol::CommandCorrelator;

/// Shared exact commands written to one fake transport.
type CommandLog = Arc<Mutex<Vec<Value>>>;

/// Strict injected transport that responds from supplied metadata without spawning Pi.
struct FakeRpcTransport {
    /// Model records returned by `get_available_models`.
    models: Vec<Value>,
    /// Responses queued after a command write.
    responses: VecDeque<Vec<u8>>,
    /// Shared command log retained by the test.
    commands: CommandLog,
    /// Trusted transport metadata under test.
    metadata: TransportMetadata,
}

impl FakeRpcTransport {
    /// Construct one isolated fake transport.
    fn new(models: Vec<Value>, commands: CommandLog, metadata: TransportMetadata) -> Self {
        Self {
            models,
            responses: VecDeque::new(),
            commands,
            metadata,
        }
    }

    /// Build the exact success response for one metadata command.
    fn response_for(&self, command: &Value) -> Value {
        let id = command.get("id").cloned().unwrap_or(Value::Null);
        let command_type = command
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let data = match command_type {
            "get_available_models" => serde_json::json!({"models": self.models}),
            "get_commands" => serde_json::json!({"commands": [
                {
                    "name": PACSEA_EXTENSION_COMMAND,
                    "sourceInfo": {"scope": "temporary", "source": "cli"}
                }
            ]}),
            "get_state" => serde_json::json!({"session": null}),
            _ => serde_json::json!({}),
        };
        serde_json::json!({
            "id": id,
            "type": "response",
            "command": command_type,
            "success": true,
            "data": data,
        })
    }
}

impl RpcTransport for FakeRpcTransport {
    fn write_record(&mut self, record: &[u8]) -> Result<(), TransportError> {
        let command: Value = serde_json::from_slice(record)
            .map_err(|error| TransportError::Io(error.to_string()))?;
        self.commands
            .lock()
            .map_err(|_| TransportError::Io("command log lock poisoned".to_string()))?
            .push(command.clone());
        let response = self.response_for(&command);
        self.responses.push_back(
            serde_json::to_vec(&response).map_err(|error| TransportError::Io(error.to_string()))?,
        );
        Ok(())
    }

    fn read_record(
        &mut self,
        _deadline: Instant,
        _cancelled: &std::sync::atomic::AtomicBool,
    ) -> Result<Vec<u8>, TransportError> {
        self.responses.pop_front().ok_or(TransportError::Closed)
    }

    fn bytes_exchanged(&self) -> u64 {
        0
    }

    fn metadata(&self) -> TransportMetadata {
        self.metadata.clone()
    }

    fn abort_and_reap(
        &mut self,
        _correlator: &mut CommandCorrelator,
    ) -> Result<(), TransportError> {
        Ok(())
    }

    fn reap(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

/// Fully injected backend controlling CLI, tools, argv, and RPC model records.
struct FakeBackend {
    /// Canonical executable returned to the adapter.
    executable: PathBuf,
    /// Synthetic exact Pi version output.
    version: String,
    /// Synthetic Pi help output.
    help: String,
    /// Model records supplied to the fake RPC transport.
    models: Vec<Value>,
    /// Tool names claimed by the fixed isolated launch.
    tools: Vec<String>,
    /// Fixed isolation argv claimed by this backend.
    argv: Vec<String>,
    /// Trusted transport metadata under test.
    metadata: TransportMetadata,
    /// Shared outbound command log.
    commands: CommandLog,
}

impl FakeBackend {
    /// Build a complete passing backend around supplied model records.
    fn passing(models: Vec<Value>) -> Self {
        Self {
            executable: PathBuf::from("/opt/pi/bin/pi"),
            version: "0.84.0\n".to_string(),
            help: REQUIRED_PI_FLAGS.join("\n"),
            models,
            tools: RESTRICTED_TOOL_NAMES
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            argv: stable_argv(),
            metadata: TransportMetadata {
                extension_sha256: EMBEDDED_EXTENSION_SHA256.to_string(),
                tool_contract_version: crate::pi_agent::TOOL_CONTRACT_VERSION.to_string(),
            },
            commands: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl PiSetupProbeBackend for FakeBackend {
    fn resolve_binary(&mut self, _configured: &str) -> Result<PathBuf, PiSetupProbeError> {
        Ok(self.executable.clone())
    }

    fn run_cli_information(
        &mut self,
        _executable: &Path,
        flag: &'static str,
    ) -> Result<String, PiSetupProbeError> {
        match flag {
            "--version" => Ok(self.version.clone()),
            "--help" => Ok(self.help.clone()),
            _ => Err(PiSetupProbeError::CliInvocation {
                flag,
                reason: "unexpected fake flag".to_string(),
            }),
        }
    }

    fn launch_isolated_rpc(
        &mut self,
        _executable: &Path,
        _workspace_parent: &Path,
    ) -> Result<Box<dyn RpcTransport>, PiSetupProbeError> {
        Ok(Box::new(FakeRpcTransport::new(
            self.models.clone(),
            Arc::clone(&self.commands),
            self.metadata.clone(),
        )))
    }

    fn isolated_tool_names(&self) -> Vec<String> {
        self.tools.clone()
    }

    fn isolation_argv(&self) -> Vec<String> {
        self.argv.clone()
    }
}

/// Return one exact advertised route with Pi-native per-million-token pricing.
fn route(provider: &str, model: &str, input: f64, output: f64) -> Value {
    serde_json::json!({
        "provider": provider,
        "id": model,
        "cost": {"input": input, "output": output},
    })
}

/// Return deterministic passing request policy.
fn request() -> PiSetupProbeRequest {
    PiSetupProbeRequest {
        binary: "pi".to_string(),
        workspace_parent: PathBuf::from("/tmp/pacsea-setup-probe-tests"),
        reservation_tokens: 10_000,
        now_unix_seconds: 1_000_000,
        maximum_pricing_age: Duration::from_mins(5),
    }
}

/// Return the exact stable argv representation expected from every backend.
fn stable_argv() -> Vec<String> {
    pi_argv(Path::new("/__pacsea_verified_extension__"))
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

/// Extract the exact command types written to the injected transport.
fn command_types(log: &CommandLog) -> Vec<String> {
    log.lock()
        .expect("command log")
        .iter()
        .filter_map(|command| {
            command
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

/// The successful adapter must expose exact routes/prices while issuing metadata commands only.
#[test]
fn adapter_exposes_exact_routes_pricing_reservations_without_prompt_or_model_call() {
    let mut backend = FakeBackend::passing(vec![
        route("anthropic", "claude-x", 3.0, 15.0),
        route("openrouter", "vendor/model-y", 1.0, 4.0),
    ]);
    let log = Arc::clone(&backend.commands);
    let snapshot = probe_pi_setup_with_backend(&request(), &mut backend).expect("probe passes");

    assert_eq!(snapshot.executable, PathBuf::from("/opt/pi/bin/pi"));
    assert_eq!(snapshot.pi_version.to_string(), "0.84.0");
    assert_eq!(snapshot.routes.len(), 2);
    assert_eq!(snapshot.routes[0].provider, "anthropic");
    assert_eq!(snapshot.routes[0].model, "claude-x");
    assert_eq!(
        snapshot.routes[0].pricing.rates.output_microusd_per_million,
        15_000_000
    );
    assert_eq!(snapshot.routes[0].reservation.tokens, 10_000);
    assert_eq!(snapshot.routes[0].reservation.cost_microusd, 150_000);
    assert_eq!(
        snapshot
            .exact_route("openrouter", "vendor/model-y")
            .expect("exact route")
            .reservation
            .cost_microusd,
        40_000
    );
    assert!(!snapshot.pricing_binding.is_empty());
    assert_eq!(command_types(&log), SETUP_PROBE_RPC_COMMANDS);
    assert!(
        !command_types(&log)
            .iter()
            .any(|command| { matches!(command.as_str(), "prompt" | "set_model" | "agent") })
    );
}

/// Re-probing unchanged Pi pricing at a later timestamp must preserve the material binding.
#[test]
fn pricing_binding_excludes_observation_time_but_freshness_remains_typed() {
    let models = vec![route("provider", "model", 2.0, 8.0)];
    let mut first_backend = FakeBackend::passing(models.clone());
    let first = probe_pi_setup_with_backend(&request(), &mut first_backend).expect("first probe");
    let mut later_request = request();
    later_request.now_unix_seconds += 60;
    let mut second_backend = FakeBackend::passing(models);
    let second =
        probe_pi_setup_with_backend(&later_request, &mut second_backend).expect("second probe");

    assert_eq!(first.pricing_binding, second.pricing_binding);
    assert_ne!(
        first.pricing_observed_at_unix_seconds,
        second.pricing_observed_at_unix_seconds
    );
}

/// Hung CLI information commands must be killed within their configured deadline.
#[cfg(unix)]
#[test]
fn cli_information_probe_has_a_bounded_process_deadline() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = tempfile::tempdir().expect("temporary root");
    let executable = root.path().join("slow-pi");
    std::fs::write(&executable, "#!/bin/sh\nsleep 2\n").expect("write fake Pi");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))
        .expect("make fake Pi executable");
    let started = Instant::now();
    let error =
        run_pi_cli_information_with_timeout(&executable, "--version", Duration::from_millis(50))
            .expect_err("hung CLI must time out");

    assert!(started.elapsed() < Duration::from_secs(1));
    assert!(error.to_string().contains("deadline"));
}

/// Missing, path-relative, and nonexistent binaries must have distinct typed failures.
#[test]
fn binary_resolution_rejects_missing_relative_and_nonexistent_values() {
    assert_eq!(
        resolve_setup_probe_binary(""),
        Err(PiSetupProbeError::MissingBinary)
    );
    assert!(matches!(
        resolve_setup_probe_binary("./pi"),
        Err(PiSetupProbeError::RelativeBinary { .. })
    ));
    assert!(matches!(
        resolve_setup_probe_binary("definitely-missing-pacsea-pi"),
        Err(PiSetupProbeError::BinaryNotFound { .. })
    ));
}

/// Unsupported Pi and missing required tools must fail before route facts are accepted.
#[test]
fn adapter_rejects_unsupported_cli_and_missing_tools() {
    let mut unsupported = FakeBackend::passing(vec![route("p", "m", 1.0, 2.0)]);
    unsupported.version = "0.83.9".to_string();
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut unsupported),
        Err(PiSetupProbeError::UnsupportedCli { .. })
    ));

    let mut missing_tool = FakeBackend::passing(vec![route("p", "m", 1.0, 2.0)]);
    missing_tool.tools.pop();
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut missing_tool),
        Err(PiSetupProbeError::ToolContract { .. })
    ));
}

/// Empty, duplicate, and malformed advertised route sets must fail closed exactly.
#[test]
fn adapter_rejects_empty_duplicate_and_malformed_routes() {
    let mut empty = FakeBackend::passing(Vec::new());
    assert_eq!(
        probe_pi_setup_with_backend(&request(), &mut empty),
        Err(PiSetupProbeError::EmptyRoutes)
    );

    let mut duplicate =
        FakeBackend::passing(vec![route("p", "m", 1.0, 2.0), route("p", "m", 1.0, 2.0)]);
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut duplicate),
        Err(PiSetupProbeError::DuplicateRoute { .. })
    ));

    let mut malformed = FakeBackend::passing(vec![serde_json::json!({
        "provider": " p ",
        "id": "m",
        "cost": {"input": 1.0, "output": 2.0}
    })]);
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut malformed),
        Err(PiSetupProbeError::MalformedRoute {
            field: "provider",
            ..
        })
    ));
}

/// Missing or malformed prices and expired reviewed facts must remain typed failures.
#[test]
fn adapter_rejects_absent_invalid_and_stale_pricing() {
    let mut absent = FakeBackend::passing(vec![serde_json::json!({
        "provider": "p",
        "id": "m"
    })]);
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut absent),
        Err(PiSetupProbeError::PricingAbsent { .. })
    ));

    let mut invalid = FakeBackend::passing(vec![serde_json::json!({
        "provider": "p",
        "id": "m",
        "cost": {"input": -1.0, "output": 2.0}
    })]);
    assert!(matches!(
        probe_pi_setup_with_backend(&request(), &mut invalid),
        Err(PiSetupProbeError::PricingInvalid { .. })
    ));

    let mut passing = FakeBackend::passing(vec![route("p", "m", 1.0, 2.0)]);
    let snapshot = probe_pi_setup_with_backend(&request(), &mut passing).expect("fresh snapshot");
    assert_eq!(
        snapshot.validate_pricing_freshness(1_000_301),
        Err(PiSetupProbeError::PricingStale {
            observed_at_unix_seconds: 1_000_000,
            now_unix_seconds: 1_000_301,
            maximum_age_seconds: 300,
        })
    );
}

/// The installed supported Pi can complete this exact probe without a prompt/model command.
#[test]
#[ignore = "requires installed Pi >=0.84.0 with configured priced routes"]
fn live_setup_probe_enumerates_priced_routes_without_model_call() {
    let temp = tempfile::tempdir().expect("private setup workspace");
    let request = PiSetupProbeRequest {
        binary: "pi".to_string(),
        workspace_parent: temp.path().join("workspaces"),
        reservation_tokens: 10_000,
        now_unix_seconds: 1_000_000,
        maximum_pricing_age: Duration::from_mins(5),
    };
    let snapshot = probe_pi_setup(&request).expect("installed Pi setup metadata probe");
    assert!(!snapshot.routes.is_empty());
    assert!(
        snapshot
            .routes
            .iter()
            .all(|route| route.reservation.tokens == 10_000)
    );
    assert!(snapshot.validate_pricing_freshness(1_000_000).is_ok());
}

/// Exact route lookup must never trim, normalize, or substitute another route.
#[test]
fn exact_route_lookup_never_normalizes_or_substitutes() {
    let mut backend = FakeBackend::passing(vec![route("Provider", "model-v1", 1.0, 2.0)]);
    let snapshot = probe_pi_setup_with_backend(&request(), &mut backend).expect("probe passes");
    assert!(snapshot.exact_route("Provider", "model-v1").is_ok());
    assert!(matches!(
        snapshot.exact_route("provider", "model-v1"),
        Err(PiSetupProbeError::RouteNotAdvertised { .. })
    ));
    assert!(matches!(
        snapshot.exact_route("Provider", "model-v1 "),
        Err(PiSetupProbeError::RouteNotAdvertised { .. })
    ));
}
