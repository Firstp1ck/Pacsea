# BlackArch Support Implementation Plan (Consume-Only, Refined)

## Goal

Support BlackArch as an already-configured optional official repository source. Detect, index, label, and filter BlackArch packages when `blackarch` exists in pacman config. Do not add bootstrap/setup logic, and do not change AUR helper behavior (`paru`/`yay` only).

## Scope

- Include BlackArch repository names in official package indexing (`pacman -Sl` path).
- Classify BlackArch repositories consistently with existing distro/optional repo logic.
- Add end-to-end filter wiring (state, UI chips/rects, mouse toggle handling).
- Add/extend tests for classification, filtering, optional-repo detection, and UI filter behavior.
- Keep fallback behavior stable: failed optional repo queries are skipped, app continues.
- No repo bootstrap, no keyring automation, no helper selection changes.

## Implementation Steps (Ordered by Risk)

1. Index-level repo detection and exports
- Update `src/index/distro.rs`:
  - Add `blackarch_repo_names()` with initial list: `["blackarch"]`.
  - Add `is_blackarch_repo(repo: &str) -> bool` (case-insensitive exact match).
- Update `src/index/mod.rs` re-exports to include `is_blackarch_repo` so downstream modules can use `crate::index::is_blackarch_repo(...)`.

2. Official package fetch integration
- Update `src/index/fetch.rs`:
  - Include BlackArch repo iteration alongside core/extra/multilib/EOS/CachyOS/Artix.
  - Keep existing failure-tolerant behavior (empty output on missing repo, continue merge/dedup).

3. Filter logic and label routing
- Update `src/logic/distro.rs`:
  - Route BlackArch repos in `repo_toggle_for(...)` to a dedicated state flag.
  - Add label mapping in `label_for_official(...)` (recommended label: `BlackArch`).
  - Update unknown-repo fallback conjunction to include the new BlackArch filter flag.

4. AppState/default filter wiring (critical)
- Update `src/state/app_state/mod.rs`:
  - Add `results_filter_show_blackarch`.
  - Add `results_filter_blackarch_rect`.
- Update `src/state/app_state/defaults.rs`:
  - Extend `DefaultFilters` tuple and `filter_rects` array for BlackArch.
  - Default BlackArch filter to `true` (aligned with other official filters).
- Update `src/state/app_state/default_impl.rs`:
  - Wire the new boolean and rect through destructuring and struct construction.

5. Optional repo detection + render context
- Update `src/ui/results/mod.rs`:
  - Add `has_blackarch` to `OptionalRepos`.
  - Add `show_blackarch` to `FilterStates`.
- Update `src/ui/results/utils.rs`:
  - Extend `detect_optional_repos(...)` to detect BlackArch in `Source::Official { repo, .. }`.
  - Extend `extract_render_context(...)` to pass `has_blackarch` and `show_blackarch`.

6. Title rendering/layout/rect recording
- Update `src/ui/results/title` modules:
  - `types.rs`: add `filter_blackarch` and `blackarch` label field.
  - `i18n.rs`: load `app.results.filters.blackarch`.
  - `layout.rs`: include BlackArch in optional labels and spacing calculations.
  - `width.rs`: include BlackArch in optional width math and in `create_repos_without_specific(...)` copy.
  - `rendering.rs`: render BlackArch chip when available.
  - `rects.rs`: record/clear `results_filter_blackarch_rect`.

7. Mouse filter toggle wiring
- Update `src/events/mouse/filters.rs`:
  - Add simple toggle handler for `results_filter_blackarch_rect` -> `results_filter_show_blackarch`.
  - Keep existing Artix dropdown behavior unchanged.

8. Translation fixture updates used by tests
- Add `"app.results.filters.blackarch"` translations to test translation initializers in:
  - `src/ui.rs`
  - `src/ui/results/mod.rs`
  - `src/ui/helpers/tests.rs`
- If there are shared i18n fixture helpers elsewhere, update those too.

9. Optional UX consistency (recommended)
- Consider adding BlackArch to special-repo badge coloring in preflight tabs:
  - `src/ui/modals/preflight/tabs/summary.rs`
  - `src/ui/modals/preflight/tabs/deps.rs`
- This is optional but keeps visual treatment aligned with EOS/CachyOS custom repos.

10. Tests
- `src/index/distro.rs`:
  - Add unit tests for `is_blackarch_repo` and `blackarch_repo_names`.
- `src/logic/distro.rs`:
  - Add tests for `repo_toggle_for("blackarch", ...)` and label mapping.
  - Add fallback test coverage to ensure unknown-repo gating includes BlackArch flag.
- `src/ui/results/utils.rs`:
  - Add/extend tests for `detect_optional_repos` with BlackArch official source.
- `src/events/mouse/filters.rs`:
  - Add a test that clicking BlackArch filter rect toggles state and re-applies filtering.
- Integration-level:
  - Add/adjust tests in `tests/` for BlackArch chip visibility and filter behavior.

11. Validation gates
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo check`
- `cargo test -- --test-threads=1`

## Constraints and Non-Goals

- No BlackArch bootstrap workflows (no keyring install/repo enable automation).
- No AUR helper expansion beyond existing `paru`/`yay` logic.
- No README/wiki updates.

## Acceptance Criteria

- BlackArch packages are indexed and appear as official packages when `blackarch` repo exists in pacman config.
- BlackArch has a dedicated filter chip/toggle behaving like existing optional repos.
- Mouse click handling works for BlackArch filter toggle (same behavior pattern as other simple filters).
- Unknown-official fallback policy remains correct and includes BlackArch toggle in all-on requirement.
- Non-BlackArch behavior remains unchanged.
- All quality gates (`fmt`, `clippy`, `check`, `test`) pass.
