//! Fail-closed capability probing for the optional host Pi executable.
//!
//! Nothing in the scanner may run until every checked capability is present:
//! the minimum version, every required CLI flag, every required RPC command, the
//! exact four-tool allowlist, and a command inventory that contains no ambient
//! user or project command source.
//!
//! Any missing, mismatched, or unparseable capability yields
//! [`PiAvailability::Unavailable`] with the full list of reasons. There is no
//! partial-enable path and no "assume present" fallback.

use std::fmt;

use super::protocol::REQUIRED_RPC_COMMANDS;
use super::{MINIMUM_PI_VERSION, PiVersion, RESTRICTED_TOOL_NAMES};

/// Required Pi CLI flags for fail-closed scanner startup.
///
/// The strings are matched against `pi --help` output and mirror the exact argv the
/// scanner builds in [`super::process`].
pub const REQUIRED_PI_FLAGS: [&str; 12] = [
    "--mode <mode>",
    "--no-session",
    "--no-builtin-tools",
    "--tools",
    "--extension",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--no-themes",
    "--no-context-files",
    "--no-approve",
    "--offline",
];

/// Command sources Pi may report that carry no ambient authority.
///
/// `cli` is the Pacsea-supplied extension command. `inline` covers Pi-owned temporary
/// commands such as the 0.84.0 `/llama` helper, which are RPC-client inputs and are not
/// model-callable tools. Every other source (notably `user` and `project`) is rejected.
pub const ALLOWED_COMMAND_SOURCES: [&str; 2] = ["cli", "inline"];

/// What: A single Pi command as reported by the `get_commands` RPC response.
///
/// Inputs: Decoded from the probe session's command inventory.
///
/// Output: Comparable descriptor used by [`validate_command_inventory`].
///
/// Details:
/// - `scope` and `source` come from Pi's `sourceInfo` object; both are required so a
///   command with missing provenance cannot pass as trusted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CommandDescriptor {
    /// Command name without the leading slash.
    pub name: String,
    /// Command lifetime scope reported by Pi, for example `temporary`.
    pub scope: String,
    /// Command origin reported by Pi, for example `cli`, `inline`, `user`, or `project`.
    pub source: String,
}

/// What: Outcome of a Pi capability probe.
///
/// Inputs: Produced by [`evaluate_capabilities`].
///
/// Output: Either a usable report or an explicit unavailable reason list.
///
/// Details:
/// - Callers must treat `Unavailable` as "the whole Pi path is disabled", never as a
///   degraded mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PiAvailability {
    /// Every checked capability is present.
    Available(CapabilityReport),
    /// At least one capability is missing or mismatched.
    Unavailable(CapabilityFailure),
}

impl PiAvailability {
    /// What: Convert the availability into a `Result` for `?` propagation.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - `Ok(report)` when available, otherwise the failure.
    ///
    /// Details:
    /// - Keeps call sites from accidentally matching only the `Available` arm.
    ///
    /// # Errors
    /// - Returns `Err` with every recorded reason when the probe failed.
    pub fn into_result(self) -> Result<CapabilityReport, CapabilityFailure> {
        match self {
            Self::Available(report) => Ok(report),
            Self::Unavailable(failure) => Err(failure),
        }
    }
}

/// What: Verified capability facts recorded into every scan result's provenance.
///
/// Inputs: Produced by [`evaluate_capabilities`] after all checks pass.
///
/// Output: Version and inventory evidence for provenance and the UI.
///
/// Details:
/// - `pi_owned_inline_commands` is an explicit inventory rather than a silent
///   weakening of the exact tool boundary; the UI discloses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityReport {
    /// Verified Pi version.
    pub version: PiVersion,
    /// Exact active tool names observed in the probe session.
    pub active_tools: Vec<String>,
    /// Pi-owned temporary inline commands observed alongside the Pacsea command.
    pub pi_owned_inline_commands: Vec<String>,
}

/// What: Aggregated fail-closed capability rejection.
///
/// Inputs: Accumulated by [`evaluate_capabilities`].
///
/// Output: Implements `Display`/`Error` listing every unmet requirement.
///
/// Details:
/// - The reason list is complete rather than first-failure, so setup can show the user
///   everything that needs fixing in one pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFailure {
    /// Every unmet requirement, in detection order.
    pub reasons: Vec<String>,
}

impl CapabilityFailure {
    /// What: Build a failure from a single reason.
    ///
    /// Inputs:
    /// - `reason`: Human-readable unmet requirement.
    ///
    /// Output:
    /// - A failure carrying exactly that reason.
    ///
    /// Details:
    /// - Used by callers that detect a capability problem outside `evaluate_capabilities`.
    #[must_use]
    pub fn single(reason: impl Into<String>) -> Self {
        Self {
            reasons: vec![reason.into()],
        }
    }
}

impl fmt::Display for CapabilityFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Pi scanning is unavailable. Fix the following and re-run setup: {}",
            self.reasons.join("; ")
        )
    }
}

impl std::error::Error for CapabilityFailure {}

/// What: Parse a three-component Pi semantic version.
///
/// Inputs:
/// - `raw`: `pi --version` output.
///
/// Output:
/// - Parsed version, or an actionable parse error.
///
/// Details:
/// - Accepts surrounding whitespace but rejects prefixes, suffixes, extra components,
///   and non-numeric parts, so a cosmetic banner cannot be read as a version.
///
/// # Errors
/// - Returns `Err` when the trimmed text is not exactly `major.minor.patch`.
pub fn parse_pi_version(raw: &str) -> Result<PiVersion, String> {
    let trimmed = raw.trim();
    let components: Vec<&str> = trimmed.split('.').collect();
    if components.len() != 3 {
        return Err(format!(
            "expected Pi version major.minor.patch, got {trimmed:?}"
        ));
    }
    let mut parsed = [0u64; 3];
    for (slot, component) in parsed.iter_mut().zip(components) {
        *slot = component
            .parse::<u64>()
            .map_err(|error| format!("invalid Pi version component {component:?}: {error}"))?;
    }
    Ok(PiVersion {
        major: parsed[0],
        minor: parsed[1],
        patch: parsed[2],
    })
}

/// What: Validate the version and required CLI flags of the resolved Pi executable.
///
/// Inputs:
/// - `version_output`: `pi --version` text.
/// - `help_output`: `pi --help` text.
///
/// Output:
/// - The verified version, or the accumulated reasons why the CLI is unusable.
///
/// Details:
/// - A satisfied minimum version never substitutes for the flag checks; both must pass.
///
/// # Errors
/// - Returns `Err` when the version cannot be parsed, is too old, or a flag is missing.
pub fn validate_cli_contract(
    version_output: &str,
    help_output: &str,
) -> Result<PiVersion, CapabilityFailure> {
    let mut reasons = Vec::new();
    let mut version = None;
    match parse_pi_version(version_output) {
        Ok(parsed) if parsed >= MINIMUM_PI_VERSION => version = Some(parsed),
        Ok(parsed) => reasons.push(format!(
            "installed Pi {parsed} is older than the required {MINIMUM_PI_VERSION}"
        )),
        Err(error) => reasons.push(error),
    }
    for flag in REQUIRED_PI_FLAGS {
        if !help_output.contains(flag) {
            reasons.push(format!("required Pi flag missing: {flag}"));
        }
    }
    match version {
        Some(parsed) if reasons.is_empty() => Ok(parsed),
        _ => Err(CapabilityFailure { reasons }),
    }
}

/// What: Require the probe session to expose exactly the four restricted tools.
///
/// Inputs:
/// - `active_tools`: Tool names reported by the probe extension.
///
/// Output:
/// - The sorted verified tool list, or the exact mismatch.
///
/// Details:
/// - Comparison is on the sorted deduplicated set, so ordering differences pass while
///   any missing, extra, or renamed tool fails closed.
///
/// # Errors
/// - Returns `Err` when the observed set differs from the compiled allowlist.
pub fn validate_active_tools(active_tools: &[String]) -> Result<Vec<String>, CapabilityFailure> {
    let mut observed: Vec<String> = active_tools.to_vec();
    observed.sort();
    let expected: Vec<String> = RESTRICTED_TOOL_NAMES
        .iter()
        .map(|n| (*n).to_string())
        .collect();
    if observed == expected {
        return Ok(observed);
    }
    let unexpected: Vec<&String> = observed.iter().filter(|n| !expected.contains(n)).collect();
    let missing: Vec<&String> = expected.iter().filter(|n| !observed.contains(n)).collect();
    let mut reasons = Vec::new();
    if !unexpected.is_empty() {
        reasons.push(format!(
            "Pi exposed unexpected active tools: {unexpected:?}"
        ));
    }
    if !missing.is_empty() {
        reasons.push(format!("Pi is missing required scanner tools: {missing:?}"));
    }
    if reasons.is_empty() {
        reasons.push(format!(
            "Pi active tool set {observed:?} does not match the required {expected:?}"
        ));
    }
    Err(CapabilityFailure { reasons })
}

/// What: Validate the probe session's command inventory.
///
/// Inputs:
/// - `commands`: Commands reported by `get_commands`.
/// - `expected_pacsea_command`: Name of the Pacsea-supplied extension command.
///
/// Output:
/// - The inventory of Pi-owned temporary inline commands, or the exact rejection.
///
/// Details:
/// - Rejects any command whose source is not in [`ALLOWED_COMMAND_SOURCES`] and any
///   command whose scope is not `temporary`, so ambient user/project commands fail closed.
/// - Requires the Pacsea command to be present with source `cli`; without it the probe
///   could not have observed the real active tool set.
///
/// # Errors
/// - Returns `Err` when an ambient command source is present or the Pacsea command is absent.
pub fn validate_command_inventory(
    commands: &[CommandDescriptor],
    expected_pacsea_command: &str,
) -> Result<Vec<String>, CapabilityFailure> {
    let mut reasons = Vec::new();
    let mut inline = Vec::new();
    for command in commands {
        if !ALLOWED_COMMAND_SOURCES.contains(&command.source.as_str()) {
            reasons.push(format!(
                "ambient Pi command {:?} from disallowed source {:?} must be disabled",
                command.name, command.source
            ));
            continue;
        }
        if command.scope != "temporary" {
            reasons.push(format!(
                "Pi command {:?} has persistent scope {:?}; only temporary commands are allowed",
                command.name, command.scope
            ));
            continue;
        }
        if command.source == "inline" {
            inline.push(command.name.clone());
        }
    }
    let has_pacsea_command = commands
        .iter()
        .any(|command| command.name == expected_pacsea_command && command.source == "cli");
    if !has_pacsea_command {
        reasons.push(format!(
            "the Pacsea probe command {expected_pacsea_command:?} was not registered from the CLI extension"
        ));
    }
    if reasons.is_empty() {
        inline.sort();
        inline.dedup();
        Ok(inline)
    } else {
        Err(CapabilityFailure { reasons })
    }
}

/// What: Require every RPC command the scanner depends on to be advertised.
///
/// Inputs:
/// - `advertised`: RPC command names reported by the probe session.
///
/// Output:
/// - `Ok(())` when the full required surface is present.
///
/// Details:
/// - Missing commands disable the whole Pi path rather than only the dependent feature,
///   because cancellation and accounting are not optional.
///
/// # Errors
/// - Returns `Err` listing every missing RPC command name.
pub fn validate_rpc_surface(advertised: &[String]) -> Result<(), CapabilityFailure> {
    let missing: Vec<String> = REQUIRED_RPC_COMMANDS
        .iter()
        .filter(|required| !advertised.iter().any(|name| name == *required))
        .map(|required| format!("required Pi RPC command missing: {required}"))
        .collect();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(CapabilityFailure { reasons: missing })
    }
}

/// What: Raw observations collected from one no-model Pi probe session.
///
/// Inputs: Assembled by the runtime probe driver from Pi's RPC responses.
///
/// Output: Input to [`evaluate_capabilities`].
///
/// Details:
/// - Keeping observation and judgement separate lets deterministic tests exercise the
///   full fail-closed matrix without launching Pi.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeObservation {
    /// `pi --version` output.
    pub version_output: String,
    /// `pi --help` output.
    pub help_output: String,
    /// Active tool names reported by the probe extension.
    pub active_tools: Vec<String>,
    /// Commands reported by `get_commands`.
    pub commands: Vec<CommandDescriptor>,
    /// RPC command names advertised by the probe session.
    pub advertised_rpc_commands: Vec<String>,
    /// Name of the Pacsea-supplied extension command.
    pub pacsea_command_name: String,
}

/// What: Judge a complete probe observation fail-closed.
///
/// Inputs:
/// - `observation`: Everything the probe session reported.
///
/// Output:
/// - [`PiAvailability::Available`] only when every check passes.
///
/// Details:
/// - Reasons from all four checks are accumulated so setup can report the whole gap set.
#[must_use]
pub fn evaluate_capabilities(observation: &ProbeObservation) -> PiAvailability {
    let mut reasons = Vec::new();
    let version = match validate_cli_contract(&observation.version_output, &observation.help_output)
    {
        Ok(version) => Some(version),
        Err(failure) => {
            reasons.extend(failure.reasons);
            None
        }
    };
    let active_tools = match validate_active_tools(&observation.active_tools) {
        Ok(tools) => Some(tools),
        Err(failure) => {
            reasons.extend(failure.reasons);
            None
        }
    };
    let inline_commands =
        match validate_command_inventory(&observation.commands, &observation.pacsea_command_name) {
            Ok(inline) => Some(inline),
            Err(failure) => {
                reasons.extend(failure.reasons);
                None
            }
        };
    if let Err(failure) = validate_rpc_surface(&observation.advertised_rpc_commands) {
        reasons.extend(failure.reasons);
    }
    match (version, active_tools, inline_commands) {
        (Some(version), Some(active_tools), Some(pi_owned_inline_commands))
            if reasons.is_empty() =>
        {
            PiAvailability::Available(CapabilityReport {
                version,
                active_tools,
                pi_owned_inline_commands,
            })
        }
        _ => PiAvailability::Unavailable(CapabilityFailure { reasons }),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CommandDescriptor, PiAvailability, ProbeObservation, REQUIRED_PI_FLAGS,
        evaluate_capabilities, parse_pi_version, validate_active_tools, validate_cli_contract,
        validate_command_inventory, validate_rpc_surface,
    };
    use crate::pi_agent::protocol::REQUIRED_RPC_COMMANDS;
    use crate::pi_agent::{PiVersion, RESTRICTED_TOOL_NAMES};

    /// Build an observation that satisfies every capability requirement.
    fn good_observation() -> ProbeObservation {
        ProbeObservation {
            version_output: "0.84.0\n".to_string(),
            help_output: REQUIRED_PI_FLAGS.join("\n"),
            active_tools: RESTRICTED_TOOL_NAMES
                .iter()
                .rev()
                .map(|n| (*n).to_string())
                .collect(),
            commands: vec![
                CommandDescriptor {
                    name: "pacsea-scan-tools".to_string(),
                    scope: "temporary".to_string(),
                    source: "cli".to_string(),
                },
                CommandDescriptor {
                    name: "llama".to_string(),
                    scope: "temporary".to_string(),
                    source: "inline".to_string(),
                },
            ],
            advertised_rpc_commands: REQUIRED_RPC_COMMANDS
                .iter()
                .map(|n| (*n).to_string())
                .collect(),
            pacsea_command_name: "pacsea-scan-tools".to_string(),
        }
    }

    /// Verify strict version parsing.
    #[test]
    fn version_parsing_is_strict() {
        assert_eq!(
            parse_pi_version(" 0.84.0 \n"),
            Ok(PiVersion {
                major: 0,
                minor: 84,
                patch: 0
            })
        );
        assert!(parse_pi_version("v0.84.0").is_err());
        assert!(parse_pi_version("0.84").is_err());
        assert!(parse_pi_version("0.84.0.1").is_err());
        assert!(parse_pi_version("0.84.0-rc1").is_err());
        assert!(parse_pi_version("pi version 0.84.0").is_err());
    }

    /// Verify the version gate and every required flag are enforced independently.
    #[test]
    fn cli_contract_requires_version_and_every_flag() {
        let help = REQUIRED_PI_FLAGS.join("\n");
        assert!(validate_cli_contract("0.84.0", &help).is_ok());
        let old = validate_cli_contract("0.83.9", &help).expect_err("old version must fail");
        assert_eq!(old.reasons.len(), 1);
        assert!(old.reasons[0].contains("older than the required 0.84.0"));

        for flag in REQUIRED_PI_FLAGS {
            let reduced: Vec<&str> = REQUIRED_PI_FLAGS
                .iter()
                .copied()
                .filter(|candidate| *candidate != flag)
                .collect();
            let failure = validate_cli_contract("0.84.0", &reduced.join("\n"))
                .expect_err("each missing flag must fail closed");
            assert_eq!(
                failure.reasons,
                vec![format!("required Pi flag missing: {flag}")]
            );
        }
    }

    /// Verify the tool allowlist is exact in both directions.
    #[test]
    fn active_tools_must_match_exactly() {
        let expected: Vec<String> = RESTRICTED_TOOL_NAMES
            .iter()
            .map(|n| (*n).to_string())
            .collect();
        assert_eq!(
            validate_active_tools(&expected).expect("exact set passes"),
            expected
        );

        let mut extra = expected.clone();
        extra.push("bash".to_string());
        let failure = validate_active_tools(&extra).expect_err("extra tool must fail");
        assert!(failure.reasons[0].contains("unexpected active tools"));

        let mut duplicate = expected.clone();
        duplicate.push(expected[0].clone());
        assert!(
            validate_active_tools(&duplicate).is_err(),
            "duplicate tool reports are a capability protocol mismatch"
        );

        let missing: Vec<String> = expected.iter().skip(1).cloned().collect();
        let failure = validate_active_tools(&missing).expect_err("missing tool must fail");
        assert!(failure.reasons[0].contains("missing required scanner tools"));

        let renamed = vec![
            "pacsea_scan_find".to_string(),
            "pacsea_scan_grep".to_string(),
            "pacsea_scan_ls".to_string(),
            "pacsea_scan_readfile".to_string(),
        ];
        assert!(validate_active_tools(&renamed).is_err());
    }

    /// Verify ambient user/project commands are rejected while Pi inline commands are inventoried.
    #[test]
    fn command_inventory_rejects_ambient_sources() {
        let commands = good_observation().commands;
        assert_eq!(
            validate_command_inventory(&commands, "pacsea-scan-tools").expect("passes"),
            vec!["llama".to_string()]
        );

        for hostile_source in ["user", "project", "global", ""] {
            let mut hostile = commands.clone();
            hostile.push(CommandDescriptor {
                name: "deploy".to_string(),
                scope: "temporary".to_string(),
                source: hostile_source.to_string(),
            });
            let failure = validate_command_inventory(&hostile, "pacsea-scan-tools")
                .expect_err("ambient command must fail");
            assert!(
                failure.reasons.iter().any(|r| r.contains("\"deploy\"")),
                "{failure:?}"
            );
        }

        let mut persistent = commands.clone();
        persistent.push(CommandDescriptor {
            name: "persisted".to_string(),
            scope: "session".to_string(),
            source: "cli".to_string(),
        });
        assert!(validate_command_inventory(&persistent, "pacsea-scan-tools").is_err());

        assert!(
            validate_command_inventory(&commands, "pacsea-other").is_err(),
            "a missing Pacsea command means the tool observation is unproven"
        );
    }

    /// Verify every required RPC command is individually mandatory.
    #[test]
    fn rpc_surface_is_individually_mandatory() {
        let all: Vec<String> = REQUIRED_RPC_COMMANDS
            .iter()
            .map(|n| (*n).to_string())
            .collect();
        assert!(validate_rpc_surface(&all).is_ok());
        for command in REQUIRED_RPC_COMMANDS {
            let reduced: Vec<String> = all.iter().filter(|n| *n != command).cloned().collect();
            let failure =
                validate_rpc_surface(&reduced).expect_err("each missing command must fail");
            assert_eq!(
                failure.reasons,
                vec![format!("required Pi RPC command missing: {command}")]
            );
        }
    }

    /// Verify a fully satisfied probe becomes available with the inline inventory recorded.
    #[test]
    fn complete_probe_is_available() {
        let PiAvailability::Available(report) = evaluate_capabilities(&good_observation()) else {
            panic!("a complete observation must be available");
        };
        assert_eq!(report.version.to_string(), "0.84.0");
        assert_eq!(
            report.active_tools,
            RESTRICTED_TOOL_NAMES
                .iter()
                .map(|n| (*n).to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(report.pi_owned_inline_commands, vec!["llama".to_string()]);
    }

    /// Verify a single defect makes the whole Pi path unavailable and accumulates reasons.
    #[test]
    fn any_defect_fails_closed_with_all_reasons() {
        let mut observation = good_observation();
        observation.version_output = "0.83.0".to_string();
        observation.active_tools.push("bash".to_string());
        observation.commands.push(CommandDescriptor {
            name: "user-cmd".to_string(),
            scope: "temporary".to_string(),
            source: "user".to_string(),
        });
        observation.advertised_rpc_commands.retain(|n| n != "abort");

        let PiAvailability::Unavailable(failure) = evaluate_capabilities(&observation) else {
            panic!("a defective observation must be unavailable");
        };
        assert!(failure.reasons.iter().any(|r| r.contains("older than")));
        assert!(
            failure
                .reasons
                .iter()
                .any(|r| r.contains("unexpected active tools"))
        );
        assert!(failure.reasons.iter().any(|r| r.contains("user-cmd")));
        assert!(
            failure
                .reasons
                .iter()
                .any(|r| r.contains("required Pi RPC command missing: abort"))
        );
        assert!(failure.to_string().contains("Pi scanning is unavailable"));
        assert!(
            PiAvailability::Unavailable(failure).into_result().is_err(),
            "unavailable must never convert into a usable report"
        );
    }
}
