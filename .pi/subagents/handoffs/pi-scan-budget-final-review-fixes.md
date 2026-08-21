# Pi Scan budget final review fixes

Implemented the three parent-accepted findings without editing `dev/PR`, plans, README/wiki, dependencies, commits, or staged state.

## Changes

### PSB-001 — owner-lock authorization linearization

- Added `PiScanUnattendedAuthorization`, a fail-closed shared `RwLock<bool>` policy cell.
- Production now publishes the complete session-level unattended predicate into that cell instead of capturing a plain Boolean before execution.
- `PiScanSequentialRunner` acquires the scheduler owner lock first, then reads and retains the unattended authorization guard through `start_next`, durable start persistence, and active registration. It releases the policy guard before external execution.
- Revocation publication acquires only the policy write lock and releases it before owner-lock persistence, preserving the owner→policy start order without holding policy→owner simultaneously.
- Foreground work bypasses the unattended policy guard.
- Added deterministic two-job ordering coverage: a second background run is issued while the first owns the scheduler, authorization is revoked before releasing the first, and the second remains queued. Separate observation-revocation and background-paid-revocation tests cover the required cases.
- Updated the production projection test to prove each accepted revocation field publishes a fail-closed decision.

### FINAL-001 — guided setup native/exact bounds

- Starts now adjust through `u32::MAX`; tokens adjust through `u64::MAX`.
- Cost adjustment parses exact micro-USD, preserves fractional precision, uses checked one-dollar steps, leaves malformed values unchanged, and remains unchanged when a step would cross either native boundary.
- Reused the exact parser-compatible micro-USD formatter already owned by theme settings.
- Added regressions above starts `5`, tokens `500000`, and cost `$10000`, plus native upper boundaries and the below-one-dollar lower-step boundary.
- Numeric zero remains the existing Unlimited representation.

### FINAL-002 — current localized validation diagnostics

- Replaced the obsolete cost matcher with exact current producer mappings for:
  - token integer syntax/native overflow;
  - cost syntax or precision above six fractional digits;
  - exact micro-USD native overflow.
- Added matching English, German, and Hungarian Setup locale keys.
- Updated locale inventory coverage.
- Added a unit regression that feeds actual `PiScanSettings::validation_issues()` output for malformed/overflow token state and malformed, over-precision, and overflowing cost values through all three locales.

## Changed files

- `config/locales/de-DE.yml`
- `config/locales/en-US.yml`
- `config/locales/hu-HU.yml`
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
- `src/app/runtime/workers/pi_scan_production.rs`
- `src/state/pi_scan_setup.rs`
- `src/theme/mod.rs`
- `src/theme/types.rs`
- `src/ui/pi_scan/setup.rs`
- `tests/pi_scan/setup_wizard.rs`
- `tests/pi_scan/ws4_tui.rs`
- `tests/pi_scan/ws9_orchestration.rs`

The worktree already contained the broader integrated Pi Scan feature and untracked review/plan artifacts. Those pre-existing files were not staged or reverted. `dev/PR` was not edited.

## Tests added or updated

- `setup_wizard::wizard_budget_adjustments_do_not_collapse_large_valid_values`
- `setup_wizard::wizard_budget_adjustments_are_checked_at_native_boundaries`
- `ws9_orchestration::observation_revocation_while_first_background_job_is_active_blocks_second`
- `ws9_orchestration::background_paid_revocation_while_first_background_job_is_active_blocks_second`
- `ui::pi_scan::setup::tests::current_budget_validation_issues_localize_in_every_setup_locale`
- Updated `pi_scan_production::tests::production_policy_projection_fails_closed_after_each_revocation`.
- Updated WS4 shipped-locale key inventory for the three current diagnostics.

## Validation and command evidence

1. Test-first compile before implementation:
   - `cargo test wizard_budget_adjustments_do_not_collapse_large_valid_values -- --test-threads=1`
   - Exit 101 as expected because the new authorization type/API did not yet exist.
2. Focused individual regressions after implementation:
   - `cargo test wizard_budget_adjustments_are_checked_at_native_boundaries -- --test-threads=1` — exit 0.
   - `cargo test current_budget_validation_issues_localize_in_every_setup_locale -- --test-threads=1` — exit 0.
   - `cargo test observation_revocation_while_first_background_job_is_active_blocks_second -- --test-threads=1` — exit 0.
   - `cargo test background_paid_revocation_while_first_background_job_is_active_blocks_second -- --test-threads=1` — exit 0.
3. Focused Pi Scan integration suite:
   - `cargo test --test pi_scan -- --test-threads=1` — exit 0; 172 passed, 0 failed, 4 ignored.
4. Focused final unit/integration filters:
   - `cargo test production_policy_projection_fails_closed_after_each_revocation -- --test-threads=1` — exit 0; target passed.
   - `cargo test current_budget_validation_issues_localize_in_every_setup_locale -- --test-threads=1` — exit 0; target passed.
   - `cargo test wizard_budget_adjustments -- --test-threads=1` — exit 0; 2 target tests passed.
5. Formatting:
   - `cargo fmt --all` — exit 0.
6. Linting:
   - Initial `cargo clippy --all-targets --all-features -- -D warnings` found two local lint issues (`redundant_pub_crate`, `field_reassign_with_default`) and exited 101.
   - After correction, `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings` — exit 0.
7. Compilation and diff integrity:
   - `cargo check && git diff --check && test -z "$(git diff --cached --name-only)"` — exit 0.
   - Final standalone `git diff --check` — exit 0.
   - Final `git diff --cached --name-only` — empty; no staged files.

## Omissions and residual risks

- The full serialized repository suite was intentionally not run because the parent owns that final gate. The focused Pi Scan integration suite and required format/clippy/check/diff checks passed.
- Four pre-existing live/external Pi tests remained ignored in the focused integration run.
- No reviewer disposition or PR record update was made; FINAL-003 remains parent-owned as instructed.
- Optional `NoLongerBlocked` polish and formatter work beyond reusing the existing exact formatter were not expanded.
- Confidence: 96/100. The changed behavior is directly covered by deterministic focused tests and clean compiler/lint checks; confidence is below 100 only because the parent-reserved full serialized suite and final independent review remain outstanding.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented only PSB-001, FINAL-001, and FINAL-002 across the permitted Pi Scan scheduler/setup/locale/test files; dev/PR, plans, dependencies, README/wiki, commits, and staged state were not changed."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Focused Pi Scan suite passed 172 tests with 0 failures, targeted regressions passed, fmt/clippy/check/diff validation passed, and the staged-file list is empty."
    }
  ],
  "changedFiles": [
    "config/locales/de-DE.yml",
    "config/locales/en-US.yml",
    "config/locales/hu-HU.yml",
    "src/app/runtime/workers/pi_scan_orchestrator.rs",
    "src/app/runtime/workers/pi_scan_production.rs",
    "src/state/pi_scan_setup.rs",
    "src/theme/mod.rs",
    "src/theme/types.rs",
    "src/ui/pi_scan/setup.rs",
    "tests/pi_scan/setup_wizard.rs",
    "tests/pi_scan/ws4_tui.rs",
    "tests/pi_scan/ws9_orchestration.rs"
  ],
  "testsAddedOrUpdated": [
    "setup_wizard::wizard_budget_adjustments_do_not_collapse_large_valid_values",
    "setup_wizard::wizard_budget_adjustments_are_checked_at_native_boundaries",
    "ws9_orchestration::observation_revocation_while_first_background_job_is_active_blocks_second",
    "ws9_orchestration::background_paid_revocation_while_first_background_job_is_active_blocks_second",
    "ui::pi_scan::setup::tests::current_budget_validation_issues_localize_in_every_setup_locale",
    "pi_scan_production::tests::production_policy_projection_fails_closed_after_each_revocation",
    "WS4 shipped-locale validation-key inventory"
  ],
  "commandsRun": [
    {
      "command": "cargo test wizard_budget_adjustments_do_not_collapse_large_valid_values -- --test-threads=1 (before implementation)",
      "result": "failed",
      "summary": "Expected test-first failure: new synchronized authorization API was not yet implemented; exit 101."
    },
    {
      "command": "cargo test --test pi_scan -- --test-threads=1",
      "result": "passed",
      "summary": "172 passed, 0 failed, 4 ignored."
    },
    {
      "command": "cargo test production_policy_projection_fails_closed_after_each_revocation -- --test-threads=1",
      "result": "passed",
      "summary": "Production observation/background-paid policy projection regression passed."
    },
    {
      "command": "cargo test current_budget_validation_issues_localize_in_every_setup_locale -- --test-threads=1",
      "result": "passed",
      "summary": "Current validation producer output localized through English, German, and Hungarian."
    },
    {
      "command": "cargo test wizard_budget_adjustments -- --test-threads=1",
      "result": "passed",
      "summary": "Both guided setup former-maximum/native-boundary regressions passed."
    },
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Formatting completed successfully."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final lint run clean after correcting two iterative lint findings."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Development profile check completed successfully."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "No whitespace errors."
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "Empty output; no staged files."
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "not-run",
      "summary": "Intentionally deferred to the parent-owned final serialized suite gate."
    }
  ],
  "validationOutput": [
    "Focused Pi Scan integration: 172 passed, 0 failed, 4 ignored.",
    "Final clippy: Finished dev profile successfully with -D warnings.",
    "cargo check: Finished dev profile successfully.",
    "git diff --check: exit 0.",
    "git diff --cached --name-only: empty."
  ],
  "residualRisks": [
    "Parent-owned full serialized cargo test suite and final reviewer disposition remain outstanding.",
    "Four external/live focused tests were ignored by their existing annotations."
  ],
  "noStagedFiles": true,
  "diffSummary": "Linearized complete unattended authorization under the scheduler owner lock, removed guided budget clamps using native/exact checked arithmetic, and localized current token/cost diagnostics in all three Setup locales with focused regressions.",
  "reviewFindings": [
    "no blockers found in focused implementation validation",
    "FINAL-003 PR update intentionally left parent-owned"
  ],
  "manualNotes": "The worktree contains broader pre-existing unstaged integrated feature changes and untracked review/plan artifacts. This worker neither staged nor reverted them. Confidence: 96/100."
}
```
