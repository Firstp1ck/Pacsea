//! Modular install subsystem.
//!
//! This module splits the previous monolithic `install.rs` into focused
//! submodules. Public API is preserved via re-exports.

/// Batch installation operations.
mod batch;
pub mod command;
/// Direct installation operations.
mod direct;
/// Executor for package operations.
mod executor;
/// Logging utilities for install operations.
mod logging;
/// Package removal operations.
mod remove;
/// Security scanning operations.
mod scan;
/// Shell command execution.
mod shell;
/// Single package installation.
mod single;
/// Utility functions for install operations.
mod utils;

pub use batch::spawn_install_all;
pub use logging::log_removed;
mod patterns;
pub use remove::{check_config_directories, spawn_remove_all};

#[cfg(not(target_os = "windows"))]
pub use scan::spawn::build_sleuth_command_for_terminal;

#[cfg(not(target_os = "windows"))]
#[allow(clippy::too_many_arguments)]
/// What: Load user-configured suspicious patterns and launch the AUR scan pipeline.
///
/// Input:
/// - `pkg`: Package name passed to the scanner.
/// - `do_clamav`/`do_trivy`/`do_semgrep`/`do_shellcheck`/`do_virustotal`/`do_custom`/`do_sleuth`: Toggles for optional scan stages.
///
/// Output:
/// - Spawns a terminal executing the scan workflow defined in `scan::spawn_aur_scan_for_with_config`.
///
/// Details:
/// - Loads `pattern.conf`, publishes severity regexes via environment variables, and lets the scan module honour them.
/// - Environment overrides take precedence so UI toggles and config-driven patterns cooperate.
#[allow(clippy::fn_params_excessive_bools)]
pub fn spawn_aur_scan_for_with_config(
    pkg: &str,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    do_sleuth: bool,
) {
    // Load configurable suspicious patterns (pattern.conf), override defaults via env vars
    let sets = crate::install::patterns::load();
    unsafe {
        std::env::set_var("PACSEA_PATTERNS_CRIT", &sets.critical);
    }
    unsafe {
        std::env::set_var("PACSEA_PATTERNS_HIGH", &sets.high);
    }
    unsafe {
        std::env::set_var("PACSEA_PATTERNS_MEDIUM", &sets.medium);
    }
    unsafe {
        std::env::set_var("PACSEA_PATTERNS_LOW", &sets.low);
    }

    // Forward to scanner; scan.rs will export defaults, but our env vars take precedence in crit/high/med/low expansions
    scan::spawn_aur_scan_for_with_config(
        pkg,
        do_clamav,
        do_trivy,
        do_semgrep,
        do_shellcheck,
        do_virustotal,
        do_custom,
        do_sleuth,
    );
}
pub use direct::{
    start_integrated_install, start_integrated_install_all, start_integrated_remove_all,
};
#[cfg(not(target_os = "windows"))]
pub use executor::build_scan_command_for_executor;
pub use executor::{
    ExecutorOutput, ExecutorRequest, build_downgrade_command_for_executor,
    build_install_command_for_executor, build_remove_command_for_executor,
    build_update_command_for_executor,
};
pub use shell::spawn_shell_commands_in_terminal;
pub use single::spawn_install;
pub use utils::command_on_path;
pub use utils::shell_single_quote;
