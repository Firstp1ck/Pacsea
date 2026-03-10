# Fingerprint-Friendly Sudo Authentication Plan

## Goal

Add support for users who authenticate sudo via fingerprint (or other PAM-based interactive methods) instead of typing a password in Pacsea's password modal.

This plan extends current auth behavior without breaking existing password and passwordless sudo flows.

## Validation Outcome

Status: **partially valid, requires architectural adjustments**.

Validated against current code paths:

- `src/args/update.rs`
- `src/logic/password.rs`
- `src/install/executor.rs`
- `src/app/runtime/workers/executor.rs`
- `src/events/preflight/keys/action_keys.rs`
- `src/events/preflight/keys/command_keys.rs`
- `src/events/modals/handlers.rs`
- `src/events/modals/optional_deps.rs`
- `src/app/terminal.rs`

Critical corrections applied in this revision:

- CLI interactive mode is **not** currently possible just by skipping password prompt, because update execution uses `stdin(Stdio::null())`.
- TUI executor path is PTY-output-only and does not support live user input passthrough; interactive sudo must not rely on current executor flow.
- Alternate-screen transitions should use existing terminal lifecycle helpers (`restore_terminal` / `setup_terminal`) instead of raw one-off screen commands.

## Problem Summary

Current logic primarily treats "no prompt needed" as "passwordless sudo available" via:

- `sudo -n true`

This excludes a common setup:

- sudo still requires authentication
- PAM allows fingerprint interaction
- Pacsea still asks for a typed password first

As a result, users with fingerprint-capable sudo cannot use that path directly from Pacsea unless they manually workaround the modal flow.

## Current State (Codebase)

Pacsea already has:

- `use_passwordless_sudo` setting (default `false`)
- auth decision in `src/logic/password.rs`
- many integration points that call `should_use_passwordless_sudo(&settings)` (7 call sites)
- strong integration test coverage in `tests/passwordless_sudo/*`

Key files involved:

- `src/logic/password.rs` — central auth decision API (`should_use_passwordless_sudo`, `check_passwordless_sudo_available`, `validate_sudo_password`)
- `src/install/executor.rs` — builds all install/remove/update commands using `printf '%s\n' password | sudo -S command` pattern
- `src/install/command.rs` — additional sudo password piping for command construction
- `src/install/direct.rs` — direct install/remove entry points (2 call sites: `start_integrated_install`, `start_integrated_install_all`)
- `src/events/preflight/keys/command_keys.rs` — preflight proceed handler (1 call site: `handle_proceed_install`)
- `src/events/modals/handlers.rs` — modal handlers (2 call sites: `handle_confirm_batch_update_modal`, `handle_confirm_reinstall_modal`)
- `src/events/modals/optional_deps.rs` — optional deps install handler (1 call site: `handle_optional_deps_enter`)
- `src/args/update.rs` — CLI update flow (1 call site: `prompt_and_validate_password`)
- `src/theme/types.rs` — `Settings` struct definition
- `src/theme/settings/parse_settings.rs` — settings file parser
- `src/theme/config/skeletons.rs` — `SETTINGS_SKELETON_CONTENT` constant (template for new installs)
- `src/theme/config/settings_ensure.rs` — appends missing settings keys to existing user configs on upgrade
- `src/theme/config/settings_save.rs` — persists setting changes to disk
- `config/settings.conf` — shipped default settings file
- `config/locales/en-US.yml`, `config/locales/de-DE.yml`, `config/locales/hu-HU.yml` — translation files

## Product Behavior Proposal

### High-level behavior

Keep existing behavior as default, and add a new mode that lets sudo/PAM handle interactive auth directly (fingerprint, password fallback, etc.).

### New auth mode concept

Introduce configurable sudo auth strategy:

- `prompt` (default): current behavior, Pacsea password modal/prompt
- `passwordless_only`: current `sudo -n true` logic
- `interactive`: skip Pacsea password entry and run sudo interactively in PTY/terminal

In `interactive` mode:

- Pacsea does not require typed password input first
- sudo prompt is handled by system PAM stack
- fingerprint works if configured
- password fallback still works if fingerprint is unavailable

### TUI vs CLI architecture divergence

The `interactive` mode has fundamentally different implementation paths for TUI and CLI:

**CLI (`src/args/update.rs`)**: Mostly straightforward, but requires one critical fix. In interactive mode, skip `rpassword::prompt_password` and run `sudo` without `-S` password piping, **and** allow interactive stdin by not forcing `stdin(Stdio::null())` in the update execution path.

**TUI (ratatui)**: Complex. Pacsea's TUI owns terminal state (raw mode + alternate screen), and current PTY executor is designed for streamed output, not user-driven sudo prompts. Implementation strategy:

- **Primary approach**: Temporarily restore normal terminal state using existing helpers (`src/app/terminal.rs`: `restore_terminal()`), run sudo interactively, then re-enter TUI via `setup_terminal()`.
- **Fallback approach**: Spawn an external terminal window (reuse existing `spawn_shell_commands_in_terminal` infrastructure, which is already used by the downgrade flow).

Important: `interactive` mode for TUI should **not** depend on the current PTY executor for auth interaction unless input passthrough is explicitly implemented.

## Configuration Design

### New setting

Add:

- `sudo_auth_mode = prompt`

Allowed values:

- `prompt`
- `passwordless_only`
- `interactive`

### Backward compatibility

Keep supporting:

- `use_passwordless_sudo = true/false`

Mapping:

- `true` -> `passwordless_only`
- `false` -> `prompt`

If both exist, prefer `sudo_auth_mode` and log a deprecation warning for legacy key usage.

## Detection Strategy for Fingerprint

Fingerprint detection should be best-effort and informational, not a hard gate.

Possible checks:

1. Enrolled fingerprints:
   - `fprintd-list $USER`
2. PAM wiring appears present:
   - search for `pam_fprintd.so` in likely sudo PAM files

Use detection for:

- optional UI hint text/icons only (non-blocking, non-authoritative)

Do not use detection to block interactive mode, because PAM layouts vary and can produce false negatives.

## Architecture and Implementation Plan

### Phase 1: Core auth strategy infrastructure

#### 1) Settings and parsing

Update:

- `src/theme/types.rs` — add `sudo_auth_mode` field to `Settings` struct
- `src/theme/settings/parse_settings.rs` — add parser in `parse_misc_settings` with validation and fallback
- `src/theme/config/skeletons.rs` — add `sudo_auth_mode` to `SETTINGS_SKELETON_CONTENT` constant
- `src/theme/config/settings_ensure.rs` — ensure existing user configs get the new key appended on upgrade
- `config/settings.conf` — add new setting with documentation comments

Tasks:

- add `SudoAuthMode` enum field to `Settings` (default: `Prompt`)
- add parser with validation and fallback to `Prompt` on invalid values
- keep legacy alias parsing for `use_passwordless_sudo` / `passwordless_sudo` / `allow_passwordless_sudo`
- document behavior and migration notes in settings comments
- add deprecation log warning when legacy key `use_passwordless_sudo` is used alongside `sudo_auth_mode`

#### 2) Central auth decision API

Update:

- `src/logic/password.rs`

Tasks:

- add strategy enum (e.g. `SudoAuthMode { Prompt, PasswordlessOnly, Interactive }`)
- add a resolver function `resolve_sudo_auth_mode(settings) -> SudoAuthMode` that:
  - prefers `sudo_auth_mode` if set
  - falls back to mapping `use_passwordless_sudo` for backward compatibility
- preserve `check_passwordless_sudo_available()` (still needed by `PasswordlessOnly`)
- add `should_skip_password_modal(settings) -> bool` convenience function
- stop treating `sudo -n true` as the only "skip password modal" condition

Expected decision behavior:

- `Prompt` -> always ask in Pacsea flow (current default)
- `PasswordlessOnly` -> skip only if `sudo -n true` succeeds
- `Interactive` -> skip Pacsea password capture and proceed with sudo interactive auth

#### 3) Command construction and execution-mode split

Update:

- `src/install/executor.rs` — `build_install_command_for_executor`, `build_remove_command_for_executor`, update command builder
- `src/install/command.rs` — additional command construction helpers
- `src/app/runtime/workers/executor.rs` — ensure strategy-aware dispatch does not attempt PTY-only interactive auth paths

Tasks:

- keep command builders capable of no-password path (`sudo command`) where needed
- in `Interactive` mode, avoid `-S` password piping
- keep existing `prompt` and `passwordless_only` semantics unchanged
- do not route interactive-auth TUI flows into non-interactive PTY execution without input passthrough

### Phase 2: Integrate strategy across operation entry points

Replace all 7 direct `should_use_passwordless_sudo(&settings)` branches:

- `src/install/direct.rs` — `start_integrated_install` (line 42), `start_integrated_install_all` (line 101)
- `src/events/preflight/keys/command_keys.rs` — `handle_proceed_install` (line 336)
- `src/events/modals/handlers.rs` — `handle_confirm_batch_update_modal` (line 277), `handle_confirm_reinstall_modal` (line 419)
- `src/events/modals/optional_deps.rs` — `handle_optional_deps_enter` (line 240)
- `src/args/update.rs` — `prompt_and_validate_password` (line 603)

Tasks:

- switch branch conditions to strategy-aware decision using `resolve_sudo_auth_mode`
- for `Interactive` mode in the TUI: implement terminal handoff flow using `restore_terminal()` / `setup_terminal()`
- preserve existing safety rules for remove and downgrade operations (see below)
- ensure interactive paths avoid pre-captured password and rely on direct sudo/PAM interaction

#### Operation-specific behavior for `Interactive` mode

- **Install**: skip Pacsea password modal, run sudo interactively
- **Update**: skip password prompt (CLI) or alternate-screen-exit flow (TUI)
- **Remove**: keep safety confirmation barrier, but do not require Pacsea password capture in `interactive` mode. Use the same terminal handoff strategy to allow PAM-driven auth.
- **Downgrade**: already spawns an external terminal; `Interactive` mode should skip the Pacsea password prompt and let the external terminal handle sudo auth
- **FileSync**: follows install behavior (skip password modal in interactive mode)

### Phase 3: Optional fingerprint-aware UX improvements

#### 4) Password modal hinting

Update:

- `src/ui/modals/password.rs`
- `config/locales/en-US.yml`, `config/locales/de-DE.yml`, `config/locales/hu-HU.yml`

Tasks:

- show concise hint in auth prompt:
  - "Press Enter to use system authentication (e.g., fingerprint)"
- optionally show fingerprint hint/icon if detection indicates likely availability
- add translation keys for all three locales

### Phase 4: CLI update parity

Update:

- `src/args/update.rs`

Tasks:

- in `Interactive` mode, skip `rpassword::prompt_password` call entirely
- run update commands with no inline `sudo -S` password piping (use raw `sudo pacman -Syu --noconfirm`)
- update CLI execution function so interactive mode can read from terminal stdin (do not force `stdin(Stdio::null())` for that path)
- allow sudo/PAM to manage interactive auth directly in the user's terminal
- this is the simplest path since CLI already runs in a normal terminal

## Detailed Behavior Matrix

### Install / Update operations

1. `prompt` + no fingerprint setup
   - current password prompt behavior

2. `prompt` + fingerprint setup
   - still password modal first (unless user changes mode)

3. `passwordless_only` + `sudo -n true` success
   - skip password modal/prompt

4. `passwordless_only` + `sudo -n true` failure
   - fallback to prompt flow

5. `interactive` + fingerprint configured in PAM
   - TUI: exit alternate screen, sudo/PAM fingerprint auth, re-enter alternate screen
   - CLI: sudo/PAM fingerprint auth directly in terminal

6. `interactive` + no fingerprint or no PAM support
   - TUI: exit alternate screen, sudo falls back to terminal password prompt, re-enter
   - CLI: sudo falls back to terminal password prompt

### Remove operations (safety-critical)

7. Any mode + remove operation
   - safety barrier remains (confirmation required)
   - in `prompt`/`passwordless_only`, existing password prompt behavior remains
   - in `interactive`, Pacsea skips password capture and hands off to terminal/PAM flow

### Downgrade operations

8. `prompt` or `passwordless_only` + downgrade
   - password prompt shown, then spawns external terminal

9. `interactive` + downgrade
   - skip password prompt, spawn external terminal (sudo handles auth in the spawned terminal)

### FileSync operations

10. Follows same behavior as install operations for the active auth mode

## Testing Plan

### Unit tests

- parse/validate `sudo_auth_mode`
- legacy key mapping behavior
- strategy resolution logic
- fingerprint detection helper behavior (if added) as non-gating

### Integration tests

Extend existing `tests/passwordless_sudo/*` patterns with strategy-based coverage:

- install/update in `interactive` mode should avoid Pacsea password prompt
- remove/downgrade/file-sync paths follow intended strategy rules
- `passwordless_only` behavior unchanged from current implementation
- legacy `use_passwordless_sudo` compatibility preserved

### Manual tests

On machine with fingerprint reader:

- verify `interactive` mode allows fingerprint prompt via sudo/PAM
- verify fallback to password if fingerprint fails

On machine without fingerprint reader:

- verify `interactive` mode still works via standard sudo password interaction

CLI:

- run update flow in each mode (`prompt`, `passwordless_only`, `interactive`)

## Risks and Mitigations

### Risk: TUI alternate screen exit/re-enter disrupts terminal state

- Mitigation: keep default as `prompt`; gate behind explicit config; use centralized `restore_terminal()`/`setup_terminal()` path; restore terminal state on panic/error paths
- Mitigation: test across multiple terminal emulators (alacritty, kitty, gnome-terminal, konsole, wezterm)

### Risk: terminal/PTY compatibility differences

- Mitigation: keep default as `prompt`; gate new behavior behind explicit config

### Risk: PAM/fprintd D-Bus session access from PTY

- Mitigation: ensure environment variables (`DBUS_SESSION_BUS_ADDRESS`, `XDG_RUNTIME_DIR`) are preserved when spawning sudo

### Risk: confusion between "passwordless" and "interactive fingerprint"

- Mitigation: explicit setting names and clear settings comments

### Risk: fingerprint detection false positives/negatives

- Mitigation: detection is informational only; never gating execution

### Risk: executor command construction produces insecure commands in interactive mode

- Mitigation: `Interactive` mode must never pass user-controlled input to shell commands without escaping; audit all `build_*_command_for_executor` paths

### Risk: CLI interactive mode silently fails due to null stdin

- Mitigation: explicitly implement stdin policy per auth mode in CLI update path and add regression tests

## Incremental Delivery Plan

### Milestone 1 (MVP — CLI only)

- Add `SudoAuthMode` enum and `sudo_auth_mode` setting (types, parser, skeleton, ensure)
- Implement strategy resolver in `src/logic/password.rs`
- Wire CLI update path (`src/args/update.rs`) for `Interactive` mode, including stdin handling fix
- Add legacy key backward compatibility and deprecation warning
- Unit tests for parsing, resolver, legacy mapping
- Keep UX minimal (no fingerprint icon yet)

### Milestone 2 (TUI integration)

- Implement TUI interactive auth handoff using terminal lifecycle helpers (`src/app/terminal.rs`)
- Route interactive-mode operations away from PTY-only auth paths where required
- Wire all 6 TUI call sites to strategy-aware decision
- Integration tests extending `tests/passwordless_sudo/`

### Milestone 3 (UX polish)

- Add optional fingerprint/PAM hint in modal and translations (all 3 locales)

### Milestone 4 (compat + docs polish)

- Finalize legacy key migration behavior
- Add release notes/changelog entry
- Manual testing across terminal emulators

## Success Criteria

1. Users with PAM fingerprint auth can complete package operations without typing password into Pacsea modal when `sudo_auth_mode=interactive`.
2. Existing default behavior remains unchanged.
3. Existing passwordless sudo users see no regression.
4. CLI and TUI auth strategy behavior is consistent.
5. Test coverage covers all auth strategy branches and fallback paths.

