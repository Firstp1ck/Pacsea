# Grill Me Results

Generated: 2026-08-09T11:58:45.222Z

## Plan

@plans/planned/pi-agent-aur-scanner.md

## Shared Understanding

The first release is a default-off, Arch/Linux-only, native-observer Pi AUR scanner. It acquires immutable AUR recipes from official per-package AUR Git repositories and upstream sources declared by immutable .SRCINFO, performs bounded in-process extraction and deterministic analysis, and gives a restricted Pi subprocess path-confined read-only tools. Results are identity-bound advisory evidence, never safety approval. External-watcher interoperability and legacy heads import are deferred. Background observation and model execution are separated: observation runs every 15 minutes, while a sequential queue runner may start at most five unattended jobs per rolling hour under explicit consent and rolling budgets.

## Questions and Answers

### 1. Which SourceSnapshotProvider should the initial feature implement?

**Recommended answer:** Direct immutable .SRCINFO acquisition, because it fulfills the identity-bound upstream-source goal; relying only on existing provenance-bound trees would leave many scans incomplete.

**User answer:** A — Direct immutable .SRCINFO acquisition.

**Status:** resolved

**Notes:** This resolves the top-level DG-1 provider choice. Supported schemes, archive formats, verification policy, retention, and VCS/dynamic behavior remain to be decided.

### 2. Should we pause so you can reload/fix the Grill Me tool exposure, or continue with a state-compatible fallback that directly updates .pi/grill-me/state.json and later writes the same Markdown format?

**Recommended answer:** Pause and reload/fix tool exposure, because the interview rules explicitly require the named Grill Me tools.

**User answer:** Retry the tools now; Grill Me tools have been enabled.

**Status:** resolved

**Notes:** Tool exposure was restored and grill_record_turn succeeded. No fallback is needed.

### 3. Which upstream source transports should v1 acquire?

**Recommended answer:** HTTPS static files/archives plus Git-over-HTTPS (git+https), while treating plain HTTP, FTP, SSH, git://, SVN, Mercurial, Bazaar, local paths, and unknown schemes as unsupported limitations that force Incomplete.

**User answer:** A — HTTPS static sources plus git+https; all other transports are unsupported and make the scan incomplete.

**Status:** resolved

**Notes:** AUR-repository files remain covered separately by the immutable recipe snapshot.

### 4. Which downloaded source formats should v1 unpack for analysis?

**Recommended answer:** Support raw regular files plus .tar, .tar.gz/.tgz, .tar.bz2, .tar.xz/.txz, .tar.zst, and .zip, detected by content and validated against the declared name; unsupported or unknown containers make the scan incomplete.

**User answer:** A — Support the recommended common bounded archive set.

**Status:** resolved

**Notes:** 7z, RAR, self-extracting installers, disk images, nested unsupported formats, and unknown containers are out of scope for v1 and produce Incomplete.

### 5. How should v1 handle links and unsafe/non-regular entries during extraction?

**Recommended answer:** Extract only normalized directories and regular files; record safe in-archive symlink/hardlink metadata without creating filesystem links; reject the snapshot as Incomplete for traversal, escaping/dangling links, duplicate normalized paths, devices, FIFOs, sockets, or path-type conflicts.

**User answer:** A — Use metadata-only safe links and fail closed on unsafe entries.

**Status:** resolved

**Notes:** Coverage is allowed only when a confined link target exists and its content is independently manifested.

### 6. What verification should be required for downloaded non-VCS sources?

**Recommended answer:** Require at least one matching strong digest declared in the immutable .SRCINFO: SHA-256, SHA-384, SHA-512, or BLAKE2. A mismatch is Failed; SKIP, missing checksums, or weak-only MD5/SHA-1/SHA-224 verification is Incomplete unless an approved signature policy supplies adequate verification.

**User answer:** A — Require a declared strong digest.

**Status:** resolved

**Notes:** Detached-signature trust and behavior remain a separate decision.

### 7. How should declared detached signatures affect scan completeness?

**Recommended answer:** A strong checksum is sufficient when no signature is declared. When the immutable recipe declares a detached signature and validpgpkeys, verification is mandatory: a bad signature or signer mismatch is Failed; unavailable verification material is Incomplete; never silently downgrade to checksum-only success.

**User answer:** A — Enforce declared signatures as recommended.

**Status:** resolved

**Notes:** The key acquisition and isolation mechanism remains to be decided.

### 8. How should Pacsea obtain public keys needed for mandatory signature verification?

**Recommended answer:** Fetch only exact validpgpkeys fingerprints from an allowlisted HTTPS key endpoint into a Pacsea-controlled private cache, verify the full fingerprint after import, and perform verification in an isolated ephemeral keyring without ambient GnuPG state or arbitrary keyserver discovery.

**User answer:** A — Use allowlisted HTTPS fingerprint fetch with isolated verification.

**Status:** resolved

**Notes:** The setup flow must disclose key lookup network requests. Key-cache expiration can follow the broader cache-retention policy.

### 9. What upstream acquisition data should persist after a scan?

**Recommended answer:** Keep verified content-addressed download blobs and private Git objects only while referenced by an active queue item, accepted baseline, or retained result, then delete them after a 7-day unreferenced grace period; extracted trees remain ephemeral.

**User answer:** B — Do not persist a source cache.

**Status:** resolved

**Notes:** Upstream download blobs, upstream VCS objects, extracted trees, and Pi-readable source workspaces are removed after each scan. Persist only result metadata, manifests, provenance, and bounded validated evidence. This does not remove the separate durable private AUR recipe repository needed for recipe history and commit-range discovery.

### 10. Which git+https references may produce a Complete source snapshot?

**Recommended answer:** Only an explicit full commit OID. Tags, branches, and unqualified HEAD are resolved to exact commits for advisory scanning but remain Incomplete; they are re-resolved before linked install/update continuation and changes make the result Stale.

**User answer:** A — Only explicit full commit OIDs can be complete.

**Status:** resolved

**Notes:** This preserves advisory value for mutable refs without treating them as reproducible source identity.

### 11. Should Pacsea unpack a supported archive for analysis when .SRCINFO marks it noextract?

**Recommended answer:** Safely unpack an isolated copy using Pacsea's bounded extractor, preserve and manifest the original archive, label it analysis-only extraction that may differ from prepare() behavior, and report Incomplete when meaningful source requires custom transformation.

**User answer:** A — Perform safe analysis-only extraction.

**Status:** resolved

**Notes:** ArchWiki evidence: /usr/share/doc/arch-wiki/html/en/PKGBUILD.html — noextract. Pacsea does not execute prepare() or any PKGBUILD function.

### 12. Should Pacsea fetch additional content referenced only by acquired upstream files or build logic, such as .gitmodules, prepare() commands, language package-manager metadata, or installer scripts?

**Recommended answer:** Fetch only sources independently declared by the immutable .SRCINFO. Analyze undeclared references as evidence but never follow their URLs automatically; mark coverage Incomplete when meaningful build content depends on them.

**User answer:** A — Fetch only immutable .SRCINFO declarations.

**Status:** resolved

**Notes:** This prevents hostile acquired content from expanding Pacsea's network authority. Pi never receives a URL-fetch tool.

### 13. What compressed-download cap profile should v1 enforce?

**Recommended answer:** Use reqwest streaming to private atomic files with 100 MiB per source and 250 MiB total per package/commit; settings may lower but not raise these limits in v1. Oversized sources become Incomplete.

**User answer:** A — Enforce 100 MiB per source and 250 MiB per package/commit.

**Status:** resolved

**Notes:** Do not weaken src/util/mod.rs::curl_args() or its 10 MiB shared curl safeguard. Reject oversized Content-Length early and stop streaming at the cap.

### 14. What archive-expansion cap profile should v1 enforce per package/commit?

**Recommended answer:** Cap extraction at 1 GiB total regular-file bytes, 50,000 entries, depth 32, and a 25:1 expansion ratio; delete partial workspaces and report Incomplete when exceeded.

**User answer:** B — Use the stricter profile: 256 MiB total regular-file bytes, 10,000 entries, depth 16, and a 10:1 expansion ratio.

**Status:** resolved

**Notes:** Settings may lower but not raise these limits in v1. Exceeding a limit stops extraction, removes the partial workspace, and records Incomplete.

### 15. Must the first release ship the external-watcher spool integration and sister-repository WSE changes?

**Recommended answer:** No. Ship the native targeted observer first, retain the RecipeHeadSource interface and spool design, and hide external-watcher/auto settings until a separately verified follow-up.

**User answer:** A — Ship a native-only first release and defer external integration.

**Status:** resolved

**Notes:** Remove WSE from the first-release execution DAG and acceptance criteria. Do not expose nonfunctional external-watcher or auto settings. The follow-up can reuse the documented spool contract after independent cross-repository verification.

### 16. Which source should the native observer use to establish each installed package base's AUR HEAD?

**Recommended answer:** Use hardened direct-argv git ls-remote against the official per-package repository at https://aur.archlinux.org/<package-base>.git, then independently fetch discovered ranges into Pacsea's private bare repository; do not treat the GitHub mirror as authoritative identity.

**User answer:** A — Use the official per-package AUR Git repositories.

**Status:** resolved

**Notes:** Canonical host validation and strict package-base grammar apply before URL construction.

### 17. How often should Pacsea query installed AUR package heads?

**Recommended answer:** Run once after enabled startup, on explicit manual refresh, and at most once every 15 minutes when update cycles occur, with bounded concurrency and jitter rather than every 60-second update check.

**User answer:** A — Use startup/manual observation plus a 15-minute periodic floor.

**Status:** resolved

**Notes:** The existing package update worker still retains its own cadence; Pi scanner head observation only piggybacks eligible cycles.

### 18. Can the existing update-check payload provide the stable cycle identity required to freeze candidate AUR HEADs?

**Recommended answer:** No. Add a typed UpdateCycleId to UpdateCheckPayload and propagate it through the update worker/channel seam; do not derive scan identity later from available_updates.txt or timestamps alone.

**User answer:** Resolved from codebase evidence: UpdateCheckPayload currently carries count, package_names, authoritative, reason_codes, and official_strategy, but no cycle ID, so a new typed cycle identifier is required.

**Status:** resolved

**Notes:** Evidence: src/app/runtime/workers/updates_helpers.rs::UpdateCheckPayload and src/app/runtime/workers/updates.rs. Prefer a process-instance UUID plus monotonic sequence so IDs remain unique without requiring persistence.

### 19. What limits should the native observer use for official AUR git ls-remote checks?

**Recommended answer:** Allow up to 4 concurrent head queries with a 15-second per-query timeout and 90-second whole-cycle deadline; keep Pi scans and repository range processing at concurrency 1.

**User answer:** B — Run head observations fully sequentially.

**Status:** resolved

**Notes:** Keep a 15-second per-query timeout and 90-second whole-cycle deadline unless a later decision changes them. Failed observations retain the prior cursor and retry on the next eligible cycle.

### 20. When setup reaches the installed AUR package list, which targets should be selected by default?

**Recommended answer:** Select none by default; show all resolvable package bases and estimated maximum input/cost, with the existing a shortcut available to select all before explicit confirmation.

**User answer:** A — Select no initial baseline targets by default.

**Status:** resolved

**Notes:** This applies to entering setup without a contextual package intent; contextual entry behavior remains to be decided.

### 21. If setup is opened from a specific package detail or update candidate, should that package base be preselected despite the global none-selected default?

**Recommended answer:** Preselect only the contextual package base while still requiring the normal consent and cost confirmation before acquisition or Pi execution.

**User answer:** A — Preselect only the contextual package.

**Status:** resolved

**Notes:** Non-contextual setup continues to select none by default.

### 22. After a validated Complete scan, should Pacsea automatically advance that package base's comparison baseline even when the result contains high/critical findings?

**Recommended answer:** Yes. Advance automatically after Complete because the baseline is a technical comparison anchor, not package approval; never advance after Incomplete, Failed, Cancelled, or Stale.

**User answer:** A — Automatically advance after every complete scan.

**Status:** resolved

**Notes:** Findings remain independently retained and linked install/update acknowledgement rules remain unchanged.

### 23. Once the user has explicitly enabled paid background scanning and set a nonzero unattended budget, should newly detected unbaselined AUR package bases scan automatically?

**Recommended answer:** Queue them as Unbaselined and process them automatically within configured daily/job limits; if background is off or the budget is zero, only queue and notify.

**User answer:** A — Automatically scan newly detected unbaselined package bases under explicit background consent and budget.

**Status:** resolved

**Notes:** Setup must clearly disclose this unattended behavior.

### 24. Should a newly requested manual scan interrupt an active background Pi scan?

**Recommended answer:** Let the active package settle, fail, or be explicitly cancelled; place the manual request at the front of the queue for next execution to avoid wasting paid work and partial results.

**User answer:** A — Run the manual request next without automatic preemption.

**Status:** resolved

**Notes:** The user can still explicitly cancel the active scan.

### 25. How should Pacsea order pending background work across package bases?

**Recommended answer:** Use deterministic round-robin across package bases while preserving oldest-first commit order within each base, so one high-churn package cannot monopolize the queue.

**User answer:** A — Use per-package round-robin with oldest-first ordering within each package base.

**Status:** resolved

**Notes:** Newly unbaselined packages participate as their package base's oldest item. Foreground manual requests retain the separate next-in-queue priority.

### 26. Which failed background operations may retry without fresh user confirmation?

**Recommended answer:** Automatically retry only transient acquisition/head-query failures and Pi startup failures that occur before any model request is accepted, using bounded exponential backoff. Once model usage may have occurred, require manual retry.

**User answer:** A — Retry only clearly pre-usage transient failures.

**Status:** resolved

**Notes:** The single bounded in-session schema correction is not a new paid scan retry.

### 27. What bounded retry schedule should apply to eligible pre-usage transient failures?

**Recommended answer:** Use three total attempts: the initial attempt, then retries after 1 minute and 5 minutes; do not hold the queue slot while waiting.

**User answer:** B — Use two total attempts with a single 1-minute backoff.

**Status:** resolved

**Notes:** After the retry is exhausted, retain the item as failed until manual retry or a separately eligible future operation; never spin within the same queue slot.

### 28. Should unattended scanning run when Pi cannot provide reliable usage statistics or Pacsea cannot determine the model's cost?

**Recommended answer:** Disable unattended scans when usage cannot be bounded; require reliable token usage and reliable cost data whenever a nonzero cost cap is configured.

**User answer:** D — Permit unattended scanning using estimates derived from prompt, tool-response, and output byte counts.

**Status:** resolved

**Notes:** A conservative estimation formula, cost handling, and UI labeling must be defined before this can safely enable background work.

### 29. How conservative should fallback byte-based accounting be?

**Recommended answer:** Count every UTF-8 byte sent to or received from Pi, then estimate ceil(total_bytes / 2) + 8,000 tokens per scan; label it Estimated, reserve before launch, and reconcile downward only if reliable statistics later arrive.

**User answer:** A — Use bytes divided by 2 plus an 8,000-token fixed reserve.

**Status:** resolved

**Notes:** All prompts, tool results, corrections, and final output bytes count toward the estimate.

### 30. If Pi provides no trustworthy price metadata, how should Pacsea estimate dollars for the unattended cost cap?

**Recommended answer:** Require a user-supplied conservative maximum USD per million tokens and never infer prices from model names.

**User answer:** C — Fetch current pricing automatically from the web.

**Status:** resolved

**Notes:** Runtime source authority, freshness, subscription-route semantics, and fail-closed behavior must be specified before implementation.

### 31. Does Pi RPC expose model pricing and post-scan token/cost statistics that Pacsea can use before any web fallback?

**Recommended answer:** Yes. Treat Pi's full Model.cost returned by get_available_models/get_state as the primary pricing source and get_session_stats as the primary post-scan accounting source; invoke web pricing lookup only when that metadata is missing or explicitly classified as unknown.

**User answer:** Resolved from official Pi documentation: get_available_models returns full Model objects with per-million-token cost fields, and get_session_stats returns aggregate token and cost statistics.

**Status:** resolved

**Notes:** Evidence: /home/firstpick/npm-packages/pi-package-webui/node_modules/@earendil-works/pi-coding-agent/docs/rpc.md — get_available_models, get_session_stats, Model; docs/models.md — Model Configuration. Custom model cost defaults to all zeros, so zero can mean either local/free or unspecified and needs explicit classification.

### 32. Which online sources may Pacsea trust for automatic model pricing?

**Recommended answer:** Only allowlisted provider-owned HTTPS machine-readable endpoints keyed by exact canonical provider and model ID; if unavailable, disable background scanning for that model.

**User answer:** C — Allow trusted third-party pricing aggregators.

**Status:** resolved

**Notes:** The trusted aggregator allowlist, route-specific semantics, exact matching, freshness, and disagreement handling remain to be defined.

### 33. Which structured catalogs may Pacsea use for automatic model pricing?

**Recommended answer:** Use LiteLLM's cost map for exact recognized direct-provider model IDs and OpenRouter's official model endpoint only for OpenRouter-routed models, with exact provider/model matching and fail-closed ambiguity handling.

**User answer:** A — Use LiteLLM for direct routes and the OpenRouter API for OpenRouter routes.

**Status:** resolved

**Notes:** Verified sources: https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json and https://openrouter.ai/api/v1/models. No fuzzy aliases or cross-route price substitution.

### 34. How should Pacsea account for subscription-backed Pi providers where per-token catalog pricing does not equal the user's actual marginal charge?

**Recommended answer:** Enforce the daily token cap, show catalog pricing only as API-equivalent reference cost, and require acknowledgement that it is not actual spend.

**User answer:** C — Treat subscription-backed Pi provider usage as zero dollar cost.

**Status:** resolved

**Notes:** The daily token cap and conservative byte-based fallback still apply; only dollar-cost accounting is zero for explicitly recognized subscription routes. The UI must label the route as subscription-backed rather than free API usage.

### 35. How long may cached online pricing remain usable for unattended direct/OpenRouter scans?

**Recommended answer:** Refresh every 24 hours; permit exact-match stale pricing for up to 7 days on transient failure; then disable unattended scans until refresh succeeds.

**User answer:** C — Refresh pricing weekly and allow cached prices to remain usable without expiry.

**Status:** resolved

**Notes:** Because prices never expire under this choice, a separate stale-price safety margin and UI warning policy must be defined.

### 36. What safety margin should apply when cached pricing is older than seven days?

**Recommended answer:** Use twice the highest price ever observed for the exact provider/model/route, never automatically lower that historical ceiling, and label the estimate stale.

**User answer:** E — Refetch the pricing when cached pricing is older than seven days.

**Status:** resolved

**Notes:** This supersedes the prior never-expire behavior for unattended enforcement: stale cached data triggers an online refresh rather than indefinite reuse. Failure behavior remains to be decided.

### 37. If the required stale-price refetch fails, should unattended scanning proceed using the old cached price?

**Recommended answer:** Pause unattended scans until pricing refresh succeeds; preserve stale pricing only for display and allow manual scans with a warning.

**User answer:** B — Continue unattended scanning using the stale cached price.

**Status:** resolved

**Notes:** The UI and persisted accounting must mark the price stale and record the failed refresh timestamp. This choice accepts that the dollar cap may underestimate current provider pricing; the token cap remains authoritative.

### 38. How should Pacsea define daily token, job, and cost caps?

**Recommended answer:** Use a rolling 24-hour window over persisted completed and reserved usage to avoid midnight double-spend, timezone changes, and boundary bursts.

**User answer:** A — Use a rolling 24-hour budget window.

**Status:** resolved

**Notes:** Budget state must persist across Pacsea restarts.

### 39. If Pacsea or Pi exits before reliable usage statistics arrive, how should the reserved background budget be reconciled?

**Recommended answer:** Charge the full pre-launch reservation for the rolling 24-hour window and reconcile downward only when trustworthy final statistics exist.

**User answer:** A — Charge the full reservation when usage is unknown.

**Status:** resolved

**Notes:** This prevents crash/restart loops from releasing possibly consumed unattended budget.

### 40. What must the selected Pi model prove before Pacsea allows real scans?

**Recommended answer:** Require a bounded synthetic readiness test using the exact provider/model/thinking level: restricted-tool use, schema-valid identity-bound JSON, cancellation handling, and accounting support or approved fallback; never silently switch models.

**User answer:** C — Warn when tool/schema readiness checks fail but still allow scanning.

**Status:** resolved

**Notes:** A follow-up decision must constrain background eligibility and Complete-result semantics for readiness-failed models.

### 41. If a model fails the synthetic tool/schema readiness test but the user proceeds, what restrictions apply?

**Recommended answer:** Permit manual scans only, disable unattended/background use, and prevent Complete results until that exact model configuration passes readiness.

**User answer:** C — Allow manual and background scans normally despite the failed readiness test.

**Status:** resolved

**Notes:** Per-scan strict schema, identity, tool-boundary, and output validation remain mandatory. Individual scans may be Complete if they satisfy all runtime validation; readiness failure is retained as a prominent warning.

### 42. If the configured provider/model becomes unavailable while work is queued, may Pacsea automatically switch to another configured model?

**Recommended answer:** Pause affected scans and require explicit model reselection because model changes alter security behavior, privacy, cost, and baseline identity.

**User answer:** C — Automatically use a configured ordered fallback list.

**Status:** resolved

**Notes:** The fallback list must be explicitly configured and consented; fallback timing and per-model readiness/accounting remain to be defined.

### 43. At what point may Pacsea advance to the next model in the configured fallback list?

**Recommended answer:** Only before Pi accepts the first package prompt; after acceptance or possible usage, fail the scan and require manual retry. Never continue one scan across models.

**User answer:** D — Continue the same scan on another model when fallback is needed.

**Status:** resolved

**Notes:** This changes the original one-model-per-scan assumption. Multi-model provenance, accounting, context replay, output authority, and privacy disclosure must be specified.

### 44. How should continue the same scan preserve context when switching models?

**Recommended answer:** Use a fresh Pi process and attempt ID from the same immutable manifests under one logical scan group; do not replay partial assistant output; use only the successful model's final result.

**User answer:** B — Use RPC set_model in the existing Pi conversation.

**Status:** resolved

**Notes:** The scan remains one short-lived Pi process but may now use multiple models. Persist ordered model/provider transitions and account for all usage. Final-output authority and correction behavior remain to be defined.

### 45. Which assistant output becomes authoritative after an in-session set_model fallback?

**Recommended answer:** Require a fresh full validation pass and accept only the last post-switch schema-valid result; never merge earlier drafts automatically.

**User answer:** B — Merge all schema-valid findings across models.

**Status:** resolved

**Notes:** The merged result must preserve per-model attribution, ordered provenance, and all usage. Deduplication and conflict-resolution rules remain to be defined.

### 46. How should Pacsea deduplicate and resolve conflicting findings from fallback models?

**Recommended answer:** Build a deterministic union keyed by exact evidence fingerprint; collapse exact duplicates but retain each model's severity, confidence, and rationale; use the highest reported severity for sorting and acknowledgement gates; preserve disagreement visibly.

**User answer:** A — Use an evidence-fingerprint union with visible disagreement.

**Status:** resolved

**Notes:** No majority vote or last-model erasure. Every contributing model remains in result provenance and accounting.

### 47. Which failures should advance to the next configured model?

**Recommended answer:** Trigger fallback for terminal provider/API errors, explicit refusal, missing final output, or schema/identity failure after one bounded same-model correction; do not fallback for source limitations, a valid Incomplete result, low confidence, or disliked findings.

**User answer:** A — Trigger fallback only for model, provider, or output-contract failures.

**Status:** resolved

**Notes:** Every fallback remains subject to the configured ordered list and budget reservation.

### 48. How many models may one logical scan try automatically?

**Recommended answer:** At most three model attempts total, including the primary, and only while the full projected reservation remains within job, token, and cost caps.

**User answer:** A — Allow a maximum of three model attempts per logical scan.

**Status:** resolved

**Notes:** Exhaustion produces Failed with all attempted model/provider identities and accounting retained.

### 49. May Pacsea send the selected package base and affected installed package names to a remote model?

**Recommended answer:** Yes, after explicit setup disclosure; send only identities for the current scan, never the full installed-package inventory, and offer local models as the privacy-preserving alternative.

**User answer:** A — Send current-scan package identities after consent.

**Status:** resolved

**Notes:** Persist provider/model and disclosure provenance with each result.

### 50. How much source metadata should Pacsea place directly in the first package prompt?

**Recommended answer:** Send only scan identity, manifest hashes/counts, bounded changed-file summary, deterministic detector findings, coverage limits, and prioritized entry paths; let the model inspect files through restricted tools and do not inline the full manifest or source bodies.

**User answer:** A — Use a bounded summary plus restricted-tool discovery.

**Status:** resolved

**Notes:** This constrains privacy, context size, and cost while retaining exact identity binding.

### 51. Should pacsea_scan_grep accept model-supplied regular expressions?

**Recommended answer:** No. Support bounded literal substring search with explicit case sensitivity and validated globs; avoid model-supplied JavaScript regex and catastrophic backtracking risk.

**User answer:** A — Support literal substring search only.

**Status:** resolved

**Notes:** The model may issue multiple bounded literal searches.

### 52. Which per-scan restricted-tool budget should v1 enforce?

**Recommended answer:** Maximum 100 tool calls and 4 MiB cumulative tool-result text per model attempt, with per-call limits of read 64 KiB, grep 200 matches/128 KiB, and find/ls 500 entries/128 KiB.

**User answer:** C — Allow up to 250 tool calls and 16 MiB cumulative tool-result text per model attempt.

**Status:** resolved

**Notes:** Keep the recommended per-call limits unless separately changed. Reaching a limit must become an explicit coverage limitation and may prevent Complete.

### 53. What wall-clock limits should accompany the larger 250-call/16 MiB tool budget?

**Recommended answer:** Allow 5 minutes per model attempt and 12 minutes per logical scan across fallback models; allow 5 seconds for RPC abort before terminating and reaping the process group.

**User answer:** A — Use 5 minutes per model attempt and 12 minutes per logical scan.

**Status:** resolved

**Notes:** Queue wait is excluded; acquisition and validation within the logical scan are included.

### 54. What maximum validated model-result size should v1 accept per model attempt?

**Recommended answer:** Accept at most 1 MiB JSON and 200 findings, with 4 KiB each for evidence, rationale, and recommendation; one correction is allowed before failing the attempt.

**User answer:** C — Allow up to 4 MiB JSON and 500 findings per model attempt.

**Status:** resolved

**Notes:** Keep the 4 KiB per-field limits for evidence, rationale, and recommendation. A repeated size/count violation fails the attempt and may trigger configured fallback.

### 55. May a model downgrade or suppress a deterministic detector finding?

**Recommended answer:** No. Keep deterministic findings as a separate attributed layer with original severity; models may add visible context but cannot erase or lower the acknowledgement gate. Only explicit user adjudication may mark it benign.

**User answer:** A — Deterministic findings cannot be downgraded or suppressed by models.

**Status:** resolved

**Notes:** User adjudication carry-forward rules remain to be defined.

### 56. When may a user's accepted-as-benign verdict suppress repeated acknowledgement for a deterministic finding?

**Recommended answer:** Only when detector ID/version, package base, relative path, exact evidence fingerprint, and relevant manifest entry hash are unchanged; any change resurfaces the finding.

**User answer:** A — Carry benign verdicts forward only on an exact identity and evidence match.

**Status:** resolved

**Notes:** Verdicts remain advisory metadata and never authorize installation by themselves.

### 57. What interaction should allow a linked install/update to continue despite high or critical advisory findings?

**Recommended answer:** Show a result-bound modal with View findings focused by default; require deliberate selection of Continue anyway; bind acknowledgement to exact scan ID, manifests, and current AUR/source identities; never offer a global always-allow.

**User answer:** A — Use deliberate result-bound confirmation.

**Status:** resolved

**Notes:** Acknowledgement remains advisory and cannot authorize installation without the user's existing install action.

### 58. If AUR HEAD or a mutable source ref changes after scanning, may the earlier high/critical acknowledgement still apply?

**Recommended answer:** Invalidate it and require a separate stale-result confirmation, with Rescan focused by default and any continuation bound to the newly observed identities.

**User answer:** A — Require a separate stale-result acknowledgement.

**Status:** resolved

**Notes:** Prior finding acknowledgements never carry across stale recipe or source identity.

### 59. How should a validated complete result with no findings be labeled?

**Recommended answer:** Complete — no findings in analyzed scope, always paired with coverage and residual uncertainty; never use Safe, Clean, Trusted, or an approval checkmark.

**User answer:** A — Label it “Complete — no findings in analyzed scope.”

**Status:** resolved

**Notes:** Severity colors may indicate findings, but completion itself must not visually imply package safety.

### 60. Should Pacsea persist the original schema-valid assistant JSON in addition to the typed result?

**Recommended answer:** No. Persist only the validated typed result and render canonical JSON behind Raw; allow original-response export only through an explicit live debug action after sanitization.

**User answer:** A — Persist typed data only and render canonical JSON.

**Status:** resolved

**Notes:** Invalid attempts, model thinking, and original raw responses are not persisted.

### 61. Which scan results should survive beyond the default 30-day retention window?

**Recommended answer:** Always retain the current accepted comparison baseline result and every unresolved high/critical result; apply 30-day cleanup to other superseded results.

**User answer:** D — Retain only the newest result per package base beyond the normal retention window.

**Status:** resolved

**Notes:** The accepted baseline record, queue, and ledger metadata remain independently durable. Whether the detailed result referenced by an older accepted baseline is exempt needs clarification.

### 62. If the newest result is failed, incomplete, cancelled, or stale, should Pacsea also retain the older detailed result that established the current accepted comparison baseline?

**Recommended answer:** Keep at most two detailed results in that case: the newest attempt and the current baseline result, so baseline evidence is not lost.

**User answer:** A — Retain the newest result plus the current baseline result when they differ.

**Status:** resolved

**Notes:** Other superseded results follow the 30-day retention policy, including older findings not represented by these retained records.

### 63. Can Pi scan persistence safely reuse Pacsea's existing generic cache loader for corrupt or newer-schema state?

**Recommended answer:** No. Implement dedicated versioned Pi scan persistence that distinguishes missing, corrupt, unsupported-newer, and I/O-failed state; never collapse them to an empty baseline.

**User answer:** Resolved from codebase evidence: src/app/cache_common.rs::load_signed_cache only logs read/parse failures and returns None, with no schema-version or quarantine contract, so it is insufficient for security baseline/backlog state.

**Status:** resolved

**Notes:** The Pi scan loader may reuse path/config conventions but needs separate private atomic state and actionable recovery semantics.

### 64. What should Pacsea do when baseline, backlog, or result JSON is corrupt or from a newer unsupported schema?

**Recommended answer:** Atomically move it without replacement into a private pi_scan/quarantine directory named by type, timestamp, and SHA-256; disable affected state and show recovery actions; never overwrite, reset, or delete automatically.

**User answer:** A — Quarantine corrupt or unsupported state and fail closed.

**Status:** resolved

**Notes:** If quarantine itself fails, leave the original untouched and keep the affected scanner state unavailable.

### 65. What destructive recovery actions should the TUI offer for quarantined Pi-scan state?

**Recommended answer:** Offer Retry load, Show quarantine path, and package-base-scoped Reset scanner state with explicit confirmation; provide global reset only in advanced settings with a second confirmation; never delete quarantine files automatically.

**User answer:** A — Provide package-scoped reset plus a guarded global reset.

**Status:** resolved

**Notes:** Quarantine artifacts remain until the user removes them outside automatic retention.

### 66. On startup, how should Pacsea handle a queue item persisted as active when the previous process ended unexpectedly?

**Recommended answer:** Reclassify it as Interrupted, retain the full budget reservation for 24 hours, clean only verified private workspaces, require manual retry, and continue other eligible queue items.

**User answer:** A — Mark the scan interrupted and require manual retry.

**Status:** resolved

**Notes:** This aligns crash accounting with the full-reservation decision.

### 67. Is Shift+A already assigned in Pacsea's shipped keybindings or current KeyMap?

**Recommended answer:** No. It is available for a new configurable Search Normal-mode shortcut, subject to adding the new KeyMap/schema/editor/help wiring.

**User answer:** Resolved from codebase evidence: config/keybinds.conf and src/theme/types.rs::KeyMap define existing Shift+C/O/P/S/I/E/U shortcuts but no Shift+A binding; repository search found no Shift+A handler.

**Status:** resolved

**Notes:** Evidence: config/keybinds.conf; src/theme/types.rs; src/theme/settings/parse_keybinds.rs; src/theme/config/schema.rs.

### 68. Should Shift+A in Search Normal mode open the Pi Scan workspace?

**Recommended answer:** Yes. It is collision-free in current defaults, mnemonic for AUR/AI analysis, configurable, and does not conflict with lowercase a selection inside the workspace.

**User answer:** A — Use Shift+A as the default Search Normal-mode shortcut.

**Status:** resolved

**Notes:** Update keybind config, parser, schema/editor, help overlay, and locale descriptions.

### 69. In --dry-run mode, may Pacsea perform AUR Git queries or upstream downloads for a Pi scan preview?

**Recommended answer:** Perform no Pi launch, Git/network acquisition, key lookup, pricing fetch, or durable queue/baseline mutation; show a local-only preview.

**User answer:** B — Allow read-only AUR and upstream source acquisition in dry-run mode, but do not launch Pi.

**Status:** resolved

**Notes:** Acquisition may use private temporary workspaces and network reads, but must not mutate durable scan queue, baseline, ledger, result, or budget state. Cleanup and signature/key-fetch behavior remain to be clarified.

### 70. May dry-run acquisition fetch public signing keys and perform checksum/signature verification?

**Recommended answer:** Allow temporary allowlisted HTTPS public-key lookup and isolated checksum/signature verification; skip Pi readiness/pricing fetches and persist no fetched key or source data after dry-run.

**User answer:** A — Verify sources in dry-run, including temporary public-key fetches.

**Status:** resolved

**Notes:** All dry-run acquisition workspaces and fetched keys are removed after the preview.

### 71. Should signature verification use an external isolated GnuPG process or a new Rust OpenPGP dependency?

**Recommended answer:** Use gpg/gpgv via direct arguments with a private ephemeral home/keyring, disabled auto-key retrieval, bounded output/time, and exact fingerprint checks; if unavailable, mark declared-signature sources Incomplete.

**User answer:** A — Use isolated external gpg/gpgv.

**Status:** resolved

**Notes:** Do not inherit ambient GnuPG configuration, agents, trustdb, or keyservers.

### 72. Which authentication mechanisms may the Pacsea-launched Pi process use?

**Recommended answer:** Allow only Pi's existing standard auth store/login state and static non-secret model metadata; do not pass provider API-key environment variables or permit models.json credential shell commands in scanner mode.

**User answer:** A — Use the standard Pi auth store only.

**Status:** resolved

**Notes:** Pacsea never prompts for, stores, logs, or forwards provider secrets. Unsupported env/command-based custom auth is reported with guidance to use Pi's normal login flow.

### 73. How should Pacsea classify a custom model endpoint as local or remote?

**Recommended answer:** Use three classes: loopback/Unix socket as Local, literal RFC1918/ULA as Private network, and everything else—including custom hostnames—as Remote unless explicitly overridden; show the class on model selection and results.

**User answer:** A — Use three-tier classification with remote-by-default custom hostnames.

**Status:** resolved

**Notes:** User overrides are non-secret settings and must be displayed as overrides rather than detected facts.

### 74. Does the currently installed Pi binary support the scanner's required RPC isolation flags?

**Recommended answer:** Yes for the installed version; pin a tested minimum version and fail closed if any required flag disappears or changes.

**User answer:** Resolved from live read-only evidence: Pi 0.84.0 at /home/firstpick/.npm-global/bin/pi supports --mode rpc, --no-session, --no-builtin-tools, --tools, --extension, --no-extensions, --no-skills, --no-prompt-templates, --no-themes, --no-context-files, and --no-approve.

**Status:** resolved

**Notes:** Evidence: `pi --version` and `pi --help`. Wave 0 should convert this into an automated capability probe rather than relying only on version comparison.

### 75. Should Pacsea enforce a narrow Pi version range or rely primarily on capability/protocol probes?

**Recommended answer:** Require Pi >= 0.84.0, then probe every required CLI flag and RPC behavior; do not set a hard maximum, but allow newer versions only when framing, commands, isolation, abort, model enumeration, and result retrieval probes pass.

**User answer:** A — Use a minimum version plus fail-closed capability probes.

**Status:** resolved

**Notes:** Version checks are advisory beyond the minimum; capability and protocol behavior are authoritative.

### 76. Should the scanner leave Pi's own automatic provider retry enabled?

**Recommended answer:** Disable Pi auto-retry so Pacsea's retry policy, fallback list, reservations, cancellation, and provenance remain authoritative.

**User answer:** B — Keep Pi automatic provider retry enabled.

**Status:** resolved

**Notes:** Nested Pi retries must be bounded, included in budget reservation/accounting, exposed in provenance, and coordinated with Pacsea-level fallback.

### 77. What limit should Pacsea enforce around Pi's enabled automatic provider retries?

**Recommended answer:** Allow at most three provider-call attempts per model attempt; reserve 3x estimated model budget, record auto_retry events, and abort retries beyond the cap.

**User answer:** A — Allow a maximum of three provider-call attempts for each low-level model request.

**Status:** resolved

**Notes:** This caps retries of each low-level provider request, not the number of normal model turns/tool loops in a scan. The 3x retry reserve applies to the projected logical model-attempt usage.

### 78. Which tab should the Pi Scan workspace open first?

**Recommended answer:** Use context-sensitive landing: top bar/Shift+A opens Overview, package shortcut opens focused Targets, active scan opens Progress, and result shortcut opens that Result; restore the last tab only without stronger context.

**User answer:** A — Use context-sensitive workspace landing.

**Status:** resolved

**Notes:** Disabled state always lands in Setup regardless of entry context.

### 79. When multiple states apply simultaneously, what should the top-bar Pi Scan chip display?

**Recommended answer:** Combine active progress with the highest unresolved condition, otherwise prioritize Unavailable/Disabled, Baseline needed, unresolved Critical/High, Backlog, then Enabled; never let backlog hide severe findings.

**User answer:** A — Combine progress with prioritized unresolved status.

**Status:** resolved

**Notes:** Example: Scanning 2/5 • Critical.

### 80. How persistent should scan-completion notifications be?

**Recommended answer:** Critical/high findings and corrupted-state/isolation/stale-identity failures create persistent typed notifications until opened or dismissed; medium/low/informational completions use timed toasts.

**User answer:** A — Use persistent critical/high and security-boundary notifications.

**Status:** resolved

**Notes:** Typed notification categories must replace string-matched classification.

### 81. Which changes should force the setup/privacy consent screen to reappear?

**Recommended answer:** Re-consent when primary/fallback provider or model changes, endpoint privacy class changes, background scanning is enabled, pricing/accounting policy changes, or scanner prompt/tool major version changes; not for minor UI changes or tighter limits.

**User answer:** A — Re-consent on material privacy, cost, or capability changes.

**Status:** resolved

**Notes:** Persist consent version and the exact disclosed configuration fingerprint.

### 82. When Pi background execution is off but the scanner feature is enabled, should native AUR head observation still run and persist backlog entries?

**Recommended answer:** Yes. Continue unpaid read-only head observation so intermediate commits are not missed; disclose periodic AUR network traffic and keep model execution paused until manual action or explicit background consent.

**User answer:** A — Observe and persist backlog while background model execution is off.

**Status:** resolved

**Notes:** Dry-run remains separate and does not mutate durable backlog state.

### 83. How many unseen AUR commits may one native observation cycle expand into the durable ledger?

**Recommended answer:** Process at most 500 commits per package base and 2,000 total per cycle, oldest-first and resumably; never coalesce the remainder.

**User answer:** D — When the expansion limit is exceeded, keep only the newest unseen commit.

**Status:** resolved

**Notes:** This conflicts with the current success criterion requiring every observed intermediate recipe-tree-changing commit to be ledgered/scanned. The intended relaxation must be confirmed explicitly.

### 84. Does your previous choice intentionally replace the plan's requirement to account for every observed intermediate commit when catch-up exceeds the cap?

**Recommended answer:** Preserve every-commit accounting with resumable bounded expansion so every commit is eventually ledgered; otherwise a malicious intermediate recipe could be silently discarded.

**User answer:** A — Preserve every-commit accounting with resumable bounds.

**Status:** resolved

**Notes:** This supersedes turn 83's coalescing choice. Use 500 commits per package base and 2,000 total per cycle, oldest-first; advance only through the last durably inserted commit and resume later.

### 85. If a newly observed AUR HEAD is not a descendant of the persisted cursor, what should Pacsea do?

**Recommended answer:** Pause that package base, preserve old ledger/results, fetch and manifest the new history without automatic scanning, and require explicit rebaseline confirmation showing old and new OIDs; never rewrite or silently discard old history.

**User answer:** A — Pause and require explicit rebaseline.

**Status:** resolved

**Notes:** The rebaseline action creates a new comparison lineage while retaining prior lineage provenance.

### 86. If several AUR commits have the same installed pkgver/pkgrel and Pacsea cannot prove which one produced the installed package, what should it scan?

**Recommended answer:** Scan the newest matching recipe commit for advisory value, mark installed provenance Unverified and the overall result Incomplete, and allow manual selection of another matching commit; never treat version equality as build identity.

**User answer:** A — Scan the newest matching commit and mark the result incomplete.

**Status:** resolved

**Notes:** This result cannot advance the accepted complete comparison baseline.

### 87. When installed-package provenance cannot be proven, what snapshot should establish future changed-recipe comparisons?

**Recommended answer:** Keep the installed-provenance scan Incomplete, then offer a separate explicit scan of the current observed AUR HEAD and bound upstream sources; a complete result may establish an observation baseline, not proof of installed bytes.

**User answer:** A — Use a separate explicit current-HEAD baseline.

**Status:** resolved

**Notes:** The UI must distinguish Installed provenance scan from Current AUR observation baseline.

### 88. If AUR HEAD changes after an update cycle freezes a candidate commit but before its scan starts, should Pacsea silently scan the newer HEAD instead?

**Recommended answer:** No. Scan the frozen candidate if reachable, mark it Stale, and queue the newer commit separately in chronological order to preserve cycle identity and every-commit accounting.

**User answer:** A — Preserve the frozen candidate and queue the newer HEAD separately.

**Status:** resolved

**Notes:** Do not run the two Pi scans concurrently; normal queue ordering applies.

### 89. Which AUR file changes should trigger a paid Pi scan?

**Recommended answer:** Trigger for PKGBUILD, .SRCINFO, install/changelog files, and tracked local source/build inputs; ledger unreferenced documentation, CI metadata, or .gitignore-only changes without a Pi call; scan when classification is uncertain.

**User answer:** A — Trigger scans for build-relevant tracked-file changes and scan when classification is uncertain.

**Status:** resolved

**Notes:** This replaces a literal any-tree-change rule with a conservative build-relevance classifier while preserving every commit in the ledger.

### 90. Must the model read every upstream source file for a result to be Complete?

**Recommended answer:** No. Require exhaustive manifesting and deterministic scanning of every eligible file, plus model inspection of all AUR recipe files, changed files, executables/scripts, detector hits, and declared entry points; explicitly label remaining files as manifest-only risk-prioritized scope.

**User answer:** A — Use exhaustive deterministic coverage plus mandatory risk-based model reads.

**Status:** resolved

**Notes:** Complete refers to this declared bounded policy, not exhaustive semantic review by the model.

### 91. Which binary files should force a source-bearing result to Incomplete?

**Recommended answer:** Mark Incomplete for executable binaries, libraries, object files, firmware, opaque generated archives, or unknown binary formats that could affect build/runtime; known non-executable media/font assets may remain manifest-only unless build/runtime-relevant.

**User answer:** A — Security-relevant or unknown binary files force an incomplete result.

**Status:** resolved

**Notes:** File classification must be deterministic and recorded; uncertainty is treated as security-relevant.

### 92. What per-file size limit should apply to full deterministic text analysis?

**Recommended answer:** Stream/hash every file within the snapshot cap, but fully classify/search text only up to 16 MiB per file; larger build/runtime-relevant text files remain manifested but force Incomplete.

**User answer:** A — Use a 16 MiB per-text-file analysis limit.

**Status:** resolved

**Notes:** Known non-build documentation above the limit may remain manifest-only; uncertainty is incomplete.

### 93. Which source encodings should restricted tools and deterministic detectors treat as analyzable text?

**Recommended answer:** Support strict UTF-8/ASCII plus UTF-16 LE/BE only with a valid BOM, normalize to UTF-8 for analysis while hashing original bytes, and mark unsupported build/runtime-relevant encodings Incomplete.

**User answer:** A — Support UTF-8/ASCII plus BOM-marked UTF-16.

**Status:** resolved

**Notes:** Evidence paths and hashes bind original bytes; displayed line ranges bind the canonical decoded text representation.

### 94. If otherwise valid model JSON contains terminal control characters or escape sequences in a string field, should Pacsea sanitize the field or reject the result?

**Recommended answer:** Reject the entire result and use one bounded correction request; silent sanitization could alter evidence or hide an injection attempt. A repeated violation fails the model attempt.

**User answer:** A — Reject the result and request one correction.

**Status:** resolved

**Notes:** JSON framing controls such as escaped newline/tab are allowed only where schema permits; terminal escape/control sequences are forbidden.

### 95. If one model finding's quoted evidence or line range does not exactly match the immutable snapshot, should Pacsea drop only that finding or reject the whole result?

**Recommended answer:** Reject the whole result and request one correction; after a repeated mismatch, fail the model attempt rather than dropping potentially important findings or accepting fabricated evidence.

**User answer:** A — Reject the whole result and allow one correction.

**Status:** resolved

**Notes:** Evidence matching uses canonical decoded text while path/hash identity binds original bytes.

### 96. How should static-source downloads handle redirects?

**Recommended answer:** Follow at most 5 redirects, requiring HTTPS and no URL userinfo at every hop; revalidate every destination, strip authorization/cookies/referrer, reject loops/downgrades/unsupported ports, and record the redirect chain.

**User answer:** A — Follow up to 5 fully revalidated HTTPS redirects.

**Status:** resolved

**Notes:** Checksums do not permit HTTP downgrade.

### 97. May untrusted .SRCINFO source URLs resolve to loopback, private, link-local, multicast, or otherwise non-public IP ranges?

**Recommended answer:** No. Resolve each hostname, reject if any address is non-public, pin validated addresses for the connection, and repeat validation on every redirect to prevent SSRF.

**User answer:** A — Allow public Internet destinations only.

**Status:** resolved

**Notes:** Apply the same destination policy to static HTTPS sources and git+https remotes. Pacsea-owned allowlisted pricing/key endpoints are separately configured trusted services but still reject non-public DNS results.

### 98. Should source acquisition inherit HTTP_PROXY, HTTPS_PROXY, or ALL_PROXY from Pacsea's environment?

**Recommended answer:** Disable ambient proxy inheritance; optionally support a separate explicit credential-free Pacsea HTTPS-proxy setting that forbids userinfo, validates destination, and is disclosed during setup.

**User answer:** A — Do not inherit ambient proxies; allow only an explicit credential-free proxy setting.

**Status:** resolved

**Notes:** Acquisition and Pi subprocess environments exclude ambient proxy variables.

### 99. Should v1 support a Pacsea-specific custom CA bundle for source, key, or pricing HTTPS requests?

**Recommended answer:** No. Use the operating system trust store only, never disable certificate verification, and require private/corporate CAs to be installed into system trust.

**User answer:** A — Use the system trust store only.

**Status:** resolved

**Notes:** Do not inherit SSL_CERT_FILE/SSL_CERT_DIR or add per-source TLS exceptions.

### 100. If an installed pacman -Qm package cannot be uniquely mapped to an AUR package base, may the user supply a custom recipe repository URL?

**Recommended answer:** Do not allow custom recipe URLs in v1; mark the target Unavailable rather than expanding the trusted-host and identity model.

**User answer:** B — Allow user-supplied official AUR repository URLs only.

**Status:** resolved

**Notes:** Manual URL validation and proof that the repository corresponds to the installed package remain to be defined.

### 101. When a user supplies an official AUR URL, what must Pacsea verify before accepting the mapping?

**Recommended answer:** Canonicalize it to https://aur.archlinux.org/<strict-pkgbase>.git, reject query/fragment/userinfo/alternate ports, fetch the immutable recipe, and require .SRCINFO to list the installed package name under that package base; retain user-confirmed provenance.

**User answer:** A — Require the canonical official AUR URL plus .SRCINFO membership proof.

**Status:** resolved

**Notes:** The mapping is package-scoped, not a global alias, and must be revalidated when recipe identity changes.

### 102. How long should a verified user-supplied AUR mapping persist?

**Recommended answer:** Persist it while the package remains installed and every newly observed HEAD still lists the package name in .SRCINFO; invalidate on removal, package-base change, membership loss, or divergence.

**User answer:** A — Persist the mapping with continuous .SRCINFO membership revalidation.

**Status:** resolved

**Notes:** Fresh confirmation is required if an invalidated mapping later reappears.

### 103. If all installed split-package names for a package base are removed while work is queued or active, what should happen?

**Recommended answer:** Remove unstarted background items from the runnable queue but retain ledger/results; let an already accepted Pi scan finish, mark it No longer installed, and do not advance a live installed-package baseline; allow explicit cancellation.

**User answer:** A — Drop queued work and let an already accepted active scan finish.

**Status:** resolved

**Notes:** Observation for that package base stops once no installed names remain.

### 104. If the package base is installed again later, may Pacsea reuse an earlier exact-identity complete result?

**Recommended answer:** Reuse only when current AUR commit, source manifests, prompt/tool/schema versions, and model policy match exactly; label installed provenance separately; never reuse based only on package version.

**User answer:** A — Reuse only on a complete exact identity match.

**Status:** resolved

**Notes:** If any identity component differs, queue the package as Unbaselined.

### 105. Should v1 still support one-time import of heads.tsv/heads.tsv.gz from the external watcher scripts?

**Recommended answer:** Defer legacy import with external-watcher interoperability so native-only v1 establishes its own explicit observation cursor and baseline without carrying external schema/import complexity.

**User answer:** A — Defer legacy heads import with external integration.

**Status:** resolved

**Notes:** Remove legacy_import.rs, related settings, tests, workstream tasks, and acceptance claims from first-release scope.

### 106. Can Pacsea reuse an existing process-group cancellation implementation for Pi child-tree cleanup?

**Recommended answer:** No. Add a dedicated Unix process group for each Pi subprocess and use bounded SIGTERM then SIGKILL group escalation with explicit wait/reap; extend the existing nix dependency only with the minimal process/signal features needed.

**User answer:** Resolved from codebase evidence: repository search found no existing pre_exec/process_group/setpgid/setsid/killpg or child-kill escalation implementation, while Cargo.toml already depends on nix without process/signal features.

**Status:** resolved

**Notes:** Use safe direct APIs such as Unix Command process_group(0) plus nix::sys::signal::killpg; avoid unsafe libc/pre_exec when a safe API is available. Central integration owns Cargo.toml/Cargo.lock and cargo audit/deny checks.

### 107. Which resource bounds are authoritative where the original plan table conflicts with later interview decisions?

**Recommended answer:** The explicit interview decisions supersede the draft table and must replace it before implementation.

**User answer:** Resolved from the user's prior explicit choices: 100 MiB/source and 250 MiB/package downloads; 256 MiB expanded bytes, 10,000 entries, depth 16, ratio 10:1; 16 MiB analyzable text/file; 250 Pi tool calls and 16 MiB tool results/model attempt; 5 minutes/model attempt and 12 minutes/logical scan; 4 MiB final JSON and 500 findings; Pi concurrency 1.

**Status:** resolved

**Notes:** Per-call tool limits remain read 64 KiB, grep 200 matches/128 KiB, find/ls 500 entries/128 KiB. The implementation plan and acceptance tests must remove obsolete 64 MiB/2,000-entry/512 KiB/4 MiB/90-second/256 KiB/200-finding values.

### 108. Should v1 extract archives in-process or invoke system bsdtar?

**Recommended answer:** Use in-process Rust readers, iterate entries manually, and materialize only validated directories/regular files; never call broad unpack helpers; choose compression decoders after advisory/license/unsafe review.

**User answer:** A — Use in-process Rust entry-by-entry extraction.

**Status:** resolved

**Notes:** Wave 0 must document exact crates, versions, licenses, unsafe/transitive dependency review, cargo audit/deny results, and archive-bomb regression tests.

### 109. How should unattended queue execution relate to the 15-minute head-observation cycle?

**Recommended answer:** Decouple them: observe every 15 minutes while a sequential runner processes backlog continuously when Pacsea is open, subject to pause, rolling budgets, and 5 job starts per rolling hour.

**User answer:** A — Use a continuous sequential runner with at most 5 job starts per rolling hour.

**Status:** resolved

**Notes:** Remove the obsolete 5-jobs/300-second per-update-cycle model. Observation discovers work; execution has independent rate/budget controls.

### 110. Should an explicitly confirmed manual scan be blocked when unattended hourly, token, or cost caps are exhausted?

**Recommended answer:** No. Manual scans may bypass unattended caps after showing the current estimate and requiring confirmation; record usage separately while retaining all per-scan safety limits.

**User answer:** A — Manual scans bypass unattended caps with explicit confirmation.

**Status:** resolved

**Notes:** The confirmation must display the worst-case primary/fallback/retry reservation.

### 111. When the user presses p, should Pacsea abort the active scan or pause only before the next queue item?

**Recommended answer:** Let the active logical scan finish, then pause before starting another item; use x for explicit cancellation to avoid wasting paid work.

**User answer:** A — Pause after the active scan finishes.

**Status:** resolved

**Notes:** Pause state persists across workspace detach/reopen but not necessarily application restarts unless separately decided.

### 112. If the user presses x during a logical scan, may Pacsea continue with the next fallback model?

**Recommended answer:** No. Cancellation aborts the entire logical scan, suppresses correction/fallback, terminates and reaps Pi after grace, and records Cancelled with consumed/reserved usage.

**User answer:** A — Cancel the whole logical scan.

**Status:** resolved

**Notes:** Cancellation never advances the accepted baseline.

### 113. Should an explicit queue pause survive Pacsea restarts?

**Recommended answer:** Yes. Persist user-initiated pause and require explicit resume so reopening cannot unexpectedly restart paid background work; distinguish it from automatic budget/service pauses.

**User answer:** A — Persist explicit pause across restarts.

**Status:** resolved

**Notes:** Only the user can clear an explicit pause.

### 114. When rolling unattended budgets become available again, should a budget-paused queue resume automatically?

**Recommended answer:** Yes, if background consent remains valid and there is no explicit user pause; recompute reservations, notify, and require relevant readiness checks for non-budget pauses.

**User answer:** Yes — choose A: automatically resume after revalidation.

**Status:** resolved

**Notes:** Automatic resume is never allowed to override a persisted user pause.

### 115. How long may Pacsea delay exit while aborting Pi, reaping children, cleaning private workspaces, and durably persisting queue state?

**Recommended answer:** Allow 10 seconds total: request RPC abort, escalate after the 5-second grace, then complete bounded cleanup/persistence; persist a recovery marker if durability fails.

**User answer:** A — Use a 10-second total shutdown deadline.

**Status:** resolved

**Notes:** Startup must surface any recovery marker and fail closed on uncertain queue/budget state.

### 116. Should the Pi scanner be hidden behind a Cargo compile-time feature in addition to the default-off runtime setting?

**Recommended answer:** No. Compile it normally with clear Arch/Linux cfg runtime boundaries, gate every trigger behind pi_scan_enabled=false by default, and keep CI coverage active.

**User answer:** A — Use runtime gating only and compile the scanner by default.

**Status:** resolved

**Notes:** Windows must continue to compile and report the runtime feature unavailable.

### 117. Should users be able to raise acquisition, extraction, tool, output, or timeout safety caps through settings.conf?

**Recommended answer:** No in v1. Expose settings that may only lower hard compiled maxima and reject higher values with actionable config errors; operational retention and unattended budgets may remain lowerable.

**User answer:** A — Users may lower but not raise hard safety caps.

**Status:** resolved

**Notes:** Persist and display both configured effective values and compiled maxima.

### 118. Which public-key service should v1 allow?

**Recommended answer:** Use only the keys.openpgp.org VKS full-fingerprint endpoint; require exact fingerprint match and mark verification unavailable/Incomplete on 404, outage, or rate limit rather than trying arbitrary keyservers.

**User answer:** A — Use keys.openpgp.org VKS only.

**Status:** resolved

**Notes:** Endpoint: https://keys.openpgp.org/vks/v1/by-fingerprint/<FULL_UPPERCASE_FINGERPRINT>. Source: https://keys.openpgp.org/about/api.

### 119. What rolling 24-hour token limit should setup propose when background scanning is enabled?

**Recommended answer:** Use 250,000 tokens, counting exact or conservative estimated usage across Pi retries and fallback models; require explicit acceptance and reserve before launch.

**User answer:** C — Use a 500,000-token rolling 24-hour unattended cap.

**Status:** resolved

**Notes:** The user may lower but not bypass the configured cap for unattended work; manual scans follow the separate confirmation policy.

### 120. What rolling 24-hour dollar cap should setup propose for direct API/OpenRouter background scans?

**Recommended answer:** Use $0.00 by default, disabling paid unattended scans until the user explicitly enters a nonzero cap; subscription-backed routes still obey the token cap.

**User answer:** A — Default the paid unattended cost cap to $0.00 and require nonzero opt-in.

**Status:** resolved

**Notes:** Stale pricing behavior follows turns 35-37; direct/OpenRouter scans cannot reserve against a zero cap.

### 121. Should setup automatically populate fallback models?

**Recommended answer:** Start with an empty fallback list and require explicit addition/order after provider, privacy, readiness, and pricing disclosure.

**User answer:** B — Automatically add the cheapest configured model as a fallback.

**Status:** resolved

**Notes:** Eligibility, privacy-class constraints, tie-breaking, and user confirmation remain to be defined.

### 122. Which models may qualify as the automatically selected cheapest fallback?

**Recommended answer:** Only an available distinct provider/model in the same privacy class as the primary, with known/estimated pricing and completed readiness probe; show it preselected but require setup confirmation; otherwise leave empty.

**User answer:** A — Use the cheapest eligible model in the same privacy class with explicit confirmation.

**Status:** resolved

**Notes:** A failed readiness probe does not disqualify the model under turns 40-41, but its warning must remain visible at confirmation and runtime.

### 123. How should setup rank eligible automatic fallback models by price?

**Recommended answer:** Compare each model's worst-case reservation for the configured scan profile, including estimated input/output/cache rates, thinking level, Pi retry multiplier, and correction allowance; do not rank by input price alone.

**User answer:** A — Rank by lowest worst-case reserved cost.

**Status:** resolved

**Notes:** Subscription-backed routes are dollar-zero under the earlier decision, so tie-breaking must use token reservation and capability/context fit rather than nominal API price.

### 124. When several eligible subscription/local models have $0 cost, how should Pacsea choose the automatic fallback?

**Recommended answer:** Use deterministic readiness, capability-fit, token-reservation, and stable-ID tie-breaks.

**User answer:** E — The user should choose among tied eligible fallback models.

**Status:** resolved

**Notes:** If the cheapest eligible price is tied, setup must present the tied candidates and require explicit selection; do not auto-populate a fallback until the tie is resolved.

### 125. What thinking level should setup preselect for the primary and fallback models?

**Recommended answer:** Preselect medium, validate supported levels per model, and adjust only downward when unsupported while showing the adjustment; never silently increase reasoning/cost.

**User answer:** A — Default to medium with downward-only compatibility adjustment.

**Status:** resolved

**Notes:** The exact selected level remains part of consent, scan identity, pricing reservation, and result provenance.

### 126. How long may Pacsea reuse a successfully fetched signing key from keys.openpgp.org?

**Recommended answer:** Cache exact fingerprint-bound public keys privately for 24 hours; after expiry refetch and mark verification Incomplete on refresh failure rather than using stale material.

**User answer:** B — Cache exact fingerprint-bound public keys for 7 days.

**Status:** resolved

**Notes:** After 7 days, refetch is required; if refresh fails, declared-signature verification is unavailable and the source is Incomplete. Persist only fingerprint/status with results, not key bodies.

## Agreed Decisions

- DG-1 uses direct immutable .SRCINFO acquisition with HTTPS static sources and git+https only.
- Supported source containers are raw files, tar, tar.gz/tgz, tar.bz2, tar.xz/txz, tar.zst, and zip; extraction is in-process, entry-by-entry, with fail-closed path/link/type validation.
- Static sources require a strong declared digest; declared OpenPGP signatures are mandatory and use isolated gpg/gpgv plus keys.openpgp.org, with a seven-day exact-fingerprint key cache.
- No persistent upstream source cache; extracted/source/Pi workspaces are ephemeral. Durable state contains manifests, provenance, bounded evidence, ledger, queue, baseline, and retained typed results.
- Only full Git commit OIDs can yield complete VCS identity. Mutable refs are resolved for advisory scanning but remain incomplete and are rechecked for staleness.
- Native-only v1 uses official AUR per-package Git repositories. External watcher, spool integration, and legacy heads import are deferred.
- Observation runs at startup/manual refresh and no more than every 15 minutes, with sequential head queries and resumable every-commit expansion capped at 500 commits/package and 2,000/cycle.
- The background model runner is sequential and independent from observation, with at most five starts per rolling hour, a 500,000-token rolling 24-hour cap, and $0 default paid cap.
- Manual scans may bypass unattended caps after explicit worst-case confirmation. User pause persists; budget pauses auto-resume after revalidation.
- Pi 0.84.0 is the minimum. Required CLI/RPC isolation behavior is capability-probed fail-closed; newer versions have no hard maximum if probes pass.
- Pi uses its standard auth store only. Pacsea does not pass provider API-key environment variables or permit credential shell-command resolution.
- Readiness failures remain warnings: the user chose to allow manual and background scans normally, while strict per-scan identity/schema/tool/output validation remains mandatory.
- Pi internal retries remain enabled but are capped at three attempts per low-level request and fully reserved/accounted.
- A configured model fallback list may continue in-session with RPC set_model; at most three models are attempted, and schema-valid findings are merged by exact evidence fingerprint with visible disagreement and highest severity gating.
- The automatic fallback is the cheapest eligible distinct model in the same privacy class, confirmed during setup; tied zero-cost candidates require user selection.
- Remote package identity disclosure is limited to the current scan. Custom endpoints use Local, Private network, or Remote classification with remote-by-default hostnames.
- Hard bounds include 100 MiB/source and 250 MiB/package compressed downloads; 256 MiB expanded bytes, 10,000 entries, depth 16, ratio 10:1; 16 MiB analyzable text/file; 250 tool calls and 16 MiB tool results/model attempt; 5 minutes/model attempt and 12 minutes/logical scan; 4 MiB final JSON and 500 findings.
- Dry-run may perform read-only AUR/source acquisition and isolated checksum/signature verification, but never launches Pi or mutates durable queue/baseline/ledger/result/budget state.
- Background observation continues while background Pi execution is off, preserving backlog without paid calls.
- Complete means exhaustive manifesting/deterministic coverage plus mandatory risk-based model reads, not that every source file was semantically reviewed and never that the package is safe.
- Deterministic findings cannot be suppressed by models. Benign verdicts carry only across exact detector/evidence/manifest identity.
- High/critical continuation uses deliberate result-bound acknowledgement. Stale identity invalidates prior acknowledgement and requires separate confirmation.
- Results persist as typed data only; canonical JSON backs the Raw view. Beyond normal retention, keep the newest result and the current baseline result when different.
- Corrupt/newer-schema state is atomically quarantined and fails closed, with package-scoped reset and guarded global reset actions.
- Shift+A opens the context-sensitive Pi Scan workspace from Search Normal mode. Status combines active progress with prioritized unresolved severity/state.
- HTTPS acquisition permits at most five revalidated HTTPS redirects, public Internet destinations only, no ambient proxies, and system TLS trust only.
- User-supplied manual mappings are limited to canonical official AUR Git URLs and require immutable .SRCINFO membership proof with continuous revalidation.
- Feature code is compiled by default with runtime gating; pi_scan_enabled remains false by default and Windows remains compile-only/unavailable at runtime.
- Hard safety caps may be lowered but not raised through settings.

## Open Risks

- Exact archive/compression/hash crate versions remain a Wave 0 dependency-audit and benchmark decision; broad unpack helpers are prohibited.
- Allowing readiness-failed models for unattended scans raises reliability risk despite strict per-scan validation.
- In-session cross-model continuation and merged findings substantially increase protocol, provenance, accounting, and test complexity.
- User-selected stale pricing reuse after refresh failure can underestimate direct/OpenRouter spend; the token cap remains authoritative.
- Subscription-backed routes are treated as zero dollar cost, so provider quota/subscription limits are represented only through token accounting and warnings.
- Dry-run performs network acquisition and public-key lookup, which differs from a fully local dry-run and must be prominently disclosed.
- The larger tool/result/time limits increase denial-of-service and cost exposure; Wave 0 benchmarks may reduce but never silently increase them.
- Exact installed-package bytes remain unprovable without a future exact-snapshot build/install feature.
- Prompt injection and model false negatives remain residual risks even with restricted tools and strict validation.
- The canonical plan and acceptance inventory must be revised to remove external/legacy work, update dry-run expectations, replace obsolete resource bounds, and incorporate pricing/fallback decisions before implementation.

## Next Decision Needed

No additional product decision blocks plan revision. Wave 0 must select and audit exact Rust archive/compression/hash dependencies, benchmark the approved maxima, and convert the installed Pi capability evidence into automated fail-closed probes.
