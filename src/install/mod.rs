//! Modular install subsystem.
//!
//! This module splits the previous monolithic `install.rs` into focused
//! submodules. Public API is preserved via re-exports.

mod batch;
pub mod command;
mod logging;
mod remove;
mod scan;
mod shell;
mod single;
mod utils;

pub use batch::spawn_install_all;
pub use logging::log_removed;
mod patterns;
pub use remove::spawn_remove_all;
#[cfg(not(target_os = "windows"))]
pub use scan::spawn_aur_scan_for;

#[cfg(not(target_os = "windows"))]
#[allow(clippy::too_many_arguments)]
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
pub use shell::spawn_shell_commands_in_terminal;
pub use single::spawn_install;
pub use utils::command_on_path;
