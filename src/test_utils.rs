//! Test utilities for common test setup.
//!
//! This module provides shared test helpers used across multiple test modules.

#[cfg(test)]
use crate::state::AppState;

#[cfg(test)]
/// What: Provide a baseline `AppState` for handler tests.
///
/// Inputs: None
///
/// Output: Fresh `AppState` with default values
pub fn new_app() -> AppState {
    AppState::default()
}
