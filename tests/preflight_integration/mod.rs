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

mod aur_mix;
mod cache_sync;
mod caching;
mod conflicts;
mod data_arrival;
mod edge_cases;
mod error_handling;
mod helpers;
mod large_datasets;
mod package_operations;
mod persistence;
mod remove_operations;
mod tab_switching;
mod tab_variations;
