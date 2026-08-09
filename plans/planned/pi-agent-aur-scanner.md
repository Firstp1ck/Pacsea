# Optional Pi-agent AUR scanner implementation plan

**Status:** Planned; Wave 0 complete  
**Feature class:** Complex — crosses hostile-source acquisition, Pi RPC, persistence, background scheduling, security boundaries, and native TUI state  
**Integration owner:** Parent Pacsea implementation session  
**Target branch:** `feat/aur-scan-integrated`  
**Base revision:** `ad3e692a8ecdd690494c480dbb7e459955283fe5`  
**Decision record:** `GRILL-ME-pi-agent-aur-scanner.md`  
**Canonical plan:** `plans/planned/pi-agent-aur-scanner.md`  
**Final report:** `reports/pi-agent-aur-scanner.html`  
**Optional dependency model:** Runtime-detected `pi` executable; Pacsea remains fully functional when Pi is absent, unavailable, or disabled

## Goal

Provide a native, advisory Pacsea workflow that uses an optional host Pi coding agent to analyze identity-bound AUR recipes and upstream source snapshots for installed packages, update candidates, and every observed build-relevant recipe commit after an explicit baseline.

The feature is keyboard-first, default-off, dry-run-safe, bounded, resilient when optional tools are absent, explicit about privacy/cost/coverage, and never represents AI output as proof that a package is safe.

## Success criteria

The feature is complete only when all of the following are verified:

- [ ] Existing browsing, updates, installation, and scanners behave as before when Pi scanning is disabled or unavailable.
- [ ] Setup detects Pi, probes the required CLI/RPC isolation contract, enumerates models, discloses provider/privacy/cost behavior, and requires explicit enablement.
- [ ] One or many installed AUR package bases and update candidates can be queued, scanned sequentially, detached, cancelled, retried, and reopened.
- [ ] Every observed commit is durably ledgered; every build-relevant or uncertain recipe-tree change is queued oldest-first without silent coalescing.
- [ ] Split packages are deduplicated by package base while retaining all affected installed names.
- [ ] Installed-provenance scans never infer build identity from version equality. A separate complete current-HEAD scan may establish an observation baseline.
- [ ] Static HTTPS and pinned `git+https` sources declared by immutable `.SRCINFO` are acquired, verified, bounded, manifested, and analyzed without executing package code.
- [ ] Background observation and paid model execution are independently consented and bounded.
- [ ] Pi receives only Pacsea-owned path-confined read-only tools; built-ins and ambient resources are disabled and verified fail-closed.
- [ ] Results are bound to recipe/source manifests, AUR commits, model/provider attempts, Pi version, prompt/schema/tool versions, and scan identity.
- [ ] Invalid, oversized, mismatched, fabricated-evidence, injected, or control-sequence-bearing model output fails validation explicitly.
- [ ] Critical/high findings and stale identity require deliberate result-bound acknowledgement before linked install/update continuation.
- [ ] Configuration, all three locales, Shift+A, help, dry-run, missing-tool guidance, persistence recovery, and rollback paths are tested.
- [ ] At least two qualifying implementation-worker outcomes, central integration evidence, two independent fresh-context reviews, finding dispositions, and the final HTML report are recorded here.
- [ ] Required project validation passes after implementation; Wave 0 pending red-contract markers are replaced with executable adversarial assertions and made to pass by their owning workstreams.
- [ ] After verified completion, this plan is moved to `plans/archive/pi-agent-aur-scanner.md`.

## Scope

### In scope for v1

- Installed foreign/AUR discovery through existing Pacsea/arch-toolkit seams.
- Manual mapping to canonical official AUR repositories when `.SRCINFO` proves membership.
- Package-name to package-base resolution and split-package grouping.
- Official per-package AUR Git observation and immutable recipe acquisition.
- Direct `.SRCINFO`-declared HTTPS/static and `git+https` upstream acquisition.
- In-process, entry-by-entry bounded archive inspection.
- Isolated GnuPG signature verification using exact fingerprints from keys.openpgp.org.
- Pi subprocess integration through strict LF-delimited RPC JSONL.
- Single, multi-package, update-candidate, installed-provenance, current-HEAD baseline, and changed-recipe scans.
- Persisted queue, ledger, baseline, budget, consent, pricing, and typed result metadata.
- A top-level native Pi Scan workspace and linked install/update acknowledgement.

### Deferred from v1

- External-watcher spool integration and sister-repository WSE changes.
- `heads.tsv`/`heads.tsv.gz` legacy import.
- Arbitrary custom recipe Git URLs.
- FTP, plain HTTP, SSH, `git://`, SVN, Mercurial, Bazaar, local/file sources, magnet links, or arbitrary DLAGENT behavior.
- Sandboxed `makepkg`, PKGBUILD execution, building, installing, or automatic remediation.
- Exact proof that scanned source produced installed bytes.
- Pi/model scans while Pacsea is not running.
- Windows runtime execution; Windows compilation remains required.
- README/wiki changes unless separately requested.

## Approved decisions and invariants

### Product and workflow

1. **Pi boundary:** user-selected host Pi, constrained by Pacsea-owned read-only tools.
2. **Recipe trigger:** ledger every commit; paid scans run for build-relevant or uncertain changes. Documentation/CI/`.gitignore`-only commits become `ObservedNoRecipeDelta`.
3. **Install behavior:** findings are advisory. High/critical and stale results require explicit result-bound acknowledgement, never automatic approval or hard model-driven authorization.
4. **Workspace:** dedicated `AppMode::PiScan`; Shift+A opens it from Search Normal mode. Landing is context-sensitive.
5. **Observation:** native-only v1 uses official `https://aur.archlinux.org/<pkgbase>.git`, startup/manual observation, and a 15-minute periodic floor. Head queries are sequential.
6. **Execution:** one Pi process and one active package scan at a time. The background runner is independent of observation and starts at most five unattended jobs per rolling hour.
7. **Model fallback:** an explicitly confirmed ordered fallback may use in-session RPC `set_model`, with at most three model attempts per logical scan.
8. **Multi-model results:** schema-valid findings are unioned by exact evidence fingerprint. Exact duplicates collapse, model assessments remain attributed, disagreement stays visible, and highest severity controls acknowledgement.
9. **Credentials:** Pacsea never accepts, stores, logs, or forwards provider secrets. Scanner Pi uses standard Pi login/auth state only; env/command-resolved custom auth is unsupported.
10. **Readiness:** readiness failures are prominent warnings but do not block manual or unattended scans. Strict per-scan validation remains mandatory. This is an accepted residual risk.
11. **Completion wording:** `Complete — no findings in analyzed scope`; never `safe`, `clean`, `trusted`, or `passed`.
12. **Feature gating:** compiled by default, runtime default-off (`pi_scan_enabled = false`), Arch/Linux runtime only.

### Source acquisition and integrity

1. **Provider:** direct immutable `.SRCINFO` acquisition.
2. **Transports:** static HTTPS and `git+https` only. All others are unsupported and force `Incomplete`.
3. **Archives:** raw files, tar, tar.gz/tgz, tar.bz2, tar.xz/txz, tar.zst, and ZIP Stored/Deflate.
4. **Extraction:** in-process Rust entry iteration only. Never use broad archive `unpack()` helpers.
5. **Entries:** materialize only normalized directories and regular files. Safe links are metadata-only; unsafe/dangling/escaping links, duplicates, path conflicts, devices, FIFOs, sockets, absolute paths, traversal, or unknown entry types make the snapshot incomplete.
6. **`noextract`:** supported archives may be unpacked in an isolated analysis-only copy; differences from real `prepare()` behavior are explicit. Pacsea never executes `prepare()`.
7. **Transitive content:** fetch only immutable `.SRCINFO` declarations. Do not follow `.gitmodules`, lockfiles, installers, scripts, or model-requested URLs.
8. **Checksums:** require at least one matching SHA-256/SHA-384/SHA-512/BLAKE2 digest for static remote sources. Mismatch is `Failed`; missing, `SKIP`, or weak-only checksums are `Incomplete` unless required signature verification succeeds under policy.
9. **Signatures:** when signature plus `validpgpkeys` is declared, verification is mandatory. Bad signature/fingerprint is `Failed`; unavailable verification is `Incomplete`.
10. **Keys:** exact full-fingerprint HTTPS retrieval from `https://keys.openpgp.org/vks/v1/by-fingerprint/<FINGERPRINT>` only; private seven-day cache; isolated `gpg`/`gpgv` home/keyring; no ambient trustdb, agent, keyserver, or config.
11. **VCS identity:** only explicit full commit OIDs may be complete. Tags, branches, and unqualified HEAD are resolved for advisory scanning but remain incomplete and are re-resolved for staleness.
12. **Cache:** no persistent upstream source/archive/VCS cache. Workspaces are private and ephemeral. Persist only manifests, provenance, bounded evidence, and scan state.
13. **Network:** public Internet destinations only; DNS answers and every redirect are checked against non-public ranges. Follow at most five HTTPS redirects, never downgrade, never accept URL userinfo, and record redirect provenance.
14. **Proxy/TLS:** no ambient proxy inheritance. Optional explicit credential-free HTTPS proxy only. System TLS trust store only; no custom CA or insecure mode.
15. **HTTP implementation:** upstream source downloads use bounded `reqwest` streaming. This does not weaken `curl_args()`; every actual curl invocation elsewhere still uses the shared helper and its 10 MiB cap.

### Security/process invariants

- Direct argv only; no new `bash -c`, AUR helper, `makepkg`, shell interpolation, or PKGBUILD/source execution.
- Launch Pi from a neutral empty private directory with:

  ```text
  --mode rpc --no-session --offline --no-builtin-tools --no-extensions
  --no-skills --no-prompt-templates --no-context-files --no-themes
  --no-approve -e <trusted-pacsea-scan-extension>
  --tools pacsea_scan_read,pacsea_scan_grep,pacsea_scan_find,pacsea_scan_ls
  ```

- `--offline` disables Pi startup updates/telemetry; provider model calls remain the selected model transport.
- Pi 0.84.0 still exposes its temporary inline `/llama` command under these flags. Slash commands are RPC-client inputs, not model-callable tools; Pacsea never constructs a slash command from package content. The capability probe rejects user/project command sources and any active tool beyond the four Pacsea tools, while allowing inventoried Pi-owned temporary inline commands that add no tool authority.
- Set `PI_OFFLINE=1`, `PI_TELEMETRY=0`, and `PI_SKIP_VERSION_CHECK=1`; pass a bounded environment and no proxy, SSH-agent, Git-credential, sudo-password, VirusTotal, or provider-key variables.
- Minimum Pi version is 0.84.0. Capability/protocol probes, not a hard maximum, are authoritative.
- Embed the trusted extension, materialize it atomically mode 0600 under a private mode-0700 runtime directory, and verify its compiled asset hash before launch.
- Snapshot roots are supplied through a private descriptor controlled by Pacsea, never model input.
- Restricted tools reject absolute/traversal/control paths, symlink escapes, special files, root replacement, oversized requests/results, and unsupported encodings.
- `pacsea_scan_grep` is bounded literal substring search only; no model-supplied regex.
- Pi internal auto-retry remains enabled but is capped at three provider attempts per low-level request and included in reservation/provenance.
- Cancellation stops the whole logical scan, suppresses correction/fallback, sends RPC abort, waits five seconds, then terminates the Unix process group and reaps it.
- Unix process groups use safe `Command::process_group(0)` plus `nix::sys::signal::killpg`; no unsafe `pre_exec`/libc path when safe APIs suffice.
- Model output is strict duplicate-key-free JSON. Reject trailing objects, mismatched identity, absolute paths, unknown enums, excessive nesting/count/length, tool-call payloads, fabricated evidence, terminal controls, or multiple final answers.
- One bounded correction is allowed. Repeated contract failure may trigger configured fallback; user cancellation never does.
- No raw prompt, source body, thinking, invalid response, or original assistant response is persisted. Raw view is canonical serialization of validated typed data.

## Authoritative resource bounds

Compiled hard maxima may be lowered by settings but never raised in v1.

| Resource | Hard maximum/default |
| --- | ---: |
| Compressed download / source | 100 MiB |
| Compressed download / package+commit | 250 MiB |
| Expanded regular-file bytes | 256 MiB |
| Archive entries | 10,000 |
| Archive path depth | 16 |
| Expanded/compressed ratio | 10:1 |
| Fully analyzable text / file | 16 MiB |
| Pi tool calls / model attempt | 250 |
| Tool-result text / model attempt | 16 MiB |
| `read` / call | 64 KiB |
| `grep` / call | 200 matches / 128 KiB |
| `find` or `ls` / call | 500 entries / 128 KiB |
| Model-attempt wall time | 5 min |
| Logical-scan wall time | 12 min |
| RPC abort grace | 5 s |
| Total app shutdown deadline | 10 s |
| Final JSON / model attempt | 4 MiB |
| Findings / model attempt | 500 |
| Evidence/rationale/recommendation field | 4 KiB each |
| Model attempts / logical scan | 3 |
| Provider retry attempts / low-level request | 3 |
| Head query / observation cycle | 15 s / 90 s, sequential |
| Observation floor | 15 min |
| Commit expansion | 500/package, 2,000/cycle, resumable |
| Unattended job starts | 5 / rolling hour |
| Unattended token cap | 500,000 / rolling 24 h |
| Unattended paid-cost cap | $0.00 default |
| Redirects | 5, HTTPS-only |
| Signing-key cache | 7 days |
| Concurrent Pi processes | 1 |

Token fallback estimate is `ceil(all Pi request/response UTF-8 bytes / 2) + 8,000` per scan reservation. Unknown usage after a crash consumes the full reservation for the rolling 24-hour window.

## Coverage and result semantics

A result may be complete only when:

- every entry is byte-hashed in canonical sorted manifests;
- deterministic classification/search covers every eligible text file within limits;
- Pi inspects every AUR recipe file, changed file, executable/script, detector hit, and declared entry point;
- remaining manifest-only files are explicitly reported under the bounded risk-prioritized policy;
- no security-relevant/unknown binary, unsupported encoding, unsupported source, dynamic mutable source, limit violation, stale identity, or required verification gap remains.

Known non-executable assets may be manifest-only when not build/runtime-relevant. Security-relevant or unknown binaries force `Incomplete`. Text analysis supports strict UTF-8/ASCII and BOM-marked UTF-16 while hashes bind original bytes.

Deterministic findings remain separately attributed and cannot be downgraded or suppressed by a model. User benign verdicts carry forward only when detector ID/version, package base, path, evidence fingerprint, and manifest entry hash all match exactly.

## Identity and observation algorithm

1. Enumerate foreign packages through `list_foreign_packages()` and resolve package bases through AUR metadata.
2. For a manual official-AUR mapping, canonicalize the URL and require immutable `.SRCINFO` membership. Revalidate while installed and on every observed HEAD.
3. Query official per-package AUR HEADs sequentially at startup/manual refresh and eligible 15-minute intervals.
4. Maintain independent accepted comparison baseline, last observed cursor, and OID-keyed queue/terminal ledger.
5. Expand unseen history topologically oldest-first, at most 500/package and 2,000/cycle. Advance only through durably inserted commits and resume later; never coalesce.
6. Classify build relevance. Ledger no-delta/non-build commits without Pi; queue build-relevant or uncertain commits.
7. On non-ancestor/force-push history, preserve old lineage and pause for explicit rebaseline.
8. Freeze candidate `{package_name, package_base, installed_version, candidate_version, observed_head_oid, cycle_id}` inside the update cycle. Never reconstruct identity from `available_updates.txt`.
9. If HEAD changes before scan, scan the frozen reachable commit, mark it stale, and queue the new commit separately.
10. Installed-version equality does not prove provenance. Ambiguous installed scans use the newest matching commit, remain incomplete, and may not advance baseline.
11. A separate explicit complete current-HEAD scan may establish an observation baseline.
12. Before linked continuation, re-resolve AUR HEAD and mutable source refs. Changed identity invalidates prior acknowledgements.

## Background and budget state

- Observation remains active after feature consent even when paid background model execution is off.
- The sequential runner processes continuously while Pacsea is open, subject to explicit pause, five starts/hour, rolling token/cost budgets, auth/readiness/pricing availability, and one active process.
- New installed package bases auto-scan only after explicit background consent and a usable budget; otherwise they queue as `Unbaselined` and notify.
- Foreground manual requests run next but do not preempt an active scan. Manual scans may bypass unattended caps after worst-case confirmation and remain separately accounted.
- User pause persists across restarts and only the user clears it. Budget pauses automatically resume after revalidation; service/security pauses require their own successful checks.
- Eligible pre-model transient acquisition/startup failures get one retry after one minute. Once model usage may have occurred, automatic Pacsea-level retry stops.
- An interrupted active item becomes `Interrupted`, keeps the full reservation, and requires manual retry; other queue work may continue.
- App shutdown has a ten-second total abort/kill/reap/persist deadline and leaves a recovery marker if durability is uncertain.

## Pricing and model selection

Primary pricing source is Pi `Model.cost`; post-scan accounting uses `get_session_stats` when reliable. Fallback token accounting uses the conservative byte formula.

- Direct-provider exact model pricing may use LiteLLM's structured cost map.
- OpenRouter-routed pricing uses `https://openrouter.ai/api/v1/models` only.
- Exact provider/model/route matches only; no fuzzy substitution.
- Refresh weekly. When cached data is older than seven days, attempt refetch; if refetch fails, the user-approved policy allows stale cached pricing to continue with explicit stale labeling. Token caps remain authoritative.
- Explicitly recognized subscription routes are dollar-accounted as zero but remain token-bounded and labeled subscription-backed, not free API usage.
- Default thinking is `medium`, adjusted only downward for unsupported levels.
- Setup preselects the cheapest eligible distinct fallback in the same privacy class after worst-case reservation comparison; user confirmation is mandatory. Tied zero-cost candidates require user choice.
- Custom endpoints are classified `Local` (loopback/Unix socket), `Private network` (literal RFC1918/ULA), or `Remote` (everything else/custom hostnames by default).
- Material model/provider/privacy/background/pricing/prompt/tool-version changes invalidate consent and require re-confirmation.

## Persistence

Persist under Pacsea's config/cache path helpers, honoring config-directory overrides:

```text
pi_scan/baseline-v1.json
pi_scan/backlog-v1.json
pi_scan/budget-v1.json
pi_scan/consent-v1.json
pi_scan/pricing-v1.json
pi_scan/results-v1/<package-base>/<scan-id>.json
pi_scan/quarantine/<type>-<timestamp>-<sha256>.json
```

No persistent source snapshot directory exists.

Writes are atomic, private, and crash-aware. A dedicated versioned loader distinguishes missing, corrupt, unsupported-newer, and I/O failure. Corrupt/newer state is moved without replacement into private quarantine and never interpreted as empty or clean. Quarantine failure leaves the original untouched and scanner state unavailable.

Retention keeps the newest detailed result and the current accepted baseline result when different; other superseded results follow 30-day cleanup. Cleanup runs only after successful load and atomic commit. Quarantine artifacts are never automatically deleted. Recovery offers retry/show-path, package-scoped reset, and guarded global reset.

## Pi RPC contract

### Capability probe

- Resolve the absolute executable once.
- Require version >=0.84.0.
- Verify every required flag.
- Launch a neutral, no-session, offline RPC smoke process with only an explicit probe extension and allowlisted tools.
- Use an extension command to report `pi.getActiveTools()` over RPC without a model call; require the exact four-tool allowlist, reject user/project command sources, and inventory Pi-owned temporary inline commands separately.
- Treat slash commands as trusted RPC-client control input only. Pacsea-owned prompts must never begin with or derive a slash command from hostile package/source content.
- Verify strict LF framing, command IDs, `get_state`, `get_available_models`, `set_model`, `set_auto_retry`, `abort_retry`, `abort`, `agent_settled`, `get_last_assistant_text`, and `get_session_stats` before enabling the corresponding runtime behavior.
- Fail closed as `Unavailable` on any missing or mismatched capability.

### Restricted tools

The embedded extension registers only:

- `pacsea_scan_read(snapshot, relative_path, offset?, limit?)`
- `pacsea_scan_grep(snapshot, literal, case_sensitive?, globs?, max_matches?)`
- `pacsea_scan_find(snapshot, glob, max_results?)`
- `pacsea_scan_ls(snapshot, relative_path?, max_entries?)`

Tool roots come from a private descriptor. The extension exposes no shell, write, edit, arbitrary URL, process, environment, UI, absolute-path, or ambient-resource capability.

### Scan flow

1. Send versioned hostile-data and package prompts containing bounded identity/coverage summaries, not full source bodies/manifests.
2. Let the model inspect immutable roots through restricted tools within attempt budgets.
3. Wait for `agent_settled`; fetch final assistant text; validate one strict JSON object and exact evidence.
4. Allow one bounded correction.
5. On eligible model/provider/output-contract failure, `set_model` to the next confirmed fallback in-session and run a fresh full validation pass.
6. Merge schema-valid findings deterministically under the approved multi-model policy.
7. Query statistics, reconcile reservations, terminate/reap, and clean workspaces.

## Dry-run behavior

Dry-run may perform read-only official AUR Git observation/acquisition, `.SRCINFO` upstream download, temporary key retrieval, checksums, signatures, extraction, and manifesting to produce a realistic preview. It must not:

- launch Pi or run model readiness/pricing calls;
- mutate durable queue, ledger, baseline, result, consent, pricing, or budget state;
- retain downloaded sources, key bodies, repositories, or workspaces.

Dry-run output lists selected targets, limits, identities obtained, verification/coverage, disclosures, and the Pi process that would be launched.

## Settings

Conservative v1 defaults include:

```text
pi_scan_enabled = false
pi_scan_background_enabled = false
pi_scan_binary = pi
pi_scan_provider =
pi_scan_model =
pi_scan_fallback_models =
pi_scan_thinking = medium
pi_scan_observation_interval_seconds = 900
pi_scan_head_query_timeout_seconds = 15
pi_scan_observation_deadline_seconds = 90
pi_scan_model_attempt_timeout_seconds = 300
pi_scan_logical_timeout_seconds = 720
pi_scan_background_starts_per_hour = 5
pi_scan_background_token_cap_24h = 500000
pi_scan_background_cost_cap_24h = 0.00
pi_scan_result_retention_days = 30
pi_scan_show_raw_output = false
pi_scan_https_proxy =
```

Do not add API-key, external-watcher, head-source, external-event-dir, external-health, or legacy-import settings. Security maxima may be lowered but not raised; invalid higher values are actionable configuration errors and both compiled/effective values are displayed.

## Implementation architecture

### New files

```text
src/state/pi_scan.rs
src/logic/pi_scan/{mod,identity,manifest,baseline,head_source,recipe,source,detectors}.rs
src/logic/pi_scan/{prompt,result,pricing,budget}.rs
src/integrations/pi_agent/{mod,capabilities,protocol,process,restricted_tools}.rs
src/integrations/pi_agent/assets/pacsea-scan-tools.ts
src/app/runtime/workers/pi_scan.rs
src/events/pi_scan/{mod,keys}.rs
src/ui/pi_scan/{mod,setup,targets,progress,results,details}.rs
tests/pi_scan.rs
tests/pi_scan/{mod,fixtures,capability_probe,security_boundary,dependency_benchmarks}.rs
tests/pi_scan/assets/pacsea-probe.ts
```

Final module names may change to satisfy size/complexity lints. There is no `legacy_import.rs`, spool module, or persistent snapshots directory in v1.

### Shared files likely modified

- `Cargo.toml`, `Cargo.lock`, `deny.toml` only when justified by selected dependencies/features.
- `src/integrations/mod.rs`, `src/state/{types,mod}.rs`, minimal `AppState` roots.
- `src/app/runtime/{channels,event_loop,tick_handler,cleanup}.rs`, update payload seam.
- `src/theme/**`, `src/state/config_editor.rs`, `config/settings.conf`, `config/keybinds.conf`, all locales.
- Top-level renderer/event dispatch, contextual package/update/preflight handlers, help/footer/mouse targets.
- Existing foreign-package and update logic only through adapters; no duplicate command construction.
- Existing branch PR record under `dev/PR/`.

## Execution DAG and workstreams

The integration owner alone updates this plan, shared state/interfaces, integration order, review dispositions, and completion claims.

```text
Wave 0 plan/dependencies/red contracts/probes
       │
       ├── WS1 recipe/source identity, acquisition, manifests, baseline, detectors
       └── WS2 Pi RPC, process lifecycle, restricted tools, prompt/result
                    │
                    ▼
             Central integration A
                    │
          ┌─────────┴─────────┐
          ▼                   ▼
 WS3 runtime/observer/queue  WS4 TUI/settings/workflow
          └─────────┬─────────┘
                    ▼
             Central integration B
                    │
                    ▼
          WS5 hardening/acceptance
                    │
                    ▼
 two fresh independent reviews + dispositions + fixes
                    │
                    ▼
 final HTML report and plan archive
```

### Wave 0 — Freeze decisions and create evidence

**Owner:** Integration owner  
**Purpose:** prevent workers from inheriting contradictory or unsafe draft behavior.

- [x] Record branch/base and dirty state.
- [x] Resolve DG-1 and first-release scope through the Grill Me record.
- [x] Verify installed Pi 0.84.0 exposes the required flags.
- [x] Collision-check and select Shift+A.
- [x] Select exact dependency versions/features and resolve the graph.
- [x] Run audit, deny, duplicate, unsafe/license checks; record accepted exceptions.
- [x] Add a deterministic ignored benchmark harness and benchmark approved maxima in release mode with peak-RSS evidence.
- [x] Add ignored pending red-contract markers for injection, identity, corrupt state, traversal/symlink escape, outside-root read, framing, output validation, extension integrity, cancellation, dry-run durable-state isolation, and stale HEAD.
- [x] Add deterministic CLI/environment contract tests and run the ignored live no-model Pi capability smoke probe.
- [x] Confirm the explicit extension exposes exactly four active tools while user/project ambient resources and commands remain disabled; record Pi's temporary inline command exception.
- [x] Update the branch PR with exact commands/outcomes and residual risks.

**Pending-marker contract:** Wave 0 markers compile, are ignored by the normal suite, and fail explicitly with named missing owner-workstream behavior rather than a compile/setup error. They preserve ownership and acceptance wording before production seams exist; they are not regression tests. Each owner must replace its marker body with executable adversarial inputs/assertions before removing the ignore, then make those assertions pass incrementally.

**Benchmark contract:** use ignored release-mode integration measurements rather than adding Criterion. Record fixture sizes/hashes, elapsed time/throughput, peak RSS via an external runner (`/usr/bin/time -v` when available, otherwise a documented `/proc/<pid>/status` sampler), CPU/OS/rustc, resolved graph, and command lines. Benchmarks are decision evidence, not CI timing thresholds.

**Acceptance:** plan and PR are current; normal project validation remains green; explicit marker run proves named pending boundaries; live capability probe passes without a model call and with a positive environment allowlist; dependency/audit/benchmark evidence is recorded. No existing scanner or user-facing runtime path changes.

### WS1 — Recipe/source identity and deterministic domain

**Worker A ownership:** `src/logic/pi_scan/{identity,manifest,baseline,head_source,recipe,source,detectors}.rs` and focused tests. No shared AppState/channels/UI/settings/plan/PR edits.

Deliverables:

- strict package-base/manual-AUR mapping validation;
- hardened official AUR Git acquisition and every-commit range expansion;
- build-relevance classifier and terminal ledger semantics;
- bounded HTTPS/git source provider, integrity verification, archive entry iteration, canonical manifests;
- deterministic detectors and exact evidence fingerprints;
- separate baseline/cursor/ledger schemas, quarantine-aware atomic persistence, split-package dedupe;
- installed provenance and current-HEAD baseline workflows.

**Handoff:** `.pi-subagents/handoffs/pi-aur-scanner/ws1-identity-source.md`

### WS2 — Pi RPC, lifecycle, restricted tools, prompt/result

**Worker B ownership:** `src/integrations/pi_agent/**`, `src/logic/pi_scan/{prompt,result}.rs`, embedded asset, focused fake/live tests. No shared AppState/channels/UI/settings/plan/PR edits.

Deliverables:

- strict bounded LF JSONL protocol and command correlation;
- capability probe, neutral direct-argv startup, bounded environment, process-group abort/kill/reap;
- private embedded extension and path-confined literal-search tools;
- prompt/schema/evidence validation, correction, set-model fallback, multi-model merge, usage/provenance;
- fake-Pi tests and ignored live smoke test.

**Handoff:** `.pi-subagents/handoffs/pi-aur-scanner/ws2-pi-bridge.md`

### Central integration A — Domain/adapter contract

**Owner:** Integration owner

- Inspect both worker diffs/handoffs and reject out-of-bound edits.
- Re-export cohesive state/interfaces without leaking Pi wire types into UI state.
- Resolve shared Cargo/features and process mechanisms centrally.
- Run focused checks and full required project validation.

### WS3 — Runtime observer, queue, budgets, and persistence

**Worker C ownership:** `src/app/runtime/workers/pi_scan.rs`, dedicated queue helpers, focused runtime tests. Shared channel/update/tick/event-loop changes are proposals applied centrally unless explicitly sequenced as sole writer.

Deliverables:

- typed request/result/progress/cancel/shutdown channels;
- sequential persistent `(package_base, commit_oid)` queue and stale-response guards;
- native sequential head observation and resumable every-commit discovery;
- independent continuous runner, hourly starts, rolling token/cost reservations, retry/pause/recovery semantics;
- explicit durability boundaries, shutdown acknowledgement, retention, and quarantine recovery.

**Handoff:** `.pi-subagents/handoffs/pi-aur-scanner/ws3-runtime.md`

### WS4 — Workspace, setup, settings, and contextual UX

**Worker D ownership:** `src/events/pi_scan/**`, `src/ui/pi_scan/**`, settings/schema/editor/config/locales/help tests. Shared AppState/top-level dispatch/contextual handlers are proposals applied centrally.

Deliverables:

- `AppMode::PiScan`, cohesive workspace state, Setup/Overview/Targets/Progress/Results;
- context-sensitive landing, Shift+A, top-bar combined status, persistent typed notifications;
- model/fallback/privacy/pricing/readiness consent and missing-tool guidance;
- manual/background confirmations, high/critical and stale acknowledgement;
- dry-run preview, help, mouse, keybind, locale/config coverage.

**Handoff:** `.pi-subagents/handoffs/pi-aur-scanner/ws4-tui.md`

### Central integration B and WS5

Integration owner sequences shared runtime/UI edits, validates combined state, and updates plan/PR. WS5 then adds adversarial acquisition, archive, RPC, environment, output, persistence, budget, fallback, pricing, cancellation, dry-run, and TUI acceptance coverage without reusing unsafe existing scanner paths.

**WS5 handoff:** `.pi-subagents/handoffs/pi-aur-scanner/ws5-hardening.md`

## Acceptance inventory

### Unit/domain

- Package/base grammar, manual official-AUR mapping, split groups, version/epoch/provenance ambiguity.
- Canonical manifests, modes/hashes/order, encodings, binaries, limits, corruption/quarantine.
- HTTPS/DNS/redirect/proxy/checksum/signature/key-cache/archive traversal/link/duplicate/special-file policy.
- Every-commit ordering, build relevance, divergence, cursor/baseline independence, first baseline.
- Detector/evidence fingerprints, benign verdict identity, model omission resistance.
- Prompt determinism, multi-model merge, duplicate-key and exact-evidence validation.
- Rolling reservations, byte estimation, stale pricing, subscription accounting, pause/resume/recovery.

### Process/security

- Fake Git direct argv, isolated environment, approved host/protocols, no hooks/submodules/textconv/credentials.
- Fake Pi exact flags/environment/CWD, no session/ambient resources/provider-key variables, strict framing and bounded output.
- Live no-model Pi probe reports the exact four active tools, rejects user/project command sources, and inventories any Pi-owned temporary inline command separately.
- Restricted tools reject outside-root access, traversal, symlink/root replacement, controls, special/oversized files, regex patterns.
- Timeout/forking child is group-killed and reaped; shutdown completes within ten seconds.
- Malicious source prompt injection cannot gain shell/write/network/host-read authority.
- Invalid model output, evidence, identity, controls, multiple objects, and oversize are rejected.

### Runtime/integration

- One/many packages, split dedupe, candidate frozen commit/source, contextual/manual/background flows.
- Native observation, every-commit resumable expansion, build-relevant queueing, no silent coalescing.
- Five starts/hour, rolling budgets, reservations, exact/manual bypass, transient retry, persisted pause, crash recovery.
- Model fallback/set-model/merge/cost/provenance and cancellation suppression.
- Feature/background disabled behavior, dry-run acquisition with zero durable mutation/Pi launch, missing tools/network/auth.
- Stale responses/HEAD/source refs cannot validate or overwrite the wrong result.

### TUI

- Setup and consent invalidation; readiness warning; model/fallback tie selection; pricing staleness.
- Baseline/unbaselined/targets/filter/select-all; detach/reopen/pause/cancel/retry/status.
- Coverage/limitations/findings/disagreement/raw canonical rendering and inert controls.
- High/critical and stale continuation acknowledgement.
- Shift+A, top-bar combined state, persistent notifications, mouse, help, config editor, all locales.

## Validation and completion gates

After every integrated change, from repository root, in order:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

After dependency changes:

```bash
cargo audit
cargo deny check
cargo tree -d
cargo tree -e features
```

Additional final checks:

```bash
cargo build --locked --release
cargo test --locked --release -- --test-threads=1
cargo test complexity -- --nocapture
./dev/scripts/security-check.sh
```

Optional local evidence tools (`cargo geiger`, `cargo license`) are run when installed; absence is recorded rather than silently replaced. All new items require rustdoc; non-trivial APIs use What/Inputs/Output/Details. New functions remain below cognitive complexity 25 and 150 lines unless a narrowly justified allow is documented.

## Independent review gate

After integration and full validation, obtain two fresh-context read-only reviewer runs from distinct provider families when available:

1. Correctness/UX: requirements, state transitions, queue/update semantics, persistence, tests, maintainability, TUI behavior.
2. Security: acquisition, dependencies, restricted tools, Pi flags/environment, secrets/privacy, process limits, manifests, TOCTOU, output parsing, advisory wording.

Unique outputs:

```text
.pi-subagents/handoffs/pi-aur-scanner/review-correctness-ux.md
.pi-subagents/handoffs/pi-aur-scanner/review-security.md
```

Every finding records run/provider/model, file/symbol, requirement/failure mode, evidence, severity, and exactly one disposition: `accepted`, `rejected`, `deferred`, or `needs verification`. Only independently verified accepted findings enter a fix pass; rerun affected and full checks afterward.

## Rollback

- Default-off capability boundary gates every trigger.
- Domain, Pi adapter, runtime, and UI land in separable waves.
- If runtime/security probes fail, disable the entire Pi path while preserving backward-readable metadata.
- If background execution is unstable, disable only background model execution; observation/manual scanning may remain.
- Rollback never reinterprets newer/corrupt state as empty or clean and never deletes quarantine automatically.
- Existing scanners remain untouched and restorable.

## Risks and accepted limitations

| Risk | Severity | Mitigation/status |
| --- | --- | --- |
| Host Pi and explicit extension run with user OS permissions | High | Exact extension hash, neutral CWD, bounded env, no built-in tools or user/project resources, exact active-tool probe, and inventory of Pi-owned temporary inline commands. A compromised Pi binary remains an accepted optional-dependency boundary. |
| Prompt injection / model false negatives | High | No shell/write/network tools, deterministic layer, strict validation, inert advisory UI. Residual risk remains. |
| Snapshot differs from installed bytes | High | Exact manifests/commits, separate installed provenance/current-HEAD baseline, stale checks, explicit limitation. |
| Readiness-failed models may run unattended | High | Prominent warning and strict per-scan validation; accepted user decision. |
| Stale cached price may continue after refetch failure | Medium-High | Explicit stale label and authoritative rolling token cap; accepted user decision. |
| In-session multi-model continuation | Medium-High | Max three models, full provenance/accounting, deterministic union/disagreement, extensive fake-Pi tests. |
| Archive/decoder supply chain and decompression bombs | High | Exact pins/features, audit/deny/license/unsafe review, entry-by-entry bounds, release benchmarks. |
| Dry-run performs network reads | Medium | Explicit disclosure, no Pi/durable mutation, private ephemeral cleanup; accepted user decision. |
| Every-commit scanning creates backlog/cost | Medium | Sequential runner, resumable discovery, hourly/rolling budgets, pause/cancel/status. |
| AUR history rewrite/deletion | Medium | Preserve lineage, pause, explicit rebaseline; never fabricate continuity. |
| Pricing catalogs drift | Medium | Exact route matching, weekly refresh, provenance, stale label, token cap. |
| Large/unsupported source cannot be fully analyzed | Medium | Explicit `Incomplete`, no silent truncation, manifest/coverage details. |

## Wave 0 evidence record

### Repository baseline

- Branch: `feat/aur-scan-integrated`
- HEAD/base/origin-main: `ad3e692a8ecdd690494c480dbb7e459955283fe5`
- Initial state: no tracked diff; untracked `.pi/`, `GRILL-ME-pi-agent-aur-scanner.md`, and this plan.
- Existing PR record: `dev/PR/PR_feat-aur-scan-integrated.md`

### Pi documentation and live evidence

- Installed executable: `/home/firstpick/.npm-global/bin/pi`
- Installed version: `0.84.0`
- Required flags observed in `pi --help`: RPC, no-session, no-builtin-tools, exact tool allowlist, explicit extension, all resource disables, no-approve, offline.
- Official docs reviewed: `README.md`, `docs/rpc.md`, `docs/models.md`, `docs/security.md`, `docs/json.md`, `docs/containerization.md`, `docs/extensions.md`, `docs/environment-variables.md`.
- Pi security documentation explicitly states project trust is not a sandbox and prompt injection from untrusted content is expected risk; the restricted process/tool boundary is therefore mandatory.

### Selected dependency record

Wave 0 resolved and verified these exact direct pins:

```toml
tar = { version = "=0.4.46", default-features = false }
flate2 = { version = "=1.1.9", default-features = false, features = ["rust_backend"] }
bzip2 = { version = "=0.6.1" }
lzma-rust2 = { version = "=0.18.1", default-features = false, features = ["std", "xz"] }
zstd = { version = "=0.13.3", default-features = false }
zip = { version = "=8.6.0", default-features = false, features = ["deflate-flate2"] }
sha2 = { version = "=0.11.0", default-features = false }
blake2 = { version = "=0.10.6", default-features = false }
```

Tokio now enables `io-util`/`process`, Reqwest enables `stream`, and Nix enables `process`/`signal`. `zstd-sys 2.0.16+zstd.1.5.7` is the intentional bundled-C/FFI exception. `bzip2 0.6.1` uses the pure-Rust `libbz2-rs-sys 0.2.5` backend; its permissive `bzip2-1.0.6` license was reviewed from the shipped license text and added to `deny.toml`. Production `lzma-rust2` keeps default features disabled and selects only `std,xz`, so its unsafe optimization feature is not selected; tests additionally enable its pure-Rust `encoder` feature to generate deterministic XZ benchmark input without a host CLI. ZIP internal bzip2/xz/zstd/deflate64 remain unsupported.

Resolved feature evidence confirms Rust-backed Flate2/Deflate, pure-Rust bzip2, XZ without unsafe optimization, ZIP Stored/Deflate only, and the single new native-link surface at `zstd-sys`. `cargo tree -d` reports an accepted Digest 0.10/0.11 split (`blake2 0.10.6` versus `sha2 0.11.0`/`lzma-rust2`) plus pre-existing project duplicates. No stable BLAKE2 release on Digest 0.11 was selected merely to remove that bounded duplication.

### Audit and compatibility evidence

Commands and outcomes:

- `cargo audit`: exit 0; 465 locked packages scanned; no vulnerability advisory. It reports one allowed, pre-existing unmaintained warning for `bincode 1.3.3` through `syntect 5.3.0`.
- `cargo deny check`: initially rejected the unlisted `bzip2-1.0.6` license; after direct license review and the narrow allow-list addition, advisories/bans/licenses/sources all pass. Duplicate-version warnings remain non-fatal under repository policy.
- `cargo tree -d` and per-crate `cargo tree -e features -i ...`: completed; findings are summarized above.
- `cargo geiger` and `cargo license` were not installed and were not globally installed without authorization. A targeted registry-source/native-link scan was used as supplemental evidence, not as a claim that transitive unsafe code is absent.
- `cargo +1.91 check --test pi_scan` and `cargo +1.91 test --test pi_scan -- --test-threads=1`: pass on the repository's minimum supported Rust toolchain.
- `./dev/scripts/security-check.sh`: 5/5 pass (`rustfmt`, Clippy, audit, deny, gitleaks).

Accepted supply-chain residuals are the bundled `zstd-sys` C/FFI boundary, wrapper/internal unsafe in parts of the selected transitive graph, the bounded Digest duplication, and the pre-existing allowed `bincode` maintenance warning. These remain subject to implementation review and hard decompression limits.

### Benchmark evidence

The release test executable was selected from Cargo JSON output, run directly, and sampled every 10 ms. This is the exact external sampler shape used because GNU `/usr/bin/time` is unavailable:

```bash
json=$(mktemp)
cargo test --release --test pi_scan --no-run --message-format=json >"$json"
bin=$(python3 - "$json" <<'PY'
import json, sys
items = [json.loads(line) for line in open(sys.argv[1], encoding="utf-8")]
print([item["executable"] for item in items
       if item.get("reason") == "compiler-artifact"
       and item.get("target", {}).get("name") == "pi_scan"
       and item.get("executable")][-1])
PY
)
log=$(mktemp)
"$bin" wave0_dependency_benchmark_at_approved_bounds --ignored --nocapture --test-threads=1 >"$log" 2>&1 &
pid=$!
peak_kib=0
while kill -0 "$pid" 2>/dev/null; do
    current=$(awk '/^VmHWM:/ {print $2}' "/proc/$pid/status" 2>/dev/null || true)
    [[ -n "$current" && "$current" -gt "$peak_kib" ]] && peak_kib=$current
    sleep 0.01
done
wait "$pid"; code=$?
cat "$log"
printf 'PI_SCAN_BENCH peak_rss_kib=%s exit=%s sample_interval_ms=10\n' "$peak_kib" "$code"
rm -f "$json" "$log"
exit "$code"
```

Environment: Linux `7.1.6-zen1-1-zen` x86_64, 32 logical CPUs, 60 GiB RAM, `rustc 1.97.1`; compatibility was separately checked on Rust 1.91. The emitted `PI_SCAN_*` values are transcribed without normalization:

| Case | Fixture identity | Timed result |
| --- | --- | --- |
| Dual SHA-256+BLAKE2 hash | generated 16 MiB repeated-byte stream | 0.015612 s; 1024.824 MiB/s |
| Gzip decode | 1,221,557 B; SHA-256 `7cf11290c7e76acd3dda0c2d4f8e6ab7a802c2a4411cb3273cdae30a742da528` | 256 MiB in 0.272087 s; 940.874 MiB/s |
| Bzip2 decode | 138 B; SHA-256 `8f1dd2d1cb5897eca1a37a1991df81813584a480c8b887972945c8cab7f7efa6` | 16 MiB in 0.040984 s; 390.396 MiB/s |
| Zstd decode | 530 B; SHA-256 `da31af0652888f92dcc944e9b21dc24c9d73af628dd84fc63c7d6664b024fe95` | 16 MiB in 0.015795 s; 1012.987 MiB/s |
| XZ decode | 2,576 B; SHA-256 `ccfbf9b68e4c2bb2c94b05f0bec89065ecb1e2f4d7c5e93a2c54a59a64d52843` | 16 MiB in 0.043564 s; 367.279 MiB/s |
| Tar iteration | 143,578 B; SHA-256 `ac85a89878cf8c0b92cd0b891378784aed71ce427cf294b651f824cf96d1dbce` | 10,000 entries in 0.004685 s |
| ZIP iteration | 1,130,022 B; SHA-256 `0d63a7cabc1098168ed901ff449af3f6da29928aa1d77d4ba4fbefaeabc266a1` | 10,000 entries in 0.002247 s |

Observed peak RSS was 12,812 KiB. `VmHWM` and throughput are run/sample dependent; fixture identities are deterministic. Results are local decision evidence, not CI thresholds. Highly compressible adversarial fixtures intentionally exercise expansion accounting; production WS1 must still stop before every hard byte/ratio/entry limit rather than relying on measured performance.

### Pending-contract and Pi capability evidence

- Normal `cargo test --test pi_scan -- --test-threads=1`: three deterministic capability/environment tests pass; 13 Wave 0 live/benchmark/contract markers remain explicitly ignored.
- Explicit `cargo test --test pi_scan wave0_red_ -- --ignored --test-threads=1`: exits 101 with exactly 11 named pending-boundary marker failures covering injection, AUR identity, corrupt state, path/symlink escape, outside-root disclosure, LF framing, model-output/evidence validation, extension asset integrity, process-group cancellation, dry-run isolation, and stale identity. There are no setup or compilation failures. These marker bodies must be replaced, not merely unignored, by their owner workstreams.
- A deterministic child-process test proves `env_clear()` plus the explicit path/state/locale/offline allowlist excludes an injected unlisted secret. This closes ambient future credential-variable drift in the Wave 0 launch probe.
- Explicit live probe: `cargo test --test pi_scan live_pi_rpc_capability_probe_exposes_exact_tools -- --ignored --test-threads=1 --nocapture` passes without a model call on Pi 0.84.0 under that positive environment allowlist. It observes only `pacsea_scan_find`, `pacsea_scan_grep`, `pacsea_scan_ls`, and `pacsea_scan_read` as active tools.
- Discovery during the first live run: Pi 0.84.0 also reports temporary inline `/llama` alongside the explicit `/pacsea-probe-tools` command. The probe rejects user/project sources while allowing inventoried Pi-owned temporary inline commands; commands are not model-callable tools. This exception is explicit rather than silently weakening the exact tool boundary.

### Wave 0 independent review and dispositions

Two fresh-context read-only reviewers from distinct provider families completed successfully. Their artifacts are under `.pi-subagents/artifacts/outputs/3ac3c174-c580-4dcc-ab49-d07fb3485cdb/.../wave0-review-correctness.md` and `.pi-subagents/artifacts/outputs/da612c93-8a37-45c9-8de1-32e7a3390f38/.../wave0-review-security.md`. These interim Wave 0 reviews do not satisfy the final implementation review gate.

| Finding | Disposition | Evidence/action |
| --- | --- | --- |
| `W0-COR-001` / security `NOTE-1`: inherited denylist environment | **accepted** | Replaced with `env_clear()` plus an explicit allowlist; added deterministic injected-secret test; live probe re-passed. |
| `W0-COR-002`: marker panics described as executable tests | **accepted** | Relabeled them as pending contract markers and added a hard gate requiring replacement bodies with adversarial assertions before ignores are removed. |
| `W0-COR-003`: benchmark depended on host `xz` | **accepted** | Enabled `lzma-rust2`'s pure-Rust encoder only for dev/test and now generates XZ in-process; fixture identity and benchmark were regenerated. |
| `W0-COR-004`: sampler command omitted | **accepted** | Recorded the exact direct-executable `/proc` sampler, interval, PID, exit handling, and raw result fields above; RSS is explicitly run/sample dependent. |
| Security `NOTE-2`: no output/asset-integrity anchors | **accepted** | Added two owned pending markers for strict model-output/evidence validation and pre-launch extension asset-hash verification. |
| Security `NOTE-3`: benchmark intentionally exceeds production ratio | **rejected** as a defect | Input is local/test-only and the plan/PR already state it is stress evidence, not limit-enforcement evidence; production WS1 limits remain mandatory. |
| Security `NOTE-4`: features/dependencies precede runtime implementation | **rejected** as a defect | Resolving exact graph/features and auditing/benchmarking them is the approved purpose of Wave 0; no runtime path is wired. |

Follow-up fresh-context verification from both provider families found no remaining findings. Artifacts: `.pi-subagents/artifacts/outputs/bd2482a1-c389-4721-b832-87d28a824177/.../wave0-followup-correctness.md` and `.pi-subagents/artifacts/outputs/53f0a10e-77bd-4b05-adb4-a23507512dfe/.../wave0-followup-security.md`. The correctness reviewer independently reproduced all deterministic fixture identities and observed a run-dependent 12,804 KiB peak RSS; the security reviewer confirmed the positive environment boundary, marker honesty, dev-only encoder isolation, and supply-chain statements. No further fix pass is required for Wave 0.

### Normal validation evidence

From repository root, in required order, all pass:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

`git diff --check` also passes. Wave 0 changes are dependencies/features, license policy, planning/PR records, deterministic/ignored test scaffolding, benchmark harnesses, and a no-model probe extension; no production scanner trigger or user-facing runtime path exists yet.

## Progress record

- [x] Complex classification and canonical plan established.
- [x] 126-turn Grill Me decision record saved and incorporated.
- [x] Native-only v1 and DG-1 frozen.
- [x] Wave 0 dependencies/audits/benchmarks/probes/pending-contract markers complete.
- [ ] WS1 and WS2 qualifying implementation outcomes integrated.
- [ ] WS3 and WS4 integrated.
- [ ] WS5 acceptance hardening complete.
- [ ] Two-review quorum and dispositions complete.
- [ ] Final HTML report linked and current.
- [ ] Plan archived after verified completion.
