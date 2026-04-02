use tokio::sync::mpsc;
use tokio::time::{Duration, sleep, timeout};

use crate::install::resolve_command_on_path;
use crate::sources;
use crate::sources::fetch_details;
use crate::state::app_state::{
    PkgbuildCheckFinding, PkgbuildCheckSeverity, PkgbuildCheckTool, PkgbuildToolRawResult,
};
use crate::state::{
    PackageDetails, PackageItem, PkgbuildCheckRequest, PkgbuildCheckResponse, Source,
};

/// What: Spawn background worker for batched package details fetching.
///
/// Inputs:
/// - `net_err_tx`: Channel sender for network errors
/// - `details_req_rx`: Channel receiver for detail requests
/// - `details_res_tx`: Channel sender for detail responses
///
/// Details:
/// - Batches requests within a 120ms window to reduce network calls
/// - Deduplicates requests by package name
/// - Filters out disallowed packages
pub fn spawn_details_worker(
    net_err_tx: &mpsc::UnboundedSender<String>,
    mut details_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    details_res_tx: mpsc::UnboundedSender<PackageDetails>,
) {
    use std::collections::HashSet;
    let net_err_tx_details = net_err_tx.clone();
    tokio::spawn(async move {
        const DETAILS_BATCH_WINDOW_MS: u64 = 120;
        loop {
            let Some(first) = details_req_rx.recv().await else {
                break;
            };
            let mut batch: Vec<PackageItem> = vec![first];
            loop {
                tokio::select! {
                    Some(next) = details_req_rx.recv() => { batch.push(next); }
                    () = sleep(Duration::from_millis(DETAILS_BATCH_WINDOW_MS)) => { break; }
                }
            }
            let mut seen: HashSet<String> = HashSet::new();
            let mut ordered: Vec<PackageItem> = Vec::with_capacity(batch.len());
            for it in batch {
                if seen.insert(it.name.clone()) {
                    ordered.push(it);
                }
            }
            for it in ordered {
                if !crate::logic::is_allowed(&it.name) {
                    continue;
                }
                match fetch_details(it.clone()).await {
                    Ok(details) => {
                        let _ = details_res_tx.send(details);
                    }
                    Err(e) => {
                        let msg = match it.source {
                            Source::Official { .. } => format!(
                                "Official package details unavailable for {}: {}",
                                it.name, e
                            ),
                            Source::Aur => {
                                format!("AUR package details unavailable for {}: {e}", it.name)
                            }
                        };
                        let _ = net_err_tx_details.send(msg);
                    }
                }
            }
        }
    });
}

/// What: Spawn background worker for PKGBUILD fetching.
///
/// Inputs:
/// - `pkgb_req_rx`: Channel receiver for PKGBUILD requests
/// - `pkgb_res_tx`: Channel sender for PKGBUILD responses
pub fn spawn_pkgbuild_worker(
    mut pkgb_req_rx: mpsc::UnboundedReceiver<PackageItem>,
    pkgb_res_tx: mpsc::UnboundedSender<(String, String)>,
) {
    tokio::spawn(async move {
        while let Some(item) = pkgb_req_rx.recv().await {
            let name = item.name.clone();
            match sources::fetch_pkgbuild_fast(&item).await {
                Ok(txt) => {
                    let _ = pkgb_res_tx.send((name, txt));
                }
                Err(e) => {
                    let _ = pkgb_res_tx.send((name, format!("Failed to fetch PKGBUILD: {e}")));
                }
            }
        }
    });
}

/// What: Spawn worker for PKGBUILD static checks.
///
/// Details:
/// - Runs `ShellCheck` and `namcap` in partial mode (if one tool is missing, still runs the other).
/// - Prefers package cache PKGBUILD paths when available and falls back to a temp PKGBUILD file.
/// - Supports dry-run by returning command previews without spawning subprocesses.
pub fn spawn_pkgbuild_checks_worker(
    mut req_rx: mpsc::UnboundedReceiver<PkgbuildCheckRequest>,
    res_tx: mpsc::UnboundedSender<PkgbuildCheckResponse>,
) {
    tokio::spawn(async move {
        while let Some(req) = req_rx.recv().await {
            let response = run_pkgbuild_checks(req).await;
            let _ = res_tx.send(response);
        }
    });
}

/// What: Try to find a cached extracted PKGBUILD file for a package.
fn find_cached_pkgbuild_path(package_name: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from)?;
    let candidates = [
        home.join(".cache")
            .join("paru")
            .join("clone")
            .join(package_name)
            .join("PKGBUILD"),
        home.join(".cache")
            .join("yay")
            .join(package_name)
            .join("PKGBUILD"),
    ];
    candidates.into_iter().find(|path| path.is_file())
}

/// What: Parse `ShellCheck` output in `gcc` format into findings.
fn parse_shellcheck_findings(output: &str) -> Vec<PkgbuildCheckFinding> {
    output
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, ':');
            let _file = parts.next()?;
            let line_no = parts.next()?.trim().parse::<u32>().ok();
            let _col = parts.next()?;
            let msg = parts.next()?.trim();
            let severity = if msg.contains("error:") {
                PkgbuildCheckSeverity::Error
            } else if msg.contains("warning:") {
                PkgbuildCheckSeverity::Warning
            } else {
                PkgbuildCheckSeverity::Info
            };
            Some(PkgbuildCheckFinding {
                tool: PkgbuildCheckTool::Shellcheck,
                severity,
                line: line_no,
                message: msg.to_string(),
            })
        })
        .collect()
}

/// What: Parse namcap output lines into findings.
fn parse_namcap_findings(output: &str) -> Vec<PkgbuildCheckFinding> {
    output
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                return None;
            }
            let severity = if trimmed.contains(" E: ") {
                PkgbuildCheckSeverity::Error
            } else if trimmed.contains(" W: ") {
                PkgbuildCheckSeverity::Warning
            } else {
                PkgbuildCheckSeverity::Info
            };
            Some(PkgbuildCheckFinding {
                tool: PkgbuildCheckTool::Namcap,
                severity,
                line: None,
                message: trimmed.to_string(),
            })
        })
        .collect()
}

/// What: Build `ShellCheck` `--exclude=CODE,...` from configured comma-separated rule IDs.
///
/// Inputs:
/// - `raw`: Value from `Settings::pkgbuild_shellcheck_exclude` (already normalized)
///
/// Output:
/// - `Some("--exclude=SC2034,...")` when at least one rule remains; `None` when unset or only whitespace
///
/// Details:
/// - Splits on commas, trims segments, drops empties, joins with commas for a single shellcheck flag.
fn shellcheck_exclude_flag_from_settings_list(raw: &str) -> Option<String> {
    let codes: Vec<&str> = raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if codes.is_empty() {
        None
    } else {
        Some(format!("--exclude={}", codes.join(",")))
    }
}

/// Duration limit for each PKGBUILD checker subprocess (`ShellCheck` / namcap).
///
/// Enforced with `tokio::time::timeout` around `spawn_blocking` so we do not depend on the
/// coreutils `timeout` binary. The process may keep running in the thread pool until it exits
/// after this limit fires; the UI still reports a timeout via `timed_out`.
const PKGBUILD_CHECK_TOOL_TIMEOUT: Duration = Duration::from_secs(8);

/// What: Execute one checker command with async wall-clock timeout and output capture.
///
/// Inputs:
/// - `tool`: Which checker produced the result
/// - `tool_path`: Resolved absolute path to the checker binary
/// - `args`: Arguments passed after the program name
/// - `working_dir`: Current directory for the subprocess
/// - `dry_run`: When true, returns a preview without spawning
///
/// Output:
/// - `PkgbuildToolRawResult` with stdout/stderr, exit status, and `timed_out` when the limit elapses
///
/// Details:
/// - Uses `tokio::time::timeout` plus `spawn_blocking(|| Command::output())` on the checker itself,
///   not an external `timeout` helper, so a missing coreutils `timeout` cannot mask the real tool.
async fn run_tool_with_timeout(
    tool: PkgbuildCheckTool,
    tool_path: &std::path::Path,
    args: &[&str],
    working_dir: &std::path::Path,
    dry_run: bool,
) -> PkgbuildToolRawResult {
    let cmd_preview = format!("{} {}", tool_path.display(), args.join(" "));
    if dry_run {
        return PkgbuildToolRawResult {
            tool,
            available: true,
            exit_code: None,
            timed_out: false,
            command: cmd_preview,
            stdout: "dry-run: command not executed".to_string(),
            stderr: String::new(),
        };
    }

    let tool_path_owned = tool_path.to_path_buf();
    let args_owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let working_dir_owned = working_dir.to_path_buf();

    let join = tokio::task::spawn_blocking(move || {
        let mut command = std::process::Command::new(tool_path_owned);
        command
            .args(&args_owned)
            .current_dir(working_dir_owned)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        command.output()
    });

    match timeout(PKGBUILD_CHECK_TOOL_TIMEOUT, join).await {
        Ok(Ok(Ok(output))) => PkgbuildToolRawResult {
            tool,
            available: true,
            exit_code: output.status.code(),
            timed_out: false,
            command: cmd_preview,
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        },
        Ok(Ok(Err(err))) => PkgbuildToolRawResult {
            tool,
            available: true,
            exit_code: None,
            timed_out: false,
            command: cmd_preview,
            stdout: String::new(),
            stderr: format!("failed to execute tool: {err}"),
        },
        Ok(Err(err)) => PkgbuildToolRawResult {
            tool,
            available: true,
            exit_code: None,
            timed_out: false,
            command: cmd_preview,
            stdout: String::new(),
            stderr: format!("failed waiting for tool result: {err}"),
        },
        Err(_elapsed) => PkgbuildToolRawResult {
            tool,
            available: true,
            exit_code: None,
            timed_out: true,
            command: cmd_preview,
            stdout: String::new(),
            stderr: String::new(),
        },
    }
}

/// What: Run `ShellCheck` + `namcap` checks for PKGBUILD preview.
async fn run_pkgbuild_checks(req: PkgbuildCheckRequest) -> PkgbuildCheckResponse {
    let shellcheck_path = resolve_command_on_path("shellcheck");
    let namcap_path = resolve_command_on_path("namcap");

    let mut missing_tools = Vec::new();
    if shellcheck_path.is_none() {
        missing_tools.push("shellcheck missing: install package `shellcheck`".to_string());
    }
    if namcap_path.is_none() {
        missing_tools.push("namcap missing: install package `namcap`".to_string());
    }

    let mut temp_path: Option<std::path::PathBuf> = None;
    let pkgbuild_path = if let Some(path) = find_cached_pkgbuild_path(&req.package_name) {
        path
    } else {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "pacsea_pkgbuild_check_{}_{}.PKGBUILD",
            req.package_name,
            std::process::id()
        ));
        if let Err(err) = std::fs::write(&path, req.pkgbuild_text.as_bytes()) {
            return PkgbuildCheckResponse {
                package_name: req.package_name,
                findings: Vec::new(),
                raw_results: Vec::new(),
                missing_tools,
                last_error: Some(format!("failed to create temp PKGBUILD for checks: {err}")),
            };
        }
        temp_path = Some(path.clone());
        path
    };

    let working_dir = pkgbuild_path
        .parent()
        .map_or_else(std::env::temp_dir, std::path::Path::to_path_buf);

    let mut raw_results = Vec::new();
    if let Some(path) = shellcheck_path.as_ref() {
        let exclude_raw = crate::theme::settings().pkgbuild_shellcheck_exclude;
        let path_owned = pkgbuild_path.to_string_lossy().into_owned();
        let mut sc_args: Vec<String> = vec!["--format=gcc".to_string()];
        if let Some(flag) = shellcheck_exclude_flag_from_settings_list(&exclude_raw) {
            sc_args.push(flag);
        }
        sc_args.push(path_owned);
        let sc_refs: Vec<&str> = sc_args.iter().map(String::as_str).collect();
        raw_results.push(
            run_tool_with_timeout(
                PkgbuildCheckTool::Shellcheck,
                path,
                &sc_refs,
                &working_dir,
                req.dry_run,
            )
            .await,
        );
    }
    if let Some(path) = namcap_path.as_ref() {
        let path_owned = pkgbuild_path.to_string_lossy().into_owned();
        let namcap_args: Vec<&str> = vec![path_owned.as_str()];
        raw_results.push(
            run_tool_with_timeout(
                PkgbuildCheckTool::Namcap,
                path,
                &namcap_args,
                &working_dir,
                req.dry_run,
            )
            .await,
        );
    }

    let mut findings = Vec::new();
    for result in &raw_results {
        if req.dry_run {
            findings.push(PkgbuildCheckFinding {
                tool: result.tool,
                severity: PkgbuildCheckSeverity::Info,
                line: None,
                message: format!("dry-run: would run `{}`", result.command),
            });
            continue;
        }
        match result.tool {
            PkgbuildCheckTool::Shellcheck => {
                findings.extend(parse_shellcheck_findings(&result.stdout));
                findings.extend(parse_shellcheck_findings(&result.stderr));
            }
            PkgbuildCheckTool::Namcap => {
                findings.extend(parse_namcap_findings(&result.stdout));
                findings.extend(parse_namcap_findings(&result.stderr));
            }
        }
        if result.timed_out {
            findings.push(PkgbuildCheckFinding {
                tool: result.tool,
                severity: PkgbuildCheckSeverity::Error,
                line: None,
                message: "check timed out after 8 seconds".to_string(),
            });
        }
    }

    if let Some(path) = temp_path {
        let _ = std::fs::remove_file(path);
    }

    PkgbuildCheckResponse {
        package_name: req.package_name,
        findings,
        raw_results,
        missing_tools,
        last_error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        parse_namcap_findings, parse_shellcheck_findings, run_tool_with_timeout,
        shellcheck_exclude_flag_from_settings_list,
    };
    use crate::state::app_state::{PkgbuildCheckSeverity, PkgbuildCheckTool};

    #[test]
    /// What: `shellcheck_exclude_flag_from_settings_list` builds a flag from comma-separated IDs.
    fn shellcheck_exclude_flag_trims_and_joins() {
        let f = shellcheck_exclude_flag_from_settings_list(" SC2034 , SC2164 ");
        assert_eq!(f.as_deref(), Some("--exclude=SC2034,SC2164"));
    }

    #[test]
    /// What: Empty or whitespace-only exclude list yields no `--exclude` flag.
    fn shellcheck_exclude_flag_none_when_empty() {
        assert_eq!(shellcheck_exclude_flag_from_settings_list(""), None);
        assert_eq!(shellcheck_exclude_flag_from_settings_list(" , , "), None);
    }

    #[test]
    /// What: Parse `ShellCheck` gcc format lines into findings.
    fn parse_shellcheck_gcc_output() {
        let out = "PKGBUILD:12:7: warning: quote this to prevent word splitting [SC2086]";
        let findings = parse_shellcheck_findings(out);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].tool, PkgbuildCheckTool::Shellcheck);
        assert_eq!(findings[0].line, Some(12));
        assert_eq!(findings[0].severity, PkgbuildCheckSeverity::Warning);
    }

    #[tokio::test]
    /// What: Checker runs use the resolved binary directly, not a `timeout` wrapper.
    async fn run_tool_with_timeout_invokes_binary_without_coreutils_timeout() {
        let result = run_tool_with_timeout(
            PkgbuildCheckTool::Shellcheck,
            std::path::Path::new("/bin/true"),
            &[],
            std::path::Path::new("/"),
            false,
        )
        .await;
        assert!(result.available);
        assert_eq!(result.exit_code, Some(0));
        assert!(!result.timed_out);
        assert!(result.stderr.is_empty());
    }

    #[test]
    /// What: Parse namcap lines into severity-tagged findings.
    fn parse_namcap_output() {
        let out = "PKGBUILD (foo) W: Referenced library 'libx.so' is an uninstalled dependency\nPKGBUILD (foo) E: Description too short";
        let findings = parse_namcap_findings(out);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].tool, PkgbuildCheckTool::Namcap);
        assert_eq!(findings[0].severity, PkgbuildCheckSeverity::Warning);
        assert_eq!(findings[1].severity, PkgbuildCheckSeverity::Error);
    }
}
