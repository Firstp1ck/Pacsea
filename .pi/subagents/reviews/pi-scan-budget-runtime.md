# Pi Scan Budget Feature — Independent Review

## Review

### Correct

- Zero-as-Unlimited is consistently encoded in scheduler checks: finite limits are checked only when nonzero (`src/state/pi_scan.rs:1109-1118`), with focused starts/tokens/cost coverage in `tests/pi_scan/ws3_runtime.rs:281-329`.
- Double uses checked multiplication for all dimensions and builds a local result before mutating runtime state (`src/state/pi_scan.rs:1122-1159`). Overflow/no-partial-mutation coverage exists at `tests/pi_scan/ws3_runtime.rs:465-518`.
- Adjustment scope is scheduler-owned and recomputed at Apply time (`src/state/pi_scan.rs:473-518`); the UI only presents an open-time projection.
- Settings updates patch all three limits in one file replacement, preserve unrelated keys, and use a fresh mode-0600 temporary file (`src/theme/config/patch.rs:569-642`, `433-455`).
- Consent binding properly excludes mutable budgets while retaining a narrowly scoped legacy binding migration (`src/app/runtime/workers/pi_scan_production.rs:1290-1327`).
- Dry-run adjustment evaluates cloned runtime state and does not persist or dispatch (`src/app/runtime/workers/pi_scan.rs:787-826`; `src/app/runtime/workers/pi_scan_orchestrator.rs:1382-1386`).
- The TUI flow defaults to Double, supports Enter/Esc, keeps Progress `r` as Retry, and displays exact overflow and Unlimited warnings (`src/events/pi_scan/keys.rs:50-113`; `src/ui/pi_scan/mod.rs:284-311`).
- Restart, rollback, consent preservation, zero-cost behavior, and TUI projections have focused tests, notably `tests/pi_scan/ws9_orchestration.rs:870-1106` and `tests/pi_scan/ws4_tui.rs:743-786`.

## Blockers / Required Fixes

### BUD-001 — High — Production adjustment wake bypasses the successful-observation gate

- **Affected:** `src/app/runtime/workers/pi_scan_production.rs:1847-1852`, `RuntimeConsentProjection`; comparison at `src/app/runtime/workers/pi_scan_production.rs:1535-1550`.
- **Violated requirement:** Unlimited must remove only affected budget checks; service, consent, and runtime gates remain intact (`plans/planned/pi-scan-budget-adjustment.md:24`).
- **Failure mode:** Normal startup execution requires `consent.observation_started`, but the post-adjustment wake checks only background execution and paid consent. A restored budget-paused queue can therefore execute after current-session startup observation failed.
- **Evidence/reproduction:**
  1. Restore a queued background target with valid paid/background consent.
  2. Let setup probing succeed but startup observation fail, leaving `observation_started = false`.
  3. Apply Unlimited to the exceeded budget.
  4. Lines 1847-1852 enqueue an execution wake despite the failed observation.
  5. `run_next_registered` checks setup/background execution but has no `observation_started` gate (`src/app/runtime/workers/pi_scan_orchestrator.rs:1460-1478`).
- **Minimal remediation:** Centralize production background-wake eligibility and require at least `observation_enabled && observation_started`, in addition to existing paid/background/runtime gates. Add a regression proving an applied adjustment does not wake after failed startup observation.

### BUD-002 — High — Budget-only config reload changes the UI projection but not the live scheduler owner

- **Affected:** `src/state/pi_scan_ui.rs:903-909`; reload path `src/events/global.rs:407-425` and `src/app/runtime/init.rs:314-319`.
- **Violated requirement:** Scheduler ownership, restart consistency, and truthful runtime/UI state.
- **Failure mode:** Reloading edited budget settings directly replaces `app.pi_scan.runtime.budget_limits`, but the production orchestrator remains alive with its independently owned old limits. No typed runtime request or owner swap occurs.
- **Evidence/reproduction:**
  1. Start a connected production owner with token limit 500.
  2. Edit the setting to 1,000 and invoke config reload.
  3. `apply_settings` changes the UI runtime projection at line 909.
  4. `handle_reload_config` has no runtime-channel access and cannot update the owner.
  5. The UI now displays/classifies against 1,000 while dispatch still uses 500.
  6. The test at `tests/pi_scan/ws4_tui.rs:743` validates only the UI object and therefore misses the owner divergence.
- **Minimal remediation:** Either perform a bounded runtime-owner update/restart using the existing transfer mechanism while preserving consent, or leave the connected runtime projection unchanged and explicitly require restart. Add a live owner-plus-reload integration test.

### BUD-003 — High — Production does not independently revalidate rolling budget expiry

- **Affected:** `src/app/runtime/workers/pi_scan_production.rs:1808`; production request loop `src/app/runtime/workers/pi_scan_production.rs:1553-1588`.
- **Violated requirement:** Finite limits retain rolling-window accounting, and Budget pause remains derived and revalidated (`plans/planned/pi-scan-budget-adjustment.md:19,56`).
- **Failure mode:** The inert worker has a 30-second budget revalidation interval, but production ignores `RevalidateBudgets` and has no independent equivalent. Its only timer is the 15-minute-or-longer observation interval, gated by observation consent.
- **Evidence/reproduction:** Leave a background job budget-paused, then disable observation consent and wait until its one-hour/24-hour records expire. No timer branch runs, `RevalidateBudgets` is a no-op, and the queued work remains paused indefinitely despite fitting the rolling window. A stale `NoLongerBlocked` acknowledgement can likewise leave the Budget pause visible until another unrelated dispatch.
- **Minimal remediation:** Add an independent production budget-revalidation timer or implement `RevalidateBudgets` through the orchestrator owner. Persist pause transitions and conditionally wake only through the complete production gate from BUD-001. Add production expiry/no-observation coverage.

### BUD-004 — High — Settings persistence is not durable at the directory-entry boundary before wake

- **Affected:** `src/theme/config/patch.rs:373-390`, `atomic_write`.
- **Violated requirement:** No execution wake until new policy is durably accepted (`plans/planned/pi-scan-budget-adjustment.md:58`).
- **Failure mode:** The temporary file is synced, then renamed, but the parent directory is never synced. On Unix, syncing the file does not by itself durably commit the rename directory entry across a crash.
- **Evidence:** `write_temp_file` calls `sync_all` at lines 446-454, while `atomic_write` returns immediately after `fs::rename` at lines 382-390. Production treats this return as successful settings persistence and may subsequently acknowledge and wake execution.
- **Minimal remediation:** After rename, open and `sync_all` the parent directory on Unix, surfacing failure as a transaction failure. Add an injected post-rename directory-sync failure regression and verify no acknowledgement/wake.

## Other Required Fixes

### BUD-005 — Medium — Saturating rolling totals can treat aggregate overflow as fitting a finite maximum

- **Affected:** `src/state/pi_scan.rs:781-800`, especially lines 796-797; `finite_usage_exceeded` at lines 1114-1118.
- **Violated requirement:** Conservative rolling accounting and the documented rule that arithmetic overflow exceeds a finite limit.
- **Failure mode:** Historical token/cost totals saturate to `u64::MAX`, discarding whether the true sum overflowed. If the next reservation is zero and the finite limit is `u64::MAX`, `checked_add(MAX, 0)` succeeds and is not greater than the limit.
- **Evidence/reproduction:** Create two in-window background records with effective cost `u64::MAX` and `1`, set finite cost limit `u64::MAX`, and queue a zero-cost background reservation. The mathematical usage exceeds the cap, but classification sees `MAX + 0 == MAX`.
- **Minimal remediation:** Retain overflow flags while aggregating, use checked wider accumulation, or classify a dimension exceeded immediately when aggregation overflows. Add zero-reservation aggregate-overflow tests for tokens and cost.

### BUD-006 — Medium — Config editor accepts malformed token caps and silently applies a default value

- **Affected:** `src/theme/config/schema.rs:697`; generic validation at `src/events/modals/config_editor.rs:868,886-891`; parser at `src/theme/settings/parse_settings.rs:189-205`.
- **Violated requirement:** Config schema/validation must safely support enlarged finite values.
- **Failure mode:** The token cap was changed to unrestricted `ValueKind::String`, but no key-specific validator was added. A value such as `abc` is saved; parsing fails silently and leaves the default numeric token cap, which then passes runtime validation.
- **Evidence:** `validate_semantic_string_key` accepts all unlisted keys, while `assign_u64` consumes malformed known keys without reporting failure.
- **Minimal remediation:** Add a proper unsigned-64 value kind or token-specific decimal validator, and surface parse failures rather than substituting a valid-looking default. Add editor-save and startup parsing regressions for malformed, `u64::MAX`, and overflow values.

## Optional Suggestions

- Consolidate production wake predicates into one helper. The current startup, periodic observation, consent update, and adjustment paths already show predicate drift.
- Expose a shared conservative usage projection from the scheduler rather than duplicating saturating accounting in `src/ui/pi_scan/overview.rs:136-159`.

## Architecture and Plan Compliance

The typed scheduler/acknowledgement boundary, checked adjustment policy, legacy consent migration, and focused TUI state are coherent and maintainable. The main architectural weakness is split ownership: settings, UI runtime projection, production consent projection, and orchestrator runtime can diverge because update and wake predicates are implemented separately.

The feature is therefore **not ready to accept** until BUD-001 through BUD-004 are resolved and regression-tested.

## Verification Scope

- **Model/provider:** Not exposed by this reviewer runtime.
- **Commands run:** None. This reviewer had read/grep-only tools and no shell or Git-command capability.
- **Inspected directly:** Plan, current source files across scheduler/runtime/persistence/TUI/config/locales, and focused WS3/WS4/WS9 tests.
- **Gap:** The complete unstaged Git diff, staged-file state, formatting, compilation, and test execution could not be independently attested. Changed-file enumeration was therefore not fully Git-verified.
- **Confidence:** **88/100**. Findings are based on direct static control-flow evidence; confidence is reduced by the unavailable Git diff and executable test environment.