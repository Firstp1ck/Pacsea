# Implementation Plan: General System Update Commands Per Package Manager

## Goal

Align full system update behavior with each distro/package manager's standard update flow (for example `pacman/yay/paru -Syu`, `apt update && apt upgrade`, etc.) in both:

- TUI System Update modal flow (`src/events/modals/system_update.rs`)
- CLI update flow (`src/args/update.rs`)

This plan focuses on command selection, sequencing, safety, and rollout steps.

---

## Current implementation audit

## 1) TUI System Update (`SystemUpdate` modal)

Current behavior in `src/events/modals/system_update.rs`:

- Official repo update: always builds `pacman -Syu` (or `-Syyu` for force sync), wrapped by configured privilege tool.
- AUR update:
  - If both official + AUR are selected: stores a separate AUR command using `paru -Sua` or `yay -Sua`.
  - If only AUR is selected: runs `paru -Sua` or `yay -Sua`.
- Cache cleaning: pacman + helper cache cleanup commands.
- Mirror updates: distro-sensitive logic exists, but package update command itself is still pacman-centric.

Result: update command generation is strongly Arch-first.

## 2) CLI `--update`

Current behavior in `src/args/update.rs`:

- Runs official update as `pacman -Syu --noconfirm` (via selected privilege tool).
- On success, runs AUR helper update (`paru/yay -Sua --noconfirm`) when available.
- Flow is sequential and robust for Arch/AUR, but command strategy is not distro/PM-generic.

Result: CLI update logic is also Arch-first.

## 3) Planned / roadmap context already in repo

- `dev/IMPROVEMENTS/FEATURE_PRIORITY.md` (Tier 5) references future multi-PM support (`apt`, `dnf`, `Flatpak`) as a major architectural item.
- `dev/IMPROVEMENTS/CLI_POSSIBLE_COMMANDS.md` notes that `--update` should align with settings and eventually broader CLI coverage.

So the direction exists, but update command resolution is not yet abstracted for non-Arch package managers.

---

## Target command policy (authoritative mapping)

Define a normalized "full update command policy" table used by both TUI and CLI paths.

## Core PM families

- `pacman` (Arch): `pacman -Syu`
- `yay` (AUR helper): `yay -Syu` for unified update path when helper is selected as primary updater; `yay -Sua` only when explicitly running AUR-only mode
- `paru` (AUR helper): `paru -Syu` for unified update path when helper is selected as primary updater; `paru -Sua` only when explicitly running AUR-only mode
- `apt` (Debian/Ubuntu): `apt update && apt upgrade`
- `dnf` (Fedora/RHEL-family): `dnf upgrade --refresh`
- `zypper` (openSUSE): `zypper refresh && zypper update`
- `xbps-install` (Void): `xbps-install -Syu`
- `apk` (Alpine): `apk update && apk upgrade`
- `eopkg` (Solus): `eopkg upgrade`
- `flatpak` (optional additional scope): `flatpak update`

Notes:

- Preserve existing AUR-only semantics (`-Sua`) where user explicitly requests AUR-only update.
- Keep "force sync" option scoped to PMs that support an equivalent safely (initially only pacman family).
- Keep non-interactive flags configurable by mode (TUI can remain confirmation-driven; CLI currently uses non-confirm by default on Arch and should apply equivalent non-interactive behavior per PM where appropriate).

---

## Proposed architecture adjustments

## 1) Introduce PM-aware update command resolver

Add a dedicated module (example: `src/logic/update/commands.rs`) that returns a structured plan:

- detected distro and package manager family
- main update command(s)
- optional secondary command(s) (for helper ecosystems / optional flatpak pass)
- capability flags:
  - supports_force_refresh
  - supports_split_repo_vs_aur
  - supports_noninteractive_flag

This replaces hardcoded command assembly in TUI/CLI handlers.

## 1.1) Add strict "all-upgradeable selected" gating

General full-upgrade commands (for example `pacman/yay/paru -Syu`, `apt update && apt upgrade`, `dnf upgrade --refresh`) must be executed only when all upgradeable packages are included by the current update scope.

Required resolver input:

- `selection_scope`: `AllUpgradeable` | `PartialSelection` | `Unknown`
- `detected_upgradeable_count`: number from update check snapshot
- `selected_count`: number selected for execution
- `snapshot_id`: identifier/hash of the update list used for selection
- `current_snapshot_id`: latest identifier/hash at execution time

Hard rule:

- use general full-upgrade command only if:
  - `selection_scope == AllUpgradeable`
  - `detected_upgradeable_count > 0`
  - `selected_count == detected_upgradeable_count`
  - `snapshot_id == current_snapshot_id` (no stale selection)
- otherwise DO NOT run general full-upgrade command.
- fallback to safe behavior:
  - either block with actionable error
  - or execute explicit package-targeted update command path (if supported by PM mode)

## 2) Single source of truth for command templates

Move command mapping into one place (constant map or typed matcher):

- Input: PM family + mode (full, repo-only, aur-only, force-refresh)
- Output: shell-safe command sequence fragments

TUI and CLI call the same resolver with different execution preferences.

## 3) Explicit update modes

Normalize modes used by both entry points:

- `FullSystemUpdate`
- `RepoOnlyUpdate`
- `AurOnlyUpdate` (Arch-family only)
- `CacheCleanup` (where relevant)

This avoids divergent interpretation between modal toggles and CLI flags.

## 3.1) Add instant system update/upgrade buttons (UI)

Add explicit UI actions/buttons for immediate update flows that execute native system update/upgrade commands.

Proposed actions:

- `System Update (Instant)` button
  - Uses PM-native full update command path (for example `pacman -Syu`, `apt update && apt upgrade`, etc.)
  - Requires strict all-upgradeable gating checks before execution
- `System Upgrade (Instant)` button
  - Uses PM-native upgrade command semantics where distinct from update wording
  - Also gated by strict all-upgradeable checks

Behavior rules:

- Buttons are enabled only when:
  - update snapshot is present and fresh
  - all-upgradeable package scope is active/confirmed
  - required package manager tools are available
- If checks fail, button press must:
  - not execute full-upgrade command
  - show precise reason (partial selection, stale list, missing tool, unknown scope)
  - offer re-check/reload updates action
- Button actions must route through the same shared resolver and policy guards as modal/CLI update paths (no parallel ad-hoc command builder).

Suggested placement:

- Add in existing `SystemUpdate` modal and/or top-bar quick actions area.
- Keep keyboard-first bindings for both actions (mapped in help overlay).

## 4) Distinguish "helper as primary updater" vs "split pacman+aur"

For Arch-family distros, choose strategy via config/setting:

- Strategy A (current-ish): `pacman -Syu` then `yay/paru -Sua`
- Strategy B (general helper update): `yay/paru -Syu`

Default recommendation for this task:

- Keep Strategy A as default for compatibility and explicitness.
- Add Strategy B as opt-in (future setting/flag), since user asked to follow general PM command usage.

---

## Implementation phases

## Phase 1: Baseline abstraction without behavior break

1. Add update command resolver module with current Arch behavior encoded first.
2. Refactor:
   - `src/events/modals/system_update.rs`
   - `src/args/update.rs`
   to consume resolver output.
3. Keep user-visible behavior unchanged for Arch paths (except internal centralization).

Acceptance:

- Existing update tests remain green.
- No command regressions in Arch/AUR flows.
- Full-upgrade path cannot be reached unless strict all-selected checks pass.

## Phase 2: Add non-Arch PM command mappings

1. Extend distro/PM detection for update execution context.
2. Add mappings for `apt`, `dnf`, `zypper`, `xbps-install`, `apk`, `eopkg`.
3. Gate Arch-specific options (AUR toggle, force sync) when unsupported.
4. Improve user-facing messaging when option is unsupported on detected PM.

Acceptance:

- Correct command selected for each mocked PM family.
- Unsupported options are disabled or clearly rejected with actionable feedback.

## Phase 3: Optional unified helper mode for Arch

1. Add setting/flag to choose:
   - split mode (`pacman -Syu` + helper `-Sua`)
   - helper-primary mode (`yay/paru -Syu`)
2. Ensure TUI and CLI both honor this consistently.

Acceptance:

- Mode selection is deterministic and test-covered.
- No accidental duplicate updates of repo packages.

## Phase 4: Instant button integration

1. Add `System Update (Instant)` and `System Upgrade (Instant)` UI actions.
2. Wire button handlers to shared resolver (same path used by modal/CLI).
3. Add disabled-state logic and user-facing reasons.
4. Add keyboard shortcuts and update help overlay text.

Acceptance:

- Buttons cannot bypass full-upgrade gating.
- Instant actions execute correct PM-native command only when all checks pass.
- Error states are actionable and non-destructive.

## Phase 5: Hardening and UX polish

1. Add dry-run-safe command rendering tests.
2. Add missing-tool diagnostics per PM family.
3. Ensure log output states:
   - detected PM
   - selected update strategy
   - exact executed command path (password redacted where needed)
4. Add staleness guard:
   - if upgradeable set changed between preview and execution, force revalidation and require reconfirmation.

---

## Testing plan

## Unit tests

- Resolver tests:
  - distro/PM detection -> expected PM family
  - mode + PM -> expected command sequence
  - unsupported option handling
  - strict full-upgrade gating:
    - all counts equal + fresh snapshot => allow full-upgrade command
    - partial selection => deny
    - unknown selection scope => deny
    - stale snapshot => deny
    - zero upgradeable packages => deny

## Integration tests

- CLI update flow:
  - Arch split mode
  - Arch helper-primary mode (if enabled)
  - Non-Arch PM command selection under mocked environment
- TUI system update modal:
  - command generation from selected toggles
  - no Arch-only options for non-Arch PMs
  - execution blocked when selection is not complete/fresh for full-upgrade path
- Instant buttons:
  - enabled only in valid all-selected/fresh states
  - execute resolver-selected native command on success path
  - blocked with reason when checks fail

## Regression tests

- Existing tests under `tests/update/` and `src/events/modals/system_update/tests.rs` updated to assert resolver-driven behavior instead of hardcoded pacman-only assumptions.
- Add regression covering "all selected at preview, list changed before execute" to guarantee fallback/block path.

---

## Risks and mitigations

- Risk: accidental behavior drift on Arch.
  - Mitigation: Phase 1 keeps exact behavior and locks with tests.
- Risk: distro detection ambiguity.
  - Mitigation: explicit PM detection precedence and fallback messaging.
- Risk: command safety/injection.
  - Mitigation: preserve existing quote/sanitization practices and avoid unsanitized shell interpolation.
- Risk: false-positive "all selected" because of stale update list.
  - Mitigation: snapshot-id validation at execution time + mandatory revalidation on mismatch.

---

## Deliverables

1. New PM-aware update command resolver module.
2. Refactored TUI update command generation to use resolver.
3. Refactored CLI `--update` command execution to use resolver.
4. Instant `System Update/Upgrade` buttons wired to shared guarded command path.
5. Test coverage for PM command policy, mode selection, and button guard behavior.
6. Optional follow-up: helper-primary update strategy setting for Arch.

---

## Recommended default policy after rollout

- Arch without helper preference: `pacman -Syu` (+ optional AUR-only step when requested)
- Arch with helper-primary preference: `yay -Syu` or `paru -Syu`
- Debian-based: `apt update && apt upgrade`
- Fedora-based: `dnf upgrade --refresh`
- openSUSE: `zypper refresh && zypper update`
- Other supported PMs: use their native full-upgrade command as mapped above.

Important invariant:

- Native full-upgrade commands are only allowed when robust validation confirms that the execution scope is truly "all currently upgradeable packages".

