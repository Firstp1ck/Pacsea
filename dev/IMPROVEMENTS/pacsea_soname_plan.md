# Pacsea: Soname-Aware Dependency Checking — Plan (repo-aligned)

## Vision and positioning

Pacsea's soname engine has two distinct layers of ambition:

**Layer 1 — End-user protection (near-term):** Before any install/upgrade is committed, detect
`DT_NEEDED` vs on-disk SONAME mismatches and surface them in the Preflight TUI so the user can make an informed decision rather than encounter a silent broken state at runtime.

**Layer 2 — Ecosystem tooling (longer-term):** Go beyond single-machine Preflight checks: a
**materialized** repo-wide view (provided SONAMEs, reverse edges, cascade hints) suitable for a
standalone indexer, Pacsea subcommand, or maintainer workflows. Arch already ships **point tools**
(`sogrep`, `arch-rebuild-order`, devtools scripts); Layer 2 is about **orchestration**, **durable
local indexes**, **API-shaped** reuse from Rust, and **automation hooks** (e.g. post-sync) that those
CLIs do not provide as an integrated library + UX.

**Wiki caveat:** `DeveloperWiki:TheBigIdeaPage` is **archived** (it redirects into `ArchWiki:Archive`);
do not treat live wiki quotes from that title as authoritative. For a current maintainer narrative,
use **`DeveloperWiki:How_to_be_a_packager`** (soname / `sogrep` / `checkpkg` workflow).

---

## What Arch already has

Understanding the existing tooling is essential to avoid reinventing things and to know exactly where
the gaps are.

### devtools — tools that exist today

| Tool | What it does |
| --- | --- |
| `find-libdeps` | Find soname dependencies for a single package |
| `find-libprovides` | Find sonames provided by a single package |
| `sogrep` | Query **per-repo soname links databases** (downloaded on demand; see `sogrep(1)` and `SOLINKS_MIRROR`) for packages linked to a given `lib*.so*` |
| `lddd` | Find broken library links on the local system |
| `checkpkg` | Compare a freshly built package against the repo version; file-list / shared-object diffs surface soname changes |
| `pkgctl build` | Maintainer build entrypoint (clean chroot, devtools); **pair with** `checkpkg` / `sogrep` as documented on the packager wiki — verify your devtools version for bundled automation |
| `arch-rebuild-order` | Rebuild ordering from **local sync databases** (`depends` / `makedepends` / provides), **not** from ELF `DT_NEEDED` alone |
| `makechrootpkg` | Build a package in a clean chroot for verification |

### The manual workflow today

When a packager bumps a library (typical documented flow):
1. Build and run **`checkpkg`** (and related checks) to spot soname/file changes vs the repo package
2. Packager runs **`sogrep`** over affected repos/libs to list dependent packages (often scripted; see packager wiki)
3. Packager creates a TODO list and notifies other maintainers via email/mailing list
4. Each maintainer rebuilds their package independently in staging
5. Only once the full cascade is confirmed buildable does it move to stable

This means users never see a broken state — but the process is **entirely manual and human-coordinated**.

### What is still painful (gaps vs Pacsea goals)

- **`sogrep` is maintainer-centric:** it answers point queries using downloaded **links DBs**, not a
  full tree of “everything that breaks if I run `pacman -Syu` right now” inside an end-user TUI.
- **No in-process “soname API” for Pacsea:** published DBs back `sogrep`; there is still no
  first-class **Rust library + stable schema** consumed by Pacsea for Preflight (today you would
  shell out or re-implement parsing).
- **Graph semantics differ:** `arch-rebuild-order` orders by **package dependency edges** in syncdb;
  it does not replace “this ELF still `DT_NEEDED`s `libfoo.so.1` but disk will only have
  `libfoo.so.2` after sync.”
- **`namcap`** does not substitute for dynamic-linker reality: `depends=()` stays package-level.
- **Cascade coordination** remains human-heavy (mailing list / staging), and **end users** still lack
  pre-sync **ELF-grounded** warnings in their daily package UI.

---

## Scope and repo status

- **Repo state (pacsea `0.8.2`):** No `soname` / ELF parsing module yet. `Cargo.toml` has no
  `goblin` and no direct `walkdir` (only transitive via other crates). Confirmation UX is split
  across several modals, not one generic transaction summary dialog.
- **Platform:** Meaningful implementation is **Linux-only** (`#[cfg(target_os = "linux")]`). The
  crate ships Windows-specific deps; keep soname code behind `cfg` so non-Linux builds stay clean.

---

## Where this plugs into Pacsea

The main **pre-mutation review** surface is:

| Surface | Location | Role |
| --- | --- | --- |
| **`Modal::Preflight`** | `src/state/modal.rs`, `src/ui/modals/preflight/`, `src/events/preflight/` | Multi-tab review (Summary, Deps, Files, Services, Sandbox) with lazy-loaded tabs — **primary hook** for soname analysis + display. |
| **`Modal::ConfirmInstall` / `ConfirmBatchUpdate` / `ConfirmReinstall`** | `src/ui/modals/confirm.rs`, `src/events/modals/install.rs` | Lighter confirmations; some flows jump to preflight afterward — decide per-flow where soname must block. |
| **`Modal::SystemUpdate`** | `src/events/modals/system_update.rs` | `pacman -Syu`-style path; may not reuse Preflight today — document whether soname runs here or is a follow-up milestone. |
| **Post-transaction** | `PostSummary` | Too late for prevention; only for "we warned you" messaging. |

Treat **Preflight** as the default integration point (new tab or Summary subsection + optional
`PreflightTab` variant), reuse the existing lazy tab sync pattern (`preflight/helpers/sync*.rs`),
and add i18n strings alongside existing `app.modals.*` keys.

---

## Layer 1 — End-user protection (near-term phases)

### Phase 1: Detection — read ELF `DT_NEEDED` (and optionally `DT_SONAME`)

**Proposed module:** `src/soname/` (new), exported from `src/lib.rs` as `pub mod soname` (or nested
under `logic` if you prefer all policy in `logic::`).

**Responsibilities:**

- Parse a file as ELF; collect `DT_NEEDED` entries via a pure-Rust parser (`goblin` is the default).
- Optionally parse `DT_SONAME` on shared objects when building a provider map.
- Skip non-ELF, static binaries, and kernel-provided virtual libraries like `linux-vdso.so.1`.
- Enforce path safety consistent with the rest of the app (no following untrusted symlinks blindly;
  deny `..` in controlled roots). Reuse or generalize the **`is_safe_abs_path`** logic in
  `src/logic/repos/apply_plan.rs` (currently **private** and scoped to repo-apply paths): promote a
  shared helper under `util` for soname readers, or re-document the same invariants for paths under
  `/usr`, `/var/cache/pacman/pkg`, etc.

**Crate notes:**

- **`goblin`:** Confirm default features when adding the dependency; upstream docs note `Elf::parse`
  may require opting into `elf32` / `elf64` / `endian_fd` if defaults change. Pin a concrete version.
- **Do not use `ldd` for untrusted paths:** it can execute loaders/interpreters. Stick to parsing
  ELF in-process, or if shelling out, use `readelf -d` / `objdump -p` with fixed argv.

```rust
pub struct NeededLibs {
    pub path: PathBuf,
    pub needed: Vec<String>, // DT_NEEDED strings, e.g. "libssl.so.3"
}

pub fn read_dt_needed(path: &Path) -> Result<NeededLibs, SonameError> { /* goblin */ }
```

### Phase 2: Build maps of "what exists on disk"

Walk configured library directories (`/usr/lib`, `/usr/lib32`; optionally honor `ld.so.conf` later)
and index realpath-resolved shared objects. Handle:

- **Symlink chains** (`libfoo.so` → `libfoo.so.1.0.0`) — canonicalize with care for performance.
- **Multilib** — separate maps or arch-tagged keys so 32-bit and 64-bit names do not collide.
- **Duplicates** — multiple paths providing the same SONAME filename; keep all candidates or pick
  highest priority per loader rules (document the choice).

Note: `walkdir` is not used anywhere under `src/` today. A small `std::fs::read_dir` walker is
enough to avoid a new dep.

### Phase 3: Checker — correct data sources

The checker needs explicit answers to four questions:

| Question | Practical approach |
| --- | --- |
| What binaries **currently** link to what? | Start from `pacman -Ql <pkg>` file lists for installed packages, filter to ELF paths, parse `DT_NEEDED`. |
| Which package **owns** a path? | `pacman -Qo -- /path` (many calls) vs caching `pacman -Ql` once per relevant package (heavier upfront, cheaper checks). |
| What SONAMEs **will exist after** this sync? | Read `/var/cache/pacman/pkg/*.pkg.tar.zst` for packages in the transaction, extract `.so` ELFs, parse `DT_SONAME`. Without this, `WillBreakAfterUpdate` cannot be sound. |
| AUR / local builds | Inspect built artifact or `pkg.tar.zst` in makepkg output directory — scope carefully. |

**In-tree anchors (reuse before writing new walkers):**

- `src/logic/preflight/metadata.rs` — `find_aur_package_file` already scans **`/var/cache/pacman/pkg/`**
  and **paru/yay** clone dirs for `*.pkg.tar.zst` / `*.pkg.tar.xz` by package prefix (private `fn`;
  consider extracting a small `pacman_cache` helper if soname + size logic share policy).
- Same module — **`pacman -Qp`** on a package file path for metadata; reading **members** of the
  archive for ELF still needs **`tar` + `zstd`/`xz` Rust deps** (or a tightly controlled subprocess);
  neither is a direct `pacsea` dependency today.

Result types (`Ok` / `Missing` / `WouldBreak`) should be named after Pacsea types (`PackageItem`,
`PreflightAction`) and kept **pure** where possible: filesystem + archive readers in IO layer,
decision logic unit-tested with fixtures.

**Edge cases:**

- `linux-vdso.so.1` and similar kernel-provided names — skip always.
- Statically linked targets — no `DT_NEEDED`; nothing to do.
- Fresh installs — checker mostly applies to upgrades/removals that drop or replace `.so` files.
- Partial upgrades — user updates `libfoo` but not dependents; the core scenario.

### Phase 4: TUI integration

- **Default:** Extend `Modal::Preflight` — either a dedicated `Soname` tab (new `PreflightTab`
  variant in `src/state/modal.rs` + render in `src/ui/modals/preflight/tabs/`) or a Summary panel
  with a compact risk table plus drill-down.
- **Blocking vs warning:** Match existing severity patterns (chips, incomplete flags in `summary.rs`)
  and respect `dry_run` semantics — soname scan must not mutate the system.
- **Proceed / abort / details:** Map to existing key handling in `src/events/preflight/`.
- **Flows outside Preflight:** Decide explicitly for `SystemUpdate` and `ConfirmInstall` whether to
  route through Preflight or show a lightweight inline warning.

---

## Layer 2 — Ecosystem tooling (longer-term phases)

These phases go beyond protecting a single user's system and address the Arch team's own acknowledged
tooling gap. The output here could realistically be contributed back upstream or packaged as a
standalone tool.

### Phase 5: Repo-wide provided sonames database

Scan a **large corpus of `.pkg.tar.*` files** — typically a **local mirror** or a dedicated cache
tree — and build a persistent database. (For official repos alone, another design is to **ingest
Arch’s existing `sogrep` links databases** and enrich locally; scanning packages is the “ground truth
from artifacts” approach.)

```
openssl-1.1  →  provides: [libssl.so.1.1, libcrypto.so.1.1]
openssl-3.x  →  provides: [libssl.so.3, libcrypto.so.3]
curl-8.x     →  needs:    [libssl.so.3, libz.so.1, ...]
```

This requires extracting and parsing the ELF binaries inside every `.pkg.tar.zst` in the mirror —
the same data `find-libprovides` and `find-libdeps` produce per-package, but collected into a
queryable index across the whole repo. This is what the Arch Wiki calls for but does not yet exist.

**Implementation notes:**
- Run as a background process triggered after `rsync` mirror sync completes
- Store as a local SQLite database or flat file index for fast querying
- Diff against the previous index to detect soname changes between package versions

### Phase 6: Reverse dependency index

From the provided-sonames database, build the inverse map:

```
libssl.so.3   →  [curl, wget, git, openssh, python-pyopenssl, ...]
libssl.so.1.1 →  [legacy-app, ...]
```

This **overlaps the problem space of `sogrep`** (which uses Arch’s published **links databases**),
but a Pacsea-owned index can add columns (version transitions, local mirror layout, AUR paths),
support offline bulk analysis, and feed the TUI without spawning `sogrep` per question.

### Phase 7: Cascade calculator

Given a detected soname bump, compute the full affected tree via BFS/DFS over the dependency graph:

```
openssl bumps libssl.so.1.1 → libssl.so.3
    Level 1: curl, wget, git, openssh          (direct dependents)
    Level 2: python-requests, gh, ...          (depend on level 1)
    Level 3: ...                               (until no new nodes)
```

Output: an ordered rebuild list **informed by SONAME edges**. This is **orthogonal** to
`arch-rebuild-order` (syncdb package graph): the latter is fast and official but can miss “links
against `libfoo.so.1` without a package-level dep on `foo`” situations; a soname graph catches
**loader-level** orphans. In practice you may want **both** views (conflicts / ordering vs ELF risk).

### Phase 8: Mirror sync watcher + automated reporting

A daemon or hook that:
1. Detects when the local mirror syncs new package versions
2. Diffs the provided-sonames database against the new packages
3. Identifies soname bumps automatically
4. Runs the cascade calculator
5. Reports: affected packages, rebuild order, which rebuilds are already present in the mirror
   (i.e. the Arch team already handled it) vs which are still on the old soname (gap detected)

This is the component that would be genuinely novel — it gives a Pacsea user (or a distro maintainer)
an early warning system equivalent to what the Arch team does manually on their internal
infrastructure.

### Phase 9: Build verification (stretch)

Spin up a clean chroot via `makechrootpkg` and attempt to rebuild affected packages against the new
library. Report: rebuild succeeded / failed / upstream not ready. This crosses into CI territory and
is more suitable as a separate daemon or optional Pacsea subcommand than a core TUI feature.

---

## Full milestone table

| Milestone | Layer | Effort | Notes |
| --- | --- | --- | --- |
| `goblin` + `DT_NEEDED` / `DT_SONAME` readers + unit tests on fixture ELFs | 1 | Small | Include malformed ELF / non-ELF tests. |
| On-disk soname map + symlink / multilib handling | 1 | Small–medium | Performance profile on real `/usr/lib`. |
| Post-transaction SONAME extraction from `.pkg.tar.zst` | 1 | Medium | Unlocks sound `WillBreakAfterUpdate` detection. |
| Reverse-deps / consumer selection (`-Ql` / `-Qo` strategy) | 1 | Medium | Biggest runtime cost; cache aggressively per preflight session. |
| Preflight tab + i18n + event wiring | 1 | Medium | Follow existing lazy tab loading. |
| `SystemUpdate` parity | 1 | Medium | Only if product wants parity without Preflight. |
| Repo-wide provided sonames database (full mirror scan) | 2 | Large | Requires local mirror; background indexer. |
| Reverse dependency index | 2 | Large | Built from provided sonames DB; replaces per-query `sogrep`. |
| Cascade calculator | 2 | Medium | BFS/DFS over dep graph; most of the ecosystem value. |
| Mirror sync watcher + automated reporting | 2 | Large | The novel contribution; early warning system. |
| Build verification via `makechrootpkg` | 2 | Very large | Optional stretch; separate daemon or subcommand. |

---

## Why Layer 2 still matters (even with `sogrep`)

Maintainer tooling is **CLI-shaped and fragmented**: `find-libdeps` / `find-libprovides` per package,
`sogrep` for reverse links via downloaded DBs, `arch-rebuild-order` for syncdb rebuild waves, mailing
list for humans. None of that is a **library + schema** inside Pacsea, nor a **pre-sync end-user**
signal tied to Preflight.

Layer 2 is justified if Pacsea needs:

- A **single materialized database** (SQLite or other) combining “provides / needs / reverse” from
  **local `.pkg.tar.*` trees** (full mirror or cache) independent of `sogrep`’s download cadence
- **Cascade / diff reports** across index versions (useful for maintainers **and** for rich UI)
- **Hooks** after `rsync` or `pacman -Syy` to refresh indexes without manual shell glue

A polished implementation could still be factored into a **standalone crate** for reuse outside the TUI.

---

## Dependencies (when implemented)

```toml
# Cargo.toml — versions to pin at implementation time
goblin = { version = "…", default-features = true }  # verify feature flags
# tar + zstd (+ xz if you support .pkg.tar.xz) — for reading package members without pacman
# walkdir = "2"  # optional; not currently a direct dependency
# rusqlite = "…"  # optional Layer 2 local index
```

---

## Background

When a core library SONAME bumps, dependents compiled against the old name can fail at runtime.
Pacman's `depends=()` arrays are package-level and do not replace a loader-level SONAME audit.
Pacsea closes that gap before the user commits a transaction (Layer 1), and in Layer 2 extends that
capability to repo-wide proactive detection — the kind of work currently done manually by the Arch
packaging team.

The fundamental limit remains: Pacsea can detect risk and surface it, but cannot fix upstream code.
If upstream has not yet released a patch for a new library API, a rebuild will fail regardless of how
good the detection tooling is. The fix chain is always:

```
upstream fixes code
  → distro maintainer rebuilds
    → package pushed to repo
      → user gets working update
```

Pacsea's contribution is making the gap between step 1 and step 2 visible — giving maintainers and
users better information earlier than the current manual process allows.

---

## References in-tree (for implementers)

- Preflight shell: `src/ui/modals/preflight/mod.rs`, `tabs/summary.rs`, `helpers/sync*.rs`
- Modal model: `src/state/modal.rs` (`Preflight`, `PreflightTab`, `PreflightSummaryData`)
- Dependency string handling (`.so` virtual provides filtered today): `src/logic/deps/resolve.rs`
  (`should_filter_dependency`), `src/logic/sandbox/parse.rs`
- Security posture for subprocesses / quoting: `AGENTS.md` / `CLAUDE.md` — prefer `Command` argv
  or existing helpers over shell strings
- Preflight package discovery / cache scan: `src/logic/preflight/metadata.rs` (`find_aur_package_file`,
  `extract_aur_package_sizes`)

## References in the Arch ecosystem

- `devtools` source: https://gitlab.archlinux.org/archlinux/devtools
- `sogrep` man page: https://man.archlinux.org/man/sogrep.1 (soname **links databases**, optional
  `SOLINKS_MIRROR` / `SOCACHE_DIR`)
- `find-libdeps` / `find-libprovides`: https://man.archlinux.org/man/extra/devtools/devtools.7.en
- DeveloperWiki — How to be a packager (soname / `checkpkg` / `sogrep` workflow):
  https://wiki.archlinux.org/title/DeveloperWiki:How_to_be_a_packager
- `DeveloperWiki:TheBigIdeaPage` — **archived** (redirects to `ArchWiki:Archive`); use wiki history
  if you need the old text, not the live redirect target
