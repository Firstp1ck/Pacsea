# Pi Scan budget review fixes handoff

Implemented the parent-accepted BUD-001 through BUD-006 findings without the deferred Overview projection cleanup.

## Changes

- **BUD-001:** Centralized production unattended wake eligibility in `production_background_wake_eligible`, requiring observation enabled and successfully started, background execution enabled, paid execution consent, background paid consent, and non-dry-run operation. Startup, periodic observation, consent update, budget adjustment, and budget revalidation all use the shared wake seam. Added failed-startup-observation wake coverage.
- **BUD-002:** Connected config reloads now retain the live runtime owner's limits instead of replacing only the UI projection. Parsed settings remain visible, setup facts and consent remain preserved, and localized guidance tells users to restart or use the direct `b` flow. Matching direct-ack settings/runtime values do not produce divergence guidance. Updated connected runtime/UI divergence coverage.
- **BUD-003:** Added a production 30-second budget timer independent of observation consent. `RevalidateBudgets` now runs through the orchestrator owner, persists derived Budget pause transitions with in-memory rollback on persistence failure, and wakes only through the BUD-001 predicate. Added rolling-expiry/no-observation coverage.
- **BUD-004:** Unix atomic settings writes now sync the parent directory after rename and return an I/O error if the boundary fails. Added a deterministic helper-boundary failure regression without global failure hooks.
- **BUD-005:** Rolling starts/tokens/cost aggregation now retains sticky overflow evidence; any overflow exceeds a finite dimension even when the next reservation is zero. Added token and cost overflow regressions.
- **BUD-006:** Config Editor token-cap validation now accepts `0` through `u64::MAX` and rejects malformed/overflow input without restoring the removed 500000 ceiling. Added editor and parser boundary tests.

## Files changed by this fix pass

- `config/locales/de-DE.yml`
- `config/locales/en-US.yml`
- `config/locales/hu-HU.yml`
- `src/app/runtime/init.rs`
- `src/app/runtime/workers/pi_scan_orchestrator.rs`
- `src/app/runtime/workers/pi_scan_production.rs`
- `src/events/modals/config_editor.rs`
- `src/state/pi_scan.rs`
- `src/state/pi_scan_ui.rs`
- `src/theme/config/patch.rs`
- `src/theme/settings/parse_settings.rs`
- `tests/pi_scan/ws3_runtime.rs`
- `tests/pi_scan/ws4_tui.rs`

## Tests added or updated

- `aggregate_token_overflow_exceeds_finite_maximum`
- `aggregate_cost_overflow_exceeds_finite_maximum`
- `failed_startup_observation_blocks_adjustment_wake`
- `production_budget_revalidation_runs_without_observation_consent`
- `parent_directory_sync_surfaces_open_failure`
- `validate_pi_scan_token_cap_accepts_full_u64_range_only`
- `parse_settings_pi_scan_token_cap_handles_native_u64_bounds`
- `budget_only_pi_scan_reload_preserves_connected_runtime_limits`

## Validation

Final successful commands:

- `cargo fmt --all` — exit 0.
- `cargo clippy --all-targets --all-features -- -D warnings` — exit 0.
- `cargo check` — exit 0.
- `cargo test budget_ --lib -- --test-threads=1` — exit 0; 14 passed.
- `cargo test pi_scan_token --lib -- --test-threads=1` — exit 0; 2 passed.
- `cargo test parent_directory_sync_surfaces_open_failure --lib -- --test-threads=1` — exit 0; 1 passed.
- `cargo test budget_ --test pi_scan -- --test-threads=1` — exit 0; 11 passed.
- Individual focused tests for aggregate overflow, failed observation wake, no-observation revalidation, editor validation, parser bounds, connected reload divergence, and directory sync all passed.
- `git diff --check` — exit 0.
- `git diff --cached --name-only` — exit 0 with no output; no staged files.

Resolved intermediate validation failures:

- An initial `cargo test` invocation supplied two filters and exited 1 due Cargo CLI syntax; rerun with valid filters.
- The first focused compile exited 101 because the new test helper referenced `fs` without qualification; changed it to `std::fs` and reran successfully.
- The first Clippy run exited 101 on `semicolon_if_nothing_returned`; added the semicolon and reran successfully.

## Omissions and residual risks

- Per task ownership, the final full serialized `cargo test -- --test-threads=1` suite was not run; the parent owns it.
- Focused regressions were added during the fixes rather than committed/rerun as a separate red state. The old control flow is directly represented by the assertions, but no preserved pre-fix failing output exists.
- The Unix directory-sync regression directly exercises the extracted failing sync boundary using a missing parent. It does not inject a failure after a real rename, intentionally avoiding unsafe process-global hooks.
- The failed-observation adjustment regression exercises the centralized wake seam used by the adjustment handler; it avoids an actual durable adjustment because that path resolves the process-global settings location.
- Existing unrelated unstaged integrated feature changes and untracked plan/review artifacts remain untouched. No staged files exist.

Confidence: **94/100**. The accepted paths are implemented and focused/required checks pass; confidence is reduced only by the parent-owned full suite and the intentionally bounded failure-injection tests noted above.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented only accepted BUD-001 through BUD-006 across the allowed runtime, state, config, focused test, and locale paths; deferred Overview cleanup was not changed by this fix pass."
    }
  ],
  "changedFiles": [
    "config/locales/de-DE.yml",
    "config/locales/en-US.yml",
    "config/locales/hu-HU.yml",
    "src/app/runtime/init.rs",
    "src/app/runtime/workers/pi_scan_orchestrator.rs",
    "src/app/runtime/workers/pi_scan_production.rs",
    "src/events/modals/config_editor.rs",
    "src/state/pi_scan.rs",
    "src/state/pi_scan_ui.rs",
    "src/theme/config/patch.rs",
    "src/theme/settings/parse_settings.rs",
    "tests/pi_scan/ws3_runtime.rs",
    "tests/pi_scan/ws4_tui.rs"
  ],
  "testsAddedOrUpdated": [
    "tests/pi_scan/ws3_runtime.rs: aggregate token and cost overflow regressions",
    "src/app/runtime/workers/pi_scan_production.rs: failed observation wake and no-observation rolling-expiry regressions",
    "src/theme/config/patch.rs: parent-directory sync failure-boundary regression",
    "src/events/modals/config_editor.rs: full-u64 token-cap validation regression",
    "src/theme/settings/parse_settings.rs: token-cap parser boundary regression",
    "tests/pi_scan/ws4_tui.rs: connected owner/UI budget reload divergence regression"
  ],
  "commandsRun": [
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Formatting completed successfully."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final rerun completed without warnings."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Development profile compiled successfully."
    },
    {
      "command": "cargo test budget_ --lib -- --test-threads=1",
      "result": "passed",
      "summary": "14 focused library budget tests passed."
    },
    {
      "command": "cargo test pi_scan_token --lib -- --test-threads=1",
      "result": "passed",
      "summary": "2 token-cap editor/parser tests passed."
    },
    {
      "command": "cargo test parent_directory_sync_surfaces_open_failure --lib -- --test-threads=1",
      "result": "passed",
      "summary": "Directory-sync boundary regression passed."
    },
    {
      "command": "cargo test budget_ --test pi_scan -- --test-threads=1",
      "result": "passed",
      "summary": "11 focused Pi Scan integration budget tests passed."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "No whitespace errors."
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "not-run",
      "summary": "Parent owns the final full serialized suite per task instructions."
    }
  ],
  "validationOutput": [
    "Final Clippy: Finished dev profile successfully with -D warnings.",
    "Final cargo check: Finished dev profile successfully.",
    "Focused library tests: 14 budget tests, 2 token tests, and 1 directory-sync test passed.",
    "Focused integration tests: 11 budget tests passed.",
    "git diff --check produced no output.",
    "git diff --cached --name-only produced no output."
  ],
  "residualRisks": [
    "Parent-owned full serialized test suite remains to run.",
    "Directory-sync failure coverage exercises the extracted boundary rather than injecting a post-rename failure.",
    "Failed-observation adjustment coverage exercises the centralized adjustment wake seam rather than a process-global settings write."
  ],
  "noStagedFiles": true,
  "diffSummary": "Centralized production wake gates, added independent durable budget revalidation, preserved connected owner authority across external reloads with localized guidance, synced settings parent directories, retained aggregate overflow truth, and hardened token-cap editor validation with focused regressions.",
  "reviewFindings": [
    "no blockers in focused implementation validation"
  ],
  "manualNotes": "Existing unrelated unstaged integrated feature files and untracked plan/review artifacts were preserved. Initial command-syntax, test-import, and Clippy failures were corrected and all final reruns passed."
}
```
