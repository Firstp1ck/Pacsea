# BlackArch Support Implementation Plan (Consume-Only)

## Goal

Add BlackArch support by detecting and using BlackArch repositories when they are already configured in pacman. Do not add repository bootstrapping. Keep AUR helper behavior unchanged (`paru`/`yay` only).

## Scope

- Include BlackArch repository names in official package indexing.
- Classify BlackArch repositories consistently with existing distro/optional repo logic.
- Add UI/state support so BlackArch can be filtered/toggled like other optional repos.
- Add/extend tests for classification, filtering, and optional-repo detection.
- Do not change install/update helper selection logic.
- Do not add BlackArch setup commands or keyring/repo bootstrap workflows.

## Implementation Steps

1. Add BlackArch repository helpers in `src/index/distro.rs`
- Add `blackarch_repo_names()` with initial repository list (`blackarch`).
- Add `is_blackarch_repo(repo: &str) -> bool`.
- Follow existing helper patterns used for EndeavourOS, CachyOS, and Artix.

2. Extend official index fetching in `src/index/fetch.rs`
- Add BlackArch repo names to the repo iteration used for `pacman -Sl`.
- Preserve existing graceful behavior on repo query failures (skip failed repo, continue merge).

3. Extend repo toggle/label mapping in `src/logic/distro.rs`
- Map BlackArch repositories in `repo_toggle_for(...)` to a dedicated filter flag.
- Add/adjust label mapping in `label_for_official(...)` where appropriate.

4. Extend optional repo UI metadata in `src/ui/results`
- Update `src/ui/results/mod.rs` to include `has_blackarch` in `OptionalRepos`.
- Update `src/ui/results/utils.rs` (`detect_optional_repos`) to set `has_blackarch` for BlackArch official results.

5. Wire state/defaults for BlackArch filtering
- Update relevant state/default filter structs in `src/state` and related wiring code.
- Keep defaults aligned with current optional repo behavior.

6. Add tests
- `src/index/distro.rs`: unit tests for BlackArch repo name helper/classification.
- `src/logic/distro.rs`: unit tests for toggle/label routing.
- `src/ui/results/utils.rs`: tests for optional repo detection with synthetic `Source::Official { repo: "blackarch", ... }`.
- Add or adjust integration tests in `tests/` for filter/chip visibility behavior where applicable.

7. Validate quality gates
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo check`
- `cargo test -- --test-threads=1`

## Constraints and Non-Goals

- No BlackArch bootstrap (no keyring install/repo enable automation).
- No AUR helper expansion beyond existing `paru`/`yay` detection and usage.
- No README/wiki updates.

## Acceptance Criteria

- BlackArch packages appear as official packages when the `blackarch` repo exists in pacman configuration.
- BlackArch repo visibility can be controlled through the same filtering/toggle model used for existing optional repos.
- Existing repo classification/filter behavior remains unchanged for non-BlackArch repos.
- All required checks (`fmt`, `clippy`, `check`, `test`) pass.
