# Pi Scan Budget Final Verification

## Review

### Verdict

**No blocker or feature-code fix worth doing now remains.** PSB-001, FINAL-001, FINAL-002, all earlier PSB findings, and BUD-001 through BUD-006 are resolved in the current source.

One **Low-severity workflow finding remains open**: FINAL-003, the stale PR record. Executable validation and Git diff/staging state could not be independently attested.

### Correct

- **PSB-001 authorization race is resolved.**
  - Production now passes a shared authorization cell rather than a captured Boolean (`src/app/runtime/workers/pi_scan_production.rs:1444-1468, 2273-2295`).
  - The scheduler owner lock is acquired before the unattended authorization is read (`src/app/runtime/workers/pi_scan_orchestrator.rs:2731-2746`; authorization read at `1532-1545`).
  - The read guard spans `start_next`, durable persistence, and active registration, then is released before external execution (`src/app/runtime/workers/pi_scan_orchestrator.rs:1536-1562`).
  - Revocation publication takes only the authorization write lock, not the scheduler owner lock (`src/app/runtime/workers/pi_scan_orchestrator.rs:1978-2026`). The consent path publishes an ineligible decision before waiting for owner persistence (`src/app/runtime/workers/pi_scan_production.rs:2040-2049`), preventing an owner→authorization / authorization→owner lock cycle.
  - Deterministic two-job owner-lock coverage proves a waiting second start observes revocation (`tests/pi_scan/ws9_orchestration.rs:1548-1653`).

- **FINAL-001 guided clamps are resolved.**
  - Starts and tokens now use native `u32::MAX` and `u64::MAX` bounds (`src/state/pi_scan_setup.rs:559-582`).
  - Cost stepping uses exact parsed micro-USD and checked arithmetic without a legacy `$10,000` clamp (`src/state/pi_scan_setup.rs:867-883`).
  - Regressions cover values above former maxima and native boundaries (`tests/pi_scan/setup_wizard.rs:136-202`).

- **FINAL-002 localization is resolved.**
  - Current token syntax, cost precision, and cost-overflow diagnostics map to locale keys (`src/ui/pi_scan/setup.rs:335-378`).
  - English, German, and Hungarian provide the new strings (`config/locales/en-US.yml:125-133`, `config/locales/de-DE.yml:125-133`, `config/locales/hu-HU.yml:128-135`).
  - Tests feed current validation producers through every shipped locale (`src/ui/pi_scan/setup.rs:569-641`).

- Scheduler policy remains sound:
  - Zero bypasses each independent finite check; aggregate and reservation overflow fail closed (`src/state/pi_scan.rs:758-813, 1128-1149`).
  - Apply recomputes the affected set under scheduler ownership and changes only exceeded dimensions (`src/state/pi_scan.rs:473-526`).
  - Double is exact, checked, and all-or-nothing; Unlimited writes zero only to affected fields (`src/state/pi_scan.rs:1152-1188`).
  - User and Service pauses remain independent and sticky.

- Exact validation remains sound:
  - Cost uses checked integer micro-USD parsing through the full native range and never maps invalid input to Unlimited (`src/theme/types.rs:248-278`; runtime fail-closed construction at `src/app/runtime/mod.rs:273-384`).
  - Token parse failures retain an explicit validation error, including from fresh defaults (`src/theme/settings/parse_settings.rs:186-200, 677-725`; `src/theme/types.rs:201-206`).
  - Config Editor validates native token and exact-cost bounds (`src/events/modals/config_editor.rs:886-913, 1259-1276`).

- Persistence and restart behavior remain sound:
  - Budget settings are replaced atomically as one three-key patch (`src/theme/types.rs:125-161`).
  - Private `create_new` temporary files, file sync, directory sync, and post-rename rollback are present (`src/theme/config/patch.rs:382-486, 535-556`).
  - Owner-state failure restores runtime/config state and attempts both settings and state rollback (`src/app/runtime/workers/pi_scan_orchestrator.rs:1399-1440, 2915-2940`).
  - Restart and consent preservation are covered in `tests/pi_scan/ws9_orchestration.rs:918-1015`; post-rename rollback is covered at `src/app/runtime/workers/pi_scan_production.rs:2867-2971`.

- Production revalidation and UI projection remain sound:
  - A 30-second budget timer is independent of observation consent (`src/app/runtime/workers/pi_scan_production.rs:1477-1513, 1605-1657`).
  - Changed owner state is published as a complete runtime projection and applied by the event loop (`src/app/runtime/workers/pi_scan_production.rs:1945-1978`; `src/app/runtime/event_loop.rs:2084-2086`).
  - Connected config reload preserves owner authority and reports divergence (`src/state/pi_scan_ui.rs:886-929`; `src/app/runtime/init.rs:317-328`).

- Direct `b` UX and dry-run remain sound:
  - `b` is limited to eligible Overview/Progress projections; Progress retains `r` for Retry (`src/events/pi_scan/keys.rs:44-100, 553-570`).
  - The dialog defaults to Double, dispatches once on Enter, and cancels with Esc when no request is pending (`src/state/pi_scan_ui.rs:1028-1107`).
  - Dispatch is typed and never invokes guided setup (`src/app/runtime/event_loop.rs:1498-1527`).
  - Dry-run previews cloned state and suppresses settings, runtime, queue, execution, and wake mutation (`src/app/runtime/workers/pi_scan.rs:796-826`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1405-1424`; production eligibility rejects dry-run at `src/app/runtime/workers/pi_scan_production.rs:1486-1513`).

### Blocker

- **None found.**

### Open finding

#### FINAL-003 — Low / Workflow compliance — PR record remains stale

- **Location:** `dev/PR/PR_feat-aur-scan-integrated.md:1-24, 73-93`.
- **Failure mode:** Reviewers receive no branch record of zero-as-Unlimited, direct Double/Unlimited recovery, authorization linearization, persistence rollback, production revalidation, or the latest verification state.
- **Evidence:** The Summary and observed-results sections omit the integrated budget feature; a search found no `Unlimited`, `Double`, direct-`b`, or budget-adjustment entry. `AGENTS.md` requires the existing branch PR record to be updated whenever code changes.
- **Minimal remediation:** Update the existing PR record with final branch-vs-main budget behavior, prior-finding dispositions, and newly observed command results. Do not copy unverified historical totals.

## Prior Finding Status

| Finding | Status | Current evidence |
|---|---|---|
| PSB-001 | **Resolved** | Shared authorization is read after owner-lock acquisition and held through start persistence/registration. |
| PSB-002 | **Resolved** | Exact cost parsing rejects malformed and overflowing values; runtime construction fails closed. |
| PSB-003 | **Resolved** | Post-rename sync failure restores prior settings and prevents wake/restart drift. |
| PSB-004 | **Resolved** | Invalid startup token input retains an actionable diagnostic. |
| PSB-005 | **Resolved** | Production publishes complete changed revalidation state to the UI. |
| FINAL-001 | **Resolved** | Guided starts/tokens/cost use native boundaries without legacy clamps. |
| FINAL-002 | **Resolved** | Current token and exact-cost errors are localized in all three locales. |
| FINAL-003 | **Open — Low** | Existing PR record still omits the integrated budget work. |

### Earlier BUD findings

| Finding | Status |
|---|---|
| BUD-001 | **Resolved** — centralized wake predicate plus per-start authorization retain observation and paid-background gates. |
| BUD-002 | **Resolved** — connected reload preserves live owner limits and reports divergence. |
| BUD-003 | **Resolved** — production independently revalidates, persists, publishes, and conditionally wakes. |
| BUD-004 | **Resolved** — directory-entry durability and post-rename rollback are implemented. |
| BUD-005 | **Resolved** — aggregate overflow evidence remains sticky and conservative. |
| BUD-006 | **Resolved** — editor and startup token validation reject/surface malformed and overflowing input. |

## Maintainability and Plan Compliance

- The scheduler remains the canonical owner of exceeded-limit classification and mutation.
- Authorization locking is narrow and has no identified lock-order cycle.
- Exact budget formatting is still duplicated between theme and event-loop code; consolidating on `format_pi_scan_budget_microusd` is optional cleanup, not a correctness issue.
- A stale Apply whose rolling window has already expired can briefly leave the old Budget pause visible until the 30-second revalidation. This is bounded optional cleanup.
- The plan correctly remains **In progress** until required commands, review disposition, PR update, and archival are completed.

## Tools, Commands, and Gaps

### Tools used

- `functions.read`
- `functions.grep`
- `functions.find`
- `functions.ls`

Evidence came from current source, tests, plan, locales, config examples, and prior review artifacts. Worker handoff claims were not used.

### Commands not run

No shell or Git-command capability was available:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check`
4. `cargo test -- --test-threads=1`
5. `git diff --check`
6. `git diff`
7. `git status --short`

### Residual risks and gaps

- The complete Git diff, changed-file set, and staged-file state were not independently inspected.
- Compilation, formatting, Clippy, and test results are not independently attested.
- The two named observation/background revocation tests use the same shared authorization-cell helper. Together with production projection tests this covers the mechanism statically, but it is not a full end-to-end production consent-request test.
- No feature-code blocker remains; FINAL-003 and command attestation remain before workflow completion.

**Confidence: 93/100.** Current static control flow, arithmetic, localization, and test source directly support the findings. Lack of Git and executable validation prevents full attestation.