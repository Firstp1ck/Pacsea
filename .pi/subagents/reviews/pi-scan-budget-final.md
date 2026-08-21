# Pi Scan Budget Final Review

## Review

### Verdict

**Not ready for final acceptance.** PSB-002 through PSB-005 are resolved, and the original BUD-001 through BUD-006 reproductions remain fixed. However, **PSB-001 remains open as a High-severity blocker** because start authorization is captured before acquiring the scheduler-owner lock, leaving a stale-authorization race.

Two additional implementation/UX issues and one workflow issue were found.

### Correct

- Zero is consistently Unlimited in scheduler checks (`src/state/pi_scan.rs:473-518, 758-813, 1139-1185`), runtime/config projections, and localized display.
- Double is exact, checked, affected-only, and all-or-nothing (`src/state/pi_scan.rs:1139-1188`).
- Rolling accounting preserves overflow evidence; finite maximums remain fail-closed (`src/state/pi_scan.rs:758-813`).
- Cost parsing now uses exact integer micro-USD arithmetic and rejects malformed or overflowing input (`src/theme/types.rs:248-278`).
- Durable settings replacement uses private `create_new` temporary files, file synchronization, parent-directory synchronization, and rollback after post-rename sync failure (`src/theme/config/patch.rs:382-443, 467-486, 535-556`).
- Production rolling-budget revalidation is independent of observation consent and publishes changed authoritative runtime state (`src/app/runtime/workers/pi_scan_production.rs:1615-1657, 1945-1978`).
- Direct `b`, Double default, Enter/Esc behavior, Progress `r`, narrow rendering, and three-locale copy are implemented and covered by focused test source.
- Dry-run evaluates cloned state and suppresses durable/runtime/execution mutation (`src/app/runtime/workers/pi_scan.rs:796-826`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1405-1424`).

## Current Findings

### PSB-001 — High / Blocker — Start authorization still has a stale-snapshot race

- **Location:** `src/app/runtime/workers/pi_scan_production.rs:2276-2306`, `drain_eligible_queue` / `execute_one`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1532-1544, 2645-2671`, `run_next_registered` / `run_next_with_optional_progress`.
- **Failure mode:** A background execution can start after observation or paid-background authorization has been revoked and persisted.
- **Evidence:**
  - `drain_eligible_queue` reads `ProductionPolicyProjection` and reduces it to a plain `bool` before the asynchronous call and before the owner lock is acquired.
  - That Boolean is carried through `execute_one` and into `run_next_registered`.
  - A concurrent revocation can publish false and persist owner consent while the execution task waits for the owner lock. Once it acquires the lock, it still uses the previously captured `true`.
  - `run_next_registered` checks that Boolean, feature/background configuration, and scheduler paid consent, but it does not independently re-read current-session observation or background-paid authorization.
  - The added test at `src/app/runtime/workers/pi_scan_production.rs:2755-2777` only checks direct projection reads. It does not exercise a queued wake, owner-lock ordering, revocation, and subsequent start.
- **Minimal remediation:** Linearize policy authorization with the scheduler start. Hold a current policy read guard through the owner-locked start decision, or move the complete unattended predicate into owner state with generation-aware updates. Add deterministic stale-wake and two-job tests for observation and background-paid revocation.

### FINAL-001 — Medium — Guided setup retains legacy budget clamps

- **Location:** `src/state/pi_scan_setup.rs:559-582`, `PiScanSetupWizardState::adjust_focused`; helpers at `src/state/pi_scan_setup.rs:857-886`.
- **Failure mode:** A valid doubled policy can be silently reduced when adjusted in guided setup.
- **Evidence:**
  - Starts still use maximum `5`; tokens still use maximum `500_000`; cost increase still clamps to `$10,000`.
  - With starts `10`, pressing Right computes `min(11, 5) == 5`.
  - With tokens `1_000_000`, pressing Right computes `min(1_010_000, 500_000) == 500_000`.
  - A large exact cost likewise collapses to `10000.00` on increase.
  - This conflicts with the plan’s native-bound policy and localized copy/tests that claim the legacy maxima were removed.
- **Minimal remediation:** Use native `u32`/`u64` bounds or a non-destructive stepping strategy for already-large values. Add regressions starting above every former maximum and at native overflow boundaries.

### FINAL-002 — Low — New token/cost diagnostics bypass localization

- **Location:** `src/ui/pi_scan/setup.rs:335-374`, `localize_setting_issue`; producers at `src/theme/types.rs:201-206, 248-278` and `src/theme/settings/parse_settings.rs:186-200`.
- **Failure mode:** German and Hungarian Setup pages display raw English for malformed/overflowing token caps and the new exact cost errors.
- **Evidence:** The localization matcher recognizes only the obsolete cost string `must be a non-negative decimal` and has no token-cap diagnostic case. Current producers emit longer precision/maximum messages. The localization test at `src/ui/pi_scan/setup.rs:569-585` still tests the obsolete string.
- **Minimal remediation:** Prefer typed validation issues, or add mappings and locale keys for token syntax, cost precision, and native-bound overflow. Update tests to pass actual current `validation_issues()` output through every locale.

### FINAL-003 — Low / Workflow compliance — PR record is stale

- **Location:** `dev/PR/PR_feat-aur-scan-integrated.md:1-24, 75-93`.
- **Failure mode:** The branch PR record does not document zero-as-Unlimited, Double/Unlimited adjustment, persistence/revalidation, or the current review state, while still asserting older test totals and zero blocking findings.
- **Evidence:** No `Unlimited` or `Double` entry exists in the PR record. `AGENTS.md` requires the current branch PR file to be updated whenever code changes.
- **Minimal remediation:** After fixing and validating the blocker, update the existing PR record with only final branch-vs-main behavior and newly observed command results.

## Prior Finding Status

### PSB findings

| ID | Status | Evidence |
|---|---|---|
| **PSB-001** | **Open — Blocker** | The projection exists, but authorization is converted to a stale Boolean before owner-lock acquisition (`pi_scan_production.rs:2276-2306`). |
| **PSB-002** | **Resolved** | Shared exact cost parser rejects `u64::MAX + 1`; invalid runtime settings disable production rather than becoming zero (`theme/types.rs:248-278`; `app/runtime/mod.rs:273-322, 346-384`). |
| **PSB-003** | **Resolved** | Post-rename sync failure restores prior bytes and syncs the restored directory; production coverage checks rejection, no wake, owner state, and restart (`theme/config/patch.rs:390-443, 1073-1092`; `pi_scan_production.rs:2871-2971`). |
| **PSB-004** | **Resolved** | Token parse failures retain an explicit validation diagnostic, including from fresh defaults (`parse_settings.rs:186-200, 677-725`; `theme/types.rs:88-89, 201-204`). |
| **PSB-005** | **Resolved** | Changed revalidation returns and publishes the complete runtime projection; the event loop applies it (`pi_scan_orchestrator.rs:40-65, 1459-1474`; `pi_scan_production.rs:1945-1978`; `event_loop.rs:2084-2086`). |

### BUD regression status

| ID | Status |
|---|---|
| **BUD-001** | Original failed-observation wake reproduction remains resolved. PSB-001 is a distinct later start-time race. |
| **BUD-002** | Resolved; connected reload preserves owner limits and reports divergence. |
| **BUD-003** | Resolved; production has independent durable revalidation and UI publication. |
| **BUD-004** | Resolved; rename durability and post-rename rollback are both present. |
| **BUD-005** | Resolved; aggregate overflow remains sticky and conservative. |
| **BUD-006** | Resolved; editor and startup token validation now surface malformed/overflowing input. |

## Architecture, Security, UX, and Plan Compliance

- Scheduler ownership, typed adjustment acknowledgements, exact arithmetic, and settings/state transaction ordering are coherent.
- Private file modes, atomic sibling writes, test-only failure seams, consent-binding separation, and dry-run behavior satisfy the reviewed security constraints.
- Production revalidation and UI projection now address rolling-window expiry without observation activity.
- The remaining authorization race means the invariant “every unattended start is authorized at start time” is not yet satisfied.
- Guided setup’s hidden legacy clamps and untranslated new diagnostics prevent full UX/plan compliance.
- The plan appropriately remains `In progress`; review dispositions and archival should wait until findings are resolved and validation is rerun.

### Optional cleanup

- A stale Apply whose window expired can receive `NoLongerBlocked` while the old Budget pause remains until the next 30-second revalidation (`src/state/pi_scan.rs:492-501`; `pi_scan_production.rs:1908-1916`; `event_loop.rs:1582-1594`). This is bounded rather than a blocker, but immediate typed revalidation would produce cleaner feedback.
- The exact micro-USD formatter is duplicated in theme and event-loop code; consolidation would reduce drift risk.

## Tools, Commands, and Gaps

### Tools used

- `functions.read`, `functions.grep`, `functions.find`, and `functions.ls`.
- Evidence came from current source, tests, plan, locales, config examples, and prior reviews—not worker handoff claims.

### Commands

No shell or Git command capability was available. The following remain **not run**:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check`
4. `cargo test -- --test-threads=1`
5. `git diff --check`
6. `git diff`
7. `git status --short`

### Gaps

- The complete unstaged Git diff and staged-file state could not be inspected.
- Test, compile, format, and Clippy results are not independently attested.
- Current test source does not deterministically reproduce the PSB-001 owner-lock race.
- **Confidence: 88/100.** Static control-flow and arithmetic findings are direct; unavailable Git and executable validation reduce confidence.