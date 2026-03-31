# Interactive Authentication Plan (Fingerprint / PAM)

## Progress todos (2026-03-30)

**Status:** Milestones 1–4 complete. All implementation, UX polish, documentation, and tests delivered.

### Milestone 1 (CLI MVP) — DONE

- [x] Add `auth_mode` (`prompt` / `passwordless_only` / `interactive`), parser, skeleton, `settings_ensure`, legacy `use_passwordless_sudo` mapping + deprecation warning
- [x] Resolver in `src/logic/password.rs` (`resolve_auth_mode`, `should_skip_password_modal`, etc.) — tool-agnostic (sudo/doas)
- [x] `src/args/update.rs` — interactive mode skips `rpassword`, no `sudo -S` / doas PTY piping, stdin not `null()` on that path
- [x] Unit tests for parse, resolver, legacy mapping, doas edge cases (18 new tests, all passing)

### Milestone 2 (TUI integration) — DONE

- [x] Replace all TUI `should_use_passwordless_sudo` call sites with `resolve_auth_mode` 3-way branch (`direct.rs` ×2, `command_keys.rs` ×2 (install + remove), `handlers.rs` ×2, `optional_deps.rs`)
- [x] Implement TUI interactive auth handoff — `restore_terminal()` → `run_interactive_auth()` → `setup_terminal()` via `try_interactive_auth_handoff()` in `events/mod.rs`
- [x] `run_interactive_auth(tool)` in `privilege.rs` — runs `sudo -v` (credential refresh) or `doas true` (with `persist`)
- [x] Downgrade flow — interactive mode spawns in external terminal via `spawn_downgrade_in_terminal()` (no password piping, privilege tool handles auth natively)
- [x] Remove flow — interactive mode does terminal handoff + proceeds with `password: None`
- [x] Executor path: `password: None` already suppresses all password-related logic:
  - PTY injection: `.and(password)` = `None` → no injection
  - `build_password_pipe`: `password.as_deref()` = `None` → falls back to `build_privilege_command`
  - AUR warmup: `has_aur && password.is_some()` = `false` → skipped
  - `SUDO_ASKPASS`: `password.is_some()` = `false` → skipped
  - `build_credential_warmup`: same guard → skipped
- [x] `resolve_auth_mode` backward-compat: also honors `PACSEA_TEST_SUDO_PASSWORDLESS=1` → `PasswordlessOnly` for legacy test infrastructure
- [x] Added `app.errors.authentication_failed` i18n key to en-US, de-DE, hu-HU (hu-HU: English placeholder + TODO)
- [x] Made `app::terminal` module public for cross-module access
- [x] Integration tests in `tests/passwordless_sudo/interactive_auth_integration.rs` (7 new tests, all passing)
- [x] All existing tests pass (42 passwordless_sudo tests + full suite)
- [x] fmt, clippy (pedantic+nursery), check, test — all clean

### Milestone 3 (UX polish) — DONE

- [x] Fingerprint/PAM detection helper in `privilege.rs`:
  - `detect_pam_fingerprint(tool)` — reads `/etc/pam.d/{tool}`, `system-auth`, `system-local-login` for `pam_fprintd`
  - `detect_fprintd_enrolled()` — runs `fprintd-list $USER` to check for enrolled fingerprints
  - `is_fingerprint_available()` — cached `OnceLock` combining both checks
- [x] Password modal hint in `src/ui/modals/password.rs`:
  - Shows yellow "Tip: Fingerprint auth detected…" when auth_mode=prompt AND fingerprint detected
  - Dynamic height adjustment for the hint lines
- [x] Added `app.modals.password_prompt.fingerprint_hint` i18n key to en-US, de-DE, hu-HU (hu-HU: English placeholder + TODO)
- [x] 4 unit tests for detection functions (no-panic smoke tests)
- [x] fmt, clippy, check, test — all clean

### Milestone 4 (compat + docs polish) — DONE

- [x] Added `[Unreleased]` changelog entry in `CHANGELOG.md` documenting `auth_mode`, interactive auth, fingerprint detection, and legacy compatibility
- [x] Expanded `sudo -S` stalling + doas `persist` documentation in `config/settings.conf` and `skeletons.rs`
- [ ] Manual testing — fingerprint + fallback (sudo & doas), terminal emulators (alacritty, kitty, konsole, wezterm), `privilege_tool = auto` cross-tool (requires manual execution)

## Progress todos (2026-03-30)

**Status:** Not implemented. No `sudo_auth_mode` (or equivalent) in settings; TUI/CLI still follow existing password modal + passwordless `sudo -n` logic.

- [ ] **Milestone 1 (CLI MVP):** Add `sudo_auth_mode` (`prompt` / `passwordless_only` / `interactive`), parser, skeleton, `settings_ensure`, legacy `use_passwordless_sudo` mapping + deprecation warning
- [ ] **Milestone 1 (CLI MVP):** Resolver in `src/logic/password.rs` (`resolve_sudo_auth_mode`, `should_skip_password_modal`, etc.)
- [ ] **Milestone 1 (CLI MVP):** `src/args/update.rs` — interactive mode skips `rpassword`, no `sudo -S` piping, stdin not `null()` on that path
- [ ] **Milestone 1:** Unit tests for parse, resolver, legacy mapping
- [ ] **Milestone 2 (TUI):** Interactive mode via `restore_terminal()` / `setup_terminal()` (or external terminal where already used); wire all strategy call sites (install, preflight, modals, optional deps, direct install)
- [ ] **Milestone 2:** Ensure PTY executor paths do not assume interactive sudo without input passthrough
- [ ] **Milestone 3:** Optional password-modal hints + `en-US` / `de-DE` / `hu-HU` strings
- [ ] **Milestone 4:** Changelog, manual tests (fingerprint + fallback), multiple terminals

## Goal

Add support for users who authenticate via fingerprint (or other PAM-based interactive methods) instead of typing a password into Pacsea's password modal — covering **sudo**, **doas**, and **sudoless** (no privilege escalation) scenarios.

This plan extends current auth behavior without breaking existing password and passwordless flows for any privilege tool.

## Validation Outcome

Status: **partially valid, requires architectural adjustments and doas/sudoless coverage**.

Validated against current code paths:

- `src/logic/privilege.rs` — central privilege abstraction (`PrivilegeTool`, `PrivilegeMode`, `PrivilegeCapabilities`, command builders, password validation, PTY probes)
- `src/logic/password.rs` — auth decision API (`should_use_passwordless_sudo`, `check_passwordless_sudo_available`)
- `src/install/executor.rs` — command construction for executor (`build_install_command_for_executor`, etc.)
- `src/app/runtime/workers/executor.rs` — PTY execution, PTY password injection for doas, `SUDO_ASKPASS` for custom commands
- `src/events/preflight/keys/command_keys.rs` — preflight proceed handler
- `src/events/modals/handlers.rs` — modal handlers (batch update, reinstall, downgrade)
- `src/events/modals/optional_deps.rs` — optional deps install handler
- `src/install/direct.rs` — integrated install entry points
- `src/args/update.rs` — CLI update flow
- `src/app/terminal.rs` — terminal lifecycle helpers

Critical corrections applied in this revision:

- **doas coverage added throughout**: The codebase already has full `PrivilegeTool` (Sudo/Doas), `PrivilegeMode` (Auto/Sudo/Doas), and `PrivilegeCapabilities` abstractions in `src/logic/privilege.rs`. The plan must be tool-agnostic.
- **Setting renamed**: `sudo_auth_mode` → `auth_mode` (tool-agnostic, alongside existing `privilege_tool`).
- **doas PAM reality**: OpenDoas on Arch Linux ships **with PAM** (`/etc/pam.d/doas`). Fingerprint works for doas if `pam_fprintd.so` is in its PAM stack. The plan must not assume interactive auth is sudo-only.
- **doas PTY injection must be disabled in interactive mode**: The executor worker already injects passwords via PTY for doas (since doas has no `-S`). In interactive mode this injection must be suppressed so the user can interact directly.
- **`sudo -S` + fingerprint stalling**: When `pam_fprintd.so` is in the sudo PAM stack and `sudo -S` is used, fprintd may block waiting for a scan before stdin password is read. Documented as known limitation.
- CLI interactive mode is **not** currently possible just by skipping password prompt, because update execution uses `stdin(Stdio::null())`.
- TUI executor path is PTY-output-only and does not support live user input passthrough; interactive auth must not rely on current executor flow.
- Alternate-screen transitions should use existing terminal lifecycle helpers (`restore_terminal` / `setup_terminal`) instead of raw one-off screen commands.

## Problem Summary

Current logic primarily treats "no prompt needed" as "passwordless available" via:

- `{tool} -n true` (both `sudo -n true` and `doas -n true`)

This excludes a common setup:

- Privilege tool still requires authentication
- PAM allows fingerprint interaction (for both `sudo` and `doas` via their respective `/etc/pam.d/` service files)
- Pacsea still asks for a typed password first

As a result, users with fingerprint-capable PAM cannot use that path directly from Pacsea unless they manually work around the modal flow.

## Current State (Codebase)

### Privilege abstraction layer (`src/logic/privilege.rs`)

The codebase has a mature privilege abstraction:

- **`PrivilegeTool`** enum: `Sudo | Doas` — resolved at runtime via `active_tool()`
- **`PrivilegeMode`** enum: `Auto | Sudo | Doas` — from `privilege_tool` setting
- **`PrivilegeCapabilities`** struct: per-tool feature flags
  - `supports_stdin_password` — true for sudo, false for doas
  - `supports_credential_refresh` — true for sudo, false for doas
  - `supports_credential_invalidation` — true for sudo, false for doas
  - `supports_askpass` — true for sudo, false for doas
- **Command builders**: `build_privilege_command`, `build_password_pipe` (returns `None` for doas), `build_credential_warmup` (returns `None` for doas), `build_credential_invalidation`
- **Password validation**: sudo uses `sudo -k` + `sudo -S -v`; doas uses native PTY probes (`run_doas_pty_probe`)
- **`contains_password_prompt`**: detects password prompts in PTY output for doas password injection

### Supported patterns (from module docs)

| Pattern | sudo | doas |
|---|---|---|
| Non-interactive check | `sudo -n true` | `doas -n true` |
| Direct command execution | `sudo <cmd>` | `doas <cmd>` |
| Passwordless execution | sudoers `NOPASSWD` | `permit nopass` in `/etc/doas.conf` |
| Password via stdin | `sudo -S` reads stdin | **NOT supported** (uses PTY validation path) |
| Credential refresh | `sudo -v` | **NOT supported** |
| Credential invalidation | `sudo -k` | **NOT supported** |
| Askpass env var | `SUDO_ASKPASS` | **NOT supported** |

### Auth decision layer (`src/logic/password.rs`)

- `should_use_passwordless_sudo(settings)` — central decision (already tool-aware via delegation to `privilege.rs`)
- `check_passwordless_sudo_available()` — delegates to `PrivilegeTool::check_passwordless()`
- 7 call sites across the TUI and CLI

### Executor layer

- **TUI**: PTY execution via `portable-pty` in `src/app/runtime/workers/executor.rs`
  - For doas: PTY password injection when `contains_password_prompt()` matches
  - For sudo: password piped via `build_password_pipe` (`printf '%s\n' | sudo -S`)
  - AUR credential warmup: `build_credential_warmup` for sudo only
- **CLI**: `rpassword::prompt_password` then command execution in `src/args/update.rs`
- **Downgrade**: spawned in external terminal via `spawn_shell_commands_in_terminal`

### Settings

- `use_passwordless_sudo` setting (default `false`)
- `privilege_tool` setting (default `auto`, accepts `auto | sudo | doas`)
- 7 direct `should_use_passwordless_sudo(&settings)` call sites:
  - `src/install/direct.rs` — `start_integrated_install` (line 42), `start_integrated_install_all` (line 101)
  - `src/events/preflight/keys/command_keys.rs` — `handle_proceed_install` (line 336)
  - `src/events/modals/handlers.rs` — `handle_confirm_batch_update_modal` (line 277), `handle_confirm_reinstall_modal` (line 419)
  - `src/events/modals/optional_deps.rs` — `handle_optional_deps_enter` (line 240)
  - `src/args/update.rs` — `prompt_and_validate_password` (line 605)

### Key files involved

- `src/logic/privilege.rs` — privilege abstraction (`PrivilegeTool`, `PrivilegeMode`, `PrivilegeCapabilities`, all command builders, `validate_password`, PTY probes, `active_tool()`, `contains_password_prompt`)
- `src/logic/password.rs` — auth decision API (`should_use_passwordless_sudo`, `check_passwordless_sudo_available`)
- `src/install/executor.rs` — builds install/remove/update/downgrade commands for executor
- `src/install/command.rs` — additional command construction helpers
- `src/install/direct.rs` — direct install/remove entry points (2 call sites)
- `src/install/remove.rs` — remove command construction
- `src/install/batch.rs` — batch operation construction
- `src/events/preflight/keys/command_keys.rs` — preflight proceed handler (1 call site)
- `src/events/modals/handlers.rs` — modal handlers (2 call sites)
- `src/events/modals/optional_deps.rs` — optional deps install handler (1 call site)
- `src/args/update.rs` — CLI update flow (1 call site)
- `src/app/runtime/workers/executor.rs` — PTY execution, doas PTY injection, `SUDO_ASKPASS`
- `src/app/terminal.rs` — terminal lifecycle (`restore_terminal`, `setup_terminal`)
- `src/theme/types.rs` — `Settings` struct definition
- `src/theme/settings/parse_settings.rs` — settings file parser
- `src/theme/config/skeletons.rs` — `SETTINGS_SKELETON_CONTENT` constant
- `src/theme/config/settings_ensure.rs` — appends missing settings keys on upgrade
- `src/theme/config/settings_save.rs` — persists setting changes to disk
- `config/settings.conf` — shipped default settings file
- `config/locales/en-US.yml`, `config/locales/de-DE.yml`, `config/locales/hu-HU.yml` — translation files

## Product Behavior Proposal

### High-level behavior

Keep existing behavior as default, and add a new mode that lets the active privilege tool (sudo or doas) handle interactive auth directly (fingerprint, password fallback, etc.).

### New auth mode concept

Introduce configurable auth strategy (tool-agnostic):

- `prompt` (default): current behavior, Pacsea password modal/prompt
- `passwordless_only`: current `{tool} -n true` logic
- `interactive`: skip Pacsea password entry and run the privilege tool interactively

In `interactive` mode:

- Pacsea does not require typed password input first
- The privilege tool's auth prompt is handled by the system PAM stack (for both sudo and doas when built with PAM)
- Fingerprint works if configured in the tool's PAM service file (`/etc/pam.d/sudo` or `/etc/pam.d/doas`)
- Password fallback still works if fingerprint is unavailable or fails

### Tool-specific interactive behavior

**sudo (interactive mode):**
- No `-S` password piping — run `sudo <cmd>` directly
- No `build_password_pipe` — use `build_privilege_command` only
- No credential warmup (`sudo -S -v`) — rely on sudo's own credential cache after first interactive auth
- PAM handles fingerprint via `/etc/pam.d/sudo` → `pam_fprintd.so`

**doas (interactive mode):**
- No PTY password injection — run `doas <cmd>` directly
- Disable the `contains_password_prompt` → PTY write path in the executor worker
- PAM handles fingerprint via `/etc/pam.d/doas` → `pam_fprintd.so` (OpenDoas on Arch ships with PAM)
- Note: doas without PAM (`PAM=no` builds) falls back to shadow auth (password only, no fingerprint)

**sudoless / no privilege escalation:**
- AUR operations where the helper (paru/yay) manages its own sudo internally: in `interactive` mode, skip Pacsea's credential warmup (`build_credential_warmup`) and let the AUR helper trigger its own interactive auth
- Operations that do not require privilege escalation at all (search, info, listing): unaffected by auth mode
- When running as root: auth mode has no effect, privilege escalation is skipped entirely (existing behavior)

### TUI vs CLI architecture divergence

The `interactive` mode has fundamentally different implementation paths for TUI and CLI:

**CLI (`src/args/update.rs`)**: Mostly straightforward, but requires one critical fix. In interactive mode, skip `rpassword::prompt_password` and run the privilege tool without password piping (no `sudo -S`, no doas PTY injection), **and** allow interactive stdin by not forcing `stdin(Stdio::null())` in the update execution path.

**TUI (ratatui)**: Complex. Pacsea's TUI owns terminal state (raw mode + alternate screen), and current PTY executor is designed for streamed output, not user-driven auth prompts. Implementation strategy:

- **Primary approach**: Temporarily restore normal terminal state using existing helpers (`src/app/terminal.rs`: `restore_terminal()`), run the privilege tool interactively, then re-enter TUI via `setup_terminal()`.
- **Fallback approach**: Spawn an external terminal window (reuse existing `spawn_shell_commands_in_terminal` infrastructure, which is already used by the downgrade flow).

Important: `interactive` mode for TUI should **not** depend on the current PTY executor for auth interaction unless input passthrough is explicitly implemented.

## Configuration Design

### New setting

Add:

- `auth_mode = prompt`

Allowed values:

- `prompt`
- `passwordless_only`
- `interactive`

This setting is **tool-agnostic** — it works the same regardless of whether `privilege_tool` is `sudo`, `doas`, or `auto`.

Parser aliases (for discoverability): `auth_mode`, `sudo_auth_mode`, `authentication_mode`.

### Backward compatibility

Keep supporting:

- `use_passwordless_sudo = true/false`

Mapping:

- `true` → `passwordless_only`
- `false` → `prompt`

If both `auth_mode` and `use_passwordless_sudo` exist, prefer `auth_mode` and log a deprecation warning for the legacy key.

## Detection Strategy for Fingerprint

Fingerprint detection should be best-effort and informational, not a hard gate.

### Checks (tool-aware)

1. Enrolled fingerprints:
   - `fprintd-list $USER` (if `fprintd-list` is on `$PATH`)
2. PAM wiring for the active tool:
   - sudo: search for `pam_fprintd.so` in `/etc/pam.d/sudo` and any included files
   - doas: search for `pam_fprintd.so` in `/etc/pam.d/doas`
3. fprintd service running:
   - `systemctl is-active fprintd.service` (optional, may be socket-activated)

### Usage

- Optional UI hint text/icons only (non-blocking, non-authoritative)
- Do not use detection to block interactive mode, because PAM layouts vary and can produce false negatives
- Detection must check the **active** tool's PAM file, not just sudo's

## Architecture and Implementation Plan

### Phase 1: Core auth strategy infrastructure

#### 1) Settings and parsing

Update:

- `src/theme/types.rs` — add `auth_mode` field to `Settings` struct
- `src/theme/settings/parse_settings.rs` — add parser with validation, aliases, and fallback
- `src/theme/config/skeletons.rs` — add `auth_mode` to `SETTINGS_SKELETON_CONTENT`
- `src/theme/config/settings_ensure.rs` — ensure existing user configs get the new key appended on upgrade
- `config/settings.conf` — add new setting with documentation comments

Tasks:

- add `AuthMode` enum field to `Settings` (default: `Prompt`)
- add parser with validation and fallback to `Prompt` on invalid values
- register aliases: `auth_mode`, `sudo_auth_mode`, `authentication_mode`
- keep legacy alias parsing for `use_passwordless_sudo` / `passwordless_sudo` / `allow_passwordless_sudo`
- document behavior and migration notes in settings comments (mention that it works with both sudo and doas)
- add deprecation log warning when legacy key `use_passwordless_sudo` is used alongside `auth_mode`

#### 2) Central auth decision API

Update:

- `src/logic/password.rs`
- `src/logic/privilege.rs` — extend `PrivilegeCapabilities`

Tasks:

- add strategy enum: `AuthMode { Prompt, PasswordlessOnly, Interactive }`
- add a resolver function `resolve_auth_mode(settings) -> AuthMode` that:
  - prefers `auth_mode` if set
  - falls back to mapping `use_passwordless_sudo` for backward compatibility
  - is tool-agnostic (works identically for sudo and doas)
- preserve `check_passwordless_sudo_available()` (still needed by `PasswordlessOnly`)
- add `should_skip_password_modal(settings) -> bool` convenience function
- stop treating `{tool} -n true` as the only "skip password modal" condition
- extend `PrivilegeCapabilities` with:
  - `supports_interactive_pam: bool` — true for both sudo and doas-with-PAM on Linux

Expected decision behavior:

- `Prompt` → always ask in Pacsea flow (current default)
- `PasswordlessOnly` → skip only if `{tool} -n true` succeeds (both sudo and doas support this)
- `Interactive` → skip Pacsea password capture and proceed with tool's interactive auth

#### 3) Command construction and execution-mode split

Update:

- `src/logic/privilege.rs` — add `build_interactive_command(tool, command)` helper (same as `build_privilege_command` but documented as "no password piping, no warmup")
- `src/install/executor.rs` — command builders must accept auth mode
- `src/app/runtime/workers/executor.rs` — disable PTY password injection and credential warmup for interactive mode

Tasks:

**For sudo (interactive mode):**
- `build_password_pipe` is never called — use `build_privilege_command` instead
- `build_credential_warmup` is never called — sudo handles its own cache after first interactive auth
- No `SUDO_ASKPASS` — interactive mode relies on direct terminal interaction

**For doas (interactive mode):**
- PTY password injection must be suppressed: the `pty_password` variable in executor workers must be `None` when `auth_mode == Interactive`, even though `!supports_stdin_password` would normally trigger it
- Run plain `doas <cmd>` — identical to the existing `build_privilege_command` output

**For sudoless / AUR:**
- When the operation is AUR-only, skip `build_credential_warmup` in interactive mode (the AUR helper triggers its own auth)
- The existing "has_aur && password.is_some()" guard in the executor worker needs a third condition: `auth_mode != Interactive`

**Preserve unchanged:**
- `prompt` and `passwordless_only` semantics unchanged for both sudo and doas
- All existing doas PTY injection behavior stays for `prompt` mode
- All existing sudo `-S` piping stays for `prompt` mode

### Phase 2: Integrate strategy across operation entry points

Replace all 7 direct `should_use_passwordless_sudo(&settings)` branches:

- `src/install/direct.rs` — `start_integrated_install` (line 42), `start_integrated_install_all` (line 101)
- `src/events/preflight/keys/command_keys.rs` — `handle_proceed_install` (line 336)
- `src/events/modals/handlers.rs` — `handle_confirm_batch_update_modal` (line 277), `handle_confirm_reinstall_modal` (line 419)
- `src/events/modals/optional_deps.rs` — `handle_optional_deps_enter` (line 240)
- `src/args/update.rs` — `prompt_and_validate_password` (line 605)

Tasks:

- switch branch conditions to strategy-aware decision using `resolve_auth_mode`
- the new decision must be a 3-way branch: `Prompt` → show modal, `PasswordlessOnly` → check `{tool} -n true`, `Interactive` → skip modal and proceed
- for `Interactive` mode in the TUI: implement terminal handoff flow using `restore_terminal()` / `setup_terminal()`
- preserve existing safety rules for remove and downgrade operations (see below)
- ensure interactive paths avoid pre-captured password and rely on direct tool/PAM interaction
- pass `AuthMode` (or a derived flag) to executor requests so the worker can suppress PTY injection

#### Operation-specific behavior for `Interactive` mode

All operations below apply identically regardless of whether the privilege tool is sudo or doas:

- **Install (official)**: skip Pacsea password modal, run privilege tool interactively
- **Install (AUR)**: skip Pacsea password modal and credential warmup; the AUR helper (paru/yay) triggers its own interactive auth
- **Update**: skip password prompt (CLI) or alternate-screen-exit flow (TUI)
- **Remove**: keep safety confirmation barrier, but do not require Pacsea password capture in `interactive` mode. Use the same terminal handoff strategy to allow PAM-driven auth.
- **Downgrade**: already spawns an external terminal; `Interactive` mode should skip the Pacsea password prompt and let the external terminal handle auth
- **FileSync**: follows install behavior (skip password modal in interactive mode)
- **Custom commands**: skip `SUDO_ASKPASS` setup when interactive; let the terminal handle auth directly

### Phase 3: Optional fingerprint-aware UX improvements

#### 4) Password modal hinting

Update:

- `src/ui/modals/password.rs`
- `config/locales/en-US.yml`, `config/locales/de-DE.yml`, `config/locales/hu-HU.yml`

Tasks:

- show concise hint in auth prompt:
  - "Press Enter to use system authentication (e.g., fingerprint)"
- optionally show fingerprint hint/icon if detection indicates likely availability
- hint text must be tool-agnostic (do not say "sudo" — say "system authentication")
- add translation keys for all three locales
- `hu-HU`: use English placeholder with `TODO: translate to hungarian` per localization rules

### Phase 4: CLI update parity

Update:

- `src/args/update.rs`

Tasks:

- in `Interactive` mode, skip `rpassword::prompt_password` call entirely
- run update commands without password piping:
  - sudo: `sudo pacman -Syu --noconfirm` (no `sudo -S`)
  - doas: `doas pacman -Syu --noconfirm` (no PTY password injection)
- update CLI execution function so interactive mode can read from terminal stdin (do not force `stdin(Stdio::null())` for that path)
- allow the privilege tool's PAM stack to manage interactive auth directly in the user's terminal
- this is the simplest path since CLI already runs in a normal terminal

## Detailed Behavior Matrix

### Tool × Mode matrix (Install / Update operations)

| # | Tool | Auth Mode | Fingerprint PAM? | Behavior |
|---|------|-----------|-------------------|----------|
| 1 | sudo | `prompt` | no | Current password modal behavior |
| 2 | sudo | `prompt` | yes | Password modal first (user must change mode to use fingerprint). **Known issue**: `sudo -S` may stall if `pam_fprintd` runs before `pam_unix` in the PAM stack. |
| 3 | sudo | `passwordless_only` | any | Skip modal if `sudo -n true` succeeds; fallback to prompt |
| 4 | sudo | `interactive` | yes | TUI: exit alternate screen → sudo/PAM fingerprint → re-enter. CLI: direct. |
| 5 | sudo | `interactive` | no | TUI: exit alternate screen → sudo terminal password → re-enter. CLI: direct. |
| 6 | doas | `prompt` | no | Current behavior: Pacsea modal captures password, PTY injection to doas |
| 7 | doas | `prompt` | yes | Pacsea modal captures password, PTY injection. **Known issue**: doas reads from `/dev/tty` and `pam_fprintd` may activate first if in PAM stack. |
| 8 | doas | `passwordless_only` | any | Skip modal if `doas -n true` succeeds; fallback to prompt |
| 9 | doas | `interactive` | yes | TUI: exit alternate screen → doas/PAM fingerprint → re-enter. CLI: direct. |
| 10 | doas | `interactive` | no | TUI: exit alternate screen → doas terminal password → re-enter. CLI: direct. |
| 11 | none | any | any | No privilege escalation needed (AUR helper self-manages, or running as root) — auth mode has no effect |

### Remove operations (safety-critical)

| # | Tool | Auth Mode | Behavior |
|---|------|-----------|----------|
| 12 | any | `prompt` | Safety confirmation + password modal (current behavior) |
| 13 | any | `passwordless_only` | Safety confirmation, skip password if `{tool} -n true` succeeds |
| 14 | any | `interactive` | Safety confirmation remains, skip Pacsea password capture, terminal handoff for interactive auth |

### Downgrade operations

| # | Tool | Auth Mode | Behavior |
|---|------|-----------|----------|
| 15 | any | `prompt`/`passwordless_only` | Password prompt shown, then spawns external terminal |
| 16 | any | `interactive` | Skip password prompt, spawn external terminal (tool handles auth in spawned terminal) |

### AUR operations

| # | Tool | Auth Mode | Behavior |
|---|------|-----------|----------|
| 17 | sudo | `prompt` | Credential warmup via `sudo -S -v`, then AUR helper runs |
| 18 | sudo | `interactive` | No credential warmup — AUR helper triggers its own interactive sudo |
| 19 | doas | `prompt` | No credential warmup available (doas limitation); AUR helper handles auth |
| 20 | doas | `interactive` | Same as doas `prompt` for AUR — helper manages its own doas auth |
| 21 | none | any | AUR helper self-manages completely |

### FileSync / Custom Commands

| # | Behavior |
|---|----------|
| 22 | FileSync follows install behavior for the active auth mode and tool |
| 23 | Custom commands: `interactive` mode skips `SUDO_ASKPASS` setup; direct terminal auth |

## Known Limitation: `sudo -S` + fingerprint stalling

When `pam_fprintd.so` appears in the sudo PAM stack (`/etc/pam.d/sudo`) **before** `pam_unix.so` with `sufficient` control:

- **`sudo -S`** (used by Pacsea's `prompt` mode) still runs the same PAM stack
- `pam_fprintd` may **block** waiting for a fingerprint scan (up to its timeout, default 30s) **before** the password from stdin is read by `pam_unix`
- This causes **unexplained delays** for users who have fingerprint configured system-wide but use Pacsea's `prompt` mode

**Mitigations:**
- Document this in settings comments for `auth_mode`
- Recommend `interactive` mode for users with fingerprint authentication configured
- This is a PAM ordering issue outside Pacsea's control; splitting PAM stacks or reordering modules is the user's responsibility
- Reference: [sudo issue #112](https://github.com/sudo-project/sudo/issues/112)

## Known Limitation: doas without PAM

OpenDoas can be built with `PAM=no` (uses shadow/libcrypt authentication instead). In this configuration:

- Fingerprint authentication is **not available** through doas
- `interactive` mode still works but falls back to doas's own terminal password prompt
- Pacsea cannot detect whether doas was built with or without PAM; behavior is transparent

On Arch Linux, the `opendoas` package ships with PAM enabled by default.

## Testing Plan

### Unit tests

- parse/validate `auth_mode` with all aliases (`auth_mode`, `sudo_auth_mode`, `authentication_mode`)
- legacy key mapping behavior (`use_passwordless_sudo` → `AuthMode`)
- conflict resolution when both `auth_mode` and `use_passwordless_sudo` are set
- strategy resolution logic for all three modes
- `resolve_auth_mode` returns correct mode for sudo and doas
- fingerprint detection helper behavior (if added) as non-gating
- `PrivilegeCapabilities` extension tests

### Integration tests

Extend existing `tests/passwordless_sudo/*` patterns with strategy-based coverage:

- install/update in `interactive` mode should avoid Pacsea password prompt (both sudo and doas)
- remove/downgrade/file-sync paths follow intended strategy rules
- `passwordless_only` behavior unchanged from current implementation (both sudo and doas)
- legacy `use_passwordless_sudo` compatibility preserved
- executor worker does not inject PTY password when `auth_mode == Interactive` and tool is doas
- executor worker does not call `build_password_pipe` when `auth_mode == Interactive` and tool is sudo
- AUR credential warmup skipped in `interactive` mode
- custom commands skip `SUDO_ASKPASS` in `interactive` mode

### Manual tests

**On machine with fingerprint reader (sudo):**
- verify `interactive` mode allows fingerprint prompt via sudo/PAM
- verify fallback to password if fingerprint fails or times out

**On machine with fingerprint reader (doas):**
- verify `interactive` mode allows fingerprint prompt via doas/PAM (requires `/etc/pam.d/doas` with `pam_fprintd.so`)
- verify fallback to password if fingerprint fails

**On machine without fingerprint reader:**
- verify `interactive` mode still works via standard terminal password interaction (both sudo and doas)

**CLI:**
- run update flow in each mode (`prompt`, `passwordless_only`, `interactive`) with both `privilege_tool = sudo` and `privilege_tool = doas`

**Cross-tool:**
- verify `privilege_tool = auto` correctly resolves and applies the auth mode to whichever tool is selected

**Terminal emulators:**
- test TUI alternate screen exit/re-enter across: alacritty, kitty, gnome-terminal, konsole, wezterm

## Risks and Mitigations

### Risk: TUI alternate screen exit/re-enter disrupts terminal state

- Mitigation: keep default as `prompt`; gate behind explicit config; use centralized `restore_terminal()`/`setup_terminal()` path; restore terminal state on panic/error paths
- Mitigation: test across multiple terminal emulators (alacritty, kitty, gnome-terminal, konsole, wezterm)

### Risk: terminal/PTY compatibility differences

- Mitigation: keep default as `prompt`; gate new behavior behind explicit config

### Risk: PAM/fprintd D-Bus session access from PTY

- Mitigation: ensure environment variables (`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`) are preserved when spawning the privilege tool
- For sudo: may need `--preserve-env=DBUS_SESSION_BUS_ADDRESS,XDG_RUNTIME_DIR` or rely on sudoers `env_keep` (many distros already keep these)
- For doas: `setenv { DBUS_SESSION_BUS_ADDRESS XDG_RUNTIME_DIR }` in `doas.conf`, or `keepenv` rule (user responsibility — document in settings comments)

### Risk: confusion between "passwordless" and "interactive fingerprint"

- Mitigation: explicit setting names and clear settings comments; `auth_mode` is distinct from `use_passwordless_sudo`

### Risk: fingerprint detection false positives/negatives

- Mitigation: detection is informational only; never gating execution

### Risk: executor command construction produces insecure commands in interactive mode

- Mitigation: `Interactive` mode must never pass user-controlled input to shell commands without escaping; audit all `build_*` paths in `privilege.rs` and `executor.rs`

### Risk: CLI interactive mode silently fails due to null stdin

- Mitigation: explicitly implement stdin policy per auth mode in CLI update path and add regression tests

### Risk: doas PTY injection fires in interactive mode

- Mitigation: executor worker must check `auth_mode` before deciding to inject password via PTY; add `auth_mode != Interactive` guard to the `pty_password` assignment

### Risk: `sudo -S` stalling when fingerprint is system-configured

- Mitigation: document as known limitation; recommend `interactive` mode; this is a PAM ordering issue outside Pacsea's control

### Risk: AUR helper re-prompts for auth in interactive mode

- Mitigation: this is expected and correct behavior — the AUR helper manages its own auth. Document that the user may see a second fingerprint/password prompt for AUR operations.

## Incremental Delivery Plan

### Milestone 1 (MVP — CLI only)

- Add `AuthMode` enum and `auth_mode` setting (types, parser, skeleton, ensure)
- Implement strategy resolver in `src/logic/password.rs`
- Wire CLI update path (`src/args/update.rs`) for `Interactive` mode, including stdin handling fix
- Ensure it works for both `privilege_tool = sudo` and `privilege_tool = doas`
- Add legacy key backward compatibility and deprecation warning
- Unit tests for parsing, resolver, legacy mapping
- Keep UX minimal (no fingerprint icon yet)

### Milestone 2 (TUI integration)

- Implement TUI interactive auth handoff using terminal lifecycle helpers (`src/app/terminal.rs`)
- Suppress PTY password injection (doas) and password piping (sudo) when `auth_mode == Interactive`
- Route interactive-mode operations away from PTY-only auth paths where required
- Wire all 7 call sites to strategy-aware decision (including the existing command_keys.rs site)
- Add `auth_mode` to `ExecutorRequest` so the worker can make tool-aware decisions
- Integration tests extending `tests/passwordless_sudo/`

### Milestone 3 (UX polish)

- Add optional fingerprint/PAM hint in modal and translations (all 3 locales)
- Hint text is tool-agnostic ("system authentication" not "sudo")
- Detection checks the active tool's PAM file

### Milestone 4 (compat + docs polish)

- Finalize legacy key migration behavior
- Add release notes/changelog entry
- Manual testing across terminal emulators with both sudo and doas
- Document known `sudo -S` + fingerprint stalling limitation in settings.conf comments

## Success Criteria

1. Users with PAM fingerprint auth can complete package operations without typing password into Pacsea modal when `auth_mode=interactive` — with **both sudo and doas**.
2. Existing default behavior remains unchanged (both tools).
3. Existing passwordless users see no regression (both tools).
4. CLI and TUI auth strategy behavior is consistent across both privilege tools.
5. Test coverage covers all auth strategy branches, fallback paths, and tool combinations.
6. AUR operations work correctly in all modes (credential warmup skipped for interactive, AUR helper self-manages auth).
7. The setting is tool-agnostic — switching `privilege_tool` does not require changing `auth_mode`.
