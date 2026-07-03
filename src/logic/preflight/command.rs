//! Command execution abstraction for preflight operations.
//!
//! This module provides the [`CommandRunner`] trait and implementations for
//! executing system commands, enabling testability through dependency injection.
//!
//! The implementation lives in [`crate::util::command`]; this module re-exports
//! it so preflight consumers and tests keep their existing import paths.

pub use crate::util::command::{CommandError, CommandRunner, SystemCommandRunner};
