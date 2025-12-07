/// Auxiliary background workers (status, news, tick, index updates).
pub mod auxiliary;
/// AUR comments fetching worker.
pub mod comments;
/// Package details fetching worker.
pub mod details;
/// Package installation/removal executor worker.
pub mod executor;
/// News article content fetching worker.
pub mod news_content;
/// Preflight analysis workers (dependencies, files, services, sandbox, summary).
pub mod preflight;
/// Package search worker.
pub mod search;
