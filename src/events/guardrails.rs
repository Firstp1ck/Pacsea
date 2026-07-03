//! TUI-side guardrail helpers for pre-transaction checks.

use crate::logic::preflight::guardrails::{GuardrailContext, GuardrailIssue, check_db_lock};
use crate::state::AppState;

/// What: Build an actionable alert message when the pacman database is locked.
///
/// Inputs:
/// - `app`: Application state (used for i18n lookup).
///
/// Output:
/// - `Some(message)` when the database is locked; `None` when the transaction may start.
///
/// Details:
/// - A held lock (package manager running) advises waiting; a stale lock advises
///   removing the lock file.
/// - Reuses the CLI guardrail translations so wording stays consistent.
#[must_use]
pub fn db_lock_alert_message(app: &AppState) -> Option<String> {
    let ctx = GuardrailContext::default();
    let Some(GuardrailIssue::DbLocked {
        lock_path,
        pacman_running,
    }) = check_db_lock(&ctx)
    else {
        return None;
    };

    let mut message = crate::i18n::t_fmt1(app, "app.cli.guardrails.db_locked", lock_path.display());
    message.push('\n');
    if pacman_running {
        message.push_str(&crate::i18n::t(app, "app.cli.guardrails.db_locked_running"));
    } else {
        message.push_str(&crate::i18n::t_fmt1(
            app,
            "app.cli.guardrails.db_locked_stale",
            lock_path.display(),
        ));
    }
    Some(message)
}
