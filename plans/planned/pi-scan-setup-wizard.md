# Pi Scan Initial Setup Wizard — Implementation Plan

**Status:** Planned; implementation not started  
**Feature class:** Complex — the wizard crosses native TUI state, configuration persistence, Pi capability/model/pricing discovery, runtime lifecycle activation, durable consent, localization, and recovery behavior  
**Integration owner:** Parent Pacsea implementation session  
**Target branch:** `feat/aur-scan-integrated`  
**Related report:** `reports/pi-scan-setup-wizard.html` (required after implementation)  

## Problem statement

The current Setup page exposes all raw settings and consent toggles at once. Initial activation requires users to edit `settings.conf`, restart Pacsea, understand an unverified information dump, press `v`, and then infer which consent keys are mandatory. This is technically fail-closed but not discoverable or confidence-inspiring.

The initial setup must become a guided, keyboard-first wizard that can be opened while Pi Scan is disabled, discovers valid choices, explains consequences before confirmation, applies configuration safely, and leaves the user at a verified ready state without manual file editing.

## Classification rationale

The inherited `complex` classification is confirmed:

1. Disabled-to-enabled activation currently changes which runtime worker owns the scanner channels.
2. Candidate settings must be probed without granting paid execution or persisting consent prematurely.
3. Model selection and exact pricing facts depend on live Pi capability output.
4. Final apply spans settings persistence, runtime activation, durable consent, TUI projection, and failure recovery.
5. The work naturally separates into runtime/configuration and TUI/interaction/test workstreams.

## Measurable success criteria

- [ ] Pressing the configured Pi Scan shortcut opens the wizard when setup is incomplete, including when `pi_scan_enabled = false`.
- [ ] No manual `settings.conf` edit or application restart is required for a successful first-time setup.
- [ ] The wizard presents one decision per step, with current progress, Back/Next/Cancel controls, validation feedback, and a final review page.
- [ ] The Pi binary is resolved and its exact supported version/tool contract is verified before provider/model selection.
- [ ] Provider/model choices come from Pi-advertised exact routes; arbitrary text is not the default selection path.
- [ ] Exact pricing provenance and worst-case token/micro-USD reservation are shown before any cost/privacy consent.
- [ ] Foreground paid execution, read-only background observation, paid background execution, ordered fallback, and readiness-warning acceptance remain independent decisions.
- [ ] Defaults remain conservative: scanner off until final Apply, background observation off, paid background execution off, no fallback, medium thinking, zero background cost cap.
- [ ] Final Apply validates the full candidate configuration before atomically persisting it and activating the production runtime.
- [ ] Failure during validation, save, or runtime activation leaves the previous configuration/runtime/consent authoritative and provides actionable retry guidance.
- [ ] Cancel never writes settings, consent, queue, budget, baseline, result, or scanner runtime state.
- [ ] Existing verified setups bypass the wizard and continue to the normal Pi Scan workspace; users can explicitly rerun setup later.
- [ ] The wizard is localized in English, German, and Hungarian and remains usable at the repository's minimum terminal dimensions.
- [ ] Existing scanner security, dry-run, consent invalidation, fallback, budget, and restart guarantees remain intact.
- [ ] Two qualifying implementation workstreams, integrated validation, two independent reviews, finding dispositions, and the final HTML report are recorded before completion.

## Scope

### In scope

- Native wizard state, rendering, keyboard/mouse handling, progress, inline validation, and review.
- Inert Pi binary/version/tool/model/pricing discovery without a model call.
- Provider/model selection from advertised routes.
- Thinking, fallback, observation, paid-execution, background budget, retention, and optional proxy decisions.
- Explicit privacy/cost/coverage/readiness confirmations.
- Transactional candidate validation, settings persistence, runtime activation, and consent persistence.
- Resume/retry behavior after recoverable setup failures.
- Entry from Shift+A, existing Setup, and Config Editor Pi Scan settings.
- Three locales, contextual help, focused tests, integration tests, and completion report.

### Non-goals

- Managing provider credentials or storing secrets in Pacsea.
- Installing or upgrading Pi automatically.
- Making a paid model call during setup.
- Enabling background observation or paid background execution by default.
- Replacing the advanced Setup/details page after setup is complete.
- Generalizing the wizard framework to unrelated Pacsea integrations in this feature.

## Approved decisions and invariants

1. **Native and keyboard-first:** Implement as Pi Scan workspace views/state, not an external editor or shell prompt.
2. **Accessible while disabled:** A lightweight setup controller remains available even when production scanning is off.
3. **No model call:** Setup discovery uses only Pi version/help/model/pricing/capability probes.
4. **Candidate isolation:** Wizard edits live in a draft object and do not mutate `AppState.settings`, disk, runtime, or consent until final Apply.
5. **Exact choices:** Provider/model selections bind to exact Pi-advertised identifiers and exact pricing provenance.
6. **Transactional activation:** Validate and construct a candidate production runtime first; atomically save settings and consent only after candidate readiness succeeds. If persistence fails, tear down the candidate and retain the previous runtime.
7. **Independent consent:** Do not collapse disclosure, foreground paid execution, observation, paid background execution, fallback, or readiness-warning acceptance into one checkbox.
8. **Conservative defaults:** Initial wizard choices never silently opt into background/network/cost behavior.
9. **Material binding:** The final consent document remains bound to Pi version, route, pricing, thinking, budgets, tool/prompt/schema/extension versions, privacy controls, and fallback chain.
10. **No secret collection:** Credential setup is explained as a Pi-owned prerequisite and never displayed, accepted, logged, or persisted by Pacsea.
11. **Dry-run:** In application dry-run mode, the wizard may validate syntax and show planned values but does not probe Pi, write configuration/consent, or activate runtime.
12. **Advanced access:** Existing Config Editor remains available for expert edits; material edits invalidate consent and route the next use through verification/review.

## Wizard information architecture

| Step | Purpose | Required outcome |
| --- | --- | --- |
| 1. Welcome | Explain advisory scope, data flow, no package execution, and setup prerequisites | Continue or Cancel |
| 2. Pi readiness | Select/enter Pi binary; verify absolute resolved path, minimum version, and required isolation flags | Verified Pi capability snapshot |
| 3. Route | Enumerate exact provider/model routes and choose primary model and thinking level | One exact supported primary route |
| 4. Pricing and privacy | Show pricing source/freshness, worst-case reservation, provider data disclosure, and coverage limitations | Disclosure and foreground-paid confirmations |
| 5. Optional behavior | Choose observation, paid background execution, ordered fallback, budgets, retention, and credential-free HTTPS proxy | Explicit choices; conservative defaults preselected |
| 6. Review | Show every effective value, compiled clamps, runtime effects, and consent binding inputs | Explicit Apply or Back/Cancel |
| 7. Activate | Revalidate candidate, activate runtime transactionally, persist config/consent, and display exact outcome | Verified RuntimeConnected or actionable retry |

## State and interface design

### Draft state

Add a dedicated `PiScanSetupWizardState` containing:

- current step and focus;
- immutable original settings/consent snapshot;
- mutable candidate `PiScanSettings`;
- binary/capability result;
- advertised route and exact pricing snapshots;
- independent confirmation flags;
- validation issues and in-flight request correlation;
- apply status (`Idle`, `Validating`, `Activating`, `Persisting`, `Complete`, `Failed`);
- whether the wizard is first-run or explicit reconfiguration.

The draft must exclude credentials, raw Pi output, prompts, source content, and provider responses.

### Runtime protocol

Introduce typed setup messages rather than overloading execution requests:

- `BeginSetupProbe { binary }`
- `SetupCapabilitiesVerified { pi_version, tool_contract, routes }`
- `ValidateSetupCandidate { candidate, confirmations }`
- `ApplySetupCandidate { candidate, confirmations, validation_binding }`
- `SetupApplied { effective_settings, setup_snapshot }`
- typed failure stages for probe, candidate validation, activation, and persistence.

Every response is correlated; stale responses are ignored. Only `ApplySetupCandidate` may cause writes or production runtime replacement.

### Persistence transaction

1. Normalize and validate the candidate against compiled limits.
2. Probe exact Pi capability/model/pricing facts again and compare with the reviewed binding.
3. Construct and health-check the candidate production orchestrator without exposing it to queue work.
4. Atomically patch only Pi Scan keys in `settings.conf` using the existing config patch/store layer.
5. Persist material-bound consent atomically.
6. Swap the candidate runtime into the channel owner and publish `RuntimeConnected`.
7. On steps 4–6 failure, tear down the candidate and restore/retain the original runtime and files; report any rollback failure explicitly and fail closed.

## Execution DAG

```text
Wave 0: contract tests + runtime lifecycle spike
  ├─ WS1: wizard state, rendering, interaction, localization
  └─ WS2: candidate probe, config transaction, runtime activation
             │
             └─ Wave 2 integration: event loop + entry routing + recovery
                         │
                         └─ Wave 3 acceptance hardening and full validation
                                     │
                                     └─ independent review quorum + HTML report
```

## Workstreams and ownership

### Wave 0 — contract and feasibility

**Owner:** Integration owner  
**Files:** tests under `tests/pi_scan/`, narrowly scoped runtime prototypes if required  

- Add failing contract tests for disabled-state wizard entry, no-write Cancel, no-model setup, advertised route selection, transactional apply/rollback, stale response rejection, dry-run inertness, consent binding, narrow rendering, and locale completeness.
- Prove the runtime channel owner can transition from setup-only to production without restart and can tear down a failed candidate boundedly.
- Stop if safe in-process activation cannot be achieved without replacing shared channel contracts; record the architecture decision before implementation.

**Acceptance:** Tests fail for missing behavior rather than harness mistakes; lifecycle spike establishes a bounded activation/rollback design.

### WS1 — wizard UI and state (implementation worker outcome 1)

**Ownership:**

- `src/state/pi_scan_ui.rs` and new wizard-focused state module(s)
- `src/ui/pi_scan/**` wizard views/components
- `src/events/pi_scan/**` wizard keyboard/mouse transitions
- wizard locale keys in all three locale files
- focused UI/state tests

**Deliverables:**

- Seven-step wizard, progress indicator, Back/Next/Cancel/Retry/Apply controls.
- Route/model selector, optional-settings controls, final effective-value review.
- Clear explanations for disclosure, foreground/background behavior, costs, advisory coverage, and restart-free activation.
- Responsive rendering and contextual footer/help.

**Forbidden/shared paths:** Runtime worker implementation and config persistence are owned by WS2; event-loop integration remains integration-owner work.

**Handoff:** `.pi-subagents/handoffs/pi-scan-setup-wizard/ws1-ui.md`

### WS2 — setup controller and transaction (implementation worker outcome 2)

**Ownership:**

- setup-related typed messages in `src/app/runtime/workers/pi_scan.rs`
- setup controller/runtime lifecycle in `src/app/runtime/workers/pi_scan_production.rs` or a new cohesive module
- Pi capability/model/pricing adapter extensions
- Pi Scan settings patch transaction through the existing config layer
- focused runtime/persistence tests

**Deliverables:**

- Setup-only controller available while scanning is disabled.
- Correlated no-model capability and route discovery.
- Candidate validation and reviewed-binding recheck.
- Atomic settings/consent persistence and bounded runtime swap/rollback.
- Actionable missing Pi, unsupported version, missing route, pricing mismatch, save failure, and activation failure states.

**Forbidden/shared paths:** Wizard rendering, input layout, and translations are owned by WS1; shared event-loop wiring remains integration-owner work.

**Handoff:** `.pi-subagents/handoffs/pi-scan-setup-wizard/ws2-runtime.md`

### Integration owner — central wiring and acceptance

**Ownership:**

- `src/app/runtime/channels.rs`
- `src/app/runtime/event_loop.rs`
- `src/app/runtime/init.rs`
- shared exports and entry routing
- canonical plan, PR record, and final report

**Deliverables:**

- Route Shift+A to wizard when setup is incomplete and normal workspace when verified.
- Connect typed wizard actions/progress without leaking draft values into runtime state.
- Verify setup-to-production activation, explicit rerun, cancel, failure, and restart recovery.
- Inspect both worker diffs/handoffs and run cross-workstream validation.

## Acceptance test matrix

### Wizard UX

- Disabled scanner opens Welcome rather than a dead Setup dump.
- Progress and current decision remain visible at minimum supported width/height.
- Back preserves draft choices; Cancel restores the exact original projection and performs no writes.
- Enter/Space interactions are deterministic; mouse controls match keyboard actions.
- Advanced setup can be reopened without destroying a working runtime unless Apply succeeds.

### Capability and route selection

- Missing/relative/unsupported Pi binary gives actionable guidance.
- Exact minimum Pi version and required flags are enforced.
- Empty/duplicate/malformed advertised routes fail closed.
- Selected route must exist in the advertised snapshot.
- Pricing units/provenance/freshness and worst-case reservation are displayed and rebound before Apply.

### Transaction and recovery

- Candidate validation makes no durable change.
- Save failure leaves the old runtime/settings/consent intact.
- Activation failure leaves the old runtime/settings/consent intact.
- Consent-save failure tears down the candidate and does not expose queue execution.
- Successful Apply persists exactly the reviewed Pi keys and activates one production owner.
- Material external config drift invalidates wizard review and requires revalidation.
- Stale correlated responses cannot advance or apply the wizard.

### Security and dry-run

- Setup never accepts or logs credentials.
- No model/network acquisition call occurs during Pi capability/model/pricing discovery beyond the disclosed Pi metadata probe.
- No shell interpolation is introduced; binary execution remains direct argv with the existing positive environment policy.
- Dry-run writes nothing and launches no Pi process.
- Fallback and background behavior remain off unless independently selected and confirmed.

### Regression

- Existing verified Pi Scan users open the Targets/Overview path unchanged.
- Existing queue, cancellation, restart, budget, retention, baseline, continuation, and result flows pass.
- Existing Config Editor Pi Scan fields remain loadable/saveable.
- Existing Pacsea behavior remains unchanged when Pi is absent.

## Validation commands

Run from repository root in this order:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
cargo test --test pi_scan -- --test-threads=1
cargo test complexity -- --nocapture
git diff --check
```

Also run both existing ignored no-model Pi probes and a new ignored live setup-wizard activation probe against the supported installed Pi version. Normal tests must use fakes and remain provider/network independent.

## Independent review gate

After integration, obtain two fresh-context read-only reviews from distinct provider families:

1. Correctness/UX review: step completeness, configuration transaction, state transitions, restart-free activation, error recovery, tests, and plan compliance.
2. Security/privacy review: no secrets, no premature consent, no model call, binding freshness, rollback, process/environment boundaries, and dry-run behavior.

Every finding receives an integration-owner disposition (`accepted`, `rejected`, `deferred`, or `needs verification`). Accepted fixes require focused and full revalidation.

## Rollback strategy

- Keep the existing advanced Setup page and pre-wizard setup request path until the wizard passes acceptance; they provide a code-level fallback during development.
- The runtime swap retains the old controller until candidate activation and persistence commit.
- If post-release rollback is needed, route incomplete setup back to the existing Setup page and remove the setup-only activation messages; existing `settings.conf` keys and consent documents remain compatible.
- Never delete user configuration or consent during rollback. Material mismatches continue to invalidate consent fail-closed.

## Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| Partial settings/runtime activation | Candidate runtime plus atomic persistence and bounded swap/rollback tests |
| Consent confirmed against stale pricing/model facts | Reviewed binding re-probed immediately before Apply |
| Wizard accidentally triggers a model call | Dedicated setup-only adapter and fake transport command-order tests |
| Runtime duplication during activation | One lifecycle owner and explicit old/candidate swap invariant |
| User believes setup validates package safety | Repeated advisory-coverage language at Welcome, Review, and Complete |
| Secrets entered into proxy/model fields | Credential rejection, no credential UI, and sanitization tests |
| Narrow terminal becomes unusable | Responsive layout tests and scrollable step bodies |
| Configuration edited externally mid-wizard | File fingerprint plus candidate revalidation before commit |

## Decision record

- **Accepted:** Replace manual initial configuration with a native wizard.
- **Accepted:** Keep expert Config Editor access after setup.
- **Accepted:** Require exact advertised model selection and exact pricing review.
- **Accepted:** Support restart-free activation through a setup-only runtime controller.
- **Rejected:** One-click “enable all” because consent and background cost/network behavior must remain independent.
- **Rejected:** Credential entry because Pacsea must not own provider secrets.
- **Rejected:** Persisting each step because Cancel must be side-effect free.
- **Deferred:** General reusable wizard framework; implement only cohesive Pi Scan wizard primitives.

## Progress record

- [x] Repository setup/config/runtime/footer architecture inspected.
- [x] Complex classification confirmed and canonical plan created.
- [ ] Wave 0 failing contracts and lifecycle spike complete.
- [ ] WS1 wizard UI/state outcome integrated.
- [ ] WS2 setup controller/transaction outcome integrated.
- [ ] Central integration and cross-workstream validation complete.
- [ ] Two-review quorum and finding dispositions complete.
- [ ] Final HTML report current and linked.
- [ ] Plan archived after verified completion.
