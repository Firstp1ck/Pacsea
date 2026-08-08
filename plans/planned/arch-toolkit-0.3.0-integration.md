# Pacsea integration plan for arch-toolkit 0.3.0

**Status:** Planned; implementation not started  
**Canonical owner:** Pacsea repository  
**Target crate version:** `arch-toolkit` 0.3.0  
**Inspected baseline revision:** `7c96301d3681c3a50d15bb6a23ae6ceb057e4023`  
**Baseline verified:** local `arch-toolkit/main`, remote `origin/main`, and manifest agree as of 2026-07-31  
**Latest published release:** 0.2.0; 0.3.0 is release-ready but is not tagged or published  
**Pacsea baseline:** `b6511fe8a6ca2fe10792f81b9634c7e743ac5e38`  
**Supersedes for execution:** every Pacsea roadmap checklist and historical migration plan listed under [Plan consolidation](#plan-consolidation)

## Goal

Make Pacsea consume the newest 0.3.0 `arch-toolkit` source for its reusable AUR, dependency, index, install-planning, news, official-metadata, and sandbox capabilities while preserving Pacsea's TUI, runtime, cache, distro, privilege, dry-run, PTY, security-scanner, and transaction behavior.

The migration must be incremental, test-first, reversible by domain, and safe on systems where `pacman`, AUR helpers, privilege tools, or the network are unavailable.

## Completion criteria

The migration is complete only when all of the following are true:

- [ ] Pacsea uses one immutable, auditable 0.3.0 dependency source; no floating branch or local path is mergeable.
- [ ] The final dependency enables only `aur`, `deps`, `index`, `install`, `news`, and `sandbox`; default features are disabled explicitly.
- [ ] Pacsea has one crate-private anti-corruption layer for toolkit clients, conversions, error policy, and host/runtime boundaries.
- [ ] Toolkit APIs power AUR search/info/comments/AUR-PKGBUILD, dependency analysis, official index primitives, safe install planning, list-level news/advisories, official metadata, and sandbox analysis where the parity contracts below permit it.
- [ ] Pacsea retains app-specific orchestration, state, rendering, caches, distro/repository policy, dry-run, PTY/password handling, command execution, scanners, and richer article behavior.
- [ ] Every replaced path has a fixture-based parity test, a missing-tool/error test, and a domain-level rollback commit before duplicate code is removed.
- [ ] No direct duplicate remains unless this plan records the Pacsea-specific behavior that requires it.
- [ ] Clean-checkout, release, security, dependency, and AUR packaging gates pass.
- [ ] The branch PR record describes only the final diff from `main`.
- [ ] This plan is fully checked, goal completion is verified, and the file is moved from `plans/planned/` to ignored `plans/archive/`.

## Plan consolidation

This file is the only active implementation checklist for the dependency integration.

| Prior source | Status and disposition |
| --- | --- |
| Deleted `dev/IMPROVEMENTS/IMPLEMENTATION_PLAN_arch-toolkit.md` (last pre-deletion content at `59ce5c0c^`) | Superseded. Its A1-A5 and B1-B6 tasks are reconciled here; branch-only `repos` and `system::privilege` claims are rejected. |
| `dev/IMPROVEMENTS/ROADMAP.md`, former `arch-toolkit migration` section and P2/P3 bullets | Replaced with a non-checklist pointer to this file so migration TODOs are not duplicated. |
| `arch-toolkit/plans/archive/ARCH_TOOLKIT_ROADMAP.md`, Phase 6 | Historical external handoff. Its nine open bullets are incorporated here; the archived plan is evidence, not a second active plan. |
| `arch-toolkit/reports/arch-toolkit-roadmap.html` | Historical verification report for toolkit Phases 1-5; no implementation authority in Pacsea. |
| `arch-toolkit/dev/PR/PR_feat-install-news-repos-syspriv-apis.md` and superseded branch `feat/install-news-repos-syspriv-apis` | Evidence only. Current `arch-toolkit/main` is authoritative; branch-only APIs must not be consumed. |
| Pacsea source TODO/FIXME markers unrelated to this dependency | Excluded. Transaction cancellation, service restart, scan, snapshot, and CLI refresh work remain under their own roadmap items. |

## Verified current state

### Dependency identity

- `arch-toolkit/Cargo.toml` declares 0.3.0, Rust 2024, and `rust-version = "1.91"`.
- Local and remote `main` both resolve to `7c96301d3681c3a50d15bb6a23ae6ceb057e4023`.
- `cargo search arch-toolkit` reports only 0.2.0; no `v0.3.0` tag exists.
- Pacsea has no `arch-toolkit` declaration, lock record, import, adapter, or parity test.
- Pacsea already uses compatible `reqwest` 0.13 and Tokio 1.x lines. Enabling the toolkit introduces older parallel `scraper` and `rand` lines during coexistence; toolkit `lru` 0.16 can unify with the `lru` 0.16 line already pulled by `ratatui-core`.
- Toolkit 0.3.0 requires Rust 1.91; Pacsea currently declares no MSRV.

### Actual 0.3.0 feature set

| Feature | Final disposition |
| --- | --- |
| `aur` | Adopt through a Pacsea adapter. |
| `deps` | Adopt parsers/queries first, then direct resolution and bounded graph resolution. |
| `index` | Adopt fetch/query/persistence primitives behind Pacsea wrappers. |
| `install` | Adopt validated `CommandSpec` planning only; Pacsea remains executor. |
| `news` | Adopt list feeds and selected pure parsing; preserve Pacsea feed policy and article UX. |
| `sandbox` | Adopt dependency/static analysis; preserve fetch/scanner/build policy. |
| `fuzzy-search` | Do not enable. Pacsea owns ranking, result merging, and UI sorting. |
| `cache-disk` | Do not enable. Pacsea owns cache locations, TTL, persistence, signatures, and fallback behavior. |
| `repos` / `repos-apply` | Does not exist on current main. Keep Pacsea's repository planner local. |
| `system::privilege` | Does not exist. Pacsea retains `logic::privilege`; toolkit only exposes install `PrivilegeTool` detection/types. |
| `preflight` / `status` | Does not exist. Keep Pacsea implementations local; extraction is outside this migration. |

### API coverage decisions

| Toolkit surface | Use in Pacsea | Boundary |
| --- | --- | --- |
| `ArchClient`, `AurApi`, AUR search/info/comments/pkgbuild | Yes | Pacsea configures and owns the client, result caps, UI models, errors, cancellation, and caches. |
| `RetryPolicy`, `ValidationConfig`, health and cache configuration | Limited | Start with retries and toolkit caches disabled to preserve current behavior; health is diagnostic only. |
| AUR low-level cache/client helpers | No | Internal infrastructure, not a Pacsea integration seam. |
| Dependency parsers/version helpers/queries | Yes | Convert errors and preserve C-locale/missing-tool UX. |
| Legacy `DependencyResolver::resolve` | Transitional | Preserve direct behavior before graph adoption. |
| `resolve_graph` and `DependencyMetadataProvider` | Yes, later | Pacsea supplies bounded verified `.SRCINFO` metadata and maps diagnostics into preflight state. |
| `OfficialIndex`, repo detection/fetch/query/persistence | Yes | Encapsulate mutation and always rebuild name indexes; Pacsea owns enrichment and custom repo merge. |
| Mirror discovery/generation and official metadata | Selective | Adopt pure/fetch primitives only; Pacsea owns mirror ranking/application and UI. |
| Install builders and `CommandSpec` | Yes, parity-proven paths only | Pacsea validates more strictly, executes argv where possible, and owns password/PTY/dry-run/locking. |
| News/advisory feed parsing/fetching/cache trait | Yes for list-level data | Pacsea retains TTL/disk cache, retries, circuit breaker, filtering, read state, aggregation, and CLI schema. |
| Article extraction | Evaluate and adopt only as an inner parser after fixtures | AUR-comment rendering, package-change decoration, cache, and presentation remain local. |
| Sandbox dependency/static analysis | Yes | Fetching, scanner integrations, chroot/build, cache policy, and UI remain local. |
| AUR voting, repository apply, status monitor, UI/runtime APIs | No | Not provided or intentionally app-owned. |

## Locked implementation decisions

1. **Dependency source:** start with an immutable Git revision because 0.3.0 is not published:

   ```toml
   arch-toolkit = {
     git = "https://github.com/Firstp1ck/arch-toolkit.git",
     rev = "7c96301d3681c3a50d15bb6a23ae6ceb057e4023",
     default-features = false,
     features = ["aur", "deps", "index", "install", "news", "sandbox"]
   }
   ```

   Add the exact repository to `deny.toml`'s `allow-git`. A local path may be used only for temporary differential tests and must not enter committed manifests or lockfiles. If required hardening lands after the inspected baseline, advance only to an audited immutable descendant and record the new revision in this header and the PR. If 0.3.0 is published before merge, switch to `version = "=0.3.0"` only after proving the registry artifact matches the audited source and rerunning every dependency/packaging gate.

2. **MSRV:** add `rust-version = "1.91"` to Pacsea and verify that CI and AUR builds use at least that compiler.
3. **Feature policy:** enable the final six features once in the dependency-foundation wave. Do not rely on the toolkit's default `aur` feature.
4. **Integration architecture:** add a crate-private `src/integrations/arch_toolkit/` anti-corruption layer. Toolkit models must not leak into Pacsea UI/event public surfaces.
5. **HTTP ownership:** accept two pooled clients because 0.3.0's AUR `ArchClient` cannot accept a caller `reqwest::Client`:
   - `Arc<ArchClient>` for AUR operations;
   - one cloned caller-owned `reqwest::Client` for news, official metadata, mirrors, and `.SRCINFO` fetches.
6. **Initial network policy:** explicit Pacsea user agent; 10-second AUR timeout; 30-second caller-client timeout; toolkit AUR retries and caches disabled until differential tests approve changes. Existing Pacsea cache/retry/circuit-breaker behavior remains outside the toolkit.
7. **Search policy:** preserve Pacsea's visible 200-result cap by calling toolkit search and truncating in the adapter. The uncapped toolkit method remains available internally for future separately approved UX changes.
8. **Security boundary:** using reqwest does not violate the project rule that any *curl invocation* must use `curl_args()`. It does change response-size behavior. Comments, PKGBUILD, info, and `.SRCINFO` cutovers require bounded-body evidence or an upstream 0.3.0 hardening revision before their local curl paths are removed.
9. **Package validation:** reject empty names, uppercase, disallowed characters, and names beginning with `-` or `.` before any toolkit install builder. Prefer direct argv execution and add `--` separators upstream/locally where the called tool supports them.
10. **Privilege/execution:** toolkit produces neutral command plans. Pacsea alone applies active privilege policy, `SecureString` password handling/redaction, PTY/terminal hold tails, dry-run, lock checks, confirmation, ordering, cancellation, logging, and exit-status handling. Never privilege-wrap AUR helpers.
11. **Repository planner:** `src/logic/repos/apply_plan.rs` remains authoritative. Do not merge or consume the stale toolkit branch to recover removed APIs.
12. **Persistence:** preserve Pacsea cache paths and formats during cutover. Convert at wrappers or regenerate only after an explicit schema fixture proves migration behavior.
13. **Platform:** host-query integrations are Linux/Arch paths. Preserve current Windows compilation and behavior with `cfg` boundaries; do not invoke pacman-dependent toolkit paths on Windows.
14. **Documentation:** implementation updates rustdoc, tests, this plan, and the required branch PR record. README/wiki changes remain outside scope unless separately requested.

## Ownership and seam contract

### `arch-toolkit` owns

- Reusable typed models, pure parsers, bounded network fetchers, deterministic graph logic, official index primitives, command specifications, and text-only sandbox findings.
- Feature isolation and its own Rust quality matrix.

### Pacsea owns

- `AppState`, channels, workers, events, modals, rendering, i18n, settings, result sorting, and all user-facing error wording.
- A crate-private adapter that converts toolkit values into `PackageItem`, `PackageDetails`, `AurComment`, dependency modal types, index wrappers, news types, and sandbox state.
- Distro/custom-repository policy, index enrichment, caches, feed aggregation/read state, scanners, AUR voting, article decoration, and repository apply planning.
- Command execution, dry-run, privilege/password/session handling, PTY lifecycle, lock checks, cancellation, logging, confirmation, and transaction ordering.

### Planned adapter shape

Create only the files required by the active wave:

```text
src/integrations/mod.rs
src/integrations/arch_toolkit/mod.rs       # ToolkitContext and shared policy
src/integrations/arch_toolkit/aur.rs       # AUR calls and model conversion
src/integrations/arch_toolkit/deps.rs      # dependency provider/conversion
src/integrations/arch_toolkit/index.rs     # index conversion/repo merge
src/integrations/arch_toolkit/install.rs   # strict validation/CommandSpec conversion
src/integrations/arch_toolkit/news.rs      # feed conversion/cache boundary
src/integrations/arch_toolkit/sandbox.rs   # sandbox conversion
```

`ToolkitContext` contains `Arc<ArchClient>` plus a cloneable `reqwest::Client`. Construct it once for the TUI in `app::runtime::run`, clone it into workers through `Channels::new`, and construct one context per standalone CLI invocation. Do not add hidden mutable globals.

## Execution order

### Wave 0 — Freeze baseline and harden the dependency contract

**Purpose:** make the exact source, toolchain, security boundaries, and differential fixtures reproducible before replacing behavior.

- [ ] Create a migration branch and create/update `dev/PR/PR_<branch>.md` from `.github/PULL_REQUEST_TEMPLATE.md`.
- [ ] Record Pacsea and toolkit base revisions and confirm both worktrees are clean before implementation.
- [ ] Add fixture snapshots for current AUR search/details/comments/PKGBUILD, dependency rows, index JSON, install command strings, news/advisory identities, and sandbox JSON.
- [ ] Add failing security tests for leading `-`/`.` package names at the Pacsea adapter boundary.
- [ ] In `arch-toolkit`, add failing tests and a bounded-read fix for any AUR comments, PKGBUILD, info, or `.SRCINFO` response that Pacsea will consume without its existing curl 10 MiB cap; never execute PKGBUILD content.
- [ ] If toolkit hardening changes HEAD, run its full matrix, audit the diff, advance the immutable revision, and update this plan header before changing Pacsea's lockfile.
- [ ] Verify Rust 1.91 locally or in CI and document the exact compiler used by clean package builds.
- [ ] Confirm `origin/main` contains the selected revision and that no branch-only `repos`/`system` API enters the plan.

**Acceptance:** deterministic pre-migration fixtures fail only when behavior changes; selected toolkit revision is immutable, remotely available, bounded for adopted network paths, and green.  
**Rollback:** no Pacsea production call site has changed.

### Wave 1 — Add dependency and anti-corruption foundation

**Files:** `Cargo.toml`, `Cargo.lock`, `deny.toml`, `src/lib.rs`, new `src/integrations/**`, `src/app/runtime/{mod,channels}.rs`, standalone CLI entry paths as needed.

- [ ] Add the exact dependency declaration and `rust-version = "1.91"`.
- [ ] Allow only `https://github.com/Firstp1ck/arch-toolkit.git` in `deny.toml` if the Git source remains necessary.
- [ ] Add `ToolkitContext` with explicit user agent, timeouts, disabled toolkit cache/retries, and actionable construction errors.
- [ ] Thread the context through TUI workers without adding UI state to the toolkit or global mutable state to Pacsea.
- [ ] Add pure conversion tests for every model introduced in later waves; keep production paths on existing implementations.
- [ ] Add error mapping that says what failed and what the user can do next; preserve nonfatal search/news error channels.
- [ ] Run `cargo tree -e features`, `cargo tree -d`, `cargo metadata --locked`, `cargo audit`, and `cargo deny check`; disposition every new high/critical advisory before continuing.
- [ ] Prove a clean checkout resolves and builds without a sibling `arch-toolkit` directory.

**Acceptance:** Pacsea compiles with the final feature set, no call site is cut over, dependency provenance is auditable, and existing tests remain green.  
**Rollback:** revert manifest/lock/deny and isolated adapter/context files.

### Wave 2 — Cut over complete AUR data paths

**Files/symbols:**

- `src/sources/search.rs::fetch_all_with_errors`
- `src/sources/details.rs::{fetch_details,fetch_aur_details}`
- `src/sources/comments.rs::fetch_aur_comments`
- AUR branch of `src/sources/pkgbuild.rs::fetch_pkgbuild_fast`
- `src/app/runtime/workers/{search,comments,details}.rs`
- `src/ui/helpers/query.rs` only if context threading reaches helper previews
- `src/integrations/arch_toolkit/aur.rs`

- [ ] Write adapter tests using toolkit mocks/fixtures before changing production calls.
- [ ] Replace AUR search with toolkit search, preserving empty-name filtering, `PackageItem` fields, nonfatal errors, sorting inputs, and the 200-row cap.
- [ ] Replace AUR info/details, preserving `PackageItem` fallback fields, `PackageDetails` defaults, timestamp formatting, and missing-result behavior.
- [ ] Replace comments only after fixture parity for stable IDs, author/content, pinned-first ordering, timestamps, local display dates, deduplication, links, and bounded HTML.
- [ ] Preserve paru/yay offline PKGBUILD cache first; replace only the AUR network branch after bounded-response evidence. Keep official GitLab `main` then `master` fallback local.
- [ ] Preserve worker cancellation/stale-response behavior and user-facing network error routing.
- [ ] Keep AUR voting and SSH setup local.
- [ ] Remove direct AUR search/info/comments/cgit URL construction only after the replacement tests pass.

**Targeted tests:** empty/invalid query; 200/201 rows; out-of-date/orphan fields; partial error; missing info result; pinned/duplicate/date comment fixtures; cache-hit-without-network; AUR timeout/body limit; official PKGBUILD fallback unchanged.  
**Acceptance:** all four AUR data paths are toolkit-backed, while AUR voting, caches, UI policy, and official PKGBUILD behavior remain Pacsea-owned.  
**Rollback:** one AUR-domain revert restores all four former source functions without touching later domains.

### Wave 3 — Adopt sandbox analysis

**Files/symbols:** `src/logic/sandbox/**`, `src/app/runtime/workers/preflight.rs::spawn_sandbox_worker`, `src/app/sandbox_cache.rs`, `src/integrations/arch_toolkit/sandbox.rs`.

- [ ] Add differential fixtures for PKGBUILD and `.SRCINFO` dependency categories, providers, missing packages, versions, optional dependencies, and persisted JSON.
- [ ] Keep fetch/fallback orchestration in Pacsea; call toolkit `analyze_srcinfo`, `analyze_pkgbuild`, and selected text-only security analysis through the adapter.
- [ ] Preserve installed/provided set injection and explicitly test toolkit's additional host queries and missing-pacman degradation.
- [ ] Decide readiness from both installation and version satisfaction in Pacsea's adapter; do not silently adopt toolkit's installation-only `is_ready_to_build` behavior.
- [ ] Preserve cache compatibility or add a tested one-time regeneration path.
- [ ] Keep ShellCheck, namcap, Semgrep, VirusTotal, aur-sleuth, chroot/build behavior, dry-run, and scanners local.
- [ ] Remove local pure parser/analyzer code only after differential tests are green; retain orchestration wrappers.

**Acceptance:** toolkit powers reusable dependency/static analysis, no PKGBUILD is sourced or executed, and Pacsea scanner/UI/cache behavior is unchanged.  
**Rollback:** switch wrapper imports back to local analysis; persisted state remains readable.

### Wave 4 — Adopt dependency APIs and bounded graph resolution

**Files/symbols:** `src/logic/deps.rs`, `src/logic/deps/**`, `src/app/runtime/workers/preflight.rs::spawn_dependency_worker`, `src/state/modal.rs` dependency types, `src/integrations/arch_toolkit/deps.rs`.

- [ ] Replace pure dependency/spec/PKGBUILD/`.SRCINFO` parsers and version helpers behind parity tests.
- [ ] Differential-test epoch/pkgver/pkgrel and hyphenated versions against current Pacsea behavior and host `vercmp` where available.
- [ ] Replace installed/upgradable/version/source queries while preserving C locale and actionable missing-tool distinctions instead of treating every empty set as success.
- [ ] Do not use `get_provided_packages()` as a provider inventory; retain/inject Pacsea's provider data and test virtual providers.
- [ ] Migrate direct resolution through `DependencyResolver::resolve` first and preserve current row/status/conflict/root filtering.
- [ ] Implement a Pacsea-owned `DependencyMetadataProvider` for verified, bounded, batched `.SRCINFO` metadata.
- [ ] Add bounded graph resolution with explicit depth, node, timeout, and batch limits; map cycles, malformed metadata, missing nodes, provider provenance, and conflicts into preflight output.
- [ ] Migrate reverse dependency analysis and removal summaries.
- [ ] Remove local modules in leaf-first order only after each substep's tests pass.

**Targeted tests:** direct-only compatibility; optional/make/check dependency policy; `.so` filtering; providers; conflicts; missing pacman/helper; cycles; duplicate nodes; incompatible constraints; timeout/depth/node diagnostics; reverse direct/transitive summaries.  
**Acceptance:** toolkit owns reusable parsing/query/resolution; Pacsea owns modal models, action policy, fallback wording, and verified metadata transport.  
**Rollback:** separate commits for parser, query, direct resolver, graph, and reverse analysis.

### Wave 5 — Adopt official index and metadata primitives

**Files/symbols:** `src/index/{mod,fetch,installed,explicit,persist,query,update}.rs`, `src/sources/details.rs::fetch_official_details`, index/search/auxiliary workers, `src/integrations/arch_toolkit/index.rs`.

- [ ] Add schema fixtures comparing Pacsea and toolkit `OfficialIndex` JSON, including a corrupt-cache case and duplicate package names across repos.
- [ ] Detect enabled system repositories with the toolkit, merge Pacsea `repos.conf` additions in deterministic order, and call explicit-repository fetch APIs.
- [ ] Encapsulate toolkit `OfficialIndex`; rebuild `name_to_idx` after every replacement, mutation, or deserialization.
- [ ] Preserve Pacsea process-wide wrapper API initially so broad UI/event consumers do not change in one wave.
- [ ] Keep `distro.rs`, enrichment, custom repository policy, notifications, and refresh scheduling local.
- [ ] Adopt official package detail fetching only after fallback/order/size fixtures prove parity with pacman-first behavior.
- [ ] Evaluate mirror discovery/generation as an inner primitive; keep ranking, selection, file application, privilege, and UI local.
- [ ] Preserve Windows behavior with cfg-gated local paths; do not call host pacman APIs on Windows.
- [ ] Remove local fetch/query/persist internals only after wrappers and cache migration/regeneration pass.

**Acceptance:** toolkit powers official repository detection/fetch/query/persistence and selected metadata; Pacsea keeps enrichment, distro/custom-repo policy, state ownership, and mirror application.  
**Rollback:** wrapper internals return to local implementations without changing the on-disk format.

### Wave 6 — Adopt safe install planning, never execution

**Files/symbols:** `src/install/{command,batch,executor,utils}.rs`, `src/logic/privilege.rs`, relevant CLI install/update/remove paths, `src/integrations/arch_toolkit/install.rs`.

- [ ] Add failing tests for leading-option package names and all shell metacharacter/injection cases before delegating to toolkit builders.
- [ ] Convert Pacsea `PackageItem` and settings into toolkit `PackageRef`, `InstallOptions`, helper, cascade, and privilege types only inside the adapter.
- [ ] Use direct `CommandSpec` argv for non-PTY execution paths; render shell only where Pacsea's external-terminal workflow requires it and quote every variable fragment.
- [ ] Preserve `--aur`, `--needed`, reinstall, official/AUR split, mixed-install short-circuiting, helper preference, and configured doas/sudo behavior.
- [ ] Keep implicit sync behavior local unless an existing Pacsea path intentionally requires it; toolkit must not introduce or remove `pacman -Sy` silently.
- [ ] Do not use toolkit shell fallback's successful `echo` as an error contract; Pacsea must return an actionable missing-helper error.
- [ ] Preserve `SecureString`, password redaction, mode `0o600` sensitive files, PTY/terminal hold tails, dry-run no-op, lock checks, cancellation, logging, confirmation, and executor protocol.
- [ ] Verify AUR helper commands are never privilege-wrapped and official commands use the active Pacsea privilege policy.
- [ ] Remove local quote/validation/planner duplication only when every adopted builder has golden parity; retain execution/orchestration modules.

**Targeted tests:** official/AUR/mixed/reinstall; `--needed`; helper absence; `-`/`.` leading names; metacharacters; direct argv; short-circuit on official failure; doas/sudo; dry-run no execution; password never logged; remove cascade; update split; PTY hold tail.  
**Acceptance:** toolkit constructs neutral safe plans; Pacsea remains the sole executor and security/session owner.  
**Rollback:** revert individual builder delegation without changing runtime protocol.

### Wave 7 — Adopt list-level news/advisories and selected article parsing

**Files/symbols:** `src/sources/news/{fetch,parse}.rs`, `src/sources/advisories.rs`, `src/sources/feeds/news_fetch.rs`, news/auxiliary/content workers, `src/args/news.rs`, `src/integrations/arch_toolkit/news.rs`.

- [ ] Add fixtures for RSS entities/dates/cutoffs, advisory IDs/severity/packages, ordering, read-state keys, oversized responses, stale-cache fallback, and CLI JSON.
- [ ] Use the caller-owned HTTP client with toolkit list-level news and advisory fetchers.
- [ ] Convert toolkit types into existing `NewsFeedItem` and preserve source, IDs, dates, ordering, and nonfatal error semantics.
- [ ] Migrate advisory read-state deliberately if toolkit identity differs; never mark existing items unread without a tested policy.
- [ ] Keep Pacsea memory/disk TTL cache, retry/backoff, circuit breaker, source filters, installed-only filtering, aggregation, startup sequencing, and CLI schema.
- [ ] Differential-test toolkit `extract_article_text`; adopt it only as an inner parser if it preserves required paragraphs/lists/code/links and safety bounds.
- [ ] Keep AUR-comment article rendering, package-change decoration, conditional requests, content cache, and UI presentation local.
- [ ] Remove only list-level duplicate parsers/fetchers after parity; retain intentional policy wrappers.

**Acceptance:** toolkit powers bounded list feeds and approved pure article parsing; Pacsea feed composition, caches, read state, article behavior, and UX remain stable.  
**Rollback:** restore list fetch adapters; persisted read/cache state remains valid.

### Wave 8 — Delete duplicates and close deferred scope

- [ ] Run reference searches before every deletion and prove the replacement path executes in tests.
- [ ] Remove dead AUR JSON/scraper helpers after all AUR paths pass.
- [ ] Remove sandbox parser/analyzer/type duplication after persisted-state parity.
- [ ] Remove dependency leaf modules, then direct/reverse wrappers, only after graph and UI conversions pass.
- [ ] Remove index fetch/query/persist internals after wrapper/cache compatibility passes; retain distro/enrichment/custom policy.
- [ ] Remove install quote/validation/planning duplicates only where toolkit plus Pacsea security adapter fully replaces them.
- [ ] Remove news list parsers only after read-state/CLI/TUI parity; retain article/cache/policy modules.
- [ ] Record retained local code with one of these reasons: UI/runtime ownership, stronger security bound, cache compatibility, distro policy, execution/privilege, scanner behavior, or missing toolkit API.
- [ ] Confirm repository apply, preflight compute, status monitor, AUR voting, PTY/password handling, and UI/runtime extraction are explicitly deferred—not silently omitted.

**Acceptance:** no unexplained duplicate remains and no missing toolkit API was reimplemented under the guise of migration.  
**Rollback:** each domain cleanup is its own commit after its cutover commit.

### Wave 9 — Full validation, packaging, review, and plan closure

Run from Pacsea root after each wave, in project-required order:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

Final dependency/security matrix:

```bash
cargo tree -e features
cargo tree -d
cargo metadata --locked --format-version 1
cargo audit
cargo deny check
cargo build --locked --release
cargo test --locked --release -- --test-threads=1
```

Selected toolkit revision:

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo check --locked --no-default-features
cargo check --locked --no-default-features --features aur,deps,index,install,news,sandbox
cargo test --locked --all-features -- --test-threads=1
cargo +1.91 check --all-features  # or an equivalent pinned-toolchain invocation outside rustup
cargo publish --dry-run
```

- [ ] Run every command above and record exit codes in the branch PR.
- [ ] Run deterministic differential suites without live endpoints or undeclared host state.
- [ ] Exercise dry-run user flows for install/remove/update and prove no mutating command executes.
- [ ] Verify missing `pacman`, `paru`, `yay`, `sudo`, and `doas` produce documented empty/error behavior and actionable messages.
- [ ] Build/test a clean Pacsea checkout with no sibling toolkit directory and a cold Cargo Git cache.
- [ ] Run `PKGBUILD-git` `prepare`, `build`, and `check` in an isolated build root without installing dependencies or modifying the host.
- [ ] Verify frozen/offline behavior after dependency fetch and confirm the immutable source remains reachable.
- [ ] Run supported Linux targets and existing Windows compile checks; document unavailable target toolchains.
- [ ] Obtain independent review of correctness/regressions, test/validation coverage, and security/privilege/network boundaries.
- [ ] Disposition every reviewer finding as accepted, rejected, deferred, or needs verification; re-run affected gates after fixes.
- [ ] Inspect the final diff against `main`, update the branch PR, and remove reverted/intermediate claims.
- [ ] Verify every completion criterion at the top of this plan.
- [ ] Move this fully completed plan to `plans/archive/arch-toolkit-0.3.0-integration.md` and update the roadmap pointer.

## Test inventory to add

Tests may be grouped by existing subsystem conventions rather than forced into one file per row.

| Test area | Required evidence |
| --- | --- |
| Adapter construction | Explicit policy, cloning/threading, construction errors, no global mutation. |
| AUR | Search cap/mapping/errors, info fallback, comments ordering/content/IDs, PKGBUILD cache and bound. |
| Sandbox | Parser/delta/version/provider parity, no execution, JSON compatibility, missing pacman. |
| Dependencies | Parsing/version corpus, providers/conflicts, direct and graph bounds/diagnostics, reverse deps. |
| Index | Repo detection/merge, duplicate names, mutation rebuild, persistence migration, corruption, Windows cfg. |
| Install | Golden argv/shell, option injection, helper/privilege policy, mixed short-circuit, dry-run/password safety. |
| News | Feed parsing/bounds, advisory identity/read state, caching/fallback, filtering, article parser parity, CLI JSON. |
| Packaging | Exact source, Rust 1.91, clean checkout, frozen release, AUR build/check. |

## Reviewer-finding dispositions incorporated into this plan

| Finding | Disposition |
| --- | --- |
| Roadmap claimed current `repos`, `repos-apply`, `system::privilege`, and completed B1 | **Accepted.** Claims are stale branch history; removed from migration scope. |
| Roadmap called current toolkit v0.2.x | **Accepted.** Target is unreleased 0.3.0 source; published 0.2.0 is insufficient. |
| “One shared reqwest client” | **Accepted.** Impossible for AUR in 0.3.0; plan uses `ArchClient` plus caller client. |
| AUR details omitted from old A2 | **Accepted.** Details are included in Wave 2. |
| Sandbox and article APIs still listed as future extraction | **Accepted.** Reclassified to consumption/parity. |
| Reqwest adoption violates Pacsea's curl rule | **Rejected in part.** The rule governs curl invocations, not all HTTP. The loss of uniform curl response bounds is valid and gated in Waves 0/2. |
| Install leading-option injection | **Accepted.** Strict leading-character rejection and argv tests are mandatory before adoption. |
| Toolkit shell fallback returns success when helper is missing | **Accepted.** Pacsea does not use that status as its error contract. |
| Toolkit privilege wrapping could absorb password/session policy | **Accepted.** Toolkit output is neutral planning only; Pacsea owns all execution/auth behavior. |
| Legacy resolver fields imply graph behavior | **Accepted.** Direct and graph migration are separate substeps with separate configs/providers. |
| `get_provided_packages` is an inventory | **Rejected as an integration assumption.** Plan retains/injects Pacsea provider data. |
| Index model/persistence is drop-in | **Rejected.** Wrapper and schema migration tests are mandatory. |
| Toolkit caches can replace Pacsea caches | **Rejected.** Toolkit cache features remain disabled. |
| Dependency skew and Rust 1.91 are harmless | **Rejected.** Both are explicit dependency and packaging gates. |

## Residual risks

- The final immutable revision may advance from the inspected baseline if required network-bound hardening lands; every advance requires a full source and lockfile re-audit.
- Fixture parity cannot prove live AUR/Arch HTML and feed stability; live tests remain diagnostics, not release gates.
- Coexisting local/toolkit implementations temporarily increase binary size and duplicate dependency versions.
- Host-query APIs sometimes conflate unavailable tools with empty results; Pacsea adapters must preserve actionable diagnostics where UX depends on the distinction.
- The synchronous dependency provider cannot be preempted by the toolkit; Pacsea must enforce I/O timeouts inside the provider.
- Official indexes key primarily by package name, so duplicate names across repositories need deterministic Pacsea policy.
- Toolkit public structs are not uniformly `#[non_exhaustive]`; the exact revision pin and adapter boundary are the compatibility controls.
- Windows can compile the feature set but cannot execute Arch host queries; cfg coverage must prevent accidental calls.

## Research and verification record

This plan was reconciled against:

- Pacsea `Cargo.toml`, `Cargo.lock`, `deny.toml`, `PKGBUILD-git`, `AGENTS.md`, roadmap, historical deleted migration plan, runtime/channel setup, AUR sources, details, dependencies, index, install, news, sandbox, tests, and git history.
- arch-toolkit 0.3.0 manifest/lock, git tags/remote/history, crate exports, every feature family, public models/errors/client/builders, examples, integration tests, archived roadmap, report, changelog, README, CI, and stale branch provenance.
- The dedicated API-contract research run at the inspected revision reported 457 passed, 30 ignored, 0 failed and a clean all-target/all-feature Clippy run. Independent plan review did not rerun that suite. Rust 1.91 itself was not installed in the research environment, so both the test matrix and MSRV check remain implementation gates.

**Planning confidence:** 95/100. Source, git, manifest, remote, registry, test, and multi-review evidence agree. Remaining uncertainty is limited to the future hardening revision, live-service behavior, and clean packaging execution after the dependency is actually added.
