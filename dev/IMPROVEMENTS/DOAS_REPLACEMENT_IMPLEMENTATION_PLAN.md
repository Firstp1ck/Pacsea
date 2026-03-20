# Doas Replacement Implementation Plan

## Goal

Implement support for `doas` as a replacement for `sudo` in privileged package operations while preserving current behavior, security guarantees, and graceful fallbacks.

## Success Criteria

- Pacsea can execute privileged commands through `doas` when configured or auto-detected.
- Existing `sudo` behavior remains fully functional and backward compatible.
- Passwordless and password-prompt flows still work (or degrade clearly when tool limitations apply).
- Dry-run output accurately reflects the selected privilege tool (`sudo` or `doas`).
- Integration tests cover both privilege tool paths and fallback behavior.

## Current State Snapshot

`sudo` is hardcoded in multiple areas, especially:

- Runtime command execution and askpass handling in `src/app/runtime/workers/executor.rs`
- Install/downgrade flows and password modal logic in `src/events/modals/handlers.rs`
- Integration tests under `tests/downgrade`, `tests/install`, and `tests/passwordless_sudo`

There is currently no `doas` usage in the repository.

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

## Rollout Checklist

- [ ] Phase 0 capability spike documented in code comments.
- [ ] Privilege tool setting added and parsed.
- [ ] `executor` and modal handlers migrated.
- [ ] Tests generalized and passing.
- [ ] No remaining hardcoded `sudo` strings in production paths (except intentional logs/messages).
- [ ] Dry-run output validated for both tools.
- [ ] Quality gate commands pass.



## Statement against the implementation

We decided not to add `doas` support because it would slightly increase security risk and noticeably increase maintenance work, without giving most users a clear benefit over the existing `sudo` support. [en.wikipedia](https://en.wikipedia.org/wiki/Setuid)

Things that would increase risk and maintenance cost:

- More ways to run as admin: Every extra “run as administrator” tool is another powerful program attackers can try to break. [cbtnuggets](https://www.cbtnuggets.com/blog/technology/system-admin/linux-file-permissions-understanding-setuid-setgid-and-the-sticky-bit)
- Two configs to get right: We’d have to support and document both `sudo` and `doas` setups, which doubles the chance of misconfiguration on user systems. [manual.siduction](https://manual.siduction.org/sys-admin-doas_en.html)
- Extra code paths to test: The package manager would need separate logic for `sudo` and `doas`, meaning more code to write, test, and keep bug‑free. [fluca1978.github](https://fluca1978.github.io/2021/11/08/SUDOvsDOAS.html)
- More support questions: Users would run into issues specific to each tool and platform, increasing support, debugging, and documentation overhead. [manual.siduction](https://manual.siduction.org/sys-admin-doas_en.html)