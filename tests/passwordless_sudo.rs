//! Integration and end-to-end tests for passwordless sudo implementation.
//!
//! Tests cover:
//! - Install workflow with passwordless sudo (both preflight and direct workflows)
//! - Update workflow with passwordless sudo
//! - Remove workflow (always requires password)
//! - Downgrade workflow with passwordless sudo
//! - `FileSync` workflow with passwordless sudo
//! - Modal state verification for both active and deactivated passwordless sudo

#[path = "passwordless_sudo/mod.rs"]
mod passwordless_sudo_tests;
