//! Integration tests for preflight modal optimization features.
//!
//! Tests cover:
//! - Out-of-order data arrival (stages completing in different orders)
//! - Cancellation support (aborting work when modal closes)
//! - Caching and data synchronization
//! - Package operations and management
//! - Tab switching and state management
//! - Error handling and edge cases
//! - AUR and official package mixing
//! - Large datasets and performance
//! - Persistence across tabs
//! - Conflict resolution

#![cfg(test)]

mod helpers;
mod data_arrival;
mod caching;
mod package_operations;
mod tab_switching;
mod error_handling;
mod remove_operations;
mod tab_variations;
mod edge_cases;
mod cache_sync;
mod aur_mix;
mod large_datasets;
mod persistence;
mod conflicts;

pub use helpers::*;

