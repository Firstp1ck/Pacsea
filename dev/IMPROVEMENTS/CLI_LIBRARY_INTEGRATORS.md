# Pacsea CLI — library & integrator contracts

**Audience:** Teams that depend on the `pacsea` **crate** (`src/lib.rs`: `logic`, `index`, `app`, …) and apps in other languages that **spawn the `pacsea` binary** and parse its output. They need stable **API-adjacent CLI** contracts: versioning, JSON shape, paths, exit codes.

**Sibling doc:** General CLI candidates and priority tiers live in [`CLI_POSSIBLE_COMMANDS.md`](./CLI_POSSIBLE_COMMANDS.md).

**Priority scale:** Same tier definitions as [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) (🔴 Tier 1 … 🔵 Tier 5). Related todos are tracked in [`CLI_POSSIBLE_COMMANDS.md`](./CLI_POSSIBLE_COMMANDS.md) under *CLI progress todos*.

---

## Library & downstream integrators (summary)

**Rust:** Depend on the `pacsea` crate and call `logic`, `index`, `install`, etc., directly (see crate root [`src/lib.rs`](../../src/lib.rs) rustdoc). **Still useful:** matching **binary semver**, **resolved paths**, and **JSON payload versions** to the library version in CI and integration tests.

**Non-Rust / GUI / automation:** Often **subprocess** the `pacsea` binary. They need the same guarantees as shell scripts, plus **discoverability** (what commands exist, what tools are on `PATH`, where caches live).

---

## Goal & principles

**Goal:** Let downstream developers **probe** and **parse** Pacsea safely—whether they link the **library** or **exec** the binary from Electron, Python, CI, or another TUI.

**Principles:**

- **Single JSON envelope** (when `--json` / `--output-format json`) with a top-level `schema_version` (integer or semver) incremented on breaking output changes.
- **Stdout vs stderr** — parseable payloads only on stdout; diagnostics and human text on stderr unless `--json-errors` sends structured errors to stderr as JSON lines.
- **No TTY assumptions** — integrators often run with stdin/stdout not a terminal; respect `NO_COLOR` / `--no-color` and avoid paging.
- **Align with the crate** — document which CLI subcommands map to which public modules (`index`, `logic`, `sources`, …) so Rust users can choose **in-process** vs **subprocess**.

---

## 🔴 Tier 1 — Integrator-critical

| Candidate | Purpose |
|-----------|---------|
| Global **`--json`** + **`schema_version` field** | Every JSON-capable subcommand includes a version; breaking changes bump it. |
| **`--output-format json\|jsonl`** | `jsonl` for large streams (search hits, file lists, news items) without huge arrays. |
| **Structured errors when `--json`** | Stable `{"error":{"code","message","detail"}}` (or shared envelope) so callers don’t scrape English text. |
| **`PACSEA_JSON=1`** (optional env) | Inherit JSON mode in child processes / wrappers without repeating flags. |

---

## 🟠 Tier 2 — Introspection (`pacsea api` or `pacsea debug`)

| Candidate | Purpose |
|-----------|---------|
| **`pacsea api version`** | Binary version, optional embedded **library/crate** version string, `json_schema_version`, target triple, build features (if any become `cfg` gated). |
| **`pacsea api capabilities`** | JSON: `pacman`/`paru`/`yay`/`checkupdates`/`curl`/ShellCheck/Namcap/`sudo`/`doas` presence, OS family, **effective** `privilege_tool` / helper from `settings.conf`. |
| **`pacsea api paths`** | Resolved **config**, **cache/lists**, **logs** directories (after `--config-dir` / XDG rules)—for tests and GUIs that read/write adjacent files. |
| **`pacsea api exit-codes`** | Print or emit JSON map of **documented** exit codes (success, user abort, missing tool, partial apply, …). |
| **`pacsea api schema [--command <name>]`** | Dump **JSON Schema** (or OpenAPI-style component) for each command’s stdout payload—CI can validate client parsers. |
| **`pacsea doctor --json`** | Same as `doctor` but machine-readable; overlaps `capabilities` + health checks. |

---

## 🟡 Tier 3 — Deeper hooks for tooling

| Candidate | Purpose |
|-----------|---------|
| **`pacsea index stats`** | Row counts, last build time, repos represented—mirrors `pacsea::index` without loading the TUI. |
| **`pacsea index export --format json`** | Optional **sanitized** export of official index snapshot shape (document fields = those in `OfficialIndex` / public types). |
| **`pacsea batch --dry-run --file -`** | Read **NDJSON** or JSON array of operations (`{"op":"query","term":"..."}`) for automation; strict validation and per-line errors. |
| **`pacsea rpc` / stdin protocol** (optional) | Long-lived **stdio JSON-RPC** or **ndjson request/response** for GUIs that want one process—**high maintenance**; only if demand is clear. |

---

## 🟢 Tier 4 — Documentation & ergonomics

| Candidate | Purpose |
|-----------|---------|
| **`pacsea api modules`** | List public **rustdoc module** names and one-line responsibility (mirrors `lib.rs` table)—helps devs pick crate vs CLI. |
| **Link to docs.rs** in `api version` output | Pinned documentation URL for the matching crate version. |
| **`--locale` + JSON** | Include active locale in `api capabilities` for UI wrappers. |

---

## 🔵 Tier 5 — Long-term / policy

| Candidate | Purpose |
|-----------|---------|
| **gRPC / HTTP sidecar** | Separate from the TUI binary; only if Pacsea grows a formal remote API. |
| **C ABI / dynamic library** | Unlikely; Rust crate + JSON CLI is the default integration story. |

---

## Non-goals (clarify for integrators)

- **Guaranteeing** every internal `logic::*` function gets a 1:1 CLI—only **stable, documented** surfaces need contracts.
- **Stable Rust API across minor releases** beyond what SemVer already promises—document breaking changes in release notes; CLI `schema_version` is for **JSON**, not Rust ABI.

---

## Quick reference table

| Area | Examples |
|------|-----------|
| Versioning | `pacsea api version` — binary + crate semver, **`json_schema_version`** for stdout payloads |
| Discoverability | `pacsea api capabilities` — JSON: tools on `PATH`, OS, effective helper / `privilege_tool` |
| Paths | `pacsea api paths` — resolved config, cache/lists, logs (respects `--config-dir`) |
| Contracts | `pacsea api schema`, `pacsea api exit-codes`; global `--json`, `--output-format json\|jsonl` |
| Errors | Structured JSON errors (`--json-errors`) so GUIs don’t parse English |
| Env | `PACSEA_JSON=1` to force JSON for wrapped invocations |
| Index / automation | `pacsea index stats`, optional `index export`; `pacsea batch` / NDJSON job files; `doctor --json` |
| Long-term | Optional stdio JSON-RPC; only if there is clear demand |

---

*Planning doc; adjust as scope changes. Keep [`FEATURE_PRIORITY.md`](./FEATURE_PRIORITY.md) as the product-wide source of truth for non-CLI priorities.*
