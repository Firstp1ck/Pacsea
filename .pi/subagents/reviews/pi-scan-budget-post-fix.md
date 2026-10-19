# Pi Scan Budget Post-Fix Review

## Review

### Correct

- **Scheduler-owned budget policy is coherent.** Zero bypasses each finite check, overflow is conservative, and Apply recomputes the affected set in `src/state/pi_scan.rs:473-518, 758-813, 1139-1185`.
- **Double is checked and all-or-nothing.** A local limit copy is completed before assignment, so overflow cannot leak partial changes (`src/state/pi_scan.rs:1152-1188`; tests at `tests/pi_scan/ws3_runtime.rs:557-603`).
- **Connected config reload authority is no longer split.** A connected UI retains owner limits (`src/state/pi_scan_ui.rs:908-916`), reports divergence (`src/app/runtime/init.rs:320-328`), and has focused coverage (`tests/pi_scan/ws4_tui.rs:741-802`).
- **Production now has observation-independent budget revalidation.** The 30-second timer and explicit request both use the orchestrator owner (`src/app/runtime/workers/pi_scan_production.rs:1579-1619, 1842-1851, 1914-1932`), with rollback on owner-state persistence failure (`src/app/runtime/workers/pi_scan_orchestrator.rs:1431-1444`).
- **Parent-directory synchronization was added** after settings rename (`src/theme/config/patch.rs:375-408`).
- **Config Editor token validation accepts the full `u64` range** and rejects malformed/overflow values (`src/events/modals/config_editor.rs:886-905`; tests at `1255-1262`).
- **Direct `b` interaction meets the requested flow.** Overview and Progress use it, the dialog defaults to Double, Enter submits once, Esc cancels only when safe, and Progress retains `r` as Retry (`src/events/pi_scan/keys.rs:30-103, 545-570`; `src/state/pi_scan_ui.rs:1043-1125`).
- **Zero displays as localized Unlimited** in runtime/setup projections (`src/ui/pi_scan/mod.rs:394-424`; `src/ui/pi_scan/setup.rs:279-314`) and the English, German, and Hungarian locale files contain the direct-flow and warning copy.
- **Dry-run paths are non-mutating by construction.** Both inert and production adjustments operate on cloned state and suppress execution wake (`src/app/runtime/workers/pi_scan.rs:787-826`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1381-1387`; `src/app/runtime/workers/pi_scan_production.rs:1887-1897`). Focused tests exist at `tests/pi_scan/ws3_runtime.rs:806-851` and `src/app/runtime/event_loop.rs:3518-3568`.
- Restart, state-persistence rollback, consent preservation, direct-flow rendering, and zero-cost production behavior have focused tests in `tests/pi_scan/ws9_orchestration.rs:870-1106` and `tests/pi_scan/ws4_tui.rs:1035-1205`.

### Prior BUD Finding Status

| Finding | Status | Evidence |
|---|---|---|
| **BUD-001** | **Resolved for its original reproduction** | All unattended wake producers use `request_background_execution_if_eligible`, whose predicate includes observation enabled/started, background execution, paid execution, background-paid consent, and non-dry-run operation (`src/app/runtime/workers/pi_scan_production.rs:1446-1469, 1574, 1605-1610, 1891-1898, 1923-1930, 2054`). Failed-startup coverage exists at `2647-2666`. PSB-001 below identifies a separate start-time race. |
| **BUD-002** | **Resolved** | Connected reload preserves owner limits and emits explicit restart/direct-`b` guidance (`src/state/pi_scan_ui.rs:908-947`; `src/app/runtime/init.rs:317-328`; `tests/pi_scan/ws4_tui.rs:743-802`). |
| **BUD-003** | **Resolved at the production owner/liveness layer** | Independent timer, typed revalidation request, durable transition, rollback, and no-observation coverage exist (`src/app/runtime/workers/pi_scan_production.rs:1579-1619, 1914-1932, 2669-2750`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1431-1444`). PSB-005 records the remaining UI projection problem. |
| **BUD-004** | **Still open / partially fixed** | The parent directory is now synced, but a post-rename sync failure leaves changed settings on disk while the adjustment is rejected; no faithful injected post-rename regression exists. See PSB-003. |
| **BUD-005** | **Resolved** | Sticky overflow flags force finite dimensions exceeded even for zero next reservation (`src/state/pi_scan.rs:786-813, 1121-1149`), covered at `tests/pi_scan/ws3_runtime.rs:465-554`. |
| **BUD-006** | **Still open / partially fixed** | Config Editor validation is fixed, but startup parsing still silently consumes malformed token values and retains a prior/default limit (`src/theme/settings/parse_settings.rs:189-205, 662-691`). See PSB-004. |

## Current Findings

### PSB-001 — High / Blocker — Wake eligibility is not revalidated at each unattended start

- **Location:** `src/app/runtime/workers/pi_scan_production.rs:2088-2097`, `2183-2208`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1490-1508`; `src/state/pi_scan.rs:573-581`.
- **Violated requirement / failure mode:** Observation and independent paid-background consent must remain effective gates after a budget adjustment. The centralized helper gates only creation of a wake message. The separate execution task consumes an already queued wake and drains repeatedly without consulting `RuntimeConsentProjection`.
- **Evidence:** `drain_eligible_queue` checks fallback confirmation, then loops over `execute_one`. The owner start seam checks feature/background configuration and `paid_execution`, but not current-session `observation_enabled`, `observation_started`, or `background_paid_execution`. If observation or paid-background consent is revoked while the first background scan is active—or after a wake is queued—the drain can start a subsequent background item.
- **Minimal remediation:** Revalidate the full unattended predicate immediately before every background start through a shared/watch-backed production policy projection or an owner-level typed gate. Add tests that queue two background jobs, revoke observation and paid-background consent while the first is active, and prove the second and any stale wake do not start.

### PSB-002 — High / Blocker — Out-of-range finite cost silently becomes Unlimited on restart

- **Location:** `src/theme/types.rs:199-202, 228-240`; `src/app/runtime/mod.rs:274-322`; Config Editor dispatch at `src/events/modals/config_editor.rs:886-892`.
- **Violated requirement / failure mode:** Native integer bounds must remain authoritative, arithmetic overflow must reject rather than clamp, and zero must represent Unlimited only when explicitly entered.
- **Evidence:** `PiScanSettings::validation_issues` checks only decimal syntax, not whether the value fits integer micro-USD. For example, `18446744073709.551616` passes validation but is `u64::MAX + 1` micro-USD. `pi_scan_cost_cap_microusd` returns `None`, then runtime construction applies `.unwrap_or(0)`, silently enabling Unlimited.
- **Minimal remediation:** Use one shared exact checked decimal-to-micro-USD validator in settings validation, Config Editor, setup, UI, and runtime construction. Invalid/overflow values must disable or reject the runtime, never map to zero. Test exact `u64::MAX`, `u64::MAX + 1`, malformed input, and explicit `0`/`0.00`.

### PSB-003 — High / Blocker — Post-rename directory-sync failure is rejected without restoring settings

- **Location:** `src/theme/config/patch.rs:375-408`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1398-1401`; restart authority at `src/app/runtime/workers/pi_scan_orchestrator.rs:825`; inadequate test at `src/theme/config/patch.rs:975-984`.
- **Violated requirement / failure mode:** Durable updates must fail closed and restore prior policy where possible. A rejected adjustment must not become effective after restart.
- **Evidence:** `atomic_write` renames the new settings file before syncing its parent. If directory sync fails, the new file is already visible. `adjust_budgets` treats this as if settings did not commit, restores only in-memory runtime state, and returns an error without settings rollback. On restart, config limits overwrite loaded state at line 825, so the rejected higher or Unlimited policy can become active if the renamed file survives.
- **Minimal remediation:** Preserve the exact old settings snapshot and restore it atomically when post-rename sync fails, returning compound rollback evidence if restoration also fails. Add an injected failure specifically after rename and verify rejection, no wake, old settings bytes, old owner limits, and restart consistency.

### PSB-004 — Medium — Malformed startup token caps remain silent

- **Location:** `src/theme/settings/parse_settings.rs:189-205`; regression at `662-691`.
- **Violated requirement / failure mode:** Malformed known budget settings should be surfaced rather than replaced by an apparently valid default/prior value.
- **Evidence:** `assign_u64` returns `true` for malformed and overflowing text without emitting a diagnostic. The added test explicitly confirms malformed/overflow input retains the previous `u64::MAX`; with fresh settings it would silently retain the default 500,000.
- **Minimal remediation:** Return or record a parse diagnostic for malformed known numeric keys and project it through configuration validation. Preserve the previous value only alongside an actionable warning/error. Add a fresh-default startup test, not only a mutation-of-existing-value test.

### PSB-005 — Medium — Production budget expiry does not update the connected UI projection

- **Location:** `src/app/runtime/workers/pi_scan_production.rs:1914-1932`; startup-only snapshot publication at `1491-1495`; UI projection handling at `src/app/runtime/event_loop.rs:2010-2100`; test at `src/app/runtime/workers/pi_scan_production.rs:2669-2750`.
- **Violated requirement / failure mode:** Budget pause is derived and revalidated, and Overview/Progress must show current conservative truth.
- **Evidence:** Successful periodic revalidation persists owner state and may send an execution wake, but publishes no runtime/progress update. When observation consent is disabled, the wake is correctly suppressed, leaving the connected UI’s Budget pause and records stale indefinitely. The existing test checks only the owner snapshot and absence of wake.
- **Minimal remediation:** Return a typed changed/unchanged revalidation result and publish an authoritative runtime or pause/accounting projection when changed. Add a no-observation test proving the connected UI clears its expired Budget pause without starting execution.

## Architecture, Security, UX, and Plan Compliance

- The scheduler domain model, typed adjustment acknowledgement, checked arithmetic, and direct TUI interaction are maintainable and appropriately centralized.
- The remaining principal architecture risk is that session-only production authorization is owned by the request task while execution starts occur in another task. Wake-time validation is not a sufficient authorization boundary.
- Settings/runtime persistence remains vulnerable to a post-rename partial failure, and oversized cost validation can silently weaken policy to Unlimited.
- Localized copy and narrow-dialog behavior are adequate across the three shipped locales.
- The feature does **not** yet satisfy the plan’s fail-closed consent, native-bound validation, rollback, truthful TUI, or completed-validation gates.
- **Current blockers/fixes remain:** PSB-001, PSB-002, and PSB-003 must be fixed before acceptance; PSB-004 and PSB-005 are required correctness follow-ups.

## Verification Scope and Gaps

### Tools used

- `functions.read` for the plan, prior review, implementation, tests, and locales.
- `functions.grep` for control-flow call sites, budget comparisons, validation, persistence, wake paths, and coverage.
- `functions.ls` and `functions.find` for repository inventory.

No handoff claim was used as evidence; findings above derive from current source and tests.

### Commands

No shell or Git command capability was available. Therefore none of the following was executed:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check`
4. `cargo test -- --test-threads=1`
5. `git diff --check`
6. `git diff` / staged-state inspection

### Gaps

- The complete unstaged Git diff and staged-file state could not be independently inspected.
- Tests, formatting, compilation, and Clippy results were not executable in this reviewer environment.
- The post-rename sync failure and consent-revocation drain races lack deterministic tests.
- **Confidence: 90/100.** The findings follow directly from current static control flow and exact arithmetic, reduced by unavailable Git and executable-test tooling.