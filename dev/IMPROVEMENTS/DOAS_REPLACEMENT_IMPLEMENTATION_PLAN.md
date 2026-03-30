# Doas Replacement Implementation Plan

## Progress todos (2026-03-30)

**Implementation on branch:** feature work is implemented in-tree (`src/logic/privilege.rs`, `privilege_tool` setting, migrated executors/modals/CLI). **Release:** not yet on `origin/main` / not in `CHANGELOG.md` for a stable tag at last review.

- [x] Phases 0–5: abstraction, settings, executor/modal/CLI migration, tests, hardening (see checklist below)
- [ ] Merge `feature/doas-sudo-replacement` → `main` and cut release
- [ ] Add `CHANGELOG.md` entry (`privilege_tool`, auto/explicit doas/sudo, doas limitations vs sudo)
- [ ] Post-merge: grep audit for unintended hardcoded `sudo` in production paths; update docs/wiki only if you maintain them elsewhere

## Goal

Implement support for `doas` as a replacement for `sudo` in privileged package operations while preserving current behavior, security guarantees, and graceful fallbacks.

## Success Criteria

- Pacsea can execute privileged commands through `doas` when configured or auto-detected.
- Existing `sudo` behavior remains fully functional and backward compatible.
- Passwordless and password-prompt flows still work (or degrade clearly when tool limitations apply).
- Dry-run output accurately reflects the selected privilege tool (`sudo` or `doas`).
- Integration tests cover both privilege tool paths and fallback behavior.

## Current State Snapshot

**Implemented (this branch):** `src/logic/privilege.rs` centralizes `sudo`/`doas` resolution, command builders, and capability flags; settings include `privilege_tool = auto|sudo|doas`; call sites use the abstraction instead of hardcoded `sudo` for privileged operations.

**Pre-refactor baseline (historical):** `sudo` was hardcoded across the runtime executor, install/remove/update builders, modals, and tests. That layout motivated the phases below.

## Design Strategy

### 1) Introduce a privilege tool abstraction

Create a single abstraction that defines how privileged commands are built and executed.

Proposed model:

- `PrivilegeTool` enum: `Sudo`, `Doas`
- `PrivilegeMode` selector in settings: `auto | sudo | doas` (default `auto`)
- Resolver behavior:
  - `auto`: prefer `doas` if available, else `sudo`
  - explicit mode: use requested tool or return actionable error if unavailable

### 2) Centralize command construction

Move hardcoded privilege command string building to one module (for example `src/system/privilege.rs` or equivalent existing location).

Responsibilities:

- Build non-interactive availability checks
- Build command prefix for runtime execution
- Handle password-passing strategy per tool
- Expose tool-specific capabilities (for example: supports stdin password pipe)

### 3) Handle tool capability differences explicitly

Do not assume `doas` supports the same flags/flows as `sudo`.

Implementation rule:

- Detect capability once (startup or first privileged operation), cache it, and route behavior accordingly.
- If a requested flow is unsupported for `doas`, fail with a clear, actionable message and fallback policy (based on configured mode).

## Implementation Phases

### Phase 0: Validation spike (required)

Before code refactor, verify supported flags and password handling of the target `doas` package used by Arch users.

Deliverables:

- Confirm supported non-interactive check pattern.
- Confirm password prompt strategy compatibility with current runtime architecture.
- Define minimum supported `doas` variant/version behavior in code comments.

### Phase 1: Config and resolution plumbing

- Add privilege tool selection setting (`auto/sudo/doas`) with default `auto`.
- Keep current passwordless `sudo` setting behavior backward compatible.
- Add a resolver function used everywhere privileged operations start.

### Phase 2: Runtime executor migration

Refactor `src/app/runtime/workers/executor.rs` to use the abstraction:

- Replace direct `"sudo"` string checks.
- Replace direct `sudo -S` composition with tool-specific command building.
- Keep askpass logic tool-aware rather than sudo-only.

### Phase 3: Modal/install/downgrade flow migration

Refactor `src/events/modals/handlers.rs`:

- Replace hardcoded `sudo downgrade ...` and related builders.
- Update password prompt decision logic to use selected privilege tool and capability checks.
- Preserve existing UX behavior where possible.

### Phase 4: Test migration and expansion

Update and add tests:

- Rename/generalize `tests/passwordless_sudo` helpers to privilege-tool-aware helpers.
- Add coverage for:
  - `auto` selection (`doas` present vs absent)
  - forced `doas` mode when unavailable
  - dry-run command rendering with selected tool
  - downgrade/install command generation via abstraction
- Keep deterministic behavior via env-controlled test shims.

### Phase 5: Hardening and rollout

- Verify actionable error messages when neither `doas` nor `sudo` is available.
- Add structured logging for selected tool and fallback reason.
- Ensure no regression in AUR security handling and privileged command constraints.

## File-Level Change Plan

- `config/settings.conf`
  - Add documented setting for privilege tool selection.
- `src/app/runtime/workers/executor.rs`
  - Replace `sudo`-specific execution logic with abstraction calls.
- `src/events/modals/handlers.rs`
  - Replace hardcoded privileged command strings and checks.
- New module for privilege abstraction (path to be finalized after implementation spike).
- `tests/passwordless_sudo/*`
  - Generalize to privilege-tool tests.
- `tests/downgrade/downgrade_integration.rs`
- `tests/install/optional_deps_integration.rs`
  - Update expected command strings and add tool-specific scenarios.

## Risks and Mitigations

- Behavior mismatch between `sudo` and `doas`
  - Mitigation: capability detection + explicit unsupported-flow handling.
- Regression in password prompt flows
  - Mitigation: integration tests for prompt-required and passwordless paths.
- Security regressions in privileged operations
  - Mitigation: keep current safety constraints unchanged; only swap privilege launcher.
- Breaking existing users relying on `sudo`
  - Mitigation: backward-compatible default and explicit mode override.

## Test Plan

- Unit tests:
  - Privilege tool resolver logic
  - Command builder output by mode/capability
  - Unsupported capability branches
- Integration tests:
  - Install/update/downgrade command flows under `sudo` and `doas` modes
  - Auto fallback behavior
  - Dry-run output assertions
- Full quality gate after implementation:
  - `cargo fmt --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo check`
  - `cargo test -- --test-threads=1`

## Rollout Checklist (branch — done)

- [x] Phase 0 capability spike documented in code comments.
- [x] Privilege tool setting added and parsed.
- [x] `executor` and modal handlers migrated.
- [x] Tests generalized and passing.
- [x] Hardcoded `sudo` in production paths removed except intentional messages (minor exceptions noted in implementation notes).
- [x] Dry-run output validated for both tools (tests use active tool binary name).
- [x] Quality gate commands pass.

## Implementation Status (updated 2026-03-29)

**All phases (0–5) complete.**

### Phase 0: Validation spike — DONE
- [x] Confirm supported non-interactive check pattern for `doas` — documented in `src/logic/privilege.rs` module docs
- [x] Confirm password prompt strategy compatibility with runtime architecture — doas cannot pipe via stdin, documented
- [x] Define minimum supported `doas` variant/version behavior in code comments — `OpenDoas` on Arch

### Phase 1: Config and resolution plumbing — DONE
- [x] `PrivilegeTool` enum (`Sudo`, `Doas`) — `src/logic/privilege.rs`
- [x] `PrivilegeMode` selector setting (`auto/sudo/doas`) — `src/logic/privilege.rs`, field in `Settings`
- [x] `config/settings.conf` privilege tool selection key — `privilege_tool = auto`
- [x] Skeleton updated — `src/theme/config/skeletons.rs`
- [x] Settings parsing — `src/theme/settings/parse_settings.rs`
- [x] Settings ensure — `src/theme/config/settings_ensure.rs`
- [x] Resolver function `resolve_privilege_tool()` — implemented with auto/explicit modes
- [x] Convenience resolver `active_tool()` — reads settings + fallback to sudo
- [x] Command builders — `build_privilege_command`, `build_password_pipe`, `build_credential_warmup`, `build_credential_invalidation`, `validate_password`
- [x] `PrivilegeCapabilities` struct — documents tool differences
- [x] 32 unit tests passing — types, resolver, builders, availability
- [x] Backward-compatible `use_passwordless_sudo` preserved — unchanged

### Phase 2: Runtime executor migration — DONE
- [x] `src/app/runtime/workers/executor.rs` — credential warmup uses `build_credential_warmup()`
- [x] `sudo -S` composition replaced with tool-specific builder — `build_password_pipe()`
- [x] Askpass logic made tool-aware — gates on `tool.capabilities().supports_askpass`

### Phase 3: Modal/install/downgrade flow migration — DONE
- [x] `src/events/modals/handlers.rs` — downgrade uses `build_privilege_command` / `build_password_pipe`
- [x] `src/install/executor.rs` — all build_*_command_for_executor use privilege abstraction
- [x] `src/install/command.rs` — `build_install_command` uses privilege abstraction
- [x] `src/install/remove.rs` — `spawn_remove_all` uses privilege abstraction
- [x] `src/install/batch.rs` — `build_batch_install_command` uses privilege abstraction
- [x] `src/events/modals/system_update.rs` — update commands use privilege abstraction
- [x] `src/events/modals/common.rs` — terminal install uses privilege abstraction
- [x] `src/events/install/mod.rs` — downgrade shortcut uses privilege abstraction
- [x] `src/events/distro.rs` — mirror update commands use `{bin}` from active tool
- [x] `src/args/update.rs` — CLI update uses `active_tool()` for commands and credential warmup
- [x] `src/args/install.rs` — CLI install uses `active_tool()` for `Command::new()`
- [x] `src/args/package.rs` — CLI package search uses `active_tool()` for `Command::new()`
- [x] `src/args/remove.rs` — CLI remove uses `active_tool()` for `Command::new()`
- [x] `src/logic/password.rs` — delegates to privilege module for validation and checks
- [x] `src/app/runtime/tick_handler.rs` — file sync uses `build_privilege_command()`
- [x] `src/ui/modals/preflight/tabs/summary.rs` — plan preview uses `build_privilege_command()`
- [x] Password prompt decision logic preserved with test override via `is_integration_test()`
- [x] `sudo pacman -Qi` in `install/utils.rs`, `install/scan/*.rs`, `events/distro.rs` cleaned up (read-only queries don't need root)

### Phase 4: Test migration and expansion — DONE
- [x] 48 privilege module unit tests (up from 32): `is_integration_test`, `active_tool`, printf format, doas fallbacks, passwordless overrides, tool symmetry
- [x] `install/executor.rs` tests made tool-agnostic: assertions use `active_tool().binary_name()` instead of hardcoded `sudo`
- [x] `install/command.rs` tests made tool-agnostic: same approach
- [x] `tests/install/direct_install_integration.rs` — AUR test now handles faillock `Alert` gracefully
- [x] Removed `validate_password_sudo_does_not_panic` test that triggered real faillock entries
- [x] 967 tests passing (up from 950), 0 failures, 11 ignored

### Phase 5: Hardening and rollout — DONE
- [x] Actionable error messages with install commands (`pacman -S sudo` / `pacman -S opendoas`)
- [x] Structured `tracing` logging: mode, doas/sudo availability, selection reason, fallback warnings
- [x] No regression in AUR security handling verified — scan patterns correctly detect `sudo` usage
- [x] Cosmetic: removed unnecessary `sudo`/`{bin}` from `pacman -Qi` queries in `install/utils.rs`, `install/scan/common.rs`, `install/scan/pkg.rs`, `events/distro.rs`
- [x] `events/distro.rs` test assertions made tool-agnostic

### Rollout Checklist

Same items as **“Rollout Checklist (branch — done)”** earlier in this document; all checked on the feature branch. Outstanding items are under **Progress todos** at the top (merge, changelog, post-merge audit).