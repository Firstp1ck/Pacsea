# Roadmap: [FEATURE] Vote for AUR packages via SSH connection (#137)

**Created:** 2026-04-02  
**Status:** Phases 0, 1+2, 3, 4 (action + confirmation flow), 5 complete; Phase 6 UI flow tests in progress  
**Target:** `v0.9.0` (aligned with `dev/IMPROVEMENTS/FEATURE_PRIORITY.md`)  
**Scope:** In-app AUR package voting with secure SSH authentication, actionable errors, and strict dry-run behavior

## Progress todos (2026-04-02)

- [x] **Phase 0: Contract verification (blocking)** -- COMPLETED 2026-04-02
  - [x] Identify authoritative upstream reference(s) (docs + live behavior) for voting
  - [x] Confirm **vote operation semantics**: vote/unvote only (no downvote); input is pkgbase name
  - [x] Confirm **auth mechanism**: SSH key via `ssh aur@aur.archlinux.org vote <pkgbase>`
  - [x] Confirm SSH **is** supported for voting (confirmed via `aurweb/git/serve.py` command registry)
  - [x] Confirm no CSRF tokens / special headers needed for SSH transport
  - [x] Confirm rate limits: SSH has no documented rate limit beyond IP ban for abuse
  - [x] Decide auth flow for v0.9.0: **SSH only** -- zero stored credentials, uses existing SSH key
  - [x] Contract note written (see "Phase 0 verified contract" section below)
  - [x] Error buckets defined (see SSH failure mapping table below)
  - [x] **Done:** contract is unambiguous and implementable without guesswork

- [x] **Phase 1: Transport/auth spike (minimal viable proof)** -- COMPLETED 2026-04-02 (merged with Phase 2)
  - [x] Choose transport approach: **SSH** via `std::process::Command` (confirmed by Phase 0 contract)
  - [x] Create a minimal, non-interactive proof that can run unattended (SSH key auth, no password prompts)
  - [x] Prove the following outcomes end-to-end:
    - [x] success vote (`ssh aur@aur.archlinux.org vote <pkgbase>` -> exit 0, empty stdout)
    - [x] already voted (exit 1, stderr: `vote: already voted for package base: <name>`)
    - [x] package not found (exit 1, stderr: `vote: package base not found: <name>`)
    - [x] SSH auth failure (exit 255, stderr from SSH)
    - [x] network timeout/offline (SSH connection failure)
  - [x] Ensure proof does not leak private key paths in stdout/stderr logs
  - [x] Capture the proof invocation and expected outputs in this roadmap (high level, no secrets)
  - [x] **Done:** implemented as `src/sources/aur_vote.rs` with `SshVoteTransport` trait, `RealSshTransport`, and 20 unit tests

- [x] **Phase 2: `AurVoteService` core module + error contract** -- COMPLETED 2026-04-02 (merged with Phase 1)
  - [x] Add new module: `src/sources/aur_vote.rs`
  - [x] Add types:
    - [x] `VoteAction` enum: `Vote` / `Unvote` (no downvote)
    - [x] `AurVoteOutcome` (user-facing success text + action performed)
    - [x] `AurVoteError` (typed: `AlreadyVoted`, `NotVoted`, `NotFound`, `AuthFailed`, `Maintenance`, `Banned`, `Timeout`, `NetworkError`, `SshNotFound`, `Unexpected`)
    - [x] `AurVoteContext` (includes `dry_run` + SSH timeout + SSH command)
  - [x] Add transport abstraction for testability (trait + real SSH impl + mock impl)
  - [x] Implement `dry_run` short-circuit:
    - [x] no SSH subprocess spawned
    - [x] returns deterministic "would vote/unvote ..." outcome
  - [x] Implement SSH subprocess with explicit timeout (via `ConnectTimeout` SSH option)
  - [x] Parse stderr to map SSH/aurweb errors into typed `AurVoteError` variants
  - [x] Add log redaction policy:
    - [x] never log SSH key paths or identity file contents
    - [x] stderr from SSH is captured but sanitized before user display
  - [x] **Done:** module wired via `src/sources/mod.rs`, all types exported, all quality gates pass

- [x] **Phase 3: Runtime worker integration (async + UI-safe)** -- COMPLETED 2026-04-02
  - [x] Add a runtime request/response path for vote execution (pattern-match existing workers)
  - [x] Ensure vote execution never blocks the TUI render loop
  - [x] Ensure result is returned to UI layer as:
    - [x] toast for success
    - [x] alert/modal for actionable failures (as appropriate)
  - [x] Ensure `dry_run` wiring uses the app’s `app.dry_run` field (no duplicated flags)
  - [x] **Done when:** the app can execute a vote in the background and surface a result message reliably

- [x] **Phase 4: TUI action + confirmation flow** -- COMPLETED 2026-04-02
  - [x] **Phase 4.1: Live voted indicator (results + details)** -- COMPLETED 2026-04-02
    - [x] Add source-level vote-state contract/check API (`aur_vote_state`)
    - [x] Add runtime vote-state worker + request/response channels
    - [x] Trigger non-blocking vote-state checks on AUR selection changes
    - [x] Render vote-state indicator in both results row and details pane
    - [x] Add tests for trigger/runtime/event-loop vote-state handling
    - [x] Validate quality gates (`fmt`, `clippy -D warnings`, `check`, `test --test-threads=1`)
  - [x] Decide interaction surface:
    - [x] keybind-only action ("Ctrl + e" default vote, "Ctrl + Shift + e" default unvote)
  - [x] Add UI affordance for:
    - [x] Vote (add vote for package)
    - [x] Unvote (remove existing vote)
  - [x] Add confirm modal:
    - [x] shows pkg name + effect
    - [x] shows dry-run indicator when enabled
    - [x] requires explicit confirmation (no accidental voting)
  - [x] Guardrails:
    - [x] action only available for `Source::Aur`
    - [x] non-AUR selection yields clear “not available” message
  - [x] **Done when:** a user can trigger vote from the TUI for an AUR package and gets a confirm + completion message

- [x] **Phase 5: Settings/config keys (only after contract is final)** -- COMPLETED 2026-04-02
  - [x] Minimal config keys for SSH transport:
    - [x] `aur_vote_enabled` (bool, default false)
    - [x] `aur_vote_ssh_timeout_seconds` (u32, default 10)
    - [x] `aur_vote_ssh_command` (string, default `ssh`) -- for users with non-standard SSH setups
  - [x] Add keys to settings skeleton and example config:
    - [x] `src/theme/config/skeletons.rs` (`SETTINGS_SKELETON_CONTENT`)
    - [x] `config/settings.conf`
  - [x] Wire keys through settings system:
    - [x] `src/theme/types.rs` (fields + defaults)
    - [x] `src/theme/settings/parse_settings.rs` (parsing via `parse_aur_vote_settings`)
    - [x] `src/theme/config/settings_ensure.rs` (save/ensure)
  - [x] **Done:** config keys added, parsed, and round-tripped via skeleton/ensure system

- [ ] **Phase 6: Tests (unit + integration + regression coverage)** -- unit tests COMPLETED 2026-04-02
  - [x] Unit tests (SSH transport mocked/stubbed) -- 20 tests in `src/sources/aur_vote.rs`:
    - [x] dry-run does not spawn SSH subprocess (vote + unvote)
    - [x] success (exit 0, empty stderr) maps to expected outcome text (vote + unvote)
    - [x] already voted (exit 1, stderr pattern) -> `AlreadyVoted` error
    - [x] not voted / unvote (exit 1, stderr pattern) -> `NotVoted` error
    - [x] package not found (exit 1, stderr pattern) -> `NotFound` error
    - [x] SSH auth failure (exit 255) -> `AuthFailed` error
    - [x] network timeout/offline -> `Timeout` / `NetworkError` error
    - [x] AUR maintenance (exit 1, stderr pattern) -> `Maintenance` error
    - [x] SSH binary not found (`io::Error` NotFound) -> `SshNotFound` error
    - [x] redaction tests: no SSH key paths appear in user-facing error strings
    - [x] stderr truncation test: long output is bounded
    - [x] Display impls + context defaults
  - [ ] UI flow tests (where feasible in current test harness): -- PARTIAL 2026-04-02
    - [x] action unavailable for non-AUR packages
    - [x] confirm modal shows correct package/effect
  - [ ] Integration test (env-gated, not in CI by default):
    - [ ] only runs when env var is set + credentials present
    - [ ] verifies one real vote round-trip (or a non-mutating validation if contract supports it)
  - [ ] **Done when:** tests cover success + all major failure modes, and local `cargo test` is green

- [ ] **Phase 7: Manual QA checklist + hardening + release gate**
  - [ ] Add manual test checklist under `dev/TESTING/` (only when implementation starts)
  - [ ] Verify “no secret logging” by scanning debug output during vote flow
  - [ ] Validate behavior on common misconfigs:
    - [ ] no SSH key uploaded to AUR account (SSH auth failure)
    - [ ] no SSH agent running / key not loaded
    - [ ] SSH binary not found (`aur_vote_ssh_command` points to nonexistent binary)
  - [ ] Validate `--dry-run` from end-to-end:
    - [ ] no network mutation
    - [ ] clear simulated outcome message
  - [ ] Run release gates:
    - [ ] `cargo fmt --all` produces no diff
    - [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
    - [ ] `cargo check` clean
    - [ ] `cargo test -- --test-threads=1` clean
  - [ ] **Done when:** feature meets acceptance criteria and is release-ready for v0.9.0

## Phase 0 verified contract (2026-04-02)

### Transport: SSH (primary and only for v0.9.0)

AUR provides a full SSH command interface at `aur@aur.archlinux.org` that supports voting.
Source: `aurweb/git/serve.py` in [aurweb v6.3.4](https://gitlab.archlinux.org/archlinux/aurweb/-/tree/v6.3.4).

**Commands:**
- **Vote**: `ssh aur@aur.archlinux.org vote <pkgbase>`
- **Unvote**: `ssh aur@aur.archlinux.org unvote <pkgbase>`

**Auth**: User's SSH key must be uploaded to their AUR account profile. No passwords, cookies, or tokens involved.

**Input**: `<pkgbase>` is the package base name (e.g., `pacsea-bin`, `yay-bin`), NOT the package name.

**Success**: exit code 0, empty stdout, empty stderr.

**No downvote**: AUR supports only vote (add) and unvote (remove). There is no negative vote.

### SSH failure mapping

| Condition | Exit code | Stderr pattern | Pacsea error variant | User-facing message |
|---|---|---|---|---|
| Success | 0 | (empty) | `Ok(Voted)` / `Ok(Unvoted)` | "Voted for {pkg}" / "Removed vote for {pkg}" |
| Already voted | 1 | `vote: already voted for package base: {name}` | `AlreadyVoted` | "You have already voted for {pkg}" |
| Not voted (unvote) | 1 | `unvote: missing vote for package base: {name}` | `NotVoted` | "You haven't voted for {pkg}" |
| Package not found | 1 | `package base not found: {name}` | `NotFound` | "Package base '{pkg}' not found on AUR" |
| AUR maintenance | 1 | `The AUR is down due to maintenance` | `Maintenance` | "AUR is under maintenance. Try again later" |
| IP banned | 1 | `The SSH interface is disabled for your IP address` | `Banned` | "SSH interface disabled for your IP. Contact AUR support" |
| SSH auth failure | 255 | SSH client error (e.g., `Permission denied`) | `AuthFailed` | "SSH auth failed. Ensure your SSH key is uploaded to your AUR account at https://aur.archlinux.org/account" |
| SSH timeout | varies | SSH timeout/connection refused | `Timeout` | "Connection to aur.archlinux.org timed out. Check network" |
| DNS/network failure | varies | SSH resolution/connect error | `NetworkError` | "Could not connect to aur.archlinux.org. Check connectivity" |
| SSH binary missing | N/A | `std::io::Error` (NotFound) | `SshNotFound` | "SSH binary not found. Install openssh or configure aur_vote_ssh_command" |

### HTTP alternative (documented for completeness, NOT used in v0.9.0)

AUR also supports HTTP-based voting for browser clients:
- `POST /pkgbase/{name}/vote` (requires `AURSID` session cookie + `Referer` header)
- `POST /pkgbase/{name}/unvote` (same auth requirements)
- Auth: `AURSID` cookie from browser login session
- CSRF: `Referer` header must start with `https://aur.archlinux.org`
- No form fields required (POST body can be empty)
- Response: `303 See Other` redirect to `/pkgbase/{name}`
- Failure: missing auth -> `303` redirect to `/login`; bad Referer -> `400 Bad Request` with "Bad Referer header."

This path requires cookie management and is reserved for potential future HTTP fallback.

### Upstream source references

- `aurweb/git/serve.py` -- SSH command registry (`vote`, `unvote`, and 10+ other commands)
- `aurweb/routers/pkgbase.py` -- HTTP vote/unvote route handlers
- `aurweb/auth/__init__.py` -- `BasicAuthBackend` (AURSID cookie) + POST Referer validation
- `aurweb/packages/util.py` -- `get_pkg_or_base()` (404 on missing pkgbase)
- `templates/partials/packages/actions.html` -- vote/unvote form HTML (no hidden fields, no CSRF token)

### Open questions resolved

1. **Is AUR vote API-supported for non-browser clients?** YES -- via SSH command interface, fully supported.
2. **Safest credential bootstrap for Pacsea?** SSH key auth -- zero credential storage. User uploads their existing SSH public key to AUR account.
3. **Upvote only initially?** NO -- both `vote` and `unvote` are trivially supported from day one via the same SSH interface.
4. **Vote state shown inline?** Deferred to implementation (Phase 3/4). v0.9.0 will use toast confirmation; inline state display is a follow-up.

---

## 1) Problem and constraints

Users want to vote on AUR packages without leaving Pacsea. The implementation must preserve Pacsea’s security posture:

- no credential/token/private-key logging
- no shell string concatenation for network/auth commands
- actionable user-facing failures (missing auth, denied vote, package not found, network/timeouts)
- strict `dry_run` behavior (no remote mutation when `app.dry_run == true`)
- graceful degradation when required tools/credentials are not present

Verified: AUR voting is supported via SSH command (`ssh aur@aur.archlinux.org vote <pkgbase>`). This is the primary transport for v0.9.0.

## 2) Architecture decision plan

### 2.1 Proposed service boundary

Create a focused service module:

- `src/sources/aur_vote.rs` (or `src/logic/aur_vote.rs`)
- main API:
  - `vote(pkgbase: &str, action: VoteAction, ctx: AurVoteContext) -> Result<AurVoteOutcome, AurVoteError>`

Where:

- `VoteAction`: `Vote` | `Unvote`
- `AurVoteContext` includes:
  - `dry_run: bool`
  - SSH timeout (seconds)
  - SSH command path (default: `ssh`)
- `AurVoteOutcome`: user-facing success copy + action performed
- `AurVoteError`: typed enum (`AlreadyVoted`, `NotVoted`, `NotFound`, `AuthFailed`, `Maintenance`, `Banned`, `Timeout`, `NetworkError`, `SshNotFound`, `Unexpected`)

### 2.2 Why this shape

- keeps transport/auth logic isolated from TUI event handlers
- testable with mocked transport/subprocess adapter
- consistent with current source/service style (`src/sources/*`, event layer dispatch in `src/events/*`)

## 3) Phase-by-phase implementation details

## Phase 0: Contract verification (blocking) -- COMPLETE

See "Phase 0 verified contract" section above for full results.

Summary: AUR voting uses SSH command interface (`ssh aur@aur.archlinux.org vote/unvote <pkgbase>`), authenticated by user's SSH key uploaded to AUR account. No credentials stored by Pacsea. Full failure mapping documented.

## Phase 1: Transport/auth spike

Goal: prove the SSH vote command works from Rust's `std::process::Command`.

Approach:

1. Use `std::process::Command` to run `ssh aur@aur.archlinux.org vote <pkgbase>`
2. No shell interpolation -- pass arguments as a vector
3. Capture both stdout and stderr, parse exit code
4. Apply `ConnectTimeout` SSH option for bounded execution
5. No credentials stored -- SSH agent / key file handles auth transparently

Deliverable:

- spike helper demonstrating:
  - success path (exit 0)
  - already voted (exit 1 + stderr pattern match)
  - package not found (exit 1 + stderr pattern match)
  - SSH auth failure (exit 255)
  - timeout/network failure

## Phase 2: Core `AurVoteService`

Implementation tasks:

1. Add `VoteAction`, `AurVoteOutcome`, `AurVoteError`, `AurVoteContext` types.
2. Implement vote entrypoint with early `dry_run` return:
   - returns simulated "would vote/unvote" message
   - does not spawn SSH subprocess
3. Add transport trait:
   - `SshVoteTransport` trait with `execute(action, pkgbase) -> Result<Output>`
   - `RealSshTransport` (spawns `ssh` subprocess)
   - `MockSshTransport` (returns configured responses for testing)
4. Implement stderr parsing to map aurweb error messages to typed `AurVoteError` variants.
5. Add safe logging policy:
   - never log SSH key paths or identity file contents
   - sanitize stderr before including in user-facing errors

Suggested file touchpoints:

- `src/sources/mod.rs` (module export)
- `src/sources/aur_vote.rs` (new service)

## Phase 3: TUI integration

Goal: add voting action to selected AUR package flow.

Tasks:

1. Add action trigger in package-focused key/mouse flows:
   - likely in `src/events/search/*`, `src/events/mouse/*`, and/or global key handler wiring
2. Add confirmation modal:
   - includes package base name and intended effect ("Vote" or "Unvote")
   - explicit dry-run indicator when enabled
3. Execute vote asynchronously via runtime worker channel:
   - spawns SSH subprocess in background
   - keep UI responsive
   - report success/failure via toast/alert modal
4. Guard action availability:
   - only enabled for `Source::Aur`
   - only enabled when `aur_vote_enabled = true` in settings
   - for non-AUR packages show disabled state or concise reason

Likely touchpoints:

- `src/events/mod.rs`
- `src/events/search/*`
- `src/events/mouse/*`
- `src/state/modal.rs` (new modal variant if needed)
- `src/ui/modals/*` (confirm renderer, message copy)
- `src/app/runtime/workers/*` (async execution path)

## Phase 4: Config and settings integration

Goal: add SSH voting config keys.

Config keys (verified minimal set for SSH transport):

- `aur_vote_enabled` (bool, default: `false`) -- master switch
- `aur_vote_ssh_timeout_seconds` (u32, default: `10`) -- SSH ConnectTimeout value
- `aur_vote_ssh_command` (string, default: `ssh`) -- for non-standard SSH setups

Required updates when keys are added:

- `src/theme/types.rs` (`Settings` fields + defaults)
- `src/theme/settings/parse_settings.rs` (parser)
- `src/theme/config/settings_ensure.rs` (persist/ensure flow)
- `src/theme/config/skeletons.rs` (`SETTINGS_SKELETON_CONTENT`)
- `config/settings.conf` example

## Phase 5: Tests (failing-first where applicable)

Unit tests (using `MockSshTransport`):

- dry-run returns simulated outcome without spawning SSH
- success (exit 0) maps to `Ok(Voted)` / `Ok(Unvoted)`
- already voted (exit 1, specific stderr) maps to `AlreadyVoted`
- not voted on unvote (exit 1, specific stderr) maps to `NotVoted`
- package not found (exit 1, specific stderr) maps to `NotFound`
- SSH auth failure (exit 255) maps to `AuthFailed`
- timeout/network maps to `Timeout` / `NetworkError`
- AUR maintenance (exit 1, specific stderr) maps to `Maintenance`
- no SSH key paths in user-facing error strings

Integration tests:

- gated (env var `PACSEA_AUR_VOTE_LIVE_TEST=1` + SSH key configured); disabled in CI by default
- verifies one real vote round-trip against live AUR

Recommended locations:

- `src/sources/aur_vote.rs` inline unit tests
- `src/events/*/tests.rs` for UI trigger and modal flow
- optional integration test under `tests/aur_vote_integration.rs` (env-gated)

## Phase 6: QA, hardening, release readiness

Manual QA checklist (to be added under `dev/TESTING/` during implementation):

1. Vote for known AUR package (SSH key configured)
2. Unvote the same package
3. Vote again when already voted (expect "already voted" message)
4. Unvote when not voted (expect "not voted" message)
5. Vote for nonexistent package (expect "not found" message)
6. Dry-run vote from UI (no SSH subprocess spawned, clear simulated output)
7. No SSH key uploaded to AUR (expect "SSH auth failed" message)
8. Network offline/timeout (expect "connection timed out" message)
9. Non-AUR package selected (action unavailable or guarded)
10. AUR voting disabled in settings (action unavailable)

Release gate commands:

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check`
4. `cargo test -- --test-threads=1`

## 4) Security model

- **no credential storage**: SSH key auth means zero credentials in Pacsea config/memory
- **no password prompts**: SSH agent handles key passphrase; Pacsea never collects passwords
- **no secret logging**: never log SSH key paths, identity file contents, or agent socket paths
- **explicit argument vectors**: `std::process::Command` with args array, no shell interpolation
- **bounded timeouts**: `ConnectTimeout` SSH option prevents indefinite hangs
- **no temporary credential artifacts**: SSH transport needs no cookie/token files

## 5) Error message contract (user-facing)

All failures should answer:

1. what failed
2. why it likely failed
3. what user can do next

Examples:

- "AUR vote failed: SSH authentication denied. Ensure your SSH key is uploaded to your AUR account at https://aur.archlinux.org/account"
- "AUR vote: you have already voted for 'yay-bin'."
- "AUR vote failed: package base 'nonexistent-pkg' not found on AUR. Verify the package name."
- "AUR vote failed: connection timed out. Check network connectivity and retry."
- "AUR vote failed: SSH binary not found. Install openssh or set aur_vote_ssh_command in settings."
- "AUR vote failed: AUR is under maintenance. Try again later."

## 6) Open questions -- ALL RESOLVED in Phase 0

1. ~~Is AUR vote currently API-supported for non-browser clients?~~ **YES** -- SSH command interface: `ssh aur@aur.archlinux.org vote <pkgbase>`
2. ~~If HTTP session is required, what is the safest credential bootstrap flow?~~ **N/A** -- SSH key auth, zero credential storage
3. ~~Should Pacsea support only upvote initially?~~ **NO** -- both `vote` and `unvote` supported from day one via SSH
4. ~~Should vote state be shown inline?~~ **Implemented as follow-up (Phase 4.1)** -- live vote-state indicator now shown in results + details

## 7) Minimal implementation order (recommended)

1. Phase 0 contract verification
2. Phase 1 spike
3. Phase 2 service + unit tests
4. Phase 3 UI wiring + worker path
5. Phase 4 config keys
6. Phase 5/6 validation and release gate

This order minimizes wasted refactors if AUR auth mechanics differ from current assumptions.
