High-impact priorities (80/20 view)
- Pre-flight guardrails: add disk space, mirror health, and pacman lock checks with actionable fixes before confirmation (install/remove/update).
- Richer dry-runs: show conflicts, reverse deps/orphans, and estimated download/space/time for batch and direct flows.
- Clear confirms and retries: unified “what will happen”, continue-on-failure toggle, and guided retry with alternate helper/mirror on failure.
- Safer system updates: optional pre-snapshot/rollback note, mirror freshness check, conflict auto-scope, and reboot scheduling (now/later/remind).
- Startup sanity: single blocking popup queue, offline notice with limited actions + retry timer, remember dismissals per session.

General improvements (status at a glance)
| Suggestion | Status | Notes |
| --- | --- | --- |
| Standardize pre-flight checks (tools/disk/mirror/lock + fixes) | Partially implemented | Metadata/risk only; no disk/mirror/lock checks or fix hints. |
| Rich dry-run output (actions/conflicts/orphans/ETA) | Partially implemented | Size deltas + DRY RUN echo; missing conflicts/orphan/ETA. |
| Consistent confirm UX (what happens, per-item overrides) | Partially implemented | Modal flows + keybind blocking; no unified summary outside preflight. |
| Resilience (alt mirrors/tool, lock handling, structured logs) | Partially implemented | Paru/yay fallback only; no lock or mirror fallback, no retry guidance. |
| Performance (cache/batch/parallel reads) | Implemented | Metadata/deps caches + batched pacman; limited parallelism beyond batching. |
| State recovery/resume | Not implemented | No resumable batches or rollback hints. |

Removal via batch
| Suggestion | Status | Notes |
| --- | --- | --- |
| Reverse-dependency tree + skip/force/keep | Implemented | Preflight Deps tab shows dependents and meta warnings. |
| Base/protected guard (typed confirm/disable force) | Partially implemented | Core/system notes only; no typed confirmation or hard block. |
| Orphan cleanup preview/staged cleanup | Not implemented |  |
| Blocked-item reasons + retry guidance | Not implemented |  |
| Parallel dry-run dep checks | Not implemented |  |

```1:8:dev/WORKFLOWS/developer/removal_via_batch.mmd
A([Start]) --> B[Add packages to Removal list]
C -- No --> E[Normalize list: dedupe, verify installed]
F -- Yes --> F1[Warn; require explicit override per item]
```

System update (all options)
| Suggestion | Status | Notes |
| --- | --- | --- |
| Pre-snapshot/rollback note + disk/mirror health pre-check | Not implemented |  |
| Auto-scope conflict resolution | Not implemented |  |
| Guided retry with alt helper/mirror + log link | Not implemented |  |
| Reboot scheduling (now/later/remind) | Not implemented |  |

```1:14:dev/WORKFLOWS/developer/system_update_all_options.mmd
B -- Dry-run --> D[Simulate update; detect conflicts]
I -- Yes --> J{Confirmation?}
K --> L[Execute update per scope]
```

Install via batch
| Suggestion | Status | Notes |
| --- | --- | --- |
| Consolidated plan (source/version/size/conflicts/deps) | Implemented | Preflight summary covers source/version/size; conflicts not surfaced. |
| Continue-on-failure toggle + retry buttons | Not implemented |  |
| Prefetch deps in parallel and reuse | Partially implemented | Batched fetch + cached deps/files/services; no parallel resolver toggle. |
| Optional post-install steps | Not implemented |  |

```1:9:dev/WORKFLOWS/developer/install_via_batch.mmd
E --> F{Conflicts or protected pkgs?}
H -- Yes --> I[Run dry-run batch; collect per-item status]
M -- Yes --> N[Resolve deps for item]
```

Install direct over results
| Suggestion | Status | Notes |
| --- | --- | --- |
| Better metadata error with retry/alt source | Not implemented |  |
| Dry-run deps/conflicts inline with result | Not implemented | Direct flow bypasses preflight. |
| Retry with different helper/mirror while keeping selection | Partially implemented | Paru/yay fallback only; no UI retry. |

```1:7:dev/WORKFLOWS/developer/install_direct_over_results.mmd
B -- Yes --> D{Package metadata ok?}
F -- Yes --> G[Run dry-run install; capture deps/conflicts]
```

Downgrade via batch
| Suggestion | Status | Notes |
| --- | --- | --- |
| Cache versions/signatures + provenance/integrity before confirm | Not implemented |  |
| Rollback/hold guidance after success | Not implemented |  |
| Pre-download + verify before mutate | Not implemented |  |
| Skip/unhold suggestions when unavailable | Not implemented |  |

```1:9:dev/WORKFLOWS/developer/downgrade_via_batch.mmd
E["Resolve available versions (cache or repos)"] --> F{Version selectable?}
G --> H{Tool available?<br/>pacman/paru/yay}
I -- Yes --> J[Run dry-run downgrade; list changes and risks]
```

App startup popups
| Suggestion | Status | Notes |
| --- | --- | --- |
| Single blocking popup with priority queue | Not implemented |  |
| Remember dismissals per session | Not implemented |  |
| Stale cache prompt with last refresh + background refresh | Not implemented |  |
| Offline notice with limited actions + retry timer | Not implemented |  |

```1:7:dev/WORKFLOWS/developer/app_startup_popups.mmd
C -- Yes --> C1[Show offline notice; limited actions]
E -- Yes --> E1[Show failure popup with logs/retry]
F -- Yes --> F1[Show Update modal trigger]
```