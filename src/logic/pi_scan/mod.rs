//! Domain logic for Pi scanning: identity, baselines, manifests, detectors, prompts, observation,
//! pricing, results, and validated result storage.

pub mod acquisition;
pub mod baseline;
pub mod detectors;
pub mod head_source;
pub mod identity;
pub mod manifest;
pub mod network;
pub mod observer;
pub mod pricing;
pub mod prompt;
pub mod recipe;
pub mod result;
pub mod result_store;
pub mod signature;
pub mod source;
