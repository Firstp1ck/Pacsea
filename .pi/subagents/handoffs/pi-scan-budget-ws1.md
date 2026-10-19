# WS1 Pi Scan budget/runtime handoff

## Identity and status

- Workstream: **WS1 runtime/domain/persistence**
- Run id: `pi-scan-budget-ws1`
- Status: **completed; ready for parent review and WS2 integration**
- Base revision: `5e55329ec411f83c0246c187f9ef6c56d8c42021`
- Resulting revision: unchanged (`5e55329ec411f83c0246c187f9ef6c56d8c42021`); no commit was created
- Confidence: **96/100**. Focused tests, Clippy, check, formatting, and diff checks pass. The parent-owned full repository test gate was not run.

## Changed files

- `src/state/pi_scan.rs`
  - Added scheduler-owned exceeded-dimension classification.
  - Made numeric zero Unlimited for starts, tokens, and cost.
  - Added exact checked Double, affected-only Unlimited, overflow rejection, residual revalidation, and typed adjustment results.
- `src/app/runtime/workers/pi_scan.rs`
  - Added typed `AdjustBudgets` request with request-owned typed applied/rejected acknowledgement channel.
  - Added inert-owner persistence-before-ack handling and dry-run preview without mutation.
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
  - Added owner-locked settings/runtime transaction, restart persistence, and best-effort rollback.
- `src/app/runtime/workers/pi_scan_production.rs`
  - Added production request handling and execution wake only after successful durable adjustment and cleared budget pause.
  - Removed budgets from the material consent binding so route/privacy/setup consent survives budget changes.
- `src/theme/config/patch.rs`
  - Added one-read/one-rename atomic multi-key settings transaction with dry-run behavior.
- `src/theme/types.rs`
  - Removed legacy starts/token maxima, added exact micro-USD persistence, and wired the atomic budget settings transaction.
- `src/theme/config/schema.rs`
  - Raised starts to the native `u32` range and changed tokens to an unbounded string editor value validated by the existing `u64` parser.
- `src/theme/config/skeletons.rs`
  - Documented numeric zero as Unlimited for all three independent budgets.
- `tests/pi_scan/ws3_runtime.rs`
  - Added zero/Unlimited, finite boundary, exact Double, affected-only Unlimited, overflow, residual pause, sticky pause, inert acknowledgement/persistence, and dry-run tests.
- `tests/pi_scan/ws9_orchestration.rs`
  - Revised the old zero-cost expectation and added settings/runtime/consent restart, rollback, and post-adjustment drain tests.

No files outside the approved WS1 write boundary were modified. The untracked plan file was present before WS1 and was not edited.

## Initial red-test evidence

1. `cargo test zero_budget_limits_are_unlimited_for_background_starts -- --test-threads=1`
   - Exit `101` as expected.
   - Failure: `eligible: Budget` at the new zero-limit regression, reproducing the old zero-cost/zero-limit pause behavior.
2. `cargo test double_adjusts_only_exceeded_limits_and_revalidates_pause -- --test-threads=1`
   - Exit `101` as expected.
   - Compile failures showed the missing adjustment contract: unresolved `PiScanBudgetAdjustment`, `PiScanBudgetAdjustmentError`, `PiScanBudgetDimension`, and missing `exceeded_budget_limits` / `adjust_exceeded_budgets` methods.

## Contract and integration notes

- `PiScanBudgetDimension::{Starts, Tokens, Cost}` is the authoritative exceeded set.
- `PiScanBudgetAdjustment::{Double, Unlimited}` is the action contract.
- `PiScanRuntimeState::exceeded_budget_limits(now_unix)` and `adjust_exceeded_budgets(...)` own classification and mutation. WS2 should not independently choose affected fields.
- `PiScanRequestMessage::AdjustBudgets` carries:
  - the selected adjustment;
  - deterministic `now_unix`;
  - an `mpsc::UnboundedSender<PiScanBudgetAdjustmentAcknowledgement>` owned by the caller.
- The acknowledgement is typed as `Applied { result, durable, dry_run, ... }` or `Rejected { reason, ... }`. This avoids changing the existing exhaustive UI result/progress enums inside WS1’s forbidden event-loop boundary.
- Because the request contains a sender, `PiScanRequestMessage` now derives `Debug` rather than `Clone/PartialEq/Eq`; current runtime code does not require those traits.
- Production holds the orchestrator owner lock across settings and orchestration persistence. It wakes execution only after success, when no budget dimension remains exceeded and paid/background gates remain active.
- User and Service pauses are untouched by budget revalidation. A wake is harmless under either sticky pause because the scheduler still blocks dispatch.
- The production consent binding no longer includes budgets; provider/model/background enablement/thinking/proxy and code/schema identities remain bound.
- Settings commit all three budget keys atomically. A subsequent orchestration persistence failure restores prior in-memory policy and attempts both settings and state rollback; rollback failures are included in the returned actionable error.
- Dry-run adjusts a clone for preview only and performs no settings, runtime, queue, accounting, execution, or durable mutation.

## Validation commands and outcomes

### Required/final checks

- `cargo fmt --all` — exit `0` (run twice; final run after lint fixes also exit `0`).
- `cargo clippy --all-targets --all-features -- -D warnings` — initial iteration exit `101` with five new lint findings; all were fixed. Final rerun exit `0`.
- `cargo check` — exit `0`.
- Final focused serialized test script:
  - `cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1` — exit `0`; 14 passed.
  - `cargo test --test pi_scan ws9_orchestration -- --test-threads=1` — exit `0`; 24 passed.
  - `cargo test theme::config::patch::tests:: -- --test-threads=1` — exit `0`; 10 passed.
  - Combined shell command exit `0`.
- `cargo test pi_scan_finite_budgets_are_not_limited_by_legacy_defaults -- --test-threads=1` — exit `0`; 1 passed.
- `cargo test production_consent_binding_excludes_mutable_budget_policy -- --test-threads=1` — exit `0`; 1 passed.
- `git diff --check` — exit `0` (both inspections).
- `git diff --cached --name-only` — exit `0`, empty output; no staged files.

### Focused iteration checks

- `cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1` — all post-implementation invocations exit `0`.
- `cargo test --test pi_scan ws9_orchestration -- --test-threads=1` — all post-implementation invocations exit `0`.
- `cargo test theme::config::patch::tests:: -- --test-threads=1` — all post-implementation invocations exit `0`.

## Omissions, deviations, and residual risks

- Omitted by instruction/ownership: full `cargo test -- --test-threads=1`; parent owns the full repository gate.
- No dependency, README/wiki, locale, UI, event-loop, plan, report, or PR-file edits.
- No direct end-to-end TUI acknowledgement polling exists yet; WS2 must retain/poll the request-owned acknowledgement receiver and project the authoritative result.
- Production wake behavior is implemented in the production owner and post-adjustment queue drain is exercised through the orchestrator/restart test. A full live production-channel test would require constructing production executable/network seams and was not added.
- Crash consistency across two separate durable files cannot be a single filesystem atomic operation. The chosen fail-closed ordering commits settings while holding the execution owner lock, then orchestration state, with rollback on reported failure. A process crash between those commits restarts from the already durable settings policy without an in-process premature wake.
- `plans/planned/pi-scan-budget-adjustment.md` remains untracked and untouched; it is not a WS1 change.

## Recommended parent next step

Review the typed request/acknowledgement seam and transaction ordering, then integrate WS1 before launching WS2. After WS2, run the full serialized repository test gate and independent reviewers.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented zero-as-Unlimited scheduling, checked affected-only adjustment, typed inert/production runtime handling, atomic settings/runtime persistence, consent preservation, rollback, gated wake, and dry-run behavior exclusively in approved WS1 files."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Captured two expected red failures, added focused WS3/WS9/config regressions, and recorded passing fmt, Clippy, check, focused serialized tests, diff check, and no-staged-files evidence."
    }
  ],
  "changedFiles": [
    "src/app/runtime/workers/pi_scan.rs",
    "src/app/runtime/workers/pi_scan_orchestrator.rs",
    "src/app/runtime/workers/pi_scan_production.rs",
    "src/state/pi_scan.rs",
    "src/theme/config/patch.rs",
    "src/theme/config/schema.rs",
    "src/theme/config/skeletons.rs",
    "src/theme/types.rs",
    "tests/pi_scan/ws3_runtime.rs",
    "tests/pi_scan/ws9_orchestration.rs"
  ],
  "testsAddedOrUpdated": [
    "tests/pi_scan/ws3_runtime.rs: zero/Unlimited, exact Double, affected-only Unlimited, overflow, residual/sticky pauses, inert durability acknowledgement, dry-run",
    "tests/pi_scan/ws9_orchestration.rs: zero-cost execution, atomic settings/runtime/consent restart, rollback, post-adjustment drain",
    "src/theme/config/patch.rs tests: atomic multi-key commit and dry-run/no-mutation",
    "src/theme/types.rs tests: native-range finite budgets and exact micro-USD formatting",
    "src/app/runtime/workers/pi_scan_production.rs tests: mutable budgets excluded from consent binding"
  ],
  "commandsRun": [
    {
      "command": "cargo test zero_budget_limits_are_unlimited_for_background_starts -- --test-threads=1",
      "result": "failed",
      "summary": "Expected red, exit 101: old scheduler returned Budget for zero/Unlimited."
    },
    {
      "command": "cargo test double_adjusts_only_exceeded_limits_and_revalidates_pause -- --test-threads=1",
      "result": "failed",
      "summary": "Expected red, exit 101: adjustment types and methods did not exist."
    },
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Final exit 0."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final exit 0 after fixing the initial lint iteration (exit 101)."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Exit 0."
    },
    {
      "command": "cargo test app::runtime::workers::pi_scan::ws3_runtime:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 14 passed."
    },
    {
      "command": "cargo test --test pi_scan ws9_orchestration -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 24 passed."
    },
    {
      "command": "cargo test theme::config::patch::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 10 passed."
    },
    {
      "command": "cargo test pi_scan_finite_budgets_are_not_limited_by_legacy_defaults -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 1 passed."
    },
    {
      "command": "cargo test production_consent_binding_excludes_mutable_budget_policy -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 1 passed."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "Exit 0."
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "not-run",
      "summary": "Parent-owned full repository gate."
    }
  ],
  "validationOutput": [
    "WS3: 14 passed, 0 failed",
    "WS9: 24 passed, 0 failed",
    "Config patch: 10 passed, 0 failed",
    "Clippy all targets/features: exit 0",
    "cargo check: exit 0",
    "git diff --check: exit 0",
    "git diff --cached --name-only: empty"
  ],
  "residualRisks": [
    "WS2 must consume the request-owned acknowledgement receiver and project the authoritative result without reclassifying fields.",
    "The full serialized repository test suite remains for the parent integration gate.",
    "Two-file crash consistency uses owner locking, durable ordering, and rollback rather than an impossible cross-file atomic rename."
  ],
  "noStagedFiles": true,
  "diffSummary": "Adds the authoritative zero/Unlimited budget classifier and checked adjustment contract, typed runtime request/acknowledgement, atomic settings/runtime transaction with consent preservation and rollback, production wake gating, and focused deterministic regressions.",
  "reviewFindings": [
    "no blockers found in focused implementation validation; independent reviewer gate remains required"
  ],
  "manualNotes": "No commit/stage/push/publish occurred. The pre-existing untracked plan file was not edited."
}
```
