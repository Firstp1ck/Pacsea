//! Passwordless sudo integration tests module.
//!
//! This module contains tests for verifying the passwordless sudo implementation
//! works correctly across all workflows (install, update, remove, downgrade, filesync).

mod helpers;
mod interactive_auth_integration;
mod passwordless_sudo_integration;
