//! CLI-side rendering and enforcement of pre-transaction guardrails.
//!
//! Runs the read-only checks from [`pacsea::logic::preflight::guardrails`] before
//! install/remove/update transactions: a locked pacman database aborts with guidance,
//! while low disk space and stale sync databases print actionable warnings.

use crate::args::i18n;
use pacsea::logic::preflight::guardrails::{
    GuardrailContext, GuardrailIssue, GuardrailOperation, run_guardrails,
};

/// What: Run guardrail checks for a CLI transaction, printing findings to stderr.
///
/// Inputs:
/// - `op`: The transaction kind about to run (install/remove/update).
///
/// Output:
/// - Returns normally when the transaction may proceed; exits with code 1 when the
///   pacman database is locked.
///
/// Details:
/// - A held lock (package manager running) advises waiting; a stale lock advises
///   removing the lock file.
/// - Low disk space and stale sync databases are warnings only; the transaction
///   continues so pacman can make the final call.
pub fn enforce(op: GuardrailOperation) {
    let ctx = GuardrailContext::default();
    let mut blocked = false;

    for issue in run_guardrails(&ctx, op) {
        match issue {
            GuardrailIssue::DbLocked {
                lock_path,
                pacman_running,
            } => {
                blocked = true;
                eprintln!(
                    "{}",
                    i18n::t_fmt1("app.cli.guardrails.db_locked", lock_path.display())
                );
                if pacman_running {
                    eprintln!("{}", i18n::t("app.cli.guardrails.db_locked_running"));
                } else {
                    eprintln!(
                        "{}",
                        i18n::t_fmt1("app.cli.guardrails.db_locked_stale", lock_path.display())
                    );
                }
            }
            GuardrailIssue::LowDiskSpace {
                path,
                available_mib,
                min_free_mib,
            } => {
                eprintln!(
                    "{}",
                    i18n::t_fmt(
                        "app.cli.guardrails.low_disk",
                        &[
                            &available_mib as &dyn std::fmt::Display,
                            &path.display(),
                            &min_free_mib,
                        ]
                    )
                );
                eprintln!("{}", i18n::t("app.cli.guardrails.low_disk_hint"));
            }
            GuardrailIssue::SyncDbStale { age_days } => {
                eprintln!(
                    "{}",
                    i18n::t_fmt1("app.cli.guardrails.sync_stale", age_days)
                );
                eprintln!("{}", i18n::t("app.cli.guardrails.sync_stale_hint"));
            }
        }
    }

    if blocked {
        tracing::error!("Aborting CLI transaction: pacman database is locked");
        std::process::exit(1);
    }
}
