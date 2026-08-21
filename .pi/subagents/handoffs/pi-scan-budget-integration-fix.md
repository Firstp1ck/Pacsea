# Pi Scan budget integration fix handoff

Implemented the accepted F4–F6 consistency fixes without changing WS1 scheduler/runtime/persistence files.

## Changes

- **F4:** Applied budget acknowledgements now project authoritative starts, token, and cost limits into both `runtime.budget_limits` and `PiScanSettings`. Cost uses exact parser-compatible decimal formatting (including six-digit micro-USD precision). Dry-run and `NoLongerBlocked` acknowledgements remain settings/runtime mutation-free. Queue-intent regression coverage confirms later snapshots use the synchronized token/cost values.
- **F5:** `PiScanWorkspaceState::apply_settings` now classifies changes after excluding the three mutable budget fields. Budget-only reloads preserve setup facts, readiness, the setup draft, and every independent consent/confirmation while synchronizing runtime limits from valid parsed settings. Provider/material changes retain the prior reset and wizard-close behavior. Invalid decimal cost is not converted into permissive Unlimited runtime policy.
- **F6:** Advanced Setup and guided wizard optional/review projections render numeric-zero budgets as localized Unlimited. English, German, and Hungarian copy now explains zero semantics, removes legacy starts/token maxima claims, and aligns Config Editor summaries. `config/settings.conf` now carries the shipped zero-as-Unlimited comment.

## Changed files

- `src/state/pi_scan_ui.rs`
- `src/app/runtime/event_loop.rs`
- `src/ui/pi_scan/setup.rs`
- `src/ui/pi_scan/wizard.rs`
- `config/locales/en-US.yml`
- `config/locales/de-DE.yml`
- `config/locales/hu-HU.yml`
- `config/settings.conf`
- `tests/pi_scan/ws4_tui.rs`

No files outside the requested write boundary were edited by this fix pass. The checkout still contains the integration owner's pre-existing WS1/WS2 unstaged changes.

## Focused regressions

Added or strengthened coverage for:

- Applied acknowledgement synchronization into settings and subsequent queue-intent snapshots.
- Dry-run and `NoLongerBlocked` settings/runtime non-mutation.
- Budget-only reload preservation of setup facts, readiness, wizard state, and all confirmations/consents, plus exact `u64::MAX` micro-USD parsing.
- Provider/material reload reset behavior.
- Advanced Setup zero-as-Unlimited rendering.
- Guided wizard optional/review zero-as-Unlimited rendering.
- Three-locale removal of legacy maxima claims, Config Editor summary consistency, and shipped settings comment presence.

The new F4, F5, and F6 regressions were run before implementation and failed with exit code 101 on the old behavior. They pass after the fixes.

## Commands and results

### Red regressions before implementation

Each logical test command below exited **101** as expected:

- `cargo test budget_applied_projects_limits_residual_pause_and_sticky_pauses -- --test-threads=1` — settings stayed stale (`5` instead of applied `0`).
- `cargo test budget_only_pi_scan_reload_preserves_setup_and_synchronizes_runtime_limits -- --test-threads=1` — setup facts were reset.
- `cargo test advanced_setup_renders_zero_budgets_as_unlimited -- --test-threads=1` — rendered raw `starts 0/5` and `tokens 0/500000`.
- `cargo test guided_setup_renders_zero_budgets_as_unlimited_without_legacy_maxima -- --test-threads=1` — rendered raw zero and legacy maxima copy.
- `cargo test budget_copy_is_localized_without_legacy_maxima -- --test-threads=1` — locale copy retained legacy maxima.

### Focused pass after implementation

The seven-test filter loop exited **0**; each command passed:

- `cargo test budget_applied_projects_limits_residual_pause_and_sticky_pauses -- --test-threads=1`
- `cargo test budget_acknowledgements_handle_preview_no_longer_blocked_and_rejection -- --test-threads=1`
- `cargo test budget_only_pi_scan_reload_preserves_setup_and_synchronizes_runtime_limits -- --test-threads=1`
- `cargo test material_pi_scan_reload_closes_wizard_with_typed_notice -- --test-threads=1`
- `cargo test advanced_setup_renders_zero_budgets_as_unlimited -- --test-threads=1`
- `cargo test guided_setup_renders_zero_budgets_as_unlimited_without_legacy_maxima -- --test-threads=1`
- `cargo test budget_copy_is_localized_without_legacy_maxima -- --test-threads=1`

### Final validation

- `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings` — exit **0**; final run clean.
- `cargo check` — exit **0**.
- `cargo test --test pi_scan -- --test-threads=1 && cargo test 'app::runtime::event_loop::tests::budget_' -- --test-threads=1 && cargo test 'ui::pi_scan::setup::tests' -- --test-threads=1` — exit **0**:
  - Pi Scan integration target: **168 passed, 0 failed, 4 ignored** (declared external/Wave 0 tests).
  - Event-loop budget projection: **3 passed, 0 failed**.
  - Advanced Setup unit tests: **6 passed, 0 failed**.
- `git diff --check` — exit **0**.
- `git diff --cached --name-only` — exit **0**, no output; no staged files.

Two intermediate Clippy attempts exited 101 while tightening the new code (`assigning_clones`, then `single_char_pattern`); both findings were fixed before the clean final run.

## Omissions and residual risks

- The full `cargo test -- --test-threads=1` suite was not run because the task explicitly assigns the full serialized suite to the parent integration owner.
- Four Pi Scan tests remain intentionally ignored because they require installed/configured Pi or explicit Wave 0 benchmarking; no new ignored tests were added.
- No dependencies, plans, reports, PR files, README/wiki files, staged state, commits, or WS1 scheduler/runtime/persistence files were changed.
- Confidence: **97/100**. The focused and complete Pi Scan integration target passed; remaining uncertainty is limited to the parent-owned full repository suite and the four pre-existing environment-dependent ignored tests.

## Acceptance

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented only accepted F4-F6 changes in the nine allowed source/config/test files: authoritative settings projection, budget-only reload preservation/runtime synchronization, and truthful Unlimited UI/locale/config copy."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Captured red-before-fix regressions, clean format/lint/check, serialized Pi Scan results (168 passed, 4 declared ignored), focused unit results, diff check, omissions, and empty staged-file output."
    }
  ],
  "changedFiles": [
    "src/state/pi_scan_ui.rs",
    "src/app/runtime/event_loop.rs",
    "src/ui/pi_scan/setup.rs",
    "src/ui/pi_scan/wizard.rs",
    "config/locales/en-US.yml",
    "config/locales/de-DE.yml",
    "config/locales/hu-HU.yml",
    "config/settings.conf",
    "tests/pi_scan/ws4_tui.rs"
  ],
  "testsAddedOrUpdated": [
    "src/app/runtime/event_loop.rs: budget_applied_projects_limits_residual_pause_and_sticky_pauses",
    "src/app/runtime/event_loop.rs: budget_acknowledgements_handle_preview_no_longer_blocked_and_rejection",
    "src/ui/pi_scan/setup.rs: advanced_setup_renders_zero_budgets_as_unlimited",
    "tests/pi_scan/ws4_tui.rs: budget_only_pi_scan_reload_preserves_setup_and_synchronizes_runtime_limits",
    "tests/pi_scan/ws4_tui.rs: material_pi_scan_reload_closes_wizard_with_typed_notice",
    "tests/pi_scan/ws4_tui.rs: guided_setup_renders_zero_budgets_as_unlimited_without_legacy_maxima",
    "tests/pi_scan/ws4_tui.rs: budget_copy_is_localized_without_legacy_maxima"
  ],
  "commandsRun": [
    {
      "command": "cargo test <five new F4-F6 regression filters individually> -- --test-threads=1 (before implementation)",
      "result": "failed",
      "summary": "All five exited 101 and reproduced stale settings, consent reset, raw-zero rendering, and legacy maxima copy."
    },
    {
      "command": "cargo test <seven focused F4-F6 filters individually> -- --test-threads=1 (after implementation)",
      "result": "passed",
      "summary": "All seven focused regressions exited 0."
    },
    {
      "command": "cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final run exited 0 with no warnings."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Exited 0."
    },
    {
      "command": "cargo test --test pi_scan -- --test-threads=1 && cargo test 'app::runtime::event_loop::tests::budget_' -- --test-threads=1 && cargo test 'ui::pi_scan::setup::tests' -- --test-threads=1",
      "result": "passed",
      "summary": "Pi Scan: 168 passed/4 declared ignored; event-loop budget: 3 passed; Setup: 6 passed."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "Exited 0 with no whitespace errors."
    },
    {
      "command": "git diff --cached --name-only",
      "result": "passed",
      "summary": "Exited 0 with empty output; no staged files."
    }
  ],
  "validationOutput": [
    "Final Clippy: Finished dev profile; zero warnings under -D warnings.",
    "Final cargo check: Finished dev profile successfully.",
    "Serialized tests/pi_scan: 168 passed, 0 failed, 4 ignored.",
    "Focused event-loop budget tests: 3 passed, 0 failed.",
    "Focused Advanced Setup tests: 6 passed, 0 failed.",
    "git diff --check: exit 0.",
    "git diff --cached --name-only: empty."
  ],
  "residualRisks": [
    "Parent-owned full cargo test -- --test-threads=1 was not run in this pass.",
    "Four pre-existing environment-dependent Pi Scan tests remain intentionally ignored."
  ],
  "noStagedFiles": true,
  "diffSummary": "F4 synchronizes Applied acknowledgement limits into runtime/settings/queue projections; F5 preserves consent for budget-only reloads while resetting material changes; F6 renders and documents zero budgets as Unlimited across Setup, wizard, locales, Config Editor, and shipped settings.",
  "reviewFindings": [
    "no blockers",
    "no scope widening detected",
    "intermediate Clippy findings were resolved before final validation"
  ],
  "manualNotes": "The checkout contains pre-existing unstaged WS1/WS2 integration changes outside this pass. Full serialized repository suite remains assigned to the parent integration owner."
}
```
