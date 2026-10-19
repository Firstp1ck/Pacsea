# WS1 integration fix handoff

## Result

Implemented only the three accepted integration findings:

- **F1:** Added a bounded one-binding compatibility path for the exactly recomputed legacy budget-inclusive production consent binding. Both persisted binding fields must match the legacy value. Valid documents preserve runtime consent and every setup confirmation, then rewrite orchestration state and `consent-v1.json` to the current budget-independent binding. Partial or unrelated mismatches retain the existing fail-closed reset behavior.
- **F2:** Added `PiScanBudgetAdjustmentAcknowledgement::NoLongerBlocked`. An authoritative empty affected set now returns before budget pruning, settings persistence, orchestration persistence, inert dispatch, or production execution wake. Dry-run uses the same typed stale-modal result without mutation.
- **F3:** Added exact settings snapshot/content comparison immediately before the existing atomic temp-file/rename commit. Detected drift returns an actionable conflict and does not write the proposed budget content. No lock or generic config redesign was added.

Confidence: **97/100**. All requested focused suites and repository validation commands passed. The parent-owned full serialized test suite was intentionally omitted. Remaining uncertainty is limited to the documented optimistic pre-commit race window that cannot be removed without the explicitly forbidden cross-process locking redesign.

## Changed files

Files touched by this fix pass:

- `src/app/runtime/workers/pi_scan.rs`
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
- `src/app/runtime/workers/pi_scan_production.rs`
- `src/state/pi_scan.rs`
- `src/theme/config/patch.rs`
- `tests/pi_scan/ws3_runtime.rs`
- `tests/pi_scan/ws9_orchestration.rs`

The complete inherited WS1 unstaged diff still contains exactly these ten approved tracked files:

- `src/app/runtime/workers/pi_scan.rs`
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
- `src/app/runtime/workers/pi_scan_production.rs`
- `src/state/pi_scan.rs`
- `src/theme/config/patch.rs`
- `src/theme/config/schema.rs`
- `src/theme/config/skeletons.rs`
- `src/theme/types.rs`
- `tests/pi_scan/ws3_runtime.rs`
- `tests/pi_scan/ws9_orchestration.rs`

No UI, event-loop, locale, plan, report, PR, README/wiki, dependency, staged-state, or commit changes were made.

## Focused regressions

- `valid_legacy_budget_consent_migrates_and_rewrites_both_bindings`
- `unrelated_consent_mismatch_resets_instead_of_migrating`
- `no_hit_budget_adjustment_is_state_inert`
- `inert_no_hit_adjustment_is_write_free_and_wake_free`
- `production_no_hit_adjustment_is_write_free_and_wake_free`
- `no_hit_budget_adjustment_does_not_touch_settings_or_owner_state`
- `atomic_settings_transaction_rejects_snapshot_drift_without_writing`
- Updated the existing dry-run acknowledgement regression for the typed no-hit outcome.

## Commands and exit codes

Final focused suites:

1. `cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1` — exit `0`; 16 passed.
2. `cargo test pi_scan_production::tests:: -- --test-threads=1` — exit `0`; 13 passed, 1 pre-existing live test ignored.
3. `cargo test --test pi_scan ws9_orchestration -- --test-threads=1` — exit `0`; 25 passed.
4. `cargo test theme::config::patch::tests:: -- --test-threads=1` — exit `0`; 11 passed.

Final requested repository validation, run in order:

1. `cargo fmt --all` — exit `0`.
2. `cargo clippy --all-targets --all-features -- -D warnings` — exit `0`.
3. `cargo check` — exit `0`.
4. `git diff --check` — exit `0`.

Additional focused finding script:

- Seven individual finding filters covering valid/invalid migration, domain/inert/production/orchestrator no-hit behavior, and settings drift — every command exit `0`; combined script exit `0`.

Iteration failures retained for exactness:

- `cargo fmt --all && cargo test no_hit_budget_adjustment_is_state_inert -- --test-threads=1 && cargo test atomic_settings_transaction_rejects_snapshot_drift_without_writing -- --test-threads=1 && cargo test valid_legacy_budget_consent_migrates_and_rewrites_both_bindings -- --test-threads=1` — exit `101` on the first test compile because `Vec<String>` inference was missing in the new snapshot line builder; fixed before final validation.
- First `cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1` after adding `NoLongerBlocked` — exit `101`; 15 passed and the existing dry-run test failed because it still expected `Applied`. The regression was updated to the intentional typed no-hit acknowledgement; final WS3 suite passed 16/16.

## Omissions and residual risks

- Omitted by instruction: `cargo test -- --test-threads=1`; the parent owns the full serialized suite.
- The settings conflict defense is intentionally optimistic: it compares exact content immediately before atomic write, but does not provide cross-process locking. A writer racing after the check and before rename remains possible; eliminating that would exceed F3 scope.
- Legacy consent migrates only when both persisted binding fields exactly equal the legacy binding recomputed from the current material settings, including the current budget values. Other historical or manually diverged documents reset fail closed.
- The pre-existing untracked plan and `.pi/subagents/` directory remain untracked. No files are staged; `git diff --cached --name-only` was empty.

## Recommended next step

Parent integration owner should review the seven-file fix delta, then run the parent-owned full serialized test suite and required independent review gate.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented only F1 bounded legacy consent migration, F2 typed write-free/wake-free no-hit handling, and F3 deterministic pre-commit settings drift rejection within approved WS1 files."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Added focused regressions for every accepted finding and recorded passing WS3, production, WS9, config patch, fmt, Clippy, check, diff, scope, and no-staged-files evidence."
    }
  ],
  "changedFiles": [
    "src/app/runtime/workers/pi_scan.rs",
    "src/app/runtime/workers/pi_scan_orchestrator.rs",
    "src/app/runtime/workers/pi_scan_production.rs",
    "src/state/pi_scan.rs",
    "src/theme/config/patch.rs",
    "tests/pi_scan/ws3_runtime.rs",
    "tests/pi_scan/ws9_orchestration.rs"
  ],
  "testsAddedOrUpdated": [
    "src/app/runtime/workers/pi_scan_production.rs: valid legacy migration, unrelated mismatch reset, production no-hit no-write/no-wake",
    "tests/pi_scan/ws3_runtime.rs: domain and inert no-hit state/persistence/wake regressions plus dry-run typed acknowledgement update",
    "tests/pi_scan/ws9_orchestration.rs: no-hit settings and orchestration-state byte preservation",
    "src/theme/config/patch.rs: deterministic settings snapshot drift rejection without writing"
  ],
  "commandsRun": [
    {
      "command": "cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1",
      "result": "passed",
      "summary": "Final exit 0; 16 passed."
    },
    {
      "command": "cargo test pi_scan_production::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 13 passed and 1 pre-existing live test ignored."
    },
    {
      "command": "cargo test --test pi_scan ws9_orchestration -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 25 passed."
    },
    {
      "command": "cargo test theme::config::patch::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 11 passed."
    },
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Final exit 0."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Exit 0; clean all-target/all-feature lint."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Exit 0."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "Exit 0."
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "not-run",
      "summary": "Intentionally omitted because the parent owns the full serialized suite."
    }
  ],
  "validationOutput": [
    "WS3 final: 16 passed, 0 failed",
    "Production final: 13 passed, 0 failed, 1 ignored",
    "WS9 final: 25 passed, 0 failed",
    "Config patch final: 11 passed, 0 failed",
    "cargo fmt --all: exit 0",
    "cargo clippy --all-targets --all-features -- -D warnings: exit 0",
    "cargo check: exit 0",
    "git diff --check: exit 0",
    "git diff --cached --name-only: empty",
    "Forbidden tracked path check: empty"
  ],
  "residualRisks": [
    "Optimistic exact-content pre-commit checking cannot prevent a writer racing after the check without the explicitly out-of-scope cross-process locking redesign.",
    "The parent-owned full serialized test suite and independent review gate remain outstanding."
  ],
  "noStagedFiles": true,
  "diffSummary": "Seven-file fix delta adds exact legacy consent migration, typed no-hit acknowledgement with inert/production no-write/no-wake behavior, and exact pre-commit settings snapshot drift rejection; the complete inherited WS1 diff remains confined to ten approved tracked files.",
  "reviewFindings": [
    "no blockers in focused implementation validation; required independent reviewer gate remains with the parent"
  ],
  "manualNotes": "HEAD remains 5e55329ec411f83c0246c187f9ef6c56d8c42021. No commit, stage, push, dependency, UI/event-loop/locale, plan, report, PR, README/wiki, or full-suite action was performed."
}
```
