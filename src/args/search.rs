//! Command-line search functionality.

use crate::args::i18n;

/// What: Handle command-line search mode by executing yay/paru -Ss with the search pattern.
///
/// Inputs:
/// - `search_query`: The search pattern to use.
///
/// Output:
/// - Exits the process with the command's exit code or 1 on error.
///
/// Details:
/// - Checks for paru first, then falls back to yay.
/// - Executes the search command and outputs results to terminal.
/// - Exits immediately after showing results (doesn't launch TUI).
pub fn handle_search(search_query: &str) -> ! {
    use std::process::{Command, Stdio};

    tracing::info!(query = %search_query, "Search mode requested from CLI");

    // Check for paru first, then yay
    let has_paru = Command::new("paru")
        .args(["--version"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .is_ok();

    let has_yay = if has_paru {
        false
    } else {
        Command::new("yay")
            .args(["--version"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .is_ok()
    };

    if has_paru {
        tracing::info!("Using paru for search");
        let status = Command::new("paru").args(["-Ss", search_query]).status();

        match status {
            Ok(exit_status) => {
                std::process::exit(exit_status.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!("{}", i18n::t_fmt1("app.cli.search.paru_exec_failed", &e));
                tracing::error!(error = %e, "Failed to execute paru");
                std::process::exit(1);
            }
        }
    } else if has_yay {
        tracing::info!("Using yay for search");
        let status = Command::new("yay").args(["-Ss", search_query]).status();

        match status {
            Ok(exit_status) => {
                std::process::exit(exit_status.code().unwrap_or(1));
            }
            Err(e) => {
                eprintln!("{}", i18n::t_fmt1("app.cli.search.yay_exec_failed", &e));
                tracing::error!(error = %e, "Failed to execute yay");
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("{}", i18n::t("app.cli.search.neither_helper_available"));
        tracing::error!("Neither paru nor yay is available for search");
        std::process::exit(1);
    }
}
