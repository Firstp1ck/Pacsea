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
pub use remove::spawn_remove_all;
#[allow(unused_imports)]
pub use scan::spawn_aur_scan_for;

#[cfg(not(target_os = "windows"))]
pub use scan::spawn_aur_scan_for_with_config;
// pub use scan::spawn_aur_scan_in_dir;
pub use shell::spawn_shell_commands_in_terminal;
pub use single::spawn_install;
pub use utils::command_on_path;
// pub use command::build_install_command;
