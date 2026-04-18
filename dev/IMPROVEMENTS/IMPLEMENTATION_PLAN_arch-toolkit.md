# Implementation Plan: Migrating Pacsea to Use arch-toolkit

**Created:** 2025-01-XX  
**Status:** Planning (see **Progress todos** at top — dependency not added yet)  
**Target:** Replace custom AUR implementation with `arch-toolkit` crate

## Progress todos (2026-04-17 reconciliation)

**Status:** Integration is still not started in-tree — `arch-toolkit` is not present in `pacsea/Cargo.toml`, while `arch-toolkit` already contains multiple extracted modules beyond AUR.

### Current comparison snapshot

- `pacsea` runtime integration with `arch-toolkit`: **none yet** (plan-only references).
- `arch-toolkit` current coverage: **aur + deps + index + install + news + repos + system::privilege** (v0.2.x line).
- `pacsea` still owns a large duplicated surface that can now be consumed from `arch-toolkit`.

### GUI/TUI-agnostic extraction rule (authoritative)

`arch-toolkit` must stay frontend-agnostic so it can be reused by both GUI and TUI consumers:

- **Extract fully:** pure domain logic, parsers, typed models, deterministic planning, non-interactive fetch/query helpers.
- **Extract partially:** modules that mix domain logic with app orchestration; move only the domain core.
- **Do not extract:** UI state, event/modal flows, PTY/interactivity, password prompt/piping, frontend runtime wiring.

### Candidate classification for extraction scope

- **Extract fully:** deps/index/install-command-builders/news-feed/repo-config-analysis/status-parsers/details-normalization.
- **Extract core only:** repos apply planning, preflight compute engine, sandbox analysis, news article parsing + cache primitives.
- **Keep in app crates:** `executor` PTY/password flows, modal/event handlers, i18n binding decisions, ratatui/crossterm integration, background UI/runtime channel orchestration.

### Current extraction status in arch-toolkit

- [x] `aur` module exists and is published in current crate.
- [x] `deps` module exists (parse/query/resolve/reverse/version helpers).
- [x] `index` module exists (installed/explicit/query/fetch/persist/mirrors).
- [x] `install` command builder layer exists.
- [x] `news` feed/advisory baseline exists.
- [x] `repos` config and foreign-overlap analysis helpers exist.
- [x] `system::privilege` abstraction exists.

### Updated migration phases (consumer-side in pacsea)

- [ ] **Phase A1: Wire dependency + shared client**  
      Add `arch-toolkit` dependency in `pacsea/Cargo.toml`; initialize and thread shared client/context where required.
- [ ] **Phase A2: AUR call-site cutover**  
      Migrate `src/sources/{search,comments,pkgbuild}.rs` AUR paths to toolkit APIs.
- [ ] **Phase A3: Deps/index/install cutover**  
      Replace `src/logic/deps/*`, `src/index/*`, and install command-planning usages with `arch_toolkit::{deps,index,install}` where behavior matches.
- [ ] **Phase A4: News baseline cutover**  
      Use `arch_toolkit::news::{fetch_arch_news, fetch_security_advisories}` for list-level feeds while preserving current article-detail behavior in `pacsea`.
- [ ] **Phase A5: Behavior parity + cleanup**  
      Remove duplicate local implementations only after parity tests pass; keep app-only runtime/UI code in `pacsea`.

### Missing in arch-toolkit but extractable from pacsea (new backlog)

- [x] **Repos apply planner (core-only):** landed in `arch-toolkit` (`repos` module): enable features `index` + `install` (meta-feature `repos-apply`). Planning APIs mirror Pacsea paths/markers; `build_repo_apply_bundle` / `build_repo_key_refresh_bundle` take `PrivilegeMode` (via `resolve_privilege_tool_for_orchestration`). Consumer cutover remains Workstream A / Phase A3.
- [ ] **Sandbox module (core-only):** extract `src/logic/sandbox/{parse,analyze,types}.rs` as `feature = "sandbox"` (PKGBUILD risk/dependency delta analysis).
- [ ] **Preflight engine (core-only):** extract non-UI compute core from `src/logic/preflight/*` into a toolkit preflight module (summary/risk computation over package operations).
- [ ] **Status monitor (full extraction):** extract `src/sources/status/*` into `arch_toolkit::status` (status API + HTML fallback parser, toolkit-owned status/severity model).
- [ ] **Package details helper (full extraction):** extract reusable parts from `src/sources/details.rs` (official package metadata parsing and normalization) into `arch_toolkit::details`.
- [ ] **News content parity (core-only):** extract portable parts of `src/sources/news/{fetch,parse}.rs` for article parsing + conditional-fetch/cache primitives; keep app-level presentation/caching policy local.
- [ ] **Explicit non-goal guardrail:** do not move PTY/password prompt/pipes (`src/install/executor.rs` style logic), modal/event handlers, or frontend runtime wiring into `arch-toolkit`.
## Plan scope (current)

This plan now tracks two synchronized workstreams:

1. **Workstream A (pacsea integration):** replace duplicated internal logic with calls to current `arch-toolkit` modules.
2. **Workstream B (arch-toolkit parity backlog):** extract missing framework-agnostic cores from `pacsea` into `arch-toolkit`.

## Workstream A: pacsea integration plan

### A1. Dependency wiring and shared context

- Add `arch-toolkit` dependency in `pacsea/Cargo.toml`.
- Thread shared toolkit client/context through runtime where needed.
- Keep runtime/event-loop/UI ownership in `pacsea`.

### A2. AUR integrations

- Cut over `src/sources/search.rs` to toolkit AUR search.
- Cut over `src/sources/comments.rs` to toolkit AUR comments.
- Cut over AUR branch of `src/sources/pkgbuild.rs` to toolkit PKGBUILD.
- Keep official package PKGBUILD path and app-level cache policy in `pacsea`.

### A3. Deps/index/install integrations

- Replace `src/logic/deps/*` call sites with toolkit `deps` APIs.
- Replace `src/index/*` call sites with toolkit `index` APIs where behavior is equivalent.
- Replace install command planning uses with toolkit `install` builders.
- Keep app execution layer (`executor`, PTY, auth prompts) in `pacsea`.

### A4. News baseline integration

- Use toolkit feed/advisory APIs for list-level news/advisory data.
- Keep richer article-content flow local until toolkit content parity exists.

### A5. Cleanup gates

- Remove duplicated implementations only after parity tests pass.
- Keep explicit wrappers where `pacsea` behavior intentionally differs.
- Run quality gates: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check`, `cargo test -- --test-threads=1`.

## Workstream B: arch-toolkit updates still needed

### B1. Repos apply planner (core-only extraction)

- **Status:** Done in `arch-toolkit` (2026-04-18).
- Source in `pacsea`: `src/logic/repos/{apply_plan,pacman_conf}.rs` (ported).
- Target in toolkit: `arch_toolkit::repos` — `apply_plan` + `apply_curl` behind `feature = "install"`; extended `RepoRow` / `load_resolve_repos_from_str` under `index`; `pacman_conf` scan exported whenever `index` is on; convenience feature `repos-apply = ["index", "install"]` in toolkit `Cargo.toml`.
- Keep privileged execution sequencing and UX workflows in consumer apps; pass `PrivilegeMode` from app settings when calling `build_repo_apply_bundle`.

### B2. Sandbox module (core-only extraction)

- Source in `pacsea`: `src/logic/sandbox/{parse,analyze,types}.rs`.
- Target in toolkit: new `sandbox` feature/module.

### B3. Preflight compute engine (core-only extraction)

- Source in `pacsea`: `src/logic/preflight/*`.
- Target in toolkit: preflight module with typed summary/risk outputs.
- Keep modal wiring, tab state, and action dispatch in consumer apps.

### B4. Status monitor (full extraction)

- Source in `pacsea`: `src/sources/status/*`.
- Target in toolkit: `status` module with structured status output + severity types.

### B5. Package details helper (full extraction)

- Source in `pacsea`: reusable parsing/normalization pieces from `src/sources/details.rs`.
- Target in toolkit: `details` module for framework-agnostic metadata parsing helpers.

### B6. News article-content parity (core-only extraction)

- Source in `pacsea`: `src/sources/news/{fetch,parse}.rs`.
- Target in toolkit: article parsing + conditional fetch/cache primitives.
- Keep app-specific presentation/caching policy local.

## Definition of done

The migration is complete when all of the following are true:

- `pacsea` uses toolkit modules for AUR/deps/index/install/news baseline paths.
- `pacsea` retains only app-specific runtime/UI/executor layers.
- Remaining extraction backlog items in Workstream B are either implemented in toolkit or explicitly documented as deferred.
- No frontend/runtime coupling is introduced into `arch-toolkit`.

## Non-goals (must remain outside arch-toolkit)

- PTY lifecycle management and interactive password prompt/piping.
- Modal/event/render flow logic.
- ratatui/crossterm/frontend runtime orchestration.

