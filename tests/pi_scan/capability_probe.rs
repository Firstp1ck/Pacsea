//! Deterministic and ignored-live probes for the Pi scanner CLI/RPC contract.

#![cfg(not(target_os = "windows"))]

use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::fixtures::{
    ACTIVE_TOOLS_MARKER, REQUIRED_PI_FLAGS, RESTRICTED_TOOL_NAMES, probe_extension_path,
    restricted_tool_csv,
};

/// Minimum supported Pi version chosen during design review.
const MINIMUM_PI_VERSION: (u64, u64, u64) = (0, 84, 0);

/// Inherited environment names required for executable lookup and standard Pi state.
const PASSTHROUGH_ENVIRONMENT: [&str; 9] = [
    "PATH",
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "PI_CODING_AGENT_DIR",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
];

/// Fixed environment names controlled by the scanner probe.
const FIXED_ENVIRONMENT: [(&str, &str); 3] = [
    ("PI_OFFLINE", "1"),
    ("PI_TELEMETRY", "0"),
    ("PI_SKIP_VERSION_CHECK", "1"),
];

/// Captured output from the ignored live Pi capability probe.
struct ProbeOutput {
    /// Child exit status.
    status: ExitStatus,
    /// Strict RPC stdout stream.
    stdout: String,
    /// Diagnostic stderr stream.
    stderr: String,
}

/// What: Parse a three-component Pi semantic version.
///
/// Inputs:
/// - `raw`: `pi --version` output.
///
/// Output:
/// - Parsed `(major, minor, patch)` or an actionable parse error.
///
/// Details:
/// - Accepts surrounding whitespace but rejects prefixes, suffixes, and missing components.
fn parse_version(raw: &str) -> Result<(u64, u64, u64), String> {
    let components = raw.trim().split('.').collect::<Vec<_>>();
    if components.len() != 3 {
        return Err(format!("expected major.minor.patch, got {raw:?}"));
    }
    let major = components[0]
        .parse::<u64>()
        .map_err(|error| format!("invalid Pi major version: {error}"))?;
    let minor = components[1]
        .parse::<u64>()
        .map_err(|error| format!("invalid Pi minor version: {error}"))?;
    let patch = components[2]
        .parse::<u64>()
        .map_err(|error| format!("invalid Pi patch version: {error}"))?;
    Ok((major, minor, patch))
}

/// What: Validate version and required scanner CLI flags from bounded Pi output.
///
/// Inputs:
/// - `version`: `pi --version` text.
/// - `help`: `pi --help` text.
///
/// Output:
/// - Success or a list of fail-closed capability errors.
///
/// Details:
/// - A minimum version is necessary but never substitutes for flag checks.
fn validate_cli_contract(version: &str, help: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    match parse_version(version) {
        Ok(parsed) if parsed >= MINIMUM_PI_VERSION => {}
        Ok(parsed) => errors.push(format!(
            "Pi {parsed:?} is older than required {MINIMUM_PI_VERSION:?}"
        )),
        Err(error) => errors.push(error),
    }
    for flag in REQUIRED_PI_FLAGS {
        if !help.contains(flag) {
            errors.push(format!("required Pi flag missing: {flag}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// What: Replace inherited process state with the bounded Pi probe environment.
///
/// Inputs:
/// - `command`: Probe command under construction.
///
/// Output:
/// - The command keeps only standard Pi path/state/locale variables plus fixed offline controls.
///
/// Details:
/// - Provider keys, proxy settings, credential helpers, agent sockets, and arbitrary future
///   variables are excluded by construction rather than by a denylist.
fn configure_probe_environment(command: &mut Command) {
    command.env_clear();
    for name in PASSTHROUGH_ENVIRONMENT {
        if let Some(value) = std::env::var_os(name) {
            command.env(name, value);
        }
    }
    if std::env::var_os("PATH").is_none() {
        command.env("PATH", "/usr/bin:/bin");
    }
    for (name, value) in FIXED_ENVIRONMENT {
        command.env(name, value);
    }
}

/// What: Read one child pipe to completion on a dedicated thread.
///
/// Inputs:
/// - `reader`: Owned stdout or stderr pipe.
///
/// Output:
/// - Join handle returning captured bytes or an I/O error.
///
/// Details:
/// - Concurrent draining prevents a diagnostic pipe from blocking the smoke process.
fn read_pipe<R>(mut reader: R) -> thread::JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

/// What: Wait for the smoke child with a hard deadline.
///
/// Inputs:
/// - `child`: Running Pi process.
/// - `deadline`: Maximum wait duration.
///
/// Output:
/// - Exit status or timeout/process error.
///
/// Details:
/// - A timeout kills and reaps the probe process before returning an error.
fn wait_with_deadline(
    child: &mut std::process::Child,
    deadline: Duration,
) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("failed to poll Pi probe: {error}"))?
        {
            return Ok(status);
        }
        if started.elapsed() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("Pi capability probe exceeded {deadline:?}"));
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// What: Launch the installed Pi binary with the exact no-model scanner isolation flags.
///
/// Inputs:
/// - `pi`: Absolute Pi executable path.
///
/// Output:
/// - Captured RPC stdout/stderr and exit status.
///
/// Details:
/// - Uses a neutral cwd and positive environment allowlist, loads only the checked-in probe
///   extension, asks it to report active tools, and shuts down without an LLM call.
fn run_live_probe(pi: &std::path::Path) -> Result<ProbeOutput, String> {
    let neutral = tempfile::tempdir()
        .map_err(|error| format!("failed to create neutral Pi probe directory: {error}"))?;
    let mut command = Command::new(pi);
    command
        .current_dir(neutral.path())
        .args([
            "--mode",
            "rpc",
            "--no-session",
            "--offline",
            "--no-builtin-tools",
            "--no-extensions",
            "--no-skills",
            "--no-prompt-templates",
            "--no-context-files",
            "--no-themes",
            "--no-approve",
            "--extension",
        ])
        .arg(probe_extension_path())
        .args(["--tools", &restricted_tool_csv()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_probe_environment(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to launch Pi capability probe: {error}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Pi probe stdout was not piped".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Pi probe stderr was not piped".to_string())?;
    let stdout_reader = read_pipe(stdout);
    let stderr_reader = read_pipe(stderr);

    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Pi probe stdin was not piped".to_string())?;
    stdin
        .write_all(
            b"{\"id\":\"commands\",\"type\":\"get_commands\"}\n\
{\"id\":\"probe\",\"type\":\"prompt\",\"message\":\"/pacsea-probe-tools\"}\n",
        )
        .map_err(|error| format!("failed to write Pi probe commands: {error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("failed to flush Pi probe commands: {error}"))?;
    drop(stdin);

    let status = wait_with_deadline(&mut child, Duration::from_secs(20))?;
    let stdout = stdout_reader
        .join()
        .map_err(|_| "Pi stdout reader panicked".to_string())?
        .map_err(|error| format!("failed to read Pi probe stdout: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "Pi stderr reader panicked".to_string())?
        .map_err(|error| format!("failed to read Pi probe stderr: {error}"))?;

    Ok(ProbeOutput {
        status,
        stdout: String::from_utf8(stdout)
            .map_err(|error| format!("Pi probe stdout was not UTF-8: {error}"))?,
        stderr: String::from_utf8(stderr)
            .map_err(|error| format!("Pi probe stderr was not UTF-8: {error}"))?,
    })
}

/// What: Parse strict JSONL records from Pi probe stdout.
///
/// Inputs:
/// - `stdout`: Captured RPC output.
///
/// Output:
/// - Parsed JSON values or line-specific parse errors.
///
/// Details:
/// - Splits on LF only and strips one trailing CR for protocol parity.
fn parse_rpc_records(stdout: &str) -> Result<Vec<Value>, String> {
    stdout
        .split_terminator('\n')
        .enumerate()
        .map(|(index, raw)| {
            let line = raw.strip_suffix('\r').unwrap_or(raw);
            serde_json::from_str::<Value>(line)
                .map_err(|error| format!("invalid RPC JSON at line {}: {error}", index + 1))
        })
        .collect()
}

/// What: Verify a representative Pi 0.84 help/version contract.
///
/// Inputs:
/// - Synthetic deterministic help/version strings.
///
/// Output:
/// - Contract validation succeeds.
///
/// Details:
/// - Keeps normal CI independent from the developer's installed Pi binary.
#[test]
fn pi_cli_contract_accepts_minimum_version_and_required_flags() {
    let help = REQUIRED_PI_FLAGS.join("\n");
    assert_eq!(validate_cli_contract("0.84.0\n", &help), Ok(()));
}

/// What: Verify capability validation fails closed when one required flag disappears.
///
/// Inputs:
/// - Synthetic help text missing `--no-builtin-tools`.
///
/// Output:
/// - Validation returns the exact missing capability.
///
/// Details:
/// - Prevents version-only acceptance after CLI drift.
#[test]
fn pi_cli_contract_rejects_missing_flag() {
    let help = REQUIRED_PI_FLAGS
        .iter()
        .copied()
        .filter(|flag| *flag != "--no-builtin-tools")
        .collect::<Vec<_>>()
        .join("\n");
    let errors = validate_cli_contract("0.84.0", &help).expect_err("missing flag must fail");
    assert_eq!(errors, vec!["required Pi flag missing: --no-builtin-tools"]);
}

/// What: Verify the probe environment is a positive allowlist.
///
/// Inputs:
/// - A fake unlisted secret placed on an `env` child before probe configuration.
///
/// Output:
/// - The fake secret is absent and every emitted variable name is explicitly allowed.
///
/// Details:
/// - Runs without Pi or a model and guards against future ambient credential-variable drift.
#[test]
fn pi_probe_environment_excludes_unlisted_variables() {
    let mut command = Command::new("/usr/bin/env");
    command.env("PACSEA_UNLISTED_SECRET", "must-not-survive");
    configure_probe_environment(&mut command);
    let output = command.output().expect("bounded env probe must launch");
    assert!(output.status.success(), "env probe must succeed");
    let stdout = String::from_utf8(output.stdout).expect("env output must be UTF-8");
    let names = stdout
        .lines()
        .filter_map(|line| line.split_once('=').map(|(name, _)| name))
        .collect::<Vec<_>>();
    assert!(!names.contains(&"PACSEA_UNLISTED_SECRET"));
    assert!(names.iter().all(|name| {
        PASSTHROUGH_ENVIRONMENT.contains(name)
            || FIXED_ENVIRONMENT
                .iter()
                .any(|(fixed_name, _)| fixed_name == name)
    }));
    for (name, value) in FIXED_ENVIRONMENT {
        assert!(
            stdout.lines().any(|line| line == format!("{name}={value}")),
            "fixed probe environment missing {name}"
        );
    }
}

/// What: Probe the installed Pi binary with the exact isolated no-model RPC contract.
///
/// Inputs:
/// - Installed `pi` executable and checked-in inert probe extension.
///
/// Output:
/// - Pi starts, exposes only the four restricted tools and one explicit command, then exits.
///
/// Details:
/// - Ignored because Pi is an optional runtime dependency; Wave 0 runs it explicitly.
#[test]
#[ignore = "requires installed Pi >=0.84.0; run explicitly during Wave 0"]
fn live_pi_rpc_capability_probe_exposes_exact_tools() {
    let pi = which::which("pi").expect("Wave 0 live probe requires pi on PATH");
    let version = Command::new(&pi)
        .arg("--version")
        .output()
        .expect("must run pi --version");
    let help = Command::new(&pi)
        .arg("--help")
        .output()
        .expect("must run pi --help");
    validate_cli_contract(
        &String::from_utf8(version.stdout).expect("version output must be UTF-8"),
        &String::from_utf8(help.stdout).expect("help output must be UTF-8"),
    )
    .expect("installed Pi must satisfy the scanner CLI contract");

    let output = run_live_probe(&pi).expect("isolated no-model Pi RPC probe must run");
    assert!(
        output.status.success(),
        "Pi probe failed: status={} stderr={}",
        output.status,
        output.stderr
    );
    let records = parse_rpc_records(&output.stdout).expect("Pi stdout must be strict JSONL");
    let commands = records
        .iter()
        .find(|record| record.get("id") == Some(&Value::String("commands".to_string())))
        .expect("get_commands response must be present");
    let commands = commands["data"]["commands"]
        .as_array()
        .expect("commands data must be an array");
    assert!(
        commands.iter().any(|entry| {
            entry.get("name").and_then(Value::as_str) == Some("pacsea-probe-tools")
                && entry.pointer("/sourceInfo/source").and_then(Value::as_str) == Some("cli")
        }),
        "explicit probe command must be present: {commands:?}"
    );
    assert!(
        commands.iter().all(|entry| {
            entry.pointer("/sourceInfo/scope").and_then(Value::as_str) == Some("temporary")
                && matches!(
                    entry.pointer("/sourceInfo/source").and_then(Value::as_str),
                    Some("cli" | "inline")
                )
        }),
        "ambient project/user commands must be disabled: {commands:?}"
    );

    let active = records
        .iter()
        .filter_map(|record| record.get("message").and_then(Value::as_str))
        .find_map(|message| message.strip_prefix(ACTIVE_TOOLS_MARKER))
        .expect("probe extension must report active tools");
    let active = serde_json::from_str::<Vec<String>>(active)
        .expect("active tool notification must contain JSON names");
    assert_eq!(active, RESTRICTED_TOOL_NAMES.map(str::to_string).to_vec());
}
