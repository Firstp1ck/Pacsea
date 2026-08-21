# Pi Scan budget post-fix blocker handoff

Implemented PSB-001 through PSB-005 without changing plans, reports, PR files, dependencies, staged state, or unrelated modules.

## Changes

- **PSB-001:** Added a synchronized production consent projection and an owner-level unattended-start gate. Every drain iteration re-reads the projection immediately before scheduling. Foreground work bypasses the gate. Revocations are published before potentially blocking persistence and remain fail-closed on policy-update errors.
- **PSB-002:** Added one exact checked `parse_pi_scan_cost_microusd` path used by settings validation, Config Editor, setup construction, runtime construction, queue projection, and event-loop reservation conversion. Explicit zero and exact `u64::MAX` pass; malformed and `u64::MAX + 1` fail closed. Removed the runtime `unwrap_or(0)` weakening.
- **PSB-003:** Atomic config writes now retain exact prior bytes and restore them through a sibling-temp rename if post-rename parent sync fails. Errors report commit failure plus rollback success/failure evidence. Added a one-shot `#[cfg(test)]` internal sync-failure seam; no production environment bypass was added.
- **PSB-004:** Added a token-cap parser diagnostic to `PiScanSettings`; malformed/overflow startup values preserve the prior numeric value but make `validation_issues()` actionable. Valid `0` and `u64::MAX` clear the diagnostic. Guided setup clears stale parser evidence when the user changes the token cap.
- **PSB-005:** Added typed changed/unchanged budget revalidation and `BudgetRevalidated` progress publication. The event loop replaces the connected UI runtime projection with the authoritative pause/accounting snapshot even when observation is disabled and no execution wake is sent.

## Changed files

- `src/app/runtime/event_loop.rs`
- `src/app/runtime/mod.rs`
- `src/app/runtime/workers/pi_scan.rs`
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
- `src/app/runtime/workers/pi_scan_production.rs`
- `src/app/runtime/workers/pi_scan_setup.rs`
- `src/events/modals/config_editor.rs`
- `src/state/pi_scan_setup.rs`
- `src/state/pi_scan_ui.rs`
- `src/theme/config/patch.rs`
- `src/theme/mod.rs`
- `src/theme/settings/parse_settings.rs`
- `src/theme/types.rs`
- `tests/pi_scan/ws9_orchestration.rs`
- `.pi/subagents/handoffs/pi-scan-budget-post-fix-blockers.md` (this requested artifact)

## Tests added or updated

- Two deterministic WS9 two-background-job tests revoke observation authorization and background-paid authorization while the first execution is active; the second remains queued and emits no Started projection.
- Production projection test covers both session-only revocations.
- Exact decimal tests cover `0`, `0.00`, `u64::MAX`, `u64::MAX + 1`, malformed text, Config Editor, and runtime construction.
- Fresh-default parser test proves malformed startup token caps surface through `validation_issues`; valid zero clears the issue.
- Atomic-write and integrated adjustment tests inject post-rename sync failure and prove rejection, no wake, exact old bytes, old owner limits, and restart consistency.
- Production and event-loop tests prove no-observation budget expiry publishes authoritative cleared pause/accounting state without execution wake.

## Validation

Final commands and results:

- `cargo fmt --all` — exit 0.
- `cargo clippy --all-targets --all-features -- -D warnings` — exit 0.
- `cargo check` — exit 0.
- `cargo test --lib pi_scan_production::tests -- --test-threads=1` — exit 0; 17 passed, 1 environment-dependent live test ignored.
- `cargo test --test pi_scan ws9_orchestration -- --test-threads=1` — exit 0; 27 passed.
- `cargo test runtime_cost_budget_ -- --test-threads=1` — exit 0; 2 passed.
- `cargo test parse_settings_pi_scan_ -- --test-threads=1` — exit 0; 3 passed.
- `cargo test validate_pi_scan_cost_cap_accepts_exact_micro_usd_native_bounds_only -- --test-threads=1` — exit 0; 1 passed.
- `cargo test post_rename_sync_failure_restores_exact_prior_snapshot -- --test-threads=1` — exit 0; 1 passed.
- `cargo test budget_revalidation_progress_clears_stale_ui_pause_without_request_channel -- --test-threads=1` — exit 0; 1 passed.
- `git diff --check` — exit 0.
- `git diff --cached --name-only` — exit 0 with no output; no staged files.

Iterative failures encountered and corrected:

- Initial `cargo check` / `cargo test --no-run` runs exited 101 while signatures and new test fields were being wired; final compile/check passes.
- Initial Clippy run exited 101 for documentation/pass-by-reference/test-lock lints; all were fixed and final Clippy passes.
- The first injected rollback reproduction exited 101 because its reservation equaled rather than exceeded the cap; the fixture was corrected to a true blocked reservation, after which rollback and focused suites passed.

Omissions:

- The complete `cargo test -- --test-threads=1` suite was not run because the parent explicitly owns final full serialized tests.
- The existing live Pi setup probe remains ignored because it requires an installed Pi >= 0.84.0 and a configured model route.

## Residual risks

- No known blocker remains in the implemented PSB scope.
- Final confidence: **96/100**. Static paths, focused races, rollback evidence, Clippy, compile, formatting, and diff checks are verified; confidence is reduced only because the parent-owned full serialized suite was intentionally omitted.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "PSB-001 through PSB-005 were implemented only in relevant Pi Scan runtime/state/theme/config/event-loop code and focused tests; final formatting, Clippy, check, focused suites, and diff checks pass."
    }
  ],
  "changedFiles": [
    "src/app/runtime/event_loop.rs",
    "src/app/runtime/mod.rs",
    "src/app/runtime/workers/pi_scan.rs",
    "src/app/runtime/workers/pi_scan_orchestrator.rs",
    "src/app/runtime/workers/pi_scan_production.rs",
    "src/app/runtime/workers/pi_scan_setup.rs",
    "src/events/modals/config_editor.rs",
    "src/state/pi_scan_setup.rs",
    "src/state/pi_scan_ui.rs",
    "src/theme/config/patch.rs",
    "src/theme/mod.rs",
    "src/theme/settings/parse_settings.rs",
    "src/theme/types.rs",
    "tests/pi_scan/ws9_orchestration.rs",
    ".pi/subagents/handoffs/pi-scan-budget-post-fix-blockers.md"
  ],
  "testsAddedOrUpdated": [
    "WS9 two-background-job observation/background-paid revocation tests",
    "production start-time policy projection tests",
    "exact decimal settings/editor/runtime boundary tests",
    "fresh-default malformed token startup parser test",
    "post-rename sync rollback and restart-consistency tests",
    "no-observation budget revalidation progress/event-loop tests"
  ],
  "commandsRun": [
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "exit 0"
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "exit 0, no warnings"
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "exit 0"
    },
    {
      "command": "cargo test --lib pi_scan_production::tests -- --test-threads=1",
      "result": "passed",
      "summary": "17 passed, 1 environment-dependent live test ignored"
    },
    {
      "command": "cargo test --test pi_scan ws9_orchestration -- --test-threads=1",
      "result": "passed",
      "summary": "27 passed"
    },
    {
      "command": "focused decimal/parser/config-editor/rollback/event-loop test filters",
      "result": "passed",
      "summary": "all focused filters passed"
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "not-run",
      "summary": "intentionally deferred to the parent per task instruction"
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "exit 0"
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "exit 0 with no staged files"
    }
  ],
  "validationOutput": [
    "Final Clippy completed cleanly across all targets/features.",
    "Final cargo check completed successfully.",
    "Production focused suite: 17 passed, 0 failed, 1 ignored.",
    "WS9 orchestration suite: 27 passed, 0 failed.",
    "Exact decimal, startup parser, Config Editor, rollback, and event-loop focused tests all passed.",
    "git diff --check returned no output."
  ],
  "residualRisks": [
    "Parent-owned complete serialized test suite was intentionally not run.",
    "Existing live Pi setup probe remains ignored because required external Pi/model configuration is unavailable."
  ],
  "noStagedFiles": true,
  "diffSummary": "Adds synchronized per-start unattended authorization, exact shared micro-USD validation, post-rename rollback, startup token diagnostics, and authoritative budget-revalidation UI projection with focused regressions.",
  "reviewFindings": [
    "no blockers in implemented PSB-001 through PSB-005 scope"
  ],
  "manualNotes": "Prior BUD behavior was preserved in the focused production and WS9 suites; full serialized testing remains with the parent."
}
```
