# Pacsea Roadmap — Consolidated Planned Work

**Last updated:** 2026-07-03 (baseline: `v0.8.2`, issues synced against GitHub the same day)

This is the **single tracking document** for all planned implementations. It replaces and
consolidates the former planning docs (`FEATURE_PRIORITY.md`, `CLI_POSSIBLE_COMMANDS.md`,
`CLI_LIBRARY_INTEGRATORS.md`, `IMPLEMENTATION_PLAN_arch-toolkit.md`,
`IMPLEMENTATION_PLAN_tui_integrated_config_editing.md`, `improvement_suggestions.md`,
`pacsea_soname_plan.md`, `PERFORMANCE_IMPLEMENTATION_PRIORITY.md`,
`REFACTORING_EVALUATION.md`, `ARCHITECTURE_OTHER.md`), keeping only **open** work.
Shipped items were dropped; see `CHANGELOG.md` for release history.

**How to track:**
- Every item is a checkbox. Check it off (and note the release/PR) when it ships.
- Items carry a priority tier and, where one exists, a GitHub issue link.
- Detail sections below the master list hold implementation notes per initiative;
  delete a detail section when its items are all done.

## Priority tiers

| Tier | Meaning | Horizon |
|------|---------|---------|
| 🔴 **P1** | High impact, reasonable complexity, core UX/CLI correctness | Next 1–2 releases |
| 🟠 **P2** | Good value, moderate complexity, extends existing systems | Next 3–4 releases |
| 🟡 **P3** | Valuable for specific use cases, medium effort | Roadmap items |
| 🟢 **P4** | Niche or higher complexity, community-driven | Future consideration |
| 🔵 **P5** | Major architectural change, long-term vision | Future major version |

---

## Master tracking list

### 🔴 P1 — Next 1–2 releases

- [x] **CLI `--update` respects `settings.conf`** — `--mirrors` opt-in mirror refresh uses `selected_countries`/`mirror_count`; new `aur_helper` settings key honored by all CLI helper resolution (historic bug [#57](https://github.com/Firstp1ck/Pacsea/issues/57), closed; shipped in `dev/PR/PR_p1_roadmap_batch.md`) → [Features §F1](#f1-cli---update-respects-tui-settings)
- [x] **CLI `-R` / remove-from-file** — implemented with `-I` parity ([#93](https://github.com/Firstp1ck/Pacsea/issues/93); shipped in `dev/PR/PR_p1_roadmap_batch.md`) → [CLI §C1](#c1-finish-existing-flags)
- [x] **Honor `--config-dir`** — process-wide override in `theme::paths` covering config resolvers, caches (`lists`), and logs (shipped in `dev/PR/PR_p1_roadmap_batch.md`) → [CLI §C1](#c1-finish-existing-flags)
- [x] **Global `--json` output** — `schema_version` envelope for `--search`, `--list`, `--news` (shipped in `dev/PR/PR_p1_roadmap_batch.md`; `updates check` still future) → [CLI §C5](#c5-integrator-contracts)
- [x] **Preflight guardrails** — pacman db-lock (blocks, CLI + TUI), disk space and sync-db staleness warnings with actionable fixes (shipped in `dev/PR/PR_p1_roadmap_batch.md`; richer dry-run/confirm UX remains P2) → [UX §U1](#u1-cross-cutting-preflight)
- [x] **Refactor: unify command execution** — shared `util::command` runner (`CommandRunner`/`SystemCommandRunner`/`CommandError` + `run_capture` + `binary_available`); `logic/preflight`, `logic/services`, and `util/pacman` delegate; duplicated paru/yay probes consolidated (shipped in `dev/PR/PR_p1_roadmap_batch.md`; interactive/PTY spawns intentionally stay separate) → [Refactoring §R1](#r1-priority-refactors)
- [x] **Refactor: consolidate cache modules** — shared `app/cache_common.rs` (match modes, load/save, signatures); the four cache modules and `persist.rs` flush fns are thin wrappers, on-disk format unchanged (shipped in `dev/PR/PR_p1_roadmap_batch.md`) → [Refactoring §R1](#r1-priority-refactors)

### 🟠 P2 — Next 3–4 releases

- [ ] **Button/focus tooltips** — contextual hints for discoverability ([#140](https://github.com/Firstp1ck/Pacsea/issues/140)) → [Features §F2](#f2-tooltips)
- [ ] **Distro-specific news feeds** — EndeavourOS, Manjaro, Garuda, CachyOS ([#131](https://github.com/Firstp1ck/Pacsea/issues/131)) → [Features §F3](#f3-distro-news)
- [ ] **Richer dry-run output** — conflicts, reverse deps/orphans, rough size/time estimate for batch and direct flows → [UX §U1](#u1-cross-cutting-preflight)
- [ ] **Unified confirm UX** — one "what will happen" summary with per-item overrides beyond preflight-only views → [UX §U1](#u1-cross-cutting-preflight)
- [ ] **CLI scripting surface** — `doctor`, `which-helper`, `updates check`, `pkg show`/`pkg outdated`, `completions <shell>` → [CLI §C2](#c2-scripting--parity-subcommands)
- [ ] **CLI TUI-parity** — `preflight install|remove|update`, `pkgbuild check`, `repo list|validate|diff`, `aur vote|unvote|ssh-setup` → [CLI §C2](#c2-scripting--parity-subcommands)
- [ ] **CLI launch/install refinements** — `--mode package|news`, `--select <pkg>`, `--no-mouse`/`--mouse`, `-y`/`--refresh`, `--install` + `--as-deps`/`--needed`/`--aur-only`/`--repo-only`, granular `cache list`/`cache clear <kind>` → [CLI §C2](#c2-scripting--parity-subcommands)
- [ ] **arch-toolkit Phase A1–A2** — add dependency, wire shared client, cut AUR call sites (`search`, `comments`, `pkgbuild`) over to toolkit APIs → [arch-toolkit §T1](#t1-workstream-a-pacsea-consumes-arch-toolkit)
- [ ] **Refactor: dedupe package validation** — six similar validators in `args/package.rs` → shared helpers → [Refactoring §R1](#r1-priority-refactors)
- [ ] **Refactor: adopt or remove `util/config.rs`** — 13 files with inline string parsing vs unused utility → [Refactoring §R1](#r1-priority-refactors)
- [x] **Config editor Phase 1** — settings-center modal shell + General tab (boolean/string keys, dry-run gated) — shipped in PR #161 as a dedicated `AppMode::ConfigEditor` → [Config editing §E1](#e1-phased-rollout)

### 🟡 P3 — Roadmap items

- [ ] **Mirror search & selection UI** — interactive browser with country/speed filters and mirror stats ([#145](https://github.com/Firstp1ck/Pacsea/issues/145)) → [Features §F4](#f4-mirror-browser)
- [ ] **Update grouping by criticality** — kernel/systemd/core vs regular packages in update preview (part of umbrella [#134](https://github.com/Firstp1ck/Pacsea/issues/134)) → [Features §F5](#f5-upgrades-umbrella)
- [ ] **Service restart guidance after updates** ([#99](https://github.com/Firstp1ck/Pacsea/issues/99)) → [Features §F5](#f5-upgrades-umbrella)
- [ ] **Transaction abort / cancellation UX** ([#98](https://github.com/Firstp1ck/Pacsea/issues/98)) → [Features §F5](#f5-upgrades-umbrella)
- [ ] **Sequential multi-package AUR scans** ([#95](https://github.com/Firstp1ck/Pacsea/issues/95)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Optional-dependency descriptions (ALPM/AUR)** ([#102](https://github.com/Firstp1ck/Pacsea/issues/102)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Update packages tracked via GitHub upstreams** ([#104](https://github.com/Firstp1ck/Pacsea/issues/104)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Accessibility themes** — high-contrast/WCAG palette, ASCII symbol fallbacks ([#129](https://github.com/Firstp1ck/Pacsea/issues/129)) → [Features §F7](#f7-accessibility)
- [ ] **Config validation for config file values** ([#97](https://github.com/Firstp1ck/Pacsea/issues/97)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Tests for `parse_update_entry`** ([#94](https://github.com/Firstp1ck/Pacsea/issues/94)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Normalize executable bits on Python helper scripts** ([#159](https://github.com/Firstp1ck/Pacsea/issues/159)) → [Features §F6](#f6-smaller-tracked-features)
- [ ] **Batch-flow hardening** — removal guards (protected packages, orphan preview, blocked-item reasons), update retry/reboot scheduling, install continue-on-failure, downgrade provenance/pre-download, startup popup queue/offline notice → [UX §U2](#u2-per-flow-improvements)
- [ ] **CLI extended queries** — `pkg files|owns|deps|provides|conflicts|orphans|foreign|native|group|required-by`, `news` subcommands, `files pacnew|pacsave`, `services affected`, `sandbox analyze`, `repo apply`, `aur scan` batch, transaction helpers (`sync`, `upgrade --aur-only`, `downgrade`, `reinstall`, `clean`) → [CLI §C3](#c3-roadmap-subcommands)
- [ ] **arch-toolkit Phase A3–A5** — deps/index/install cutover, news baseline cutover, parity cleanup → [arch-toolkit §T1](#t1-workstream-a-pacsea-consumes-arch-toolkit)
- [ ] **arch-toolkit Workstream B** — extract sandbox, preflight engine, status monitor, details helper, news content parity (B1 repos-apply planner already landed) → [arch-toolkit §T2](#t2-workstream-b-extraction-backlog)
- [x] **Config editor Phase 2–3** — keybind capture + persistence; theme tab with whole-file pre-commit validation — shipped in PR #161 → [Config editing §E1](#e1-phased-rollout)
- [ ] **Soname Layer 1** — `DT_NEEDED`/`DT_SONAME` readers, on-disk soname map, `.pkg.tar.zst` extraction, Preflight tab → [Soname §S1](#s1-layer-1--end-user-protection)
- [ ] **Performance: open items** — map/B-tree package index (if profiling supports), streaming search results, stronger lazy loading, remaining `.iter().any()` tightening, criterion benches → [Performance §P1](#p1-open-optimizations)
- [ ] **Architecture: incremental refactors** — split `AppState` into domain substates, group preflight channels, extract pure logic from fat handlers → [Refactoring §R2](#r2-architecture-backlog)

### 🟢 P4 — Future consideration

- [ ] **Dependency conflict resolution wizard** — interactive conflict handling (part of [#134](https://github.com/Firstp1ck/Pacsea/issues/134)) → [Features §F5](#f5-upgrades-umbrella)
- [ ] **Custom upgrade commands / pre-post hooks** (part of [#134](https://github.com/Firstp1ck/Pacsea/issues/134)) → [Features §F5](#f5-upgrades-umbrella)
- [ ] **System tray / panel integration** ([#129](https://github.com/Firstp1ck/Pacsea/issues/129)) → [Features §F7](#f7-accessibility)
- [ ] **AUR maintainer tools** — PKGBUILD updates, pushing, co-maintainers (part of [#130](https://github.com/Firstp1ck/Pacsea/issues/130))
- [ ] **CLI niche commands** — `config get|set|validate`, `theme show`, `keybinds list`, `--print-default-config`, `man`, `--locale`, `unlock` (heavily gated) → [CLI §C4](#c4-niche-commands)
- [ ] **Architecture: `Command` enum + query layer** — explicit user-action command routing (undo/macro-friendly), memoized computed properties → [Refactoring §R2](#r2-architecture-backlog)
- [x] **Config editor Phase 4** — polish: reset row (Ctrl+Z), mtime conflict warning, effective-config export (Ctrl+E), help-overlay section — shipped in PR #161 → [Config editing §E1](#e1-phased-rollout)
- [ ] **Soname Layer 2 (early)** — repo-wide provided-sonames database, reverse dependency index, cascade calculator → [Soname §S2](#s2-layer-2--ecosystem-tooling)

### 🔵 P5 — Long-term vision

- [ ] **v1.0.0 stable release** — polish, stability, documentation, community feedback pass
- [ ] **Embedded Arch wiki viewer** (part of [#130](https://github.com/Firstp1ck/Pacsea/issues/130))
- [ ] **Multi package manager support** — apt, dnf, Flatpak behind a `PackageManager` trait; Flatpak first since it coexists with pacman ([#130](https://github.com/Firstp1ck/Pacsea/issues/130), v2.0 scale)
- [ ] **Subcommand-first CLI redesign** — `pacsea pkg|aur|repo|cache|config …` with legacy flags as hidden aliases → [CLI §C4](#c4-niche-commands)
- [ ] **Soname Layer 2 (late)** — mirror-sync watcher with automated cascade reporting; optional `makechrootpkg` build verification → [Soname §S2](#s2-layer-2--ecosystem-tooling)
- [ ] **UI: incremental/dirty-region rendering** — architectural change → [Performance §P1](#p1-open-optimizations)
- [ ] **Architecture: TEA / CQRS-lite re-evaluation** — only if a large refactor is approved; event sourcing / full Redux / component frameworks remain **not recommended** → [Refactoring §R2](#r2-architecture-backlog)

---

## GitHub issue cross-reference (open, synced 2026-07-03)

| Issue | Topic | Tier |
|-------|-------|------|
| [#145](https://github.com/Firstp1ck/Pacsea/issues/145) | Mirror search/selection UI | 🟡 P3 |
| [#140](https://github.com/Firstp1ck/Pacsea/issues/140) | Focus/hover discoverability (tooltips) | 🟠 P2 |
| [#134](https://github.com/Firstp1ck/Pacsea/issues/134) | Upgrades, rebuilds, conflicts umbrella | 🟡 P3 / 🟢 P4 |
| [#131](https://github.com/Firstp1ck/Pacsea/issues/131) | Distro-specific news | 🟠 P2 |
| [#130](https://github.com/Firstp1ck/Pacsea/issues/130) | AUR maintainer tools, embedded wiki, multi-PM | 🟢 P4 / 🔵 P5 |
| [#129](https://github.com/Firstp1ck/Pacsea/issues/129) | Accessibility themes + system tray | 🟡 P3 / 🟢 P4 |
| [#104](https://github.com/Firstp1ck/Pacsea/issues/104) | Update GitHub-tracked packages | 🟡 P3 |
| [#102](https://github.com/Firstp1ck/Pacsea/issues/102) | Optional-dependency descriptions | 🟡 P3 |
| [#99](https://github.com/Firstp1ck/Pacsea/issues/99) | Service restart logic | 🟡 P3 |
| [#98](https://github.com/Firstp1ck/Pacsea/issues/98) | Transaction abort logic | 🟡 P3 |
| [#97](https://github.com/Firstp1ck/Pacsea/issues/97) | Config value validation | 🟡 P3 |
| [#95](https://github.com/Firstp1ck/Pacsea/issues/95) | Sequential multi-package scans | 🟡 P3 |
| [#94](https://github.com/Firstp1ck/Pacsea/issues/94) | `parse_update_entry` tests | 🟡 P3 |
| [#93](https://github.com/Firstp1ck/Pacsea/issues/93) | CLI remove-from-file | 🔴 P1 |
| [#159](https://github.com/Firstp1ck/Pacsea/issues/159) | Executable bits on helper scripts | 🟡 P3 |

Recently shipped (for context): custom repos + PKGBUILD checks + AUR voting (`v0.8.0`,
[#132](https://github.com/Firstp1ck/Pacsea/issues/132)/[#133](https://github.com/Firstp1ck/Pacsea/issues/133)/[#137](https://github.com/Firstp1ck/Pacsea/issues/137));
adjustable pane heights and pane order via `main_pane_order` + per-role min/max (`v0.8.2`,
[#135](https://github.com/Firstp1ck/Pacsea/issues/135)/[#136](https://github.com/Firstp1ck/Pacsea/issues/136)).

---

# Detail sections

## Features

### F2. Tooltips
Tooltip component appearing after ~500 ms hover/focus, positioned near the focused element,
descriptions pulled from the i18n system. Help overlay (`?`) already covers keybinds.

### F3. Distro news
Add RSS URLs per distro (EOS, Manjaro, Garuda, CachyOS); detect current distro from
`/etc/os-release` (already in `src/logic/distro.rs`); allow switching source or combined feed;
handle per-source date formats. Infrastructure: `src/sources/news.rs`.

### F4. Mirror browser
Modal over existing mirror data (`src/index/mirrors.rs`, `repository/mirrors.json`): search,
country/speed filtering, last-sync/protocol columns, multi-select with ranking.

### F5. Upgrades umbrella ([#134](https://github.com/Firstp1ck/Pacsea/issues/134))
- **Criticality grouping:** classify critical packages (linux, systemd, glibc, …), group/sort in
  the update modal, visual indicators, reboot recommendation.
- **Service restart guidance ([#99](https://github.com/Firstp1ck/Pacsea/issues/99)):** identify units
  needing restart after updates; show a restart plan (dry-run capable).
- **Transaction abort ([#98](https://github.com/Firstp1ck/Pacsea/issues/98)):** cancellation UX for
  running transactions.
- **Conflict wizard (P4):** parse pacman conflict output, offer interactive resolution; risky —
  design carefully.
- **Custom upgrade commands (P4):** user-defined pre/post hooks; mind arbitrary-command security.

### F6. Smaller tracked features
- **Sequential multi-package AUR scans ([#95](https://github.com/Firstp1ck/Pacsea/issues/95))**
- **Optional-dep descriptions ([#102](https://github.com/Firstp1ck/Pacsea/issues/102))** — fetch/show
  descriptions for optional dependencies from ALPM/AUR metadata.
- **GitHub-tracked package updates ([#104](https://github.com/Firstp1ck/Pacsea/issues/104))**
- **Config value validation ([#97](https://github.com/Firstp1ck/Pacsea/issues/97))** — validate
  `settings.conf`/`theme.conf`/`keybinds.conf`/`repos.conf` values on load with clear diagnostics.
- **`parse_update_entry` tests ([#94](https://github.com/Firstp1ck/Pacsea/issues/94))**
- **Helper script executable bits ([#159](https://github.com/Firstp1ck/Pacsea/issues/159))**

### F7. Accessibility ([#129](https://github.com/Firstp1ck/Pacsea/issues/129))
`theme-high-contrast.conf` with WCAG-compliant colors, ASCII alternatives to Unicode symbols,
screen-reader testing where terminals allow. System tray / panel integration tracked in the same
issue (P4).

## CLI

Current implemented surface lives in `src/args/definition.rs` + handlers under `src/args/`.
Note: `src/args/args.rs` duplicates the `Args` struct (incl. an unexported `--refresh`); treat as
historical until reconciled.

### C2. Scripting & parity subcommands (P2)
`doctor` (preflight: pacman/helper/curl/privilege tool/config sanity), `which-helper`,
`updates check [--json]`, `pkg show|outdated`, `completions bash|zsh|fish|elvish`,
`preflight install|remove|update`, `aur vote|unvote|ssh-setup` (TUI parity),
`pkgbuild check [--tool shellcheck]`, `repo list|validate|diff`,
launch flags `--mode package|news` / `--select <pkg>` / `--no-mouse`/`--mouse`, `-y`/`--refresh`,
`cache list` / `cache clear news|details|all`,
`--install` + `--as-deps` / `--needed` / `--aur-only` / `--repo-only`.

### C3. Roadmap subcommands (P3)
Extended `pkg` queries (`files`, `owns`, `deps`, `provides`, `conflicts`, `orphans`, `foreign`,
`native`, `group`, `required-by`, structured `search`); `news fetch|list|show|mark-read|mark-unread`
and `advisories list`; `files pacnew|pacsave|merge`, `backup list|create`, `db sync-status`;
`services affected` / `services restart --dry-run` ([#99](https://github.com/Firstp1ck/Pacsea/issues/99));
`sandbox analyze`, `security advisories --installed`; `repo apply [--all]`, `repo key-fetch`,
`repo foreign-overlap`; `aur scan` batch ([#95](https://github.com/Firstp1ck/Pacsea/issues/95)),
`aur comments|pkgbuild fetch|srcinfo|vote-status`; transaction helpers `sync`, `upgrade
--aur-only`, `downgrade`, `reinstall`, `clean` — all respecting `--dry-run`.

### C4. Niche commands (P4) and structural redesign (P5)
`config path|validate|get|set` (prefer `validate` before mutation), `theme show`, `keybinds list`,
`--print-default-config`, `man`, `--locale` / `i18n list-locales`, `news export`, gated
`unlock`/db-unlock, `plan apply`, `--restore-session`. P5: promote a subcommand-first layout
(`pacsea tui|search|install|remove|update|news|pkg|aur|repo|cache|config …`) with legacy flags kept
as hidden aliases during migration.

### C5. Integrator contracts
For crate consumers (`src/lib.rs`) and subprocess integrators:
- Single JSON envelope with top-level `schema_version`, bumped on breaking changes.
- Parseable payloads only on stdout; diagnostics on stderr (`--json-errors` for structured errors:
  stable `{"error":{"code","message","detail"}}`).
- `--output-format json|jsonl` (`jsonl` for large streams); optional `PACSEA_JSON=1` env inherit.
- No TTY assumptions; respect `NO_COLOR` / `--no-color`.
- Introspection (P2/P3): `api version` (binary/crate version, `json_schema_version`, target triple),
  `api capabilities` (tool presence, OS family, effective `privilege_tool`/helper),
  `api paths` (resolved config/cache/log dirs), `api exit-codes` (documented exit-code map).

**Prioritization axes when picking CLI work:** scripting value → TUI parity → low coupling (thin
wrappers over `logic::*`) → safety (`--dry-run` + privilege rules for anything touching
`pacman.conf`, keys, or `systemctl`).

## UX / workflow improvements

Diagrams: `dev/WORKFLOWS/developer/*.mmd` (mirrored under `manager/`).

### U1. Cross-cutting preflight
- [x] Disk space check before install/remove/update with actionable hints (`logic/preflight/guardrails.rs`)
- [x] Mirror health check + fix guidance before risky operations (sync-db staleness heuristic; deeper mirror-status checks can build on it)
- [x] Pacman db-lock detection + what to do (wait, remove stale lock, …) — blocks CLI and TUI transactions with guidance
- [ ] Richer dry-run: conflicts, reverse deps/orphans, rough size/time estimate
- [ ] Unified "what will happen" confirm + optional per-item overrides
- [ ] Resilience: lock-aware retry, mirror/helper fallback UX (beyond silent paru/yay fallback), links to structured logs
- [ ] State recovery: resumable batches, rollback hints after partial failure

### U2. Per-flow improvements
**Removal (batch):** base/protected-package guard with typed confirm or hard block; orphan cleanup
preview + staged cleanup; blocked-item reasons surfaced with guided retry; parallel dry-run dep
checks where safe.
**System update:** optional pre-snapshot/rollback note + disk/mirror pre-checks; conflict
auto-scope/assist; guided retry (alternate helper/mirror) with log reference; reboot scheduling
(now / later / remind).
**Install (batch):** continue-on-failure toggle + retry actions; stronger parallel prefetch/reuse
of resolved deps; optional post-install hooks or checklist.
**Install (direct from results):** clear metadata errors with retry/alternate source; inline
dry-run deps/conflicts when bypassing full preflight; explicit UI retry keeping selection.
**Downgrade (batch):** cache versions/signatures + provenance/integrity before confirm;
rollback/package-hold guidance after success; pre-download + verify before mutation; skip/unhold
suggestions when the target is unavailable.
**App startup:** single blocking popup queue ordered by priority; remember modal dismissals per
session; stale-cache notice with last refresh + optional background refresh; offline notice with
limited actions + retry timer.

## Performance

### P1. Open optimizations
- [ ] `OfficialIndex.pkgs: Vec` → map/B-tree keyed structure **only if profiling shows benefit** (repeated sorts today)
- [ ] Stream/progressively expose search results to the UI (currently collect-then-render; network latency dominates — low priority)
- [ ] Stronger lazy/on-demand index loading (beyond "load from disk when empty")
- [ ] Tighten remaining `.iter().any()` / retain patterns in installed/removal lists
- [ ] Optional trie/BK-tree for fuzzy search (low priority — `SkimMatcherV2` acceptable)
- [ ] Incremental/dirty-region UI rendering (architectural, P5)
- [ ] `criterion` benches for hot paths before optimizing (seed 1K/10K/100K packages; watch for O(n²) scaling)

Already implemented (kept out of the list): name-index `HashMap`, list `HashSet`s, search result
memoization, sort-cache O(n) reordering, LRU recent searches, PKGBUILD parse disk-LRU, incremental
PKGBUILD highlighting, ring prefetch, PKGBUILD fetch rate limits.

## Refactoring & architecture

### R1. Priority refactors
1. **Unify command execution (P1): ✅ shipped.** Shared abstraction in `util/command.rs`;
   `logic/preflight/command.rs` re-exports it, `logic/services/command.rs` and `util/pacman.rs`
   delegate, capability probes use `binary_available`. Remaining inline `Command::new` query
   sites (e.g. `args/package.rs`) can migrate opportunistically; interactive/PTY/platform-gated
   spawns stay separate by design.
2. **Consolidate cache modules (P1): ✅ shipped.** Shared `app/cache_common.rs` (signatures,
   exact/subset/intersection match modes, load/save, generic flush); the four cache modules are
   thin wrappers with unchanged public APIs and byte-compatible files.
3. **Dedupe package validation (P2):** six similar validators in `args/package.rs`.
4. **`util/config.rs` (P2):** adopt the existing string helpers in the 13 files with inline
   parsing, or delete the unused module.
5. **Lower-effort items (P3+):** modal/handler structure and event-routing cleanup; JSON load/save
   helpers; temp-path test utility; handler HashSet extraction.

### R2. Architecture backlog
Incremental, none mandated:
- **Short-term:** split `AppState` into domain substates (`search`, `install`, `preflight`, `ui`, …)
  without breaking runtime/message flow; group preflight channels into a struct; extract pure
  business logic from fat handlers into testable modules.
- **Medium-term:** explicit user-action `Command` enum routing all mutations (undo/macro-friendly);
  query layer / memoized computed properties on hot paths.
- **Long-term (optional):** revisit TEA vs CQRS-lite only if a large refactor is approved.
  Event sourcing, full Redux, and component frameworks were evaluated and are **not recommended**.

## arch-toolkit migration

Goal: `pacsea` consumes the frontend-agnostic `arch-toolkit` crate (currently v0.2.x with
`aur + deps + index + install + news + repos + system::privilege`) instead of duplicating logic.
**Status:** not started in-tree — `arch-toolkit` is absent from `Cargo.toml`.

**Extraction rule (authoritative):** extract pure domain logic/parsers/typed models/planners fully;
extract only the domain core of mixed modules; never move UI state, event/modal flows,
PTY/interactivity, password prompt/piping, or frontend runtime wiring into the toolkit.

### T1. Workstream A: pacsea consumes arch-toolkit
- [ ] **A1** — add dependency; initialize/thread shared client/context
- [ ] **A2** — AUR cutover: `src/sources/{search,comments,pkgbuild}.rs` AUR paths → toolkit APIs
- [ ] **A3** — deps/index/install cutover: `src/logic/deps/*`, `src/index/*`, install command
      planning → `arch_toolkit::{deps,index,install}` where behavior matches (incl. consuming the
      landed repos-apply planner: features `index`+`install`, meta-feature `repos-apply`,
      `build_repo_apply_bundle` / `build_repo_key_refresh_bundle` with `PrivilegeMode`)
- [ ] **A4** — news baseline: `arch_toolkit::news::{fetch_arch_news, fetch_security_advisories}`
      for list-level feeds; keep article-detail behavior local
- [ ] **A5** — parity cleanup: remove duplicates only after parity tests pass; run full quality gates

### T2. Workstream B: extraction backlog (pacsea → arch-toolkit)
- [x] **B1** — repos apply planner (landed in arch-toolkit 2026-04-18)
- [ ] **B2** — sandbox module (core-only): `src/logic/sandbox/{parse,analyze,types}.rs` → `feature = "sandbox"`
- [ ] **B3** — preflight compute engine (core-only): non-UI core of `src/logic/preflight/*`
- [ ] **B4** — status monitor (full): `src/sources/status/*` → `arch_toolkit::status`
- [ ] **B5** — package details helper (full): reusable parsing from `src/sources/details.rs`
- [ ] **B6** — news article-content parity (core-only): portable parts of `src/sources/news/{fetch,parse}.rs`

**Definition of done:** toolkit modules power AUR/deps/index/install/news baselines; `pacsea`
retains only app-specific runtime/UI/executor layers; remaining B items implemented or explicitly
deferred; no frontend coupling introduced into `arch-toolkit`.

## Integrated config editing (TUI)

Let users view/change configuration in-app. Preserve comments and unknown keys (line-oriented
strategy of `src/theme/config/settings_save.rs`); respect **dry-run** (no writes, clear message).
Primary UX: a structured "settings center" (tabs: General · Keybinds · Theme · Advanced); raw
buffer editing is a later option. Key code: `src/theme/paths.rs` (resolution),
`src/theme/config/{settings_save,skeletons,theme_loader}.rs`, `src/theme/settings/parse_keybinds.rs`,
`src/theme/types.rs`.

### E1. Phased rollout
- [x] **Phase 1** — editor shell (`ConfigEditor` state + keybind), General tab over existing
      `save_*` helpers, read-only display of active config paths, dry-run gating + error surfacing
- [x] **Phase 2** — keybinds tab: render from `Settings.keymap`, capture mode (swallow keys until
      chord/Esc; conflict validation), persist to `keybinds.conf` with round-trip tests
- [x] **Phase 3** — theme tab: edit canonical keys, validate via `try_load_theme_from_content`
      before save, keep last good theme in memory; optional revert-to-skeleton (per-row reset
      via Ctrl+Z shipped instead)
- [x] **Phase 4** — polish: undo/reset row, mtime-changed warning, export effective config, help
      overlay entries for new keybinds

**Cross-cutting tasks:** dedupe path bootstrap (resolve + mkdir + skeleton); schema source (static
table or macro, kept in sync with parsers/tests); define which changes apply live vs on restart;
unit tests for patch helpers + integration test via existing harness patterns.
**Open questions:** instant apply for theme/keybinds? gate privilege-related settings
(`auth_mode`)? export merged effective config?

## Soname-aware dependency checking

Detect `DT_NEEDED` vs on-disk SONAME mismatches before transactions commit. Linux-only
(`#[cfg(target_os = "linux")]`). No `soname` module exists yet; `goblin` not in `Cargo.toml`.
Primary integration point: `Modal::Preflight` (`src/state/modal.rs`, `src/ui/modals/preflight/`,
`src/events/preflight/`) — new `Soname` tab or Summary panel, reusing the lazy tab-sync pattern.
Never run `ldd` on untrusted paths (it executes loaders); parse ELF in-process.

### S1. Layer 1 — end-user protection
- [ ] `goblin`-based `DT_NEEDED`/`DT_SONAME` readers (`src/soname/`), unit tests on fixture ELFs
      incl. malformed/non-ELF; skip `linux-vdso.so.1`-style virtuals; path-safety consistent with
      `is_safe_abs_path` invariants
- [ ] On-disk soname map: walk `/usr/lib`, `/usr/lib32` (std `read_dir` is enough), handle symlink
      chains, multilib arch-tagging, duplicate providers
- [ ] Post-transaction SONAME extraction from `/var/cache/pacman/pkg/*.pkg.tar.zst` (needs
      `tar`+`zstd`/`xz` deps or controlled subprocess) — unlocks sound `WillBreakAfterUpdate`;
      reuse `find_aur_package_file` scan logic in `src/logic/preflight/metadata.rs`
- [ ] Consumer selection: `pacman -Ql`/`-Qo` strategy with aggressive per-session caching
- [ ] Preflight tab + i18n + event wiring; severity chips matching existing patterns; dry-run safe
- [ ] `SystemUpdate` flow parity decision (route through Preflight or inline warning)

### S2. Layer 2 — ecosystem tooling
- [ ] Repo-wide provided-sonames database (scan local mirror `.pkg.tar.*`, or ingest Arch's
      `sogrep` links DBs; SQLite/flat index refreshed post-`rsync`)
- [ ] Reverse dependency index (soname → dependent packages; richer than per-query `sogrep`)
- [ ] Cascade calculator (BFS/DFS over soname edges; complements `arch-rebuild-order`'s syncdb view)
- [ ] Mirror-sync watcher + automated soname-bump reports (the novel piece)
- [ ] Build verification via `makechrootpkg` (stretch; separate daemon/subcommand territory)

Deps to pin at implementation time: `goblin` (verify `elf32`/`elf64`/`endian_fd` features),
`tar` + `zstd`/`xz`, optional `rusqlite` for Layer 2.

---

*Update this file whenever roadmap work ships or new work is planned. Security remediation work is
tracked separately in `SECURITY_REMEDIATION_GUIDE.md` (mandated by `CLAUDE.md`); demo storyboard in
`dev/pacsea_demo_storyboard.md`; current architecture reference in `dev/ARCHITECTURE_CURRENT.md`.*
