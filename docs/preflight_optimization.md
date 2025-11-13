# Pacsea Preflight Optimization Blueprint

## Current Pipeline Overview

### Trigger Points
- `src/events/search.rs` and `src/events/install.rs` call `logic::preflight::compute_preflight_summary` synchronously when the user opens the modal.
- `src/events/preflight.rs::handle_preflight_key` lazily kicks off deeper analyses when the user visits each tab. It sets `AppState.preflight_*_resolving` flags and stores the package list in `AppState.preflight_resolve_items`.
- `src/app/runtime.rs` listens for the flags on each tick and dispatches work to background workers via `tokio::spawn_blocking` channels.

### Background Workers
| Stage | Worker location | Entry function | Notes |
|-------|-----------------|----------------|-------|
| Dependencies | `runtime.rs` lines 436-470 | `logic::deps::resolve_dependencies` | Heavy pacman queries, synchronous logging warns if not called from a blocking thread. |
| Files | `runtime.rs` lines 473-508 | `logic::files::resolve_file_changes` | Sequential `pacman -Fl`, remote fetch, pacnew detection. |
| Services | `runtime.rs` lines 512-535 | `logic::services::resolve_service_impacts` | Two `pacman -Fl` passes, `systemctl` probes. |
| Sandbox | `runtime.rs` lines 539-563 | `logic::sandbox::resolve_sandbox_info` | Fetches `.SRCINFO`/PKGBUILD via `curl`, parses dependency arrays. |

Each worker sends results over an unbounded channel back into the main loop. Completion toggles the corresponding `AppState.install_list_*` cache, clears the `preflight_*_resolving` flag, and optionally flushes disk caches.

### Data Caches
- `src/app/deps_cache.rs`, `files_cache.rs`, `services_cache.rs`, `sandbox_cache.rs` persist install-list analyses keyed by a signature (package names + versions).
- Preflight tabs reuse these caches when the modal selection matches the install list. Mismatched selections fall back to on-demand computation, leading to duplicate work.
- The sandbox cache is scoped to AUR packages only; when none are present the tab simply marks itself as loaded.

### UX Characteristics
- Tab bodies render immediately but show loading spinners until background data arrives (`ui/modals/preflight.rs`).
- The modal stores selection state per tab but cannot cancel in-flight jobs; switching away leaves work running.
- Progress is opaque beyond a spinner. Users perceive the stages completing in the order: Sandbox -> Deps -> Files -> Services because each launches only after the previous tab interaction.

## Performance Pain Points
- **Serial kick-off**: `handle_preflight_key` schedules just one analysis at a time. Opening Summary, then Deps, then Files requires waiting for each stage sequentially.
- **Shared queue**: `AppState.preflight_resolve_items` holds a single vector. Sandbox requests overwrite install items with an AUR subset, forcing re-population for other stages.
- **Heavy synchronous commands**: Dependency and file resolvers run multiple `pacman` commands per package. Without batching, they saturate disk and CPU.
- **Network-bound sandbox**: `resolve_sandbox_info` fetches `.SRCINFO`/PKGBUILD sequentially using `curl`, so multiple AUR packages multiply latency.
- **UI tight loop**: Summary computation happens on the UI thread. Large transaction sets can freeze the modal before it appears.
- **Cache reuse gaps**: Preflight often operates on ad-hoc selections that differ from the persisted install list, so cached data is ignored even when reusable.

## Optimization Strategy

### Stage 0 – Instrumentation & Guardrails
- Add `tracing::info_span` blocks around each resolver (`logic/preflight.rs`, `logic/deps.rs`, `logic/files.rs`, `logic/services.rs`, `logic/sandbox.rs`) to capture per-stage duration and per-package timings.
- Emit structured metrics (`stage`, `item_count`, `duration_ms`) from `runtime.rs` when workers finish, and surface aggregated telemetry in debug logs.
- Introduce a hidden `PACSEA_PREFLIGHT_TRACE=1` env toggle to dump detailed per-stage timing without overwhelming default logs.

### Stage 1 – Pipeline Restructure
1. **Parallel kick-off** (`events/preflight.rs`): when opening the modal, queue dependency, file, service, and sandbox jobs in one pass. Maintain dedicated `preflight_pending` structs per stage to avoid overwriting `preflight_resolve_items`.
2. **Non-blocking summary** (`events/search.rs` & `events/install.rs`): move `compute_preflight_summary` into a blocking task. Render the modal with `summary: None` and hydrate once ready.
3. **Order-of-operations**: respect actual dependencies—services rely on file lists but only for metadata, so prefetch concurrently while still allowing staged UI reveals.
4. **Cancellation hooks**: add per-stage cancellation tokens stored in `AppState` so closing the modal or changing selections aborts obsolete work.

### Stage 2 – Data-Layer Enhancements
- **Dependencies** (`logic/deps/`): batch `pacman -Qi`/`-Si` queries, cache `get_installed_packages` and `get_provided_packages` across requests, and consider using libalpm bindings to avoid repeated command spawning.
- **Files** (`logic/files.rs`): replace repetitive `pacman -Fl` calls with a single `-Fl` using multiple package names or pre-read `.files` databases. Persist remote lists in the files cache keyed by package version.
- **Services** (`logic/services.rs`): reuse the file resolver’s cached file lists to avoid second `pacman -Fl` passes; memoize `systemctl` calls per tick.
- **Sandbox** (`logic/sandbox.rs`): switch to asynchronous HTTP (e.g., `reqwest` with HTTP/2) and run fetches with `FuturesUnordered` to parallelize. Cache `.SRCINFO` blobs on disk keyed by package+version for reuse across sessions.
- **Shared caching**: unify signature generation so arbitrary modal selections can hit caches (e.g., compute a selection hash independent of install list ordering).

### Stage 3 – UX & Responsiveness
- `ui/modals/preflight.rs`: display per-stage status with progress bars / count-down (packages processed vs total). Highlight stages completing out of order to reinforce parallelism.
- `events/preflight.rs`: eagerly populate `sandbox_loaded`, `services_loaded`, etc., as soon as data arrives and trigger toast or inline messaging for slow stages.
- Provide a `Shift+R` shortcut to re-run all analyses in one go after settings change, leveraging cached data for incremental updates.

### Stage 4 – Validation & Regression Safety
- Add integration tests in `tests/` that stub command runners, ensuring the modal hydrates all stages when data arrives out-of-order.
- Create a benchmark harness (feature-gated) that runs synthetic install lists through the resolvers to compare before/after timings.
- Write release notes documenting new env toggles and user-facing status indicators.

## Implementation Roadmap
1. **Milestone A (Tracing + Async Summary)**
   - Files: `logic/preflight.rs`, `events/search.rs`, `events/install.rs`, `app/runtime.rs`.
   - Deliverables: non-blocking summary, instrumentation glue, baseline benchmark numbers.
2. **Milestone B (Parallel Dispatch + Cancellation)**
   - Files: `events/preflight.rs`, `state/app_state.rs`, `app/runtime.rs`.
   - Deliverables: dedicated per-stage queues, abort support, UI spinner updates.
3. **Milestone C (Resolver batching + caching)**
   - Files: `logic/deps/*`, `logic/files.rs`, `logic/services.rs`, `logic/sandbox.rs`, cache modules.
   - Deliverables: reduced external command count, disk-backed blob caches, documented cache invalidation rules.
4. **Milestone D (UX polish + Telemetry surfacing)**
   - Files: `ui/modals/preflight.rs`, `events/preflight.rs`, new docs/tests.
   - Deliverables: progress indicators, hotkeys, regression tests, updated docs.

Each milestone should ship independently with gated feature flags where practical, enabling gradual rollout.

## Appendix A – Key Symbols & References
- `AppState` preflight flags: `src/state/app_state.rs` lines 386-395.
- Preflight modal renderers: `src/ui/modals/preflight.rs` (overall modal), `preflight_exec.rs` (exec view).
- Runtime worker loop: `src/app/runtime.rs` lines 430-870.
- Dependency resolver core: `src/logic/deps/resolve.rs` and `src/logic/deps.rs`.
- File resolver core: `src/logic/files.rs`.
- Service resolver core: `src/logic/services.rs`.
- Sandbox resolver core: `src/logic/sandbox.rs`.

## Appendix B – Verification Checklist
- [ ] All stages log duration and item counts under `TRACE`.
- [ ] Modal opens instantly with placeholder summary.
- [ ] Cancelling the modal drops background workload.
- [ ] Caches persist across sessions and invalidate on signature mismatch.
- [ ] UI indicates when sandbox/service data is unavailable for non-AUR or remove flows.
