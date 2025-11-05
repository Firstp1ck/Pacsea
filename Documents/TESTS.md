# Pacsea test plan (missing kinds and prioritized scenarios)

This document outlines test types and concrete scenarios that are currently missing or underrepresented in the repository. They are prioritized to maximize risk reduction and stability with minimal churn.

Conventions:
- Paths like `src/app/runtime.rs` refer to files in `Pacsea/`.
- Use inline backticks for symbols like `handle_event`, `AppState`, etc.
- Favor hermetic tests that stub PATH or inputs the same way existing tests do.

-------------------------------------------------------------------------------

Priority P0 — High impact, user-visible stability

1) End-to-end runtime smoke test (headless) - implemented
- Goal: Ensure `app::run` can initialize, render once, and shutdown cleanly without a real TTY.
- Why: The async runtime coordinates many tasks (events, search, details, status, news). A smoke test guards regressions in initialization and the main select loop.
- Scope:
  - Start `app::run(true)` in a background task with a short timeout, using a headless/fake terminal backend or a feature flag to bypass raw TTY setup in `setup_terminal` and `restore_terminal`.
  - Assert no panic and that log lines like “logging initialized” and “Pacsea starting” are produced, or that the function returns Ok(()) after a single loop.
- Files: `src/app/runtime.rs`, `src/app/terminal.rs`.
- Notes: If raw TTY cannot be bypassed in tests, gate `setup_terminal` behind a cfg or environment flag (e.g., `PACSEA_TEST_HEADLESS=1`) and use a no-op backend in tests.

2) UI menu interactions: sort/options/panels/config end-to-end
- Goal: Verify mouse and keyboard interactions open/close menus, auto-close on timeout, and apply toggles.
- Why: Menus drive core flows (sorting and filtering) and rely on rect hit-tests and timers.
- Scope:
  - Click on `sort_button_rect`, ensure `sort_menu_open == true`, assert `sort_menu_rect` set.
  - Let auto-close deadline pass and tick; assert it closes.
  - Repeat for `options_button_rect`, `panels_button_rect`, `config_button_rect`.
  - Validate toggles in options/panels affect `AppState` flags (e.g., `show_recent_pane`, `results_filter_show_*`).
- Files: `src/ui/results.rs`, `src/events/mouse.rs`, `src/state/app_state.rs`.

3) Installed-only mode correctness (results rewriting)
- Goal: Validate the complex code path that reconstructs results from explicit installs for empty query and intersects results for non-empty query.
- Why: This path manipulates `results`/`all_results` and depends on `explicit_names()` and `is_installed()`.
- Scope:
  - Seed index with a mix of official and AUR names; seed explicit and installed caches.
  - With `installed_only_mode = true` and empty query, assert results contain all explicit names, filling missing official entries as AUR or EOS as per logic.
  - With a non-empty query, assert results are intersected with explicit set only.
- Files: `src/app/runtime.rs` (results handling), `src/index/installed.rs`, `src/index/explicit.rs`.

4) Import/Export flows (Install list)
- Goal: Ensure export produces files with correct name pattern and content; import enqueues parsed names.
- Why: These are user data paths with filesystem writes and rely on rects, toasts, and sorting.
- Scope:
  - Export: Put a few items into `install_list`, click `install_export_rect`, assert file created under `~/.config/pacsea/export/` with pattern `install_list_YYYYMMDD_serial.txt`, assert content sorted, and toast message set.
  - Import: Create a temp file with mixed lines (blank, commented, official, AUR-like), click `install_import_rect`, assert queued items were sent via `add_tx`, and `AppState.install_list` eventually contains merged results.
- Files: `src/events/mouse.rs`.

5) PKGBUILD copy fallbacks and messaging
- Goal: Validate “Copy Package Build” button behavior for Wayland/X11 and missing tools.
- Why: Threaded child-process code can regress quietly; toasts guide users to install dependencies.
- Scope:
  - Case Wayland: Set `WAYLAND_DISPLAY`, stub `wl-copy` on PATH to append payload to a file, click the button, assert toast optimistic message then success.
  - Case X11 fallback: Unset `WAYLAND_DISPLAY`, stub `xclip` similarly, assert behavior.
  - Case neither present: Ensure guidance toast message text matches expectation per display mode.
- Files: `src/events/mouse.rs`.

-------------------------------------------------------------------------------

Priority P1 — Medium impact, correctness and resilience

6) News modal decision logic
- Goal: Verify behavior when today’s news exists vs. not.
- Why: Time-based logic and modal vs. toast branching can break silently.
- Scope:
  - Stub `sources::fetch_arch_news` to return items with dates matching today and not.
  - Assert `Modal::News { .. }` shown when a matching date exists, and a short-lived toast when none.
- Files: `src/app/runtime.rs`, `src/app/news.rs`.

7) Search debounce and rate limit windows
- Goal: Exercise `send_query` debounce and minimum interval logic.
- Why: Prevents chatty queries; changes can affect responsiveness.
- Scope:
  - Drive multiple `send_query` calls rapidly; assert only the last is dispatched after debounce and that a minimum interval is respected between network tasks.
- Files: `src/logic/query.rs`, `src/app/runtime.rs`.

8) Ring prefetch heuristics and gating
- Goal: Ensure ring prefetch respects gating, resumes after delay, and requests a bounded radius of details.
- Why: Wrong gating can over-fetch or starve details; interacts with scroll counters and timers.
- Scope:
  - With a small `results` set and selected index, call `ring_prefetch_from_selected`, assert `details_req_tx` got expected names within radius and that `is_allowed` toggling is honored.
  - Simulate rapid scroll to set `need_ring_prefetch` and `ring_resume_at`, tick forward and ensure it resumes and clears flags.
- Files: `src/logic/prefetch.rs`, `src/logic/gating.rs`, `src/app/runtime.rs`.

9) Index update notifications and enrichment preservation
- Goal: Validate that updates trigger notify channel and preserve enriched fields.
- Why: UI refresh depends on notify; losing enriched fields degrades UX.
- Scope:
  - Seed index with enriched fields, call `update_in_background` with a changed snapshot, assert notify channel fired and enriched fields persist.
- Files: `src/index/update.rs`, `src/index/enrich.rs`.

10) Robust parsing against malformed inputs
- Goal: Hardening for JSON and HTML scrapers.
- Why: External APIs can return malformed data.
- Scope:
  - `sources/search.rs`: malformed JSON and HTTP errors already partially covered; add additional cases (e.g., missing required fields).
  - `sources/details.rs`: minimal/malformed objects for official/AUR paths, ensure defaults and fallbacks apply without panic.
  - `sources/status.rs`: HTML with missing today rows or unexpected percentage formats still yields a safe color and message.
- Files: `src/sources/*.rs`.

11) Tiny terminal dimensions rendering
- Goal: Ensure UI never panics on very small frames and still sets rects sanely.
- Why: ratatui layout code can underflow or saturate.
- Scope:
  - Render frames at 1×1, 2×N, N×2; ensure no panic and rects are set or remain None coherently.
- Files: `src/ui.rs`, `src/ui/*`.

-------------------------------------------------------------------------------

Priority P2 — Depth and robustness improvements

12) Property-based tests (proptest) for utility and parsing helpers
- Goal: Broaden input coverage beyond fixed examples.
- Why: Catch edge cases and invariants.
- Candidates:
  - `util::percent_encode`: Unreserved characters are unchanged; others become `%XX` uppercase; round-trip decode (if implemented externally) matches.
  - `util::ts_to_date`: Non-negative seconds are formatted as `YYYY-MM-DD HH:MM:SS` and monotonic across contiguous seconds; `is_leap` invariant holds.
  - Theme key parsing: arbitrary capitalization/whitespace normalizes in `settings` and `config`.
- Files: `src/util.rs`, `src/theme/parsing.rs`, `src/theme/settings.rs`.

13) Snapshot (golden) tests for UI strings
- Goal: Detect regressions in static text composition and titles.
- Why: Easy to drift without visual CI.
- Scope:
  - Use a snapshot framework to capture formatted titles (e.g., results header, button labels, help content) as simple strings. Avoid full-frame snapshots to reduce flakiness.
- Files: `src/ui/*`, `src/ui/helpers.rs`.

14) Windows-specific import flow
- Goal: Validate the PowerShell OpenFileDialog path is formed correctly.
- Why: Code path is gated by OS and untested on non-Windows CI.
- Scope:
  - Compile-time cfg test that builds the script string and asserts key tokens (title, filter) exist.
- Files: `src/events/mouse.rs`.

15) Sorting modes and tiebreakers
- Goal: Ensure `SortMode` variants produce expected orders and are stable for ties.
- Why: UX consistency.
- Scope:
  - Create synthetic `PackageItem` sets with collisions; assert order by repo/name, AUR popularity (when set), and “best matches” rank, with stability on ties.
- Files: `src/logic/sort.rs`, `src/util.rs`.

16) Keymap conflict resolution
- Goal: Detect last-wins or merge behavior for duplicate keybinds and invalid tokens.
- Why: Users can define multiple entries; current behavior should be locked down by tests.
- Scope:
  - Feed settings with repeated `keybind_*` lines and invalid chords; assert effective `KeyMap` prefers the intended rule and defaults correctly.
- Files: `src/theme/settings.rs`, `src/theme/parsing.rs`.

-------------------------------------------------------------------------------

Priority P3 — Hardening and performance

17) Fuzzing targets (optional, separate workspace)
- Goal: Drive parsers with random bytes for crash resilience.
- Targets:
  - JSON parsers in `sources/details.rs`, `sources/search.rs`.
  - HTML parsers in `sources/status.rs`, `sources/news.rs`.
- Notes: Place under a fuzz workspace; keep binary-blob corpora.

18) Concurrency and race detection (best-effort)
- Goal: Stress concurrent cache refresh and UI updates.
- Scope:
  - Repeatedly toggle `installed_only_mode`, mutate `install_list`, and trigger `refresh_installed_cache` in rapid succession; assert no panics/data races (in tests, these are logical races).
- Files: `src/app/runtime.rs`, `src/index/*`.

-------------------------------------------------------------------------------

Implementation guidelines

- Keep tests hermetic:
  - Use temp directories under `std::env::temp_dir()`; restore environment variables.
  - Stub executables on PATH as done in existing tests (`write_fake` pattern).
- Prefer existing patterns:
  - Use `ratatui::backend::TestBackend` and `Terminal` in UI tests.
  - Reuse the crate’s `test_mutex()` guards in `index`, `logic`, and `sources` when touching shared state.
- Consider optional dev-only deps:
  - Snapshot tests: a lightweight snapshot crate.
  - Property-based tests: `proptest` as a dev-dependency.

Acceptance checklist per new test
- The test is deterministic and hermetic (no external network or actual terminals).
- It documents the “What / Input / Output” inline (match existing style).
- It restores any mutated global environment (e.g., `PATH`, `HOME`) after completion.
- On failure, it produces actionable output (e.g., includes the path to temp output).

Suggested file placement
- End-to-end and UI behavioral tests: `tests/` (integration tests).
- Unit/property tests colocated with modules where practical.
- Platform-specific tests under `#[cfg(target_os = ...)]`.

By addressing P0 and P1 first, we’ll substantially increase confidence in core workflows (startup/shutdown, search/render loop, menus, installed-only mode, import/export, and clipboard actions) and reduce regressions across releases.
