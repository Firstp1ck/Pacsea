# Pacsea CLI — current surface and possible extensions

This document inventories **today’s** command-line flags (as implemented in `src/args/definition.rs` and handlers under `src/args/`) and lists **candidate** commands and flags that could be added to expose TUI/backend logic from the shell, for scripting, or for parity with other helpers.

It is a **design brainstorm**, not a commitment or release plan.

**Priority scale:** CLI items use the **same tier definitions** as [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) (🔴 Tier 1 … 🔵 Tier 5). Product-wide shipped work and non-CLI backlog stay authoritative in that file; this document only scopes **CLI-specific** progress and prioritization.

---

## CLI progress todos

**Shipped (baseline):**

- [x] Non-TUI paths: `--search`, `--clear-cache`, `--list` (+ exp/imp/all), `--install` / `-I`, `--remove`, `--update` (Unix), `--news` (+ filters), global `--dry-run`, logging / `--no-color`
- [x] Stub flags present in parser: `-R` (remove-from-file), `--config-dir` (not fully honored)

**Still open (CLI-focused; aligns with tier sections below):**

- [ ] 🔴 **Tier 1** — Implement `-R` / remove-from-file — [#93](https://github.com/Firstp1ck/Pacsea/issues/93)
- [ ] 🔴 **Tier 1** — `--update` fully respects mirror / AUR-helper and related `settings.conf` fields — see [FEATURE_PRIORITY.md](./FEATURE_PRIORITY.md) Tier 1 #3, historic [#57](https://github.com/Firstp1ck/Pacsea/issues/57)
- [ ] 🔴 **Tier 1** — Honor `--config-dir` for config, cache, logs
- [ ] 🔴 **Tier 1** — Global `--json` (or per-command JSON) for scripting exits
- [ ] 🟠 **Tier 2** — `-y` / `--refresh` (reconcile with `src/args/args.rs` vs `definition.rs`)
- [ ] 🟠 **Tier 2** — `pacsea doctor`, `which-helper`, `updates check`, core `pkg` read-only queries, `completions`
- [ ] 🟠 **Tier 2** — `preflight` / `pkgbuild check` / `repo list|validate|diff` / AUR vote+SSH CLI parity
- [ ] 🟡 **Tier 3** — Richer `pkg`/`news`/`files`/`services`/`sandbox` subcommands; `repo apply`; `aur scan` batch [#95](https://github.com/Firstp1ck/Pacsea/issues/95)
- [ ] 🟢 **Tier 4** — `config get/set`, `theme`/`keybinds` dump, `man`, `--print-default-config`, locale overrides
- [ ] 🔵 **Tier 5** — Subcommand-first CLI redesign; multi-PM surface (with [#130](https://github.com/Firstp1ck/Pacsea/issues/130) / v2 vision)

---

## Priority tiers overview

Same scale as [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) § *Priority Tiers Overview*:

| Tier | Description | Timeline suggestion |
|------|-------------|---------------------|
| 🔴 **Tier 1** | High impact, reasonable complexity, core CLI correctness / parity | Next 1–2 releases |
| 🟠 **Tier 2** | Good value, moderate complexity, scripting and TUI parity from the shell | Next 3–4 releases |
| 🟡 **Tier 3** | Valuable for specific use cases, medium effort | Roadmap items |
| 🟢 **Tier 4** | Niche or higher complexity, community-driven | Future consideration |
| 🔵 **Tier 5** | Major structural change (CLI shape, multi-PM), long-term vision | Future major version |

---

## GitHub issue cross-reference (CLI-related)

Subset of [FEATURE_PRIORITY.md](./FEATURE_PRIORITY.md) cross-reference plus CLI-only callouts:

| Topic | Issue | State |
|-------|-------|-------|
| CLI remove-from-file (`-R`) | [#93](https://github.com/Firstp1ck/Pacsea/issues/93) | Open |
| `pacsea -u` / `--update` vs settings (historic) | [#57](https://github.com/Firstp1ck/Pacsea/issues/57) | Closed |
| Sequential multi-package AUR scans (CLI `aur scan`) | [#95](https://github.com/Firstp1ck/Pacsea/issues/95) | Open |
| Service restart hints (`services` CLI) | [#99](https://github.com/Firstp1ck/Pacsea/issues/99) | Open |
| Upgrades / conflicts / custom commands umbrella | [#134](https://github.com/Firstp1ck/Pacsea/issues/134) | Open |
| Mirror search / selection (CLI mirror helpers later) | [#145](https://github.com/Firstp1ck/Pacsea/issues/145) | Open |
| Distro-specific news (`news config` / feeds) | [#131](https://github.com/Firstp1ck/Pacsea/issues/131) | Open |
| Multi-PM + umbrella | [#130](https://github.com/Firstp1ck/Pacsea/issues/130) | Open |

---

## 🔴 Tier 1 — High priority (CLI)

1. **`-R` / remove-from-file** — Parity with `-I`; tracked [#93](https://github.com/Firstp1ck/Pacsea/issues/93).
2. **`--update` + `settings.conf`** — Same mirror/helper behavior as TUI (see FEATURE_PRIORITY Tier 1 #3).
3. **`--config-dir`** — Honor alternate config root (themes, `settings.conf`, caches, logs).
4. **Machine-readable output** — Global `--json` or per-command JSON for `search`, `list`, `news`, and future `updates check`.

*Details for candidates:* see [§2 Finish / extend existing flags](#2-finish--extend-existing-flags-high-overlap-with-code-comments--issues).

---

## 🟠 Tier 2 — Good value (CLI)

5. **`-y` / `--refresh`** — `pacman -Sy` (or policy-aligned refresh) before TUI or one-shots.
6. **`pacsea doctor`** — Preflight: `pacman`, helper, `curl`, privilege tool, config sanity.
7. **`pacsea which-helper`** — Print resolved AUR helper.
8. **`pacsea updates check`** — Pending updates (repo + AUR); optional `--json`.
9. **`pacsea pkg show`**, **`pacsea pkg outdated`** — Read-only query for scripts/automation.
10. **`pacsea completions <shell>`** — `bash`, `zsh`, `fish`, `elvish`.
11. **`pacsea preflight`** — `install` / `remove` / `update` summary without TUI.
12. **AUR CLI parity** — `aur vote` / `unvote` / `ssh-setup` (mirror v0.8.0 TUI).
13. **`pacsea pkgbuild check`** — ShellCheck/Namcap from CLI (mirror TUI).
14. **`pacsea repo list`**, **`validate`**, **`diff`** — Non-mutating repo tooling.
15. **Launch flags** — `--mode package|news`, `--select <pkg>` (optional `--no-mouse` / `--mouse`).
16. **Granular cache** — `cache list`, `cache clear <kind>` (finer than `--clear-cache`).
17. **Install refinements** — `--install` + `--as-deps`, `--needed`, `--aur-only` / `--repo-only`.

*Details:* [§3](#3-launch-modes-tui-entry), [§6](#6-updates--preflight-workers--preflight-modals), [§7](#7-aur-specific), [§8](#8-pkgbuild-checks-shellcheck--namcap), [§9](#9-repositories-reposconf-apply-plan-overlap), [§15](#15-cache-management-finer-than---clear-cache), [§5](#5-install--remove--upgrade-non-interactive--batch) (partial).

---

## 🟡 Tier 3 — Roadmap (CLI)

18. **Extended `pkg` queries** — `files`, `owns`, `deps`, `provides`, `conflicts`, `orphans`, `foreign`, `native`, `group`, `required-by`, structured `pkg search`.
19. **News / advisories** — `news fetch`, `show`, `mark-read` / `mark-unread`; `advisories list`; ties to [#131](https://github.com/Firstp1ck/Pacsea/issues/131) for feed presets.
20. **Files / backups** — `files pacnew` / `pacsave`, `backup list` / `create`, `db sync-status`.
21. **`pacsea services affected`** / restart plan — [#99](https://github.com/Firstp1ck/Pacsea/issues/99).
22. **`pacsea sandbox analyze`**, **`security advisories --installed`**.
23. **`pacsea repo apply`** — Mutating apply from shell (with `--dry-run`).
24. **`pacsea aur scan` (batch)** — [#95](https://github.com/Firstp1ck/Pacsea/issues/95).
25. **Transaction-style helpers** — `sync`, `upgrade --aur-only`, `downgrade`, `reinstall`, `clean` (with `--dry-run`).

*Details:* [§4](#4-package-information--query-mirror-tui--logic--sources), [§5](#5-install--remove--upgrade-non-interactive--batch), [§10](#10-news--advisories), [§11](#11-files-pacnew-backups-db-sync), [§12](#12-services--post-update-hints), [§13](#13-sandbox--security-scanning).

---

## 🟢 Tier 4 — Niche / higher cost (CLI)

26. **`config get` / `set`** — Prefer `validate` first; mutation is easy to get wrong.
27. **`pacsea config validate`**, **`theme show`**, **`keybinds list`**.
28. **`pacsea unlock` / db-unlock** — Dangerous; heavily gated and documented.
29. **`pacsea plan apply`**, **`--restore-session`**, exotic export formats.
30. **`--print-default-config`**, **`pacsea man`**.
31. **`--locale`**, **`pacsea i18n list-locales`**.
32. **`news export`**, low-level `pkg` (e.g. `changelog` if ever available).

*Details:* [§14](#14-configuration--theming), [§16](#16-internationalization--accessibility), [§17](#17-diagnostics--integration) (partial).

---

## 🔵 Tier 5 — Structural / long-term (CLI)

33. **Subcommand-first redesign** — Promote `pacsea pkg|aur|repo|cache|config …` as primary; keep legacy flags as hidden aliases during migration ([§18](#18-structural-cli-redesign-optional)).
34. **Multi-PM CLI** — Only if product adopts multi-PM ([#130](https://github.com/Firstp1ck/Pacsea/issues/130) / v2 vision).

---

## 1. Implemented today (non-TUI exit paths)

| Flag / form | Behavior (summary) |
|-------------|---------------------|
| `-s`, `--search <query>` | Runs AUR helper search (`paru` / `yay`); exits (no TUI). |
| `--clear-cache` | Deletes known cache files under the lists dir; exits. |
| `-l`, `--list` + `--exp` / `--imp` / `--all` | Lists installed packages (explicit / implicit / all); exits. |
| `-i`, `--install <pkg>…` | CLI install path; exits. |
| `-I <file>` | Install package names from file; exits. |
| `-r`, `--remove <pkg>…` | CLI remove path; exits. |
| `-R <file>` | **Stub**: logged; not implemented (see [#93](https://github.com/Firstp1ck/Pacsea/issues/93)). |
| `-u`, `--update` | System update path (Unix); **Windows**: error exit. |
| `-n`, `--news` + `--unread` / `--read` / `-a`, `--all-news` | Prints news feed slice to the terminal; exits. |
| `--dry-run` | Wired into the app for mutating operations where supported. |
| `--log-level`, `-v` / `--verbose`, `--no-color` | Logging / output styling. |
| `--config-dir` | **Stub** in help text; not fully honored. |
| *(no flag)* | Launches the TUI. |
| `--help`, `--version` | From `clap` defaults. |

**Note:** `src/args/args.rs` duplicates a similar `Args` struct and includes a `--refresh` (`-y`) flag that is **not** re-exported from `src/args/mod.rs` (the binary uses `definition.rs`). Treat that file as historical unless it is reconciled.

---

## 2. Finish / extend existing flags (high overlap with code comments & issues)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🔴 1 | `-R` / `--remove-from-file` | Implement remove-from-file (install-list parity). |
| 🔴 1 | `--config-dir <path>` | Honor alternate XDG-style config root for themes, `settings.conf`, caches, logs. |
| 🔴 1 | `--update` + `settings.conf` | Align mirror lists, AUR helper choice, and related settings with TUI behavior. |
| 🔴 1 | Global `--json` | Machine-readable output for **search**, **list**, **news**, **updates**, **preflight**, etc. |
| 🟠 2 | `-y` / `--refresh` | `pacman -Sy` (or full refresh policy) before TUI or before one-shot operations. |

---

## 3. Launch modes (TUI entry)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `--mode package` / `--mode news` | Start directly in Package or News mode (matches `AppMode`). |
| 🟠 2 | `--select <pkg>` | Open TUI with package focused in details (if resolvable). |
| 🟢 4 | `--restore-session` | Restore last layout/filters if you add session snapshots. |
| 🟠 2 | `--no-mouse` / `--mouse` | Force mouse capture on/off (override config). |

---

## 4. Package information & query (mirror TUI / `logic` + `sources`)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea pkg show <name>` | Version, repo, description, URL, licenses (text or `--json`). |
| 🟠 2 | `pacsea pkg outdated` | Installed packages with newer versions (repo + AUR). |
| 🟡 3 | `pacsea pkg files <name>` | File list (wrap `pacman -Ql` / cache-backed). |
| 🟡 3 | `pacsea pkg owns <path>` | Which package owns a path. |
| 🟡 3 | `pacsea pkg deps <name>` | Depends / optional / reverse deps (tree / flat). |
| 🟢 4 | `pacsea pkg changelog <name>` | If available from ALPM / AUR metadata. |
| 🟡 3 | `pacsea pkg search <query>` | Structured search (not only passthrough to `-Ss`). |
| 🟡 3 | `pacsea pkg provides <term>` | What provides a `soname` / command / virtual. |
| 🟡 3 | `pacsea pkg conflicts <name>` | Conflicts / replaces summary. |
| 🟡 3 | `pacsea pkg orphans` | List orphans (`pacman -Qdt`). |
| 🟡 3 | `pacsea pkg foreign` | List foreign packages. |
| 🟡 3 | `pacsea pkg native` | List native (repo) packages. |
| 🟡 3 | `pacsea pkg group <group>` | Members of a package group. |
| 🟡 3 | `pacsea pkg required-by <name>` | Reverse dependency listing. |

---

## 5. Install / remove / upgrade (non-interactive & batch)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `--install` + `--as-deps` | Mark installs as dependencies. |
| 🟠 2 | `--install` + `--needed` | Skip reinstall when already satisfied. |
| 🟠 2 | `--install` + `--aur-only` / `--repo-only` | Restrict resolution source. |
| 🟡 3 | `pacsea sync` | Alias for refresh + optional upgrade policy. |
| 🟡 3 | `pacsea upgrade --aur-only` | AUR-only upgrade batch. |
| 🟡 3 | `pacsea downgrade <pkg>` | Expose downgrade flow used in TUI. |
| 🟡 3 | `pacsea reinstall <pkg>…` | Reinstall without full remove. |
| 🟡 3 | `pacsea clean` | Cache clean (`pacman -Sc` policy) with `--dry-run`. |
| 🟢 4 | `pacsea unlock` / `pacsea db-unlock` | Document or wrap recovery when db lock stuck (dangerous; gated). |

---

## 6. Updates & preflight (workers / preflight modals)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea updates check` | Print pending updates (repo + AUR); optional `--json`. |
| 🟡 3 | `pacsea updates check --security` | Cross-reference advisories where data exists. |
| 🟠 2 | `pacsea preflight install <pkg>…` | Dump preflight summary (sizes, deps, risks) without TUI. |
| 🟠 2 | `pacsea preflight remove <pkg>…` | Same for removal. |
| 🟠 2 | `pacsea preflight update` | Full-system preflight report. |
| 🟢 4 | `pacsea plan apply` | **Optional**: apply a saved transaction plan (if you add plan export). |

---

## 7. AUR-specific

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea aur vote <pkgbase>` | SSH vote (parity with TUI). |
| 🟠 2 | `pacsea aur unvote <pkgbase>` | SSH unvote. |
| 🟡 3 | `pacsea aur vote-status <pkgbase>` | Query voted state if API/SSH allows. |
| 🟠 2 | `pacsea aur ssh-setup` | Guided SSH key setup (non-interactive steps + `--dry-run`). |
| 🟡 3 | `pacsea aur comments <pkg>` | Fetch / print recent comments (`--json`). |
| 🟡 3 | `pacsea aur pkgbuild fetch <pkg>` | Print or save `PKGBUILD` path. |
| 🟡 3 | `pacsea aur srcinfo <pkg>` | Parse / validate `.SRCINFO`. |
| 🟡 3 | `pacsea aur scan <pkg>…` | Security / sandbox scan from CLI (sequential batch per [#95](https://github.com/Firstp1ck/Pacsea/issues/95)). |
| 🟢 4 | `pacsea aur maintainer <name>` | List packages by maintainer (if exposed via RPC). |

---

## 8. PKGBUILD checks (ShellCheck / Namcap)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea pkgbuild check <path-or-pkg>` | Run ShellCheck / Namcap pipeline with same timeouts as TUI. |
| 🟠 2 | `pacsea pkgbuild check --tool shellcheck` | Filter tool. |
| 🟡 3 | `--format sarif` | CI integration for editors / GitHub. |

---

## 9. Repositories (`repos.conf`, apply plan, overlap)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea repo list` | List configured custom repos from `repos.conf`. |
| 🟠 2 | `pacsea repo validate` | Validate entries without applying. |
| 🟡 3 | `pacsea repo apply <name>` | Apply single repo (wrap modal flow). |
| 🟡 3 | `pacsea repo apply --all` | Apply all enabled. |
| 🟠 2 | `pacsea repo diff` | Show `pacman.conf` diff before apply. |
| 🟡 3 | `pacsea repo key-fetch <keyid>` | Wrap `pacman-key` flows used in UI. |
| 🟡 3 | `pacsea repo foreign-overlap` | Report foreign packages overlapping a sync repo name. |

---

## 10. News & advisories

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟡 3 | `pacsea news fetch` | Refresh feeds/cache without TUI. |
| 🟡 3 | `pacsea news list` | Same as `-n` but subcommand style + filters as flags. |
| 🟡 3 | `pacsea news show <id-or-url>` | Render one item (markdown/plain). |
| 🟡 3 | `pacsea news mark-read <id>` / `mark-unread` | Mutate persisted read state. |
| 🟢 4 | `pacsea news export --format opml|rss` | Export subscriptions if multi-feed grows. |
| 🟡 3 | `pacsea advisories list` | Security advisories only (`--severity ge=high`). |
| 🟢 4 | `pacsea news config` | Print effective feed URLs / distro preset (ties to [#131](https://github.com/Firstp1ck/Pacsea/issues/131)). |

---

## 11. Files, `.pacnew`, backups, DB sync

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟡 3 | `pacsea files pacnew` | List `.pacnew` candidates. |
| 🟡 3 | `pacsea files pacsave` | List `.pacsave` candidates. |
| 🟡 3 | `pacsea files merge <path>` | Launch or suggest merge tool (wrapper). |
| 🟡 3 | `pacsea backup list` / `create` | Expose backup list logic from `logic/files`. |
| 🟡 3 | `pacsea db sync-status` | Last sync / mirror age hints. |

---

## 12. Services & post-update hints

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟡 3 | `pacsea services affected` | Units likely needing restart after updates (ties to [#99](https://github.com/Firstp1ck/Pacsea/issues/99)). |
| 🟡 3 | `pacsea services restart --dry-run` | Show `systemctl restart` plan. |

---

## 13. Sandbox / security scanning

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟡 3 | `pacsea sandbox analyze <pkg>` | One-shot sandbox report. |
| 🟡 3 | `pacsea security advisories --installed` | Advisories touching installed set. |

---

## 14. Configuration & theming

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟢 4 | `pacsea config path` | Print resolved config directories. |
| 🟢 4 | `pacsea config validate` | Parse `settings.conf`, `theme.conf`, `keybinds.conf`, `repos.conf`. |
| 🟢 4 | `pacsea config get <key>` / `set <key> <value>` | Optional key-value CLI (risky; prefer `validate` + docs). |
| 🟢 4 | `pacsea theme show` | Dump effective palette (OSC / internal). |
| 🟢 4 | `pacsea keybinds list` | Dump bindings for docs / debugging. |

---

## 15. Cache management (finer than `--clear-cache`)

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea cache list` | Show cache files, sizes, ages. |
| 🟠 2 | `pacsea cache clear news` | Only news-related caches. |
| 🟠 2 | `pacsea cache clear details` | Only package details / PKGBUILD parse caches. |
| 🟠 2 | `pacsea cache clear all` | Same as `--clear-cache`. |

---

## 16. Internationalization & accessibility

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟢 4 | `--locale <code>` | Override language for one-shot CLI output. |
| 🟢 4 | `pacsea i18n list-locales` | Show compiled / available locales. |

---

## 17. Diagnostics & integration

| Tier | Candidate | Purpose |
|------|-----------|---------|
| 🟠 2 | `pacsea doctor` | Preflight: `pacman`, helper, `curl`, privilege tool, terminal size, config OK. |
| 🟠 2 | `pacsea which-helper` | Print chosen AUR helper and path. |
| 🟠 2 | `pacsea completions <shell>` | Generate shell completions (`bash`, `zsh`, `fish`, `elvish`). |
| 🟢 4 | `pacsea man` | Optional man page generation hook. |
| 🟢 4 | `--print-default-config` | Emit example `settings.conf` / `theme.conf` to stdout. |

---

## 18. Structural CLI redesign (optional)

**Tier 🔵 5** — If the flat flag surface becomes crowded, a **subcommand** layout keeps room to grow, for example:

```text
pacsea [global options] <command> [command options]

pacsea tui [--mode news]
pacsea search <query>
pacsea install <pkg>…
pacsea remove <pkg>…
pacsea update
pacsea news list [--unread|--read|--all]
pacsea pkg …
pacsea aur …
pacsea repo …
pacsea cache …
pacsea config …
```

Migration can keep **legacy flags** as hidden aliases for a transition period.

---

## 19. Cross-reference to tracked work

Some items above already appear in [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) or GitHub issues (e.g. CLI remove-from-file [#93](https://github.com/Firstp1ck/Pacsea/issues/93), `--update` vs settings [#57](https://github.com/Firstp1ck/Pacsea/issues/57) history, multi-package scans [#95](https://github.com/Firstp1ck/Pacsea/issues/95), service restart [#99](https://github.com/Firstp1ck/Pacsea/issues/99), mirror UI [#145](https://github.com/Firstp1ck/Pacsea/issues/145), umbrella upgrades [#134](https://github.com/Firstp1ck/Pacsea/issues/134), multi-PM [#130](https://github.com/Firstp1ck/Pacsea/issues/130)).

---

## 20. Suggested prioritization axes (for future trimming)

When picking what to implement first within a tier, useful filters are:

1. **Scripting value** — `--json`, `updates check`, `pkg show`, `doctor`.
2. **Parity with TUI** — vote, repo apply, preflight, PKGBUILD check.
3. **Low coupling** — thin wrappers around existing `logic::*` functions.
4. **Safety** — anything touching `pacman.conf`, keys, or `systemctl` should respect `--dry-run` and privilege rules like the TUI.

---

*Planning doc; adjust tiers as scope changes. Keep [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) as the product-wide source of truth for non-CLI priorities.*
