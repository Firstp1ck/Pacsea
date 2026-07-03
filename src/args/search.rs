//! Command-line search functionality.

use crate::args::{i18n, json, utils};

/// A single parsed entry from `-Ss` search output.
struct SsEntry {
    /// Repository the package belongs to (e.g. "extra", "aur").
    repo: String,
    /// Package name.
    name: String,
    /// Package version string.
    version: String,
    /// Package description (joined from indented continuation lines).
    description: String,
}

/// What: Parse `pacman`/`paru`/`yay` `-Ss` output into structured entries.
///
/// Inputs:
/// - `output`: Raw stdout from a `-Ss` invocation.
///
/// Output:
/// - Vector of parsed entries in input order.
///
/// Details:
/// - Header lines have the form `repo/name version [extras]`; indented lines are
///   description continuations attached to the preceding header.
/// - Lines without a `repo/name` identifier are skipped.
fn parse_ss_output(output: &str) -> Vec<SsEntry> {
    let mut entries: Vec<SsEntry> = Vec::new();
    for line in output.lines() {
        if line.starts_with(char::is_whitespace) {
            if let Some(last) = entries.last_mut() {
                let desc = line.trim();
                if !desc.is_empty() {
                    if !last.description.is_empty() {
                        last.description.push(' ');
                    }
                    last.description.push_str(desc);
                }
            }
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(ident) = parts.next() else { continue };
        let Some((repo, name)) = ident.split_once('/') else {
            continue;
        };
        if repo.is_empty() || name.is_empty() {
            continue;
        }
        entries.push(SsEntry {
            repo: repo.to_string(),
            name: name.to_string(),
            version: parts.next().unwrap_or_default().to_string(),
            description: String::new(),
        });
    }
    entries
}

/// What: Run a captured `-Ss` search and print results as a JSON envelope.
///
/// Inputs:
/// - `search_query`: The search pattern.
/// - `helper`: Resolved AUR helper, or `None` to fall back to `pacman -Ss`.
///
/// Output:
/// - Exits 0 with a JSON envelope on stdout (empty results on no match), 1 on execution failure.
///
/// Details:
/// - `-Ss` exits non-zero when nothing matches; that case emits an empty result list.
/// - Diagnostics go to stderr so stdout stays parseable.
fn handle_search_json(search_query: &str, helper: Option<&str>) -> ! {
    use std::process::{Command, Stdio};

    let program = helper.unwrap_or("pacman");
    tracing::info!(program = %program, "Using {program} for JSON search");

    let output = Command::new(program)
        .args(["-Ss", search_query])
        .stdin(Stdio::null())
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if !out.status.success() && !stdout.trim().is_empty() {
                eprintln!("{}", String::from_utf8_lossy(&out.stderr));
                tracing::error!(exit_code = out.status.code(), "Search command failed");
                std::process::exit(out.status.code().unwrap_or(1));
            }
            let results: Vec<serde_json::Value> = parse_ss_output(&stdout)
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "repo": e.repo,
                        "name": e.name,
                        "version": e.version,
                        "description": e.description,
                    })
                })
                .collect();
            json::print_envelope(
                "search",
                &serde_json::json!({
                    "query": search_query,
                    "tool": program,
                    "results": results,
                }),
            );
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.search.exec_failed", &e));
            tracing::error!(error = %e, program = %program, "Failed to execute search");
            std::process::exit(1);
        }
    }
}

/// What: Handle command-line search mode by executing an AUR helper (or pacman) `-Ss`.
///
/// Inputs:
/// - `search_query`: The search pattern to use.
/// - `json_output`: When true, capture results and print a JSON envelope on stdout.
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Resolves the helper via [`utils::get_aur_helper`], honoring the `aur_helper`
///   settings key (auto-detect prefers paru).
/// - Without `--json`, the helper's output streams directly to the terminal.
/// - With `--json`, falls back to `pacman -Ss` when no helper is available.
/// - Exits immediately after showing results (doesn't launch TUI).
pub fn handle_search(search_query: &str, json_output: bool) -> ! {
    use std::process::Command;

    tracing::info!(query = %search_query, "Search mode requested from CLI");

    let helper = utils::get_aur_helper();

    if json_output {
        handle_search_json(search_query, helper);
    }

    let Some(helper) = helper else {
        eprintln!("{}", i18n::t("app.cli.search.neither_helper_available"));
        tracing::error!("Neither paru nor yay is available for search");
        std::process::exit(1);
    };

    tracing::info!(helper = %helper, "Using {helper} for search");
    let status = Command::new(helper).args(["-Ss", search_query]).status();

    match status {
        Ok(exit_status) => {
            std::process::exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            let key = if helper == "paru" {
                "app.cli.search.paru_exec_failed"
            } else {
                "app.cli.search.yay_exec_failed"
            };
            eprintln!("{}", i18n::t_fmt1(key, &e));
            tracing::error!(error = %e, helper = %helper, "Failed to execute search");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// What: Verify `-Ss` output parsing extracts repo, name, version, and description.
    ///
    /// Inputs:
    /// - Two-entry `-Ss` style listing with indented description lines.
    ///
    /// Output:
    /// - Two entries with all fields populated.
    ///
    /// Details:
    /// - Also covers trailing metadata after the version (installed markers, vote counts).
    fn parse_ss_output_extracts_fields() {
        let output = "extra/ripgrep 14.1.0-1 [installed]\n    A fast search tool\naur/ripgrep-git 14.1.0.r5-1 (+12 0.5)\n    Development version\n    of ripgrep\n";
        let entries = parse_ss_output(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].repo, "extra");
        assert_eq!(entries[0].name, "ripgrep");
        assert_eq!(entries[0].version, "14.1.0-1");
        assert_eq!(entries[0].description, "A fast search tool");
        assert_eq!(entries[1].repo, "aur");
        assert_eq!(entries[1].name, "ripgrep-git");
        assert_eq!(entries[1].description, "Development version of ripgrep");
    }

    #[test]
    /// What: Verify malformed lines and empty input yield no entries.
    ///
    /// Inputs:
    /// - Lines lacking a `repo/name` identifier and an empty string.
    ///
    /// Output:
    /// - Empty entry list in both cases.
    ///
    /// Details:
    /// - Guards against panics on unexpected helper output.
    fn parse_ss_output_skips_malformed_lines() {
        assert!(parse_ss_output("").is_empty());
        assert!(parse_ss_output("no-slash-here 1.0\n    desc\n").is_empty());
        assert!(parse_ss_output("/missing-repo 1.0\n").is_empty());
    }
}
