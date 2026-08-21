# WS2 Pi Scan budget-adjustment TUI handoff

## Result

Implemented the approved in-place budget-adjustment UX against the fixed WS1 typed request/acknowledgement contract.

- Plain `b` on Overview or Progress opens only when queued background work is currently Budget-paused and the scheduler projection has an exceeded finite limit.
- The focused overlay defaults to Double, supports Tab/BackTab/arrows/hjkl focus, Enter confirmation, and Esc cancellation before submission.
- Progress `r` remains Retry.
- Confirmation dispatches only `PiScanRequestMessage::AdjustBudgets`, retains the request-owned typed acknowledgement receiver, and polls it from the serialized event-loop tick.
- `NoLongerBlocked` closes without projecting any write; normal `Applied` projects authoritative runtime limits and only the Budget pause; dry-run `Applied` is preview-only; `Rejected` remains visible and retryable.
- Applied Double never claims guaranteed resume: residual exceeded dimensions remain named with a Budget pause.
- Overview now uses conservative rolling effective usage and runtime-owned limits, rendering numeric zero as localized Unlimited.
- Progress exceeded-limit copy uses the scheduler-owned runtime classifier rather than raw settings or duplicated budget arithmetic.
- The overlay lists affected limits with old-to-proposed values, exact checked-overflow presentation, and an explicit Unlimited spending warning at narrow dimensions.
- English, German, and Hungarian replace the incorrect Setup+r guidance, update footers/help, and include complete new flow copy.
- No guided setup logic, scheduler/domain/persistence/consent behavior, dependencies, package acquisition, result schemas, README/wiki, plan, report, PR file, staged state, commit, push, or publication was changed.

Confidence: **96/100**. All requested focused suites, formatting, all-target/all-feature Clippy, compile check, and diff/staged checks pass. The parent-owned full serialized suite and independent review gate remain outstanding.

## Changed files

- `src/state/pi_scan_ui.rs`
  - Added deterministic dialog selection/status, eligibility, submission/cancel transitions, pending typed action, and request-owned acknowledgement receiver storage.
- `src/events/pi_scan/keys.rs`
  - Added eligible `b`, focused selection/Enter/Esc behavior, submitting protection, and regressions proving `r` remains Retry.
- `src/app/runtime/event_loop.rs`
  - Added typed adjustment dispatch, receiver polling, and authoritative `NoLongerBlocked`/`Applied`/`Rejected`/dry-run projection without direct file writes.
- `src/ui/pi_scan/mod.rs`
  - Added the responsive focused overlay, focus/status rendering, affected old-to-proposed values, Unlimited warning, and shared truthful limit formatting.
- `src/ui/pi_scan/overview.rs`
  - Switched budget presentation to conservative rolling effective usage and runtime-owned finite/Unlimited limits.
- `src/ui/pi_scan/progress.rs`
  - Replaced duplicated raw-settings classification and Setup+r guidance with scheduler-owned exceeded dimensions and direct `b` copy.
- `config/locales/en-US.yml`
- `config/locales/de-DE.yml`
- `config/locales/hu-HU.yml`
  - Added localized dialog, warnings, outcomes, Unlimited, footer/help copy, and corrected recovery guidance.
- `tests/pi_scan/ws4_tui.rs`
  - Added focused overlay, Unlimited, warning, narrow, locale, footer/help, and runtime-limit regressions; aligned the integrated raised-token-limit expectation with WS1.

All WS2 edits are within the approved write boundary. The inherited WS1 tracked changes and pre-existing untracked plan were not edited by WS2.

## Initial red-test evidence

Each regression was added before implementation and failed on the old behavior:

1. `cargo test budget_key_opens_direct_choice_without_guided_setup -- --test-threads=1`
   - Exit `101` as expected.
   - Failure at `src/events/pi_scan/keys.rs`: `handle_key(...)` returned false for eligible plain `b`.
2. `cargo test budget_pause_guidance_uses_direct_budget_key -- --test-threads=1`
   - Exit `101` as expected.
   - Rendered text still said: `open Setup (1) and press r to increase the affected limit` and did not contain direct `press b` guidance.
3. `cargo test overview_renders_zero_runtime_limits_as_unlimited -- --test-threads=1`
   - Exit `101` as expected.
   - Overview rendered settings-derived `5/h`, `500,000`, and raw `$0.00 USD` rather than runtime-owned Unlimited values.

## Tests added or updated

- `src/events/pi_scan/keys.rs`
  - Eligible Overview `b` opens without guided setup.
  - Default Double, focus movement to Unlimited, Esc cancel, Enter submit, duplicate Enter, and submitting Esc behavior.
  - Ineligible `b` ignored and Progress `r` remains `PiScanUiAction::Retry`.
- `src/app/runtime/event_loop.rs`
  - Typed dispatch owns/polls the exact request acknowledgement receiver.
  - Applied limits, cleared/residual Budget pause, and untouched User/Service pauses.
  - Dry-run preview no mutation, `NoLongerBlocked` no mutation/close, and actionable retained Rejected state.
- `src/ui/pi_scan/overview.rs`
  - Runtime numeric zero renders Unlimited for starts, tokens, and cost.
- `src/ui/pi_scan/progress.rs`
  - Direct `b` guidance replaces Setup+r.
- `tests/pi_scan/ws4_tui.rs`
  - Focused Double/Unlimited overlay, old-to-proposed values, explicit spending warning, keyboard focus, and 20x10 render.
  - English/German/Hungarian direct-flow copy and removal of Setup+r guidance.
  - Locale key coverage, updated footer/help, rolling usage timestamp, and WS1 native-range settings expectation.

## Validation commands and exact outcomes

### Required final validation

1. `cargo fmt --all` — exit `0`; no output.
2. `cargo clippy --all-targets --all-features -- -D warnings` — final exit `0`.
   - One earlier iteration exited `101` for `clippy::needless_pass_by_value` in the new Applied projection helper; changed it to borrow the typed result, then the final command passed.
3. `cargo check` — final exit `0`.
4. Final focused serialized command chain — exit `0`:
   - `cargo test events::pi_scan::keys::tests:: -- --test-threads=1` — 9 passed.
   - `cargo test app::runtime::event_loop::tests::budget_ -- --test-threads=1` — 3 passed.
   - `cargo test ui::pi_scan::overview::tests:: -- --test-threads=1` — 3 passed.
   - `cargo test ui::pi_scan::progress::tests:: -- --test-threads=1` — 7 passed.
   - `cargo test --test pi_scan ws4_tui:: -- --test-threads=1` — 38 passed.
5. `git diff --check` — exit `0`.
6. `test -z "$(git diff --cached --name-only)"` — exit `0`; no staged files.

Additional focused validation:

- `cargo test budget_dispatch_owns_and_polls_typed_acknowledgement -- --test-threads=1` — exit `0`; 1 passed.
- `cargo test budget_acknowledgements -- --test-threads=1` — exit `0`; 1 passed.
- `cargo test budget_applied_projects_limits_residual_pause_and_sticky_pauses -- --test-threads=1` — exit `0`; 1 passed.
- `cargo test budget_dialog -- --test-threads=1` — exit `0`; keyboard and TUI dialog filters passed.

## Omissions and residual risks

- By explicit ownership, `cargo test -- --test-threads=1` was not run; the parent owns the full serialized suite.
- The typed acknowledgement is polled on the existing redraw tick rather than added as another central `select!` receiver because ownership is request-scoped in UI state. Under normal operation this adds at most one redraw interval of projection latency; a heavily starved tick channel could delay visible acknowledgement while preserving correctness and single ownership.
- Independent review is still required by the acceptance gate.
- Existing inherited WS1 tracked modifications and the pre-existing untracked plan/subagent directory remain in the shared checkout; no file is staged.

## Recommended parent next step

Review the WS2-only ten-file diff, especially receiver lifecycle, dry-run/no-hit projection, residual pause wording, and narrow localized overlay; then run the parent-owned full serialized suite and independent reviewer gate.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Implemented the approved direct b budget flow, deterministic focused Double/Unlimited state, typed request-owned acknowledgement dispatch/polling, authoritative outcome projection, conservative runtime-limit display, responsive warning overlay, and three-locale copy exclusively within the WS2 write boundary."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Captured three expected red failures, added keyboard/event-loop/render/locale regressions, and recorded passing focused serialized suites, fmt, all-target/all-feature Clippy, cargo check, diff check, and empty staged-state evidence."
    }
  ],
  "changedFiles": [
    "config/locales/de-DE.yml",
    "config/locales/en-US.yml",
    "config/locales/hu-HU.yml",
    "src/app/runtime/event_loop.rs",
    "src/events/pi_scan/keys.rs",
    "src/state/pi_scan_ui.rs",
    "src/ui/pi_scan/mod.rs",
    "src/ui/pi_scan/overview.rs",
    "src/ui/pi_scan/progress.rs",
    "tests/pi_scan/ws4_tui.rs"
  ],
  "testsAddedOrUpdated": [
    "src/events/pi_scan/keys.rs: b eligibility, default/focus, Enter, Esc, pending/submitting, and r-is-Retry regressions",
    "src/app/runtime/event_loop.rs: typed dispatch/receiver polling, Applied/residual pause, NoLongerBlocked, Rejected, dry-run preview, and sticky pause regressions",
    "src/ui/pi_scan/overview.rs: runtime zero-as-Unlimited render regression",
    "src/ui/pi_scan/progress.rs: direct b guidance replacing Setup+r",
    "tests/pi_scan/ws4_tui.rs: overlay choices/values/warning/narrow dimensions, locale copy, footer/help, rolling runtime display"
  ],
  "commandsRun": [
    {
      "command": "cargo test budget_key_opens_direct_choice_without_guided_setup -- --test-threads=1",
      "result": "failed",
      "summary": "Expected red, exit 101: eligible plain b was unhandled."
    },
    {
      "command": "cargo test budget_pause_guidance_uses_direct_budget_key -- --test-threads=1",
      "result": "failed",
      "summary": "Expected red, exit 101: rendered Setup (1)+r guidance instead of direct b."
    },
    {
      "command": "cargo test overview_renders_zero_runtime_limits_as_unlimited -- --test-threads=1",
      "result": "failed",
      "summary": "Expected red, exit 101: rendered settings/raw zero rather than runtime Unlimited limits."
    },
    {
      "command": "cargo fmt --all",
      "result": "passed",
      "summary": "Final exit 0."
    },
    {
      "command": "cargo clippy --all-targets --all-features -- -D warnings",
      "result": "passed",
      "summary": "Final exit 0 after fixing one initial needless-pass-by-value lint iteration."
    },
    {
      "command": "cargo check",
      "result": "passed",
      "summary": "Final exit 0."
    },
    {
      "command": "cargo test events::pi_scan::keys::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 9 passed."
    },
    {
      "command": "cargo test app::runtime::event_loop::tests::budget_ -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 3 passed."
    },
    {
      "command": "cargo test ui::pi_scan::overview::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 3 passed."
    },
    {
      "command": "cargo test ui::pi_scan::progress::tests:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 7 passed."
    },
    {
      "command": "cargo test --test pi_scan ws4_tui:: -- --test-threads=1",
      "result": "passed",
      "summary": "Exit 0; 38 passed."
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
    "Initial red b flow: exit 101, handle_key returned false",
    "Initial red guidance: exit 101, old Setup (1)+r text rendered",
    "Initial red zero display: exit 101, raw/settings limits rendered",
    "Keyboard final: 9 passed, 0 failed",
    "Budget event-loop final: 3 passed, 0 failed",
    "Overview final: 3 passed, 0 failed",
    "Progress final: 7 passed, 0 failed",
    "WS4 TUI final: 38 passed, 0 failed",
    "cargo clippy all targets/features: exit 0",
    "cargo check: exit 0",
    "git diff --check: exit 0",
    "git diff --cached --name-only: empty"
  ],
  "residualRisks": [
    "The parent-owned full serialized suite and required independent review gate remain outstanding.",
    "Acknowledgement projection is tick-polled, so visible completion can lag by one redraw interval while retaining serialized single-owner mutation."
  ],
  "noStagedFiles": true,
  "diffSummary": "Ten approved WS2 files add the direct budget adjustment dialog, b keyboard flow, typed acknowledgement lifecycle, authoritative projection, runtime Unlimited rendering, localized responsive overlay/copy, and focused regressions.",
  "reviewFindings": [
    "no blockers found in focused implementation validation; required independent reviewer gate remains with the parent"
  ],
  "manualNotes": "No commit, stage, push, publish, dependency, WS1 implementation, plan, report, PR, README/wiki, scheduler/domain/persistence/consent, package-acquisition, result-schema, or guided-setup logic change was made."
}
```
