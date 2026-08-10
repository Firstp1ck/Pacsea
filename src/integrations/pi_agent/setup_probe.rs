//! No-model Pi capability, route, and pricing probe for guided scanner setup.
//!
//! The adapter launches Pi only through the existing isolated direct-argv process
//! boundary. It sends metadata RPC commands only: no `prompt`, `set_model`, or
//! other model-execution command is ever issued. Raw RPC records are parsed in
//! memory, never returned, persisted, or logged.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use crate::install::resolve_command_on_path;
use crate::logic::pi_scan::pricing::{
    PricingAccounting, PricingSource, RoutePricing, pricing_from_pi_model_cost,
    reserve_worst_case_microusd,
};
use crate::logic::pi_scan::result::UsageAccounting;
use crate::pi_agent::capabilities::{
    CommandDescriptor, validate_active_tools, validate_cli_contract, validate_command_inventory,
};
use crate::pi_agent::client::{PiRpcClient, RpcTransport, TransportMetadata};
use crate::pi_agent::process::{
    EMBEDDED_EXTENSION_SHA256, PACSEA_EXTENSION_COMMAND, configure_environment, pi_argv,
};
use crate::pi_agent::protocol::{CommandCorrelator, decode_record, encode_command};
use crate::pi_agent::restricted_tools::SnapshotRegistry;
use crate::pi_agent::{
    PiVersion, RESTRICTED_TOOL_NAMES, TOOL_CONTRACT_VERSION, has_forbidden_control,
};
use crate::state::pi_scan::PiScanReservation;

/// Largest accepted output from one inert Pi CLI information command.
const MAX_CLI_OUTPUT_BYTES: usize = 1024 * 1024;
/// Whole-process deadline for one inert Pi CLI information command.
const CLI_INFORMATION_TIMEOUT: Duration = Duration::from_secs(15);

/// Fixed logical-scan token reservation reviewed during setup and runtime revalidation.
pub(crate) const SETUP_PROBE_RESERVATION_TOKENS: u64 = 500_000;
/// Maximum age accepted for one exact setup pricing observation.
pub(crate) const SETUP_PROBE_MAXIMUM_PRICING_AGE: Duration = Duration::from_mins(15);
/// Metadata-only RPC commands issued by the setup probe, in exact order.
pub const SETUP_PROBE_RPC_COMMANDS: [&str; 3] =
    ["get_available_models", "get_commands", "get_state"];

/// Stable provenance label for live Pi model metadata.
pub const PI_MODEL_PRICING_PROVENANCE: &str = "pi-rpc:get_available_models/Model.cost";

/// What: Inputs for one isolated setup probe.
///
/// Inputs:
/// - Configured binary, private workspace, reservation tokens, deterministic clock, and freshness.
///
/// Output:
/// - Immutable policy consumed by [`probe_pi_setup`] or [`probe_pi_setup_with_backend`].
///
/// Details:
/// - A bare executable name may resolve through `PATH`; path-like relative values are rejected.
/// - `now_unix_seconds` is supplied by the caller so validation remains deterministic in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiSetupProbeRequest {
    /// Bare executable name or absolute executable path.
    pub binary: String,
    /// Absolute private parent for the ephemeral isolated Pi session.
    pub workspace_parent: PathBuf,
    /// Worst-case token count used for every advertised route reservation.
    pub reservation_tokens: u64,
    /// Caller-supplied current Unix timestamp.
    pub now_unix_seconds: u64,
    /// Maximum age accepted when a snapshot is rebound before Apply.
    pub maximum_pricing_age: Duration,
}

/// What: Exact verified isolation facts returned to setup.
///
/// Inputs:
/// - Validated CLI help, trusted transport metadata, fixed argv, and tool contract.
///
/// Output:
/// - Credential-free provenance that can be bound into consent.
///
/// Details:
/// - The ephemeral extension path is represented by a stable placeholder, not retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiSetupIsolationContract {
    /// Verified restricted-tool contract version.
    pub tool_contract_version: String,
    /// Verified embedded extension SHA-256.
    pub extension_sha256: String,
    /// Exact sorted active tool names selected by the isolated launch contract.
    pub active_tools: Vec<String>,
    /// Exact isolation argv with the ephemeral extension path replaced by a placeholder.
    pub argv: Vec<String>,
}

/// What: Exact pricing and reservation for one advertised route.
///
/// Inputs:
/// - One strict Pi `Model` record plus the requested token reservation.
///
/// Output:
/// - Exact route identity, normalized integer rates, provenance, and worst-case reservation.
///
/// Details:
/// - Provider and model identifiers are preserved verbatim and matched only by exact equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiSetupAdvertisedRoute {
    /// Exact Pi-advertised provider identifier.
    pub provider: String,
    /// Exact Pi-advertised model identifier.
    pub model: String,
    /// Exact parsed Pi-native pricing.
    pub pricing: RoutePricing,
    /// Exact pricing provenance label.
    pub pricing_provenance: String,
    /// Worst-case reservation at the request's token count.
    pub reservation: PiScanReservation,
}

/// What: Complete credential-free result of one setup probe.
///
/// Inputs:
/// - Verified executable/capability facts and strict advertised route records.
///
/// Output:
/// - The exact API consumed by the setup controller for selection and reviewed binding.
///
/// Details:
/// - Contains no credentials, raw CLI/RPC output, prompts, provider responses, or source content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiSetupProbeSnapshot {
    /// Canonical absolute Pi executable path that was probed.
    pub executable: PathBuf,
    /// Exact supported Pi version.
    pub pi_version: PiVersion,
    /// Exact verified tool/isolation contract.
    pub isolation: PiSetupIsolationContract,
    /// Exact advertised provider/model routes in Pi response order.
    pub routes: Vec<PiSetupAdvertisedRoute>,
    /// Unix timestamp at which live pricing metadata was observed.
    pub pricing_observed_at_unix_seconds: u64,
    /// Maximum age accepted for this pricing snapshot.
    pub maximum_pricing_age: Duration,
    /// SHA-256 over exact version, isolation, routes, prices, provenance, reservations, and time.
    pub pricing_binding: String,
}

impl PiSetupProbeSnapshot {
    /// What: Recheck that this exact pricing snapshot is still fresh.
    ///
    /// Inputs:
    /// - `now_unix_seconds`: Deterministic current Unix timestamp.
    ///
    /// Output:
    /// - `Ok(())` while the reviewed pricing is current.
    ///
    /// Details:
    /// - Clock reversal and age beyond the recorded maximum both fail closed.
    ///
    /// # Errors
    /// - Returns [`PiSetupProbeError::PricingStale`] with actionable re-probe guidance.
    pub const fn validate_pricing_freshness(
        &self,
        now_unix_seconds: u64,
    ) -> Result<(), PiSetupProbeError> {
        let Some(age_seconds) = now_unix_seconds.checked_sub(self.pricing_observed_at_unix_seconds)
        else {
            return Err(PiSetupProbeError::PricingStale {
                observed_at_unix_seconds: self.pricing_observed_at_unix_seconds,
                now_unix_seconds,
                maximum_age_seconds: self.maximum_pricing_age.as_secs(),
            });
        };
        if age_seconds <= self.maximum_pricing_age.as_secs() {
            Ok(())
        } else {
            Err(PiSetupProbeError::PricingStale {
                observed_at_unix_seconds: self.pricing_observed_at_unix_seconds,
                now_unix_seconds,
                maximum_age_seconds: self.maximum_pricing_age.as_secs(),
            })
        }
    }

    /// What: Resolve one exact advertised provider/model route.
    ///
    /// Inputs:
    /// - `provider`: Exact provider identifier.
    /// - `model`: Exact model identifier.
    ///
    /// Output:
    /// - The matching route pricing and reservation.
    ///
    /// Details:
    /// - No trimming, normalization, fallback, fuzzy matching, or substitution occurs.
    ///
    /// # Errors
    /// - Returns [`PiSetupProbeError::RouteNotAdvertised`] when the exact pair is absent.
    pub fn exact_route(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<&PiSetupAdvertisedRoute, PiSetupProbeError> {
        self.routes
            .iter()
            .find(|route| route.provider == provider && route.model == model)
            .ok_or_else(|| PiSetupProbeError::RouteNotAdvertised {
                provider: provider.to_string(),
                model: model.to_string(),
            })
    }
}

/// What: Typed fail-closed setup probe errors.
///
/// Inputs:
/// - Produced by binary resolution, capability checks, isolated RPC, route parsing, and freshness.
///
/// Output:
/// - Actionable user-facing guidance without raw Pi or provider output.
///
/// Details:
/// - Variants deliberately carry only bounded identifiers and operation names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiSetupProbeError {
    /// No executable was configured.
    MissingBinary,
    /// A path-like relative executable was supplied.
    RelativeBinary {
        /// Rejected configured value.
        configured: String,
    },
    /// A bare name or absolute path could not be resolved to an executable file.
    BinaryNotFound {
        /// Rejected configured value.
        configured: String,
    },
    /// The private setup workspace was not absolute.
    RelativeWorkspace {
        /// Rejected workspace path.
        workspace: PathBuf,
    },
    /// The token reservation was zero.
    EmptyReservation,
    /// An inert CLI information command failed without exposing its output.
    CliInvocation {
        /// Information flag that failed.
        flag: &'static str,
        /// Bounded failure category.
        reason: String,
    },
    /// Pi version or required CLI flags do not satisfy the supported contract.
    UnsupportedCli {
        /// Complete actionable capability reasons.
        reasons: Vec<String>,
    },
    /// Isolated Pi startup or teardown failed.
    Isolation {
        /// Bounded process/transport reason.
        reason: String,
    },
    /// Trusted extension/tool metadata did not match the compiled isolation contract.
    ToolContract {
        /// Complete actionable mismatch reasons.
        reasons: Vec<String>,
    },
    /// A metadata-only RPC command failed or returned an invalid envelope.
    Rpc {
        /// Exact metadata command being settled.
        command: &'static str,
        /// Bounded failure category without response content.
        reason: String,
    },
    /// Pi advertised no configured routes.
    EmptyRoutes,
    /// One advertised route record was malformed.
    MalformedRoute {
        /// Zero-based record index.
        index: usize,
        /// Malformed field name.
        field: &'static str,
        /// Actionable bounded reason.
        reason: String,
    },
    /// Pi advertised the same exact route more than once.
    DuplicateRoute {
        /// Exact duplicated provider identifier.
        provider: String,
        /// Exact duplicated model identifier.
        model: String,
    },
    /// One advertised route omitted exact pricing.
    PricingAbsent {
        /// Exact provider identifier.
        provider: String,
        /// Exact model identifier.
        model: String,
    },
    /// One advertised route carried invalid exact pricing.
    PricingInvalid {
        /// Exact provider identifier.
        provider: String,
        /// Exact model identifier.
        model: String,
        /// Bounded parser reason.
        reason: String,
    },
    /// Reviewed pricing facts are absent from the freshness window.
    PricingStale {
        /// Snapshot observation time.
        observed_at_unix_seconds: u64,
        /// Recheck time.
        now_unix_seconds: u64,
        /// Accepted maximum age.
        maximum_age_seconds: u64,
    },
    /// A requested exact route was not advertised by the verified snapshot.
    RouteNotAdvertised {
        /// Requested provider identifier.
        provider: String,
        /// Requested model identifier.
        model: String,
    },
}

impl fmt::Display for PiSetupProbeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingBinary => write!(
                formatter,
                "no Pi executable was configured; enter an absolute path or a bare executable name on PATH and retry"
            ),
            Self::RelativeBinary { configured } => write!(
                formatter,
                "Pi executable {configured:?} is a relative path; use an absolute path or a bare executable name on PATH"
            ),
            Self::BinaryNotFound { configured } => write!(
                formatter,
                "Pi executable {configured:?} was not found or is not executable; install supported Pi or choose its absolute executable path"
            ),
            Self::RelativeWorkspace { workspace } => write!(
                formatter,
                "Pi setup workspace {} is relative; configure an absolute private Pacsea state path and retry",
                workspace.display()
            ),
            Self::EmptyReservation => write!(
                formatter,
                "the Pi setup reservation token count is zero; choose a positive bounded token cap before reviewing pricing"
            ),
            Self::CliInvocation { flag, reason } => write!(
                formatter,
                "could not verify Pi {flag} ({reason}); check the executable and retry setup"
            ),
            Self::UnsupportedCli { reasons } => write!(
                formatter,
                "the selected Pi executable is unsupported; update Pi and retry setup: {}",
                reasons.join("; ")
            ),
            Self::Isolation { reason } => write!(
                formatter,
                "could not run the isolated no-model Pi metadata probe ({reason}); check the private Pacsea state directory and retry"
            ),
            Self::ToolContract { reasons } => write!(
                formatter,
                "Pi did not satisfy the exact scanner tool/isolation contract; reinstall Pacsea or update Pi and retry: {}",
                reasons.join("; ")
            ),
            Self::Rpc { command, reason } => write!(
                formatter,
                "Pi metadata command {command} failed ({reason}); no model call was made. Update Pi or retry setup"
            ),
            Self::EmptyRoutes => write!(
                formatter,
                "Pi advertised no provider/model routes; configure provider access in Pi outside Pacsea, then retry setup"
            ),
            Self::MalformedRoute {
                index,
                field,
                reason,
            } => write!(
                formatter,
                "Pi advertised malformed route record {index} field {field} ({reason}); update Pi or repair its model configuration, then retry"
            ),
            Self::DuplicateRoute { provider, model } => write!(
                formatter,
                "Pi advertised duplicate exact route {provider}/{model}; update Pi or repair its model configuration, then retry"
            ),
            Self::PricingAbsent { provider, model } => write!(
                formatter,
                "Pi route {provider}/{model} has no exact pricing; choose a priced route after reconfiguring Pi and retry"
            ),
            Self::PricingInvalid {
                provider,
                model,
                reason,
            } => write!(
                formatter,
                "Pi route {provider}/{model} has unusable exact pricing ({reason}); update Pi pricing metadata or choose another route"
            ),
            Self::PricingStale {
                observed_at_unix_seconds,
                now_unix_seconds,
                maximum_age_seconds,
            } => write!(
                formatter,
                "Pi pricing reviewed at Unix time {observed_at_unix_seconds} is stale at {now_unix_seconds} (maximum age {maximum_age_seconds}s); re-run the no-model setup probe"
            ),
            Self::RouteNotAdvertised { provider, model } => write!(
                formatter,
                "Pi route {provider}/{model} is not in the exact verified advertised snapshot; select an advertised route or re-run setup"
            ),
        }
    }
}

impl std::error::Error for PiSetupProbeError {}

/// What: Injectable process boundary used by the deterministic setup adapter.
///
/// Inputs:
/// - Binary text, information flags, and isolated workspace launch requests.
///
/// Output:
/// - Resolved executable, bounded CLI text, and strict RPC transport.
///
/// Details:
/// - Production uses [`SystemPiSetupProbeBackend`]; tests provide fakes without spawning Pi.
pub trait PiSetupProbeBackend {
    /// Resolve and canonicalize the configured Pi executable.
    ///
    /// # Errors
    /// - Returns a typed missing, relative, or non-executable error.
    fn resolve_binary(&mut self, configured: &str) -> Result<PathBuf, PiSetupProbeError>;

    /// Run one inert Pi CLI information flag.
    ///
    /// # Errors
    /// - Returns a bounded typed invocation failure without retaining raw output.
    fn run_cli_information(
        &mut self,
        executable: &Path,
        flag: &'static str,
    ) -> Result<String, PiSetupProbeError>;

    /// Launch one isolated direct-argv Pi RPC transport.
    ///
    /// # Errors
    /// - Returns a bounded typed isolation failure.
    fn launch_isolated_rpc(
        &mut self,
        executable: &Path,
        workspace_parent: &Path,
    ) -> Result<Box<dyn RpcTransport>, PiSetupProbeError>;

    /// Return the exact tool names selected by the backend's fixed isolated argv.
    fn isolated_tool_names(&self) -> Vec<String>;

    /// Return the backend's fixed isolation argv with no ephemeral path retained.
    fn isolation_argv(&self) -> Vec<String>;
}

/// Production direct-argv backend using the existing positive environment policy.
#[derive(Debug, Default)]
pub struct SystemPiSetupProbeBackend;

impl PiSetupProbeBackend for SystemPiSetupProbeBackend {
    fn resolve_binary(&mut self, configured: &str) -> Result<PathBuf, PiSetupProbeError> {
        resolve_setup_probe_binary(configured)
    }

    fn run_cli_information(
        &mut self,
        executable: &Path,
        flag: &'static str,
    ) -> Result<String, PiSetupProbeError> {
        run_pi_cli_information(executable, flag)
    }

    fn launch_isolated_rpc(
        &mut self,
        executable: &Path,
        workspace_parent: &Path,
    ) -> Result<Box<dyn RpcTransport>, PiSetupProbeError> {
        PiRpcClient::launch(workspace_parent, executable, &SnapshotRegistry::new())
            .map(|client| Box::new(client) as Box<dyn RpcTransport>)
            .map_err(|error| PiSetupProbeError::Isolation {
                reason: error.to_string(),
            })
    }

    fn isolated_tool_names(&self) -> Vec<String> {
        RESTRICTED_TOOL_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect()
    }

    fn isolation_argv(&self) -> Vec<String> {
        stable_isolation_argv()
    }
}

/// What: Run the production no-model Pi setup probe.
///
/// Inputs:
/// - `request`: Binary, workspace, token reservation, clock, and freshness policy.
///
/// Output:
/// - Exact verified capability/model/pricing snapshot.
///
/// Details:
/// - Uses direct argv and the existing positive environment policy.
/// - Issues only [`SETUP_PROBE_RPC_COMMANDS`], none of which can call a model.
///
/// # Errors
/// - Returns typed actionable failures for every fail-closed probe stage.
pub fn probe_pi_setup(
    request: &PiSetupProbeRequest,
) -> Result<PiSetupProbeSnapshot, PiSetupProbeError> {
    probe_pi_setup_with_backend(request, &mut SystemPiSetupProbeBackend)
}

/// What: Run the setup probe through an injected deterministic backend.
///
/// Inputs:
/// - `request`: Exact probe policy.
/// - `backend`: Fake or production process boundary.
///
/// Output:
/// - Exact verified capability/model/pricing snapshot.
///
/// Details:
/// - The shared parser and validator are identical for fake and production transports.
///
/// # Errors
/// - Returns the same typed failures as [`probe_pi_setup`].
pub fn probe_pi_setup_with_backend(
    request: &PiSetupProbeRequest,
    backend: &mut dyn PiSetupProbeBackend,
) -> Result<PiSetupProbeSnapshot, PiSetupProbeError> {
    validate_request(request)?;
    let executable = backend.resolve_binary(&request.binary)?;
    let version_output = backend.run_cli_information(&executable, "--version")?;
    let help_output = backend.run_cli_information(&executable, "--help")?;
    let pi_version = validate_cli_contract(&version_output, &help_output).map_err(|failure| {
        PiSetupProbeError::UnsupportedCli {
            reasons: failure.reasons,
        }
    })?;
    let mut transport = backend.launch_isolated_rpc(&executable, &request.workspace_parent)?;
    let metadata = transport.metadata();
    let mut correlator = CommandCorrelator::new();
    let probe = collect_rpc_metadata(transport.as_mut(), &mut correlator);
    let teardown = transport
        .reap()
        .map_err(|error| PiSetupProbeError::Isolation {
            reason: error.to_string(),
        });
    let (models, commands) = match (probe, teardown) {
        (_, Err(error)) | (Err(error), Ok(())) => return Err(error),
        (Ok(result), Ok(())) => result,
    };
    let tool_names = backend.isolated_tool_names();
    let isolation =
        validate_isolation_contract(metadata, &tool_names, backend.isolation_argv(), &commands)?;
    let routes = parse_advertised_routes(&models, request.reservation_tokens)?;
    build_snapshot(request, executable, pi_version, isolation, routes)
}

/// Validate request invariants before resolving or launching anything.
fn validate_request(request: &PiSetupProbeRequest) -> Result<(), PiSetupProbeError> {
    if !request.workspace_parent.is_absolute() {
        return Err(PiSetupProbeError::RelativeWorkspace {
            workspace: request.workspace_parent.clone(),
        });
    }
    if request.reservation_tokens == 0 {
        return Err(PiSetupProbeError::EmptyReservation);
    }
    Ok(())
}

/// Resolve a bare command through PATH or canonicalize one absolute executable path.
fn resolve_setup_probe_binary(configured: &str) -> Result<PathBuf, PiSetupProbeError> {
    let trimmed = configured.trim();
    if trimmed.is_empty() {
        return Err(PiSetupProbeError::MissingBinary);
    }
    let candidate = Path::new(trimmed);
    if !candidate.is_absolute() && candidate.components().count() > 1 {
        return Err(PiSetupProbeError::RelativeBinary {
            configured: trimmed.to_string(),
        });
    }
    let resolved =
        resolve_command_on_path(trimmed).ok_or_else(|| PiSetupProbeError::BinaryNotFound {
            configured: trimmed.to_string(),
        })?;
    let canonical = resolved
        .canonicalize()
        .map_err(|_| PiSetupProbeError::BinaryNotFound {
            configured: trimmed.to_string(),
        })?;
    if canonical.is_absolute() && canonical.is_file() {
        Ok(canonical)
    } else {
        Err(PiSetupProbeError::BinaryNotFound {
            configured: trimmed.to_string(),
        })
    }
}

/// Run one bounded information-only Pi CLI command with direct argv and positive environment.
fn run_pi_cli_information(
    executable: &Path,
    flag: &'static str,
) -> Result<String, PiSetupProbeError> {
    run_pi_cli_information_with_timeout(executable, flag, CLI_INFORMATION_TIMEOUT)
}

/// Run one direct-argv CLI information probe with bounded time and captured bytes.
fn run_pi_cli_information_with_timeout(
    executable: &Path,
    flag: &'static str,
    timeout: Duration,
) -> Result<String, PiSetupProbeError> {
    let mut command = Command::new(executable);
    command
        .arg(flag)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_environment(&mut command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|_| cli_invocation_error(flag, "the process could not be started"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| cli_invocation_error(flag, "stdout capture was unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| cli_invocation_error(flag, "stderr capture was unavailable"))?;
    let stdout_reader = spawn_bounded_reader(stdout);
    let stderr_reader = spawn_bounded_reader(stderr);
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                terminate_cli_child(&mut child);
                drop(stdout_reader.join());
                drop(stderr_reader.join());
                return Err(cli_invocation_error(
                    flag,
                    "the process exceeded its deadline",
                ));
            }
            Err(_) => {
                terminate_cli_child(&mut child);
                drop(stdout_reader.join());
                drop(stderr_reader.join());
                return Err(cli_invocation_error(
                    flag,
                    "process status could not be read",
                ));
            }
        }
    };
    let stdout = join_bounded_reader(stdout_reader, flag)?;
    let stderr = join_bounded_reader(stderr_reader, flag)?;
    if !status.success() {
        return Err(cli_invocation_error(
            flag,
            "the process exited unsuccessfully",
        ));
    }
    let bytes = if stdout.is_empty() { stderr } else { stdout };
    if bytes.len() > MAX_CLI_OUTPUT_BYTES {
        return Err(cli_invocation_error(
            flag,
            "output exceeded the 1 MiB bound",
        ));
    }
    String::from_utf8(bytes).map_err(|_| cli_invocation_error(flag, "output was not UTF-8"))
}

/// Kill and reap one timed-out CLI process group without leaving inherited pipes open.
fn terminate_cli_child(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, killpg};
        use nix::unistd::Pid;

        if let Ok(raw) = i32::try_from(child.id()) {
            let _ = killpg(Pid::from_raw(raw), Signal::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    drop(child.kill());
    drop(child.wait());
}

/// Drain one child pipe while retaining at most one byte beyond the output bound.
fn spawn_bounded_reader<R>(mut reader: R) -> thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let mut retained = Vec::with_capacity(MAX_CLI_OUTPUT_BYTES.saturating_add(1));
        let mut chunk = [0u8; 8192];
        loop {
            let count = reader.read(&mut chunk)?;
            if count == 0 {
                break;
            }
            let remaining = MAX_CLI_OUTPUT_BYTES
                .saturating_add(1)
                .saturating_sub(retained.len());
            retained.extend_from_slice(&chunk[..count.min(remaining)]);
        }
        Ok(retained)
    })
}

/// Join one bounded reader without exposing raw child output in failures.
fn join_bounded_reader(
    reader: thread::JoinHandle<std::io::Result<Vec<u8>>>,
    flag: &'static str,
) -> Result<Vec<u8>, PiSetupProbeError> {
    reader
        .join()
        .map_err(|_| cli_invocation_error(flag, "output reader failed"))?
        .map_err(|_| cli_invocation_error(flag, "output could not be read"))
}

/// Build one bounded CLI invocation failure without retaining raw process output.
fn cli_invocation_error(flag: &'static str, reason: &str) -> PiSetupProbeError {
    PiSetupProbeError::CliInvocation {
        flag,
        reason: reason.to_string(),
    }
}

/// Collect only strict model, command, and state metadata; never retain unsolicited events.
fn collect_rpc_metadata(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
) -> Result<(Value, Vec<CommandDescriptor>), PiSetupProbeError> {
    let timeout = Duration::from_secs(15);
    let models = metadata_rpc_call(transport, correlator, SETUP_PROBE_RPC_COMMANDS[0], timeout)?;
    let commands_value =
        metadata_rpc_call(transport, correlator, SETUP_PROBE_RPC_COMMANDS[1], timeout)?;
    let commands = parse_command_inventory(&commands_value)?;
    let state = metadata_rpc_call(transport, correlator, SETUP_PROBE_RPC_COMMANDS[2], timeout)?;
    if state.pointer("/data").and_then(Value::as_object).is_none() {
        return Err(PiSetupProbeError::Rpc {
            command: "get_state",
            reason: "response omitted the required data object".to_string(),
        });
    }
    Ok((models, commands))
}

/// Send and settle one strict metadata-only RPC command, discarding unsolicited records.
fn metadata_rpc_call(
    transport: &mut dyn RpcTransport,
    correlator: &mut CommandCorrelator,
    command: &'static str,
    timeout: Duration,
) -> Result<Value, PiSetupProbeError> {
    let id = correlator
        .issue(command)
        .map_err(|_| rpc_error(command, "command correlation was rejected"))?;
    let encoded = encode_command(&id, command, &Map::new())
        .map_err(|_| rpc_error(command, "command encoding was rejected"))?;
    transport
        .write_record(&encoded)
        .map_err(|_| rpc_error(command, "command write failed"))?;
    let deadline = Instant::now() + timeout;
    let cancelled = AtomicBool::new(false);
    loop {
        let bytes = transport
            .read_record(deadline, &cancelled)
            .map_err(|error| rpc_error(command, rpc_transport_reason(&error)))?;
        let record = decode_record(&bytes)
            .map_err(|_| rpc_error(command, "response framing or JSON was invalid"))?;
        if record.get("type").and_then(Value::as_str) != Some("response") {
            continue;
        }
        let settled = correlator
            .settle(&record)
            .map_err(|_| rpc_error(command, "response correlation was invalid"))?;
        if settled != command {
            return Err(rpc_error(command, "response settled a different command"));
        }
        if record.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(rpc_error(command, "Pi rejected the metadata command"));
        }
        return Ok(Value::Object(record));
    }
}

/// Reduce transport failures to categories that cannot contain raw provider output.
const fn rpc_transport_reason(error: &crate::pi_agent::client::TransportError) -> &'static str {
    use crate::pi_agent::client::TransportError;
    match error {
        TransportError::Protocol(_) => "response framing was invalid",
        TransportError::Io(_) => "transport I/O failed",
        TransportError::Closed => "Pi closed the metadata stream",
        TransportError::Timeout => "the metadata deadline elapsed",
        TransportError::Cancelled => "the metadata operation was cancelled",
    }
}

/// Construct one bounded typed RPC failure.
fn rpc_error(command: &'static str, reason: impl Into<String>) -> PiSetupProbeError {
    PiSetupProbeError::Rpc {
        command,
        reason: reason.into(),
    }
}

/// Parse exact temporary command provenance from `get_commands`.
fn parse_command_inventory(response: &Value) -> Result<Vec<CommandDescriptor>, PiSetupProbeError> {
    response
        .pointer("/data/commands")
        .and_then(Value::as_array)
        .ok_or_else(|| rpc_error("get_commands", "response omitted the command array"))?
        .iter()
        .map(|command| {
            Ok(CommandDescriptor {
                name: required_command_field(command, "name")?.to_string(),
                scope: required_command_pointer(command, "/sourceInfo/scope")?.to_string(),
                source: required_command_pointer(command, "/sourceInfo/source")?.to_string(),
            })
        })
        .collect()
}

/// Read one required direct string field from a command record.
fn required_command_field<'a>(
    command: &'a Value,
    field: &'static str,
) -> Result<&'a str, PiSetupProbeError> {
    command
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| rpc_error("get_commands", "a command record was malformed"))
}

/// Read one required string pointer from a command record.
fn required_command_pointer<'a>(
    command: &'a Value,
    pointer: &'static str,
) -> Result<&'a str, PiSetupProbeError> {
    command
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| rpc_error("get_commands", "a command provenance record was malformed"))
}

/// Validate trusted transport metadata, exact tools, fixed argv, and command provenance.
fn validate_isolation_contract(
    metadata: TransportMetadata,
    tool_names: &[String],
    argv: Vec<String>,
    commands: &[CommandDescriptor],
) -> Result<PiSetupIsolationContract, PiSetupProbeError> {
    let mut reasons = Vec::new();
    if metadata.extension_sha256 != EMBEDDED_EXTENSION_SHA256 {
        reasons.push(
            "the loaded extension digest differs from the compiled trusted asset".to_string(),
        );
    }
    if metadata.tool_contract_version != TOOL_CONTRACT_VERSION {
        reasons.push(format!(
            "tool contract {:?} differs from required {:?}",
            metadata.tool_contract_version, TOOL_CONTRACT_VERSION
        ));
    }
    let active_tools = match validate_active_tools(tool_names) {
        Ok(tools) => Some(tools),
        Err(failure) => {
            reasons.extend(failure.reasons);
            None
        }
    };
    if argv != stable_isolation_argv() {
        reasons.push(
            "the isolated Pi argv differs from the compiled direct-argv contract".to_string(),
        );
    }
    if let Err(failure) = validate_command_inventory(commands, PACSEA_EXTENSION_COMMAND) {
        reasons.extend(failure.reasons);
    }
    match active_tools {
        Some(active_tools) if reasons.is_empty() => Ok(PiSetupIsolationContract {
            tool_contract_version: metadata.tool_contract_version,
            extension_sha256: metadata.extension_sha256,
            active_tools,
            argv,
        }),
        _ => Err(PiSetupProbeError::ToolContract { reasons }),
    }
}

/// Return the fixed direct argv with no ephemeral extension path retained.
fn stable_isolation_argv() -> Vec<String> {
    pi_argv(Path::new("/__pacsea_verified_extension__"))
        .into_iter()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect()
}

/// Parse every exact selectable model route and reject malformed identities fail closed.
fn parse_advertised_routes(
    response: &Value,
    reservation_tokens: u64,
) -> Result<Vec<PiSetupAdvertisedRoute>, PiSetupProbeError> {
    let values = response
        .pointer("/data/models")
        .and_then(Value::as_array)
        .ok_or_else(|| rpc_error("get_available_models", "response omitted the model array"))?;
    if values.is_empty() {
        return Err(PiSetupProbeError::EmptyRoutes);
    }
    let identities = parse_route_identities(values)?;
    let mut routes = Vec::with_capacity(values.len());
    let mut first_pricing_error = None;
    for ((provider, model), value) in identities.into_iter().zip(values) {
        match parse_route_pricing(&provider, &model, value, reservation_tokens) {
            Ok(route) => routes.push(route),
            Err(
                error @ (PiSetupProbeError::PricingAbsent { .. }
                | PiSetupProbeError::PricingInvalid { .. }),
            ) => {
                first_pricing_error.get_or_insert(error);
            }
            Err(error) => return Err(error),
        }
    }
    if routes.is_empty() {
        Err(first_pricing_error.unwrap_or(PiSetupProbeError::EmptyRoutes))
    } else {
        Ok(routes)
    }
}

/// Parse and de-duplicate every advertised exact route identity before pricing filtering.
fn parse_route_identities(values: &[Value]) -> Result<Vec<(String, String)>, PiSetupProbeError> {
    let mut seen = BTreeSet::new();
    let mut identities = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let provider = parse_route_identifier(value, index, "provider")?;
        let model = parse_route_identifier(value, index, "id")?;
        if !seen.insert((provider.clone(), model.clone())) {
            return Err(PiSetupProbeError::DuplicateRoute { provider, model });
        }
        identities.push((provider, model));
    }
    Ok(identities)
}

/// Parse exact pricing for one route, excluding any unpriced route from setup selection.
fn parse_route_pricing(
    provider: &str,
    model: &str,
    value: &Value,
    reservation_tokens: u64,
) -> Result<PiSetupAdvertisedRoute, PiSetupProbeError> {
    let cost = value
        .get("cost")
        .ok_or_else(|| PiSetupProbeError::PricingAbsent {
            provider: provider.to_string(),
            model: model.to_string(),
        })?;
    let pricing = pricing_from_pi_model_cost(provider, model, cost, &[]).map_err(|error| {
        PiSetupProbeError::PricingInvalid {
            provider: provider.to_string(),
            model: model.to_string(),
            reason: pricing_error_reason(&error),
        }
    })?;
    let cost_microusd = reserve_worst_case_microusd(
        &pricing,
        UsageAccounting {
            rpc_bytes: 0,
            reported_tokens: Some(reservation_tokens),
        },
    );
    Ok(PiSetupAdvertisedRoute {
        provider: provider.to_string(),
        model: model.to_string(),
        pricing,
        pricing_provenance: PI_MODEL_PRICING_PROVENANCE.to_string(),
        reservation: PiScanReservation {
            tokens: reservation_tokens,
            cost_microusd,
        },
    })
}

/// Parse one exact bounded provider/model identifier without normalization.
fn parse_route_identifier(
    value: &Value,
    index: usize,
    field: &'static str,
) -> Result<String, PiSetupProbeError> {
    let identifier = value.get(field).and_then(Value::as_str).ok_or_else(|| {
        PiSetupProbeError::MalformedRoute {
            index,
            field,
            reason: "the required string is absent".to_string(),
        }
    })?;
    if identifier.is_empty() || identifier.trim() != identifier {
        return Err(PiSetupProbeError::MalformedRoute {
            index,
            field,
            reason: "the identifier is empty or has surrounding whitespace".to_string(),
        });
    }
    if identifier.len() > 512 || has_forbidden_control(identifier) {
        return Err(PiSetupProbeError::MalformedRoute {
            index,
            field,
            reason: "the identifier is oversized or control-bearing".to_string(),
        });
    }
    Ok(identifier.to_string())
}

/// Reduce pricing parser failures to a bounded field category.
fn pricing_error_reason(error: &crate::logic::pi_scan::pricing::PricingError) -> String {
    use crate::logic::pi_scan::pricing::PricingError;
    match error {
        PricingError::MalformedCatalog { .. } => "the pricing object shape is invalid".to_string(),
        PricingError::InvalidRate { reason, .. } => reason.clone(),
        PricingError::RouteNotFound { .. } => "the exact route price is absent".to_string(),
    }
}

/// Build and bind the final typed snapshot after every independent check passes.
fn build_snapshot(
    request: &PiSetupProbeRequest,
    executable: PathBuf,
    pi_version: PiVersion,
    isolation: PiSetupIsolationContract,
    routes: Vec<PiSetupAdvertisedRoute>,
) -> Result<PiSetupProbeSnapshot, PiSetupProbeError> {
    let pricing_binding = pricing_binding(
        pi_version,
        &isolation,
        &routes,
        request.now_unix_seconds,
        request.maximum_pricing_age,
    );
    let snapshot = PiSetupProbeSnapshot {
        executable,
        pi_version,
        isolation,
        routes,
        pricing_observed_at_unix_seconds: request.now_unix_seconds,
        maximum_pricing_age: request.maximum_pricing_age,
        pricing_binding,
    };
    snapshot.validate_pricing_freshness(request.now_unix_seconds)?;
    Ok(snapshot)
}

/// Hash the exact typed setup facts without retaining Pi's raw response object.
fn pricing_binding(
    pi_version: PiVersion,
    isolation: &PiSetupIsolationContract,
    routes: &[PiSetupAdvertisedRoute],
    _observed_at: u64,
    _maximum_age: Duration,
) -> String {
    let route_values: Vec<Value> = routes
        .iter()
        .map(|route| {
            serde_json::json!({
                "provider": route.provider,
                "model": route.model,
                "input_microusd_per_million": route.pricing.rates.input_microusd_per_million,
                "output_microusd_per_million": route.pricing.rates.output_microusd_per_million,
                "source": pricing_source_label(route.pricing.source),
                "accounting": pricing_accounting_label(route.pricing.accounting),
                "provenance": route.pricing_provenance,
                "reservation_tokens": route.reservation.tokens,
                "reservation_cost_microusd": route.reservation.cost_microusd,
            })
        })
        .collect();
    let value = serde_json::json!({
        "pi_version": pi_version.to_string(),
        "tool_contract_version": isolation.tool_contract_version,
        "extension_sha256": isolation.extension_sha256,
        "active_tools": isolation.active_tools,
        "isolation_argv": isolation.argv,
        "routes": route_values,
    });
    let canonical = value.to_string();
    crate::pi_agent::to_hex(&crate::pi_agent::sha256(canonical.as_bytes()))
}

/// Stable pricing-source label used by the reviewed binding.
const fn pricing_source_label(source: PricingSource) -> &'static str {
    match source {
        PricingSource::PiModelCost => "pi-model-cost",
        PricingSource::LiteLlmCatalog => "litellm-catalog",
        PricingSource::OpenRouterCatalog => "openrouter-catalog",
    }
}

/// Stable accounting label used by the reviewed binding.
const fn pricing_accounting_label(accounting: PricingAccounting) -> &'static str {
    match accounting {
        PricingAccounting::Metered => "metered",
        PricingAccounting::SubscriptionBacked => "subscription-backed",
    }
}

#[cfg(test)]
#[path = "../../../tests/pi_scan/ws_setup_probe.rs"]
mod tests;
