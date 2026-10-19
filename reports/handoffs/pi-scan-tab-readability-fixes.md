# Pi Scan readability accepted-finding fixes

## Workstream status

- **Status:** Complete; ready for the required fresh independent review gate.
- **Run role:** Sole active writer in the shared dirty worktree.
- **Base revision:** `f50d65dd3c94126fcfa751af19e1906a033990c1`
- **Resulting revision:** `f50d65dd3c94126fcfa751af19e1906a033990c1` (no commit created).
- **Scope:** Only accepted findings 1–4 were implemented. Deferred/rejected findings and scanner/state/settings-validation behavior were not changed.
- **Confidence:** 98/100. All focused and full automated checks pass; confidence is reduced only by the absence of manual pointer/custom-theme visual review.

## Changed files

Files changed by this fix workstream:

- `src/ui/pi_scan/mod.rs`
- `src/ui/pi_scan/wizard.rs`
- `src/ui/pi_scan/setup.rs`
- `src/ui/pi_scan/results.rs`
- `config/locales/en-US.yml`
- `config/locales/de-DE.yml`
- `config/locales/hu-HU.yml`
- `tests/pi_scan/ws4_tui.rs`
- `reports/handoffs/pi-scan-tab-readability-fixes.md`

The worktree was already dirty at the start. Pre-existing changes/untracked reports outside this fix boundary remain untouched, including the canonical plan and other Pi Scan renderer/report files.

## Implementation summary

### 1. Wrapped wizard control hit rows

- Wizard body lines are now built once and shared by rendering and hit-row calculation.
- Page-local `Control(index)` rectangles use the existing display-width word-wrapping counter, inner body width, wrapped row span, viewport height, and `body_scroll`.
- Every visible wrapped row of a control maps to that control only; scrolled-out rows receive no hit rectangle.
- Mouse regression coverage clicks Readiness, Route, and Pricing labels at 80 and 48 columns. It verifies focus/action or independent value mutation, and the narrow Pricing case must use non-zero body scroll.
- The pre-existing Optional Behavior hit-row expectation was updated from row 10 to the truthful wrapped row 11.

### 2. Truthful wizard notice semantics

- Known pending notices (`verifying`, `validating`) remain yellow with `⏳`.
- Known completed notices (`readiness_verified`, `validation_write_free`) are green with `✓`.
- Dry-run review and dynamic route-reselection notices are yellow warnings with `⚠`.
- Unknown persistent notices preserve the previous yellow pending fallback rather than introducing an unapproved state policy.
- Focused tests assert both marker and foreground color for all approved categories.

### 3. Localized Results completion wording

- Results rows no longer call the fixed-English `completion_wording()` renderer.
- UI wording is selected from typed `Coverage` plus validated finding count using locale keys for complete zero, incomplete zero, one, and multiple findings.
- English preserves the existing exact strings, including `finding(s)` for non-zero counts.
- German renders natural zero/one/multiple wording; tests assert all three cases.
- New Hungarian values are the English strings and each carries `# TODO: translate to Hungarian`.

### 4. Localized advanced-Setup validation issues

- The finite current `PiScanSettings::validation_issues()` strings are mapped at the Setup UI boundary to localized templates.
- Setting identifiers and numeric bounds remain explicit format arguments.
- Validation generation and validation logic are unchanged.
- Unknown/future messages return verbatim and have a focused unit test.
- A German invalid timeout render test proves the advanced Setup row contains localized actionable text.
- New Hungarian values are English with the required TODO marker.

## Red/green regression evidence

### Finding 1 — wrapped wizard clicks

- **RED:** `cargo test --test pi_scan ws4_tui::wrapped_wizard_labels_activate_their_own_controls -- --exact --nocapture --test-threads=1` → exit **101**. Failed at width 80: clicking `Pi executable: pi_` left focus on control 1 instead of control 0.
- **GREEN:** Same command → exit **0**, 1 passed.

### Finding 2 — notice markers/colors

- Initial exact-filter attempt: `cargo test wizard_notices_use_truthful_markers_and_colors --lib -- --exact --nocapture --test-threads=1` → exit **0**, but matched 0 tests and is not counted as red evidence.
- **RED:** `cargo test wizard_notices_use_truthful_markers_and_colors --lib -- --nocapture --test-threads=1` → exit **101**. `readiness_verified` rendered `⏳` instead of `✓`.
- **GREEN:** Same non-exact command → exit **0**, 1 passed.

### Finding 3 — German Results completion wording

- **RED:** `cargo test --test pi_scan ws4_tui::german_results_localize_completion_wording_by_finding_count -- --exact --nocapture --test-threads=1` → exit **101**. German rows contained fixed English completion strings.
- **GREEN:** Same command → exit **0**, 1 passed.

### Finding 4 — German advanced-Setup validation

- **RED:** `cargo test advanced_setup_localizes_german_validation_issue --lib -- --nocapture --test-threads=1` → exit **101**. The row contained `must be between 1 and 15`.
- **GREEN:** Same command → exit **0**, 1 passed.

## Validation commands and exit codes

- `cargo fmt --all` → exit **0** on every run (three runs; final run followed the last test adjustment).
- `cargo clippy --all-targets --all-features -- -D warnings` → exit **0** on both runs, including the final run.
- `cargo check` → exit **0** on both runs, including the final run.
- `cargo test ui::pi_scan --lib -- --test-threads=1` → first post-fix run exit **101** because the old Optional Behavior test expected unwrapped row 10; after updating that regression expectation, rerun exit **0**, **20 passed**.
- `cargo test --test pi_scan ws4_tui -- --test-threads=1` → exit **0**, **36 passed**.
- `cargo test -- --test-threads=1` → exit **0**. Notable results: library **1299 passed / 7 ignored**; Pi Scan integration **161 passed / 4 ignored**; all other integration and doctest groups passed with only their declared ignores.
- `git diff --check` → exit **0**.
- `git diff --cached --name-only` → exit **0**, no output; no staged files.

## Omissions and deviations

- **Omissions:** None from the requested automated validation. No manual live mouse-device session or custom-theme contrast assessment was performed.
- **Deviation:** The full serialized repository suite and `git diff --check` were run in addition to the explicitly requested focused checks, following repository policy.
- **Necessary test adjustment:** The existing hard-coded Optional Behavior last-control row changed from 10 to 11 because wrapped-row accounting now reflects the visible line.
- **No commit, stage, reset, revert, publish, dependency change, state/event change, or scanner/runtime change was performed.**

## Assumptions and unresolved decisions

- The finite validation issue set is the exact current output of `PiScanSettings::validation_issues()`; future or dynamic strings intentionally remain visible verbatim.
- Existing English non-zero completion wording (`N finding(s) in analyzed scope`) is preserved exactly to avoid an unapproved copy change.
- No unresolved product, architecture, interface, security, migration, dependency, locale-policy, or ownership decision was encountered.

## Residual risks and integration notes

- TestBackend coverage proves mouse dispatch and styling at 80/48 columns, including scrolled narrow Pricing controls, but does not replace a manual terminal/pointer check.
- The shared worktree still contains pre-existing dirty files unrelated to this bounded fix. Integrators should review this report’s file list rather than treating the whole worktree diff as fix-worker output.
- The canonical plan requires fresh provider-diverse read-only review after this fix; that remains the parent’s next integration gate.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented only accepted findings 1-4 within the authorized UI, locale, test, and handoff files; scanner/state/event/validation logic and deferred findings were untouched."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Focused red/green evidence is recorded for each finding; formatting, Clippy, check, focused UI/ws4, full serialized tests, and diff checks are recorded with exit codes."
    }
  ],
  "changedFiles": [
    "src/ui/pi_scan/mod.rs",
    "src/ui/pi_scan/wizard.rs",
    "src/ui/pi_scan/setup.rs",
    "src/ui/pi_scan/results.rs",
    "config/locales/en-US.yml",
    "config/locales/de-DE.yml",
    "config/locales/hu-HU.yml",
    "tests/pi_scan/ws4_tui.rs",
    "reports/handoffs/pi-scan-tab-readability-fixes.md"
  ],
  "testsAddedOrUpdated": [
    "src/ui/pi_scan/wizard.rs: wizard_notices_use_truthful_markers_and_colors and wrapped hit-row expectation",
    "src/ui/pi_scan/setup.rs: German render, finite issue mapping, and unknown verbatim tests",
    "tests/pi_scan/ws4_tui.rs: wrapped wizard mouse clicks at 80/48 columns and German zero/one/multiple Results wording",
    "tests/pi_scan/ws4_tui.rs: locale key completeness assertions"
  ],
  "commandsRun": [
    {
      "command": "cargo test --test pi_scan ws4_tui::wrapped_wizard_labels_activate_their_own_controls -- --exact --nocapture --test-threads=1",
      "result": "failed",
      "summary": "Intended red: exit 101; visible Readiness label selected the wrong control."
    },
    {
      "command": "cargo test wizard_notices_use_truthful_markers_and_colors --lib -- --nocapture --test-threads=1",
      "result": "failed",
      "summary": "Intended red: exit 101; completed notice still used the pending marker."
    },
    {
      "command": "cargo test --test pi_scan ws4_tui::german_results_localize_completion_wording_by_finding_count -- --exact --nocapture --test-threads=1",
      "result": "failed",
      "summary": "Intended red: exit 101; fixed English completion wording appeared in German."
    },
    {
      "command": "cargo test advanced_setup_localizes_german_validation_issue --lib -- --nocapture --test-threads=1",
      "result": "failed",
      "summary": "Intended red: exit 101; raw English timeout validation appeared in German Setup."
    },
    {
      "command": "cargo test --test pi_scan ws4_tui::wrapped_wizard_labels_activate_their_own_controls -- --exact --nocapture --test-threads=1",
      "result": "passed",
      "summary": "Green: exit 0; 1 passed at 80/48 columns with narrow body scroll."
    },
    {
      "command": "cargo test wizard_notices_use_truthful_markers_and_colors --lib -- --nocapture --test-threads=1",
      "result": "passed",
      "summary": "Green: exit 0; 1 passed."
    },
    {
      "command": "cargo test --test pi_scan ws4_tui::german_results_localize_completion_wording_by_finding_count -- --exact --nocapture --test-threads=1",
      "result": "passed",
      "summary": "Green: exit 0; 1 passed."
    },
    {
      "command": "cargo test advanced_setup_localizes_german_validation_issue --lib -- --nocapture --test-threads=1",
      "result": "passed",
      "summary": "Green: exit 0; 1 passed."
    },
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Exit 0 on all runs."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final exit 0 with warnings denied."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Final exit 0."
    },
    {
      "command": "cargo test ui::pi_scan --lib -- --test-threads=1",
      "result": "passed",
      "summary": "Final rerun exit 0; 20 passed."
    },
    {
      "command": "cargo test --test pi_scan ws4_tui -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 36 passed."
    },
    {
      "command": "cargo test -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; all suites passed with only declared ignores."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "Exit 0; no whitespace errors."
    }
  ],
  "validationOutput": [
    "Focused Pi Scan UI: 20 passed, 0 failed.",
    "Focused ws4_tui: 36 passed, 0 failed.",
    "Full library suite: 1299 passed, 0 failed, 7 ignored.",
    "Full Pi Scan integration suite: 161 passed, 0 failed, 4 ignored.",
    "Clippy with -D warnings and cargo check both completed with exit 0.",
    "git diff --check completed with exit 0."
  ],
  "residualRisks": [
    "No manual live pointer/custom-theme visual assessment; deterministic TestBackend mouse and color coverage passed.",
    "The shared worktree contains pre-existing unrelated dirty files that must remain preserved during integration."
  ],
  "noStagedFiles": true,
  "diffSummary": "Wrapped wizard hit seams now follow displayed/scrolled rows; notice semantics are truthful; Results completion and finite Setup validation messages are localized in English/German/Hungarian policy form; focused regressions cover all accepted findings.",
  "reviewFindings": [
    "no implementation blocker found by automated validation; fresh independent reviewer gate remains required"
  ],
  "manualNotes": "Base and resulting revision are both f50d65dd3c94126fcfa751af19e1906a033990c1 because this workstream did not commit."
}
```
