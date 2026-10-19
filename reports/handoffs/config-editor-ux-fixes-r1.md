# Config editor UX — accepted fixes, round 1

Implementation worker: Codex / gpt-6.1-sol, branch `feat/config-editor-ux`, worktree `/home/firstpick/.t3/worktrees/pacsea/feat-config-editor-ux`.

All accepted findings are implemented. Changes are unstaged on top of the staged WS1/WS2 feature. No staging, commits, stash, restore, checkout, reset, branch switches, dependency changes, or edits to the main checkout were performed. B4 strict-picker Tab cycling and P5 shared footer heading casing remain unchanged.

## Regression-first evidence

The following commands were run **after adding regression tests and before changing production code**:

| Reference | Command | Exit | Observed result |
|---|---|---:|---|
| U | `cargo test --lib regression_ -- --test-threads=1` | 101 | 16 tests failed, 0 passed; each expected defect was reproduced. |
| L | `cargo test --lib renderer_locale_keys_exist_in_all_locales -- --test-threads=1` | 101 | Missing German settings labels/summaries, starting with `sort_mode.label` and `sort_mode.summary`. |
| E | `cargo test --test other config_editor_phase4_export_writes_effective_values_with_redaction -- --test-threads=1` | 101 | `assertion failed: exported.contains("pi_scan_https_proxy = [REDACTED]")`. |

Initial test drafting caught two test problems before implementation: a UTF-8 byte-boundary mistake in the hanging-indent assertion, and a viewport test that happened to pass because ten total rows contained eight suggestions plus two indicators. Both were corrected and U was rerun against unchanged production code. The corrected hanging-indent test failed on `│zeta eta theta iota kappa lambda mu ...│` starting at the left border instead of the value column; the corrected viewport test failed with 9 suggestion rows versus 8 expected. U above refers to this corrected run.

Focused passing commands after implementation:

| Reference | Command | Exit | Observed result |
|---|---|---:|---|
| UP | `cargo test --lib config_editor -- --test-threads=1` | 0 | 113 unit/render tests passed, including all 16 regression tests and locale parity. |
| EP | `cargo test --test other config_editor -- --test-threads=1` | 0 | 21 integration tests passed, including export redaction and explicit persistence loading. |

The final full-suite run below also covers all these tests after final formatting and lint fixes.

## Per-finding changes and validation

Paths below are relative to the worktree. Test names omit their module prefixes for readability. Every U/L/E reference includes its exact command and exit code above.

| Finding | Files and final behavior | Tests and failing-before evidence | Passing-after |
|---|---|---|---|
| **B1 / A5** | `src/state/config_editor/suggestions.rs`, `validate.rs`, `ux_tests.rs`; `src/events/modals/config_editor/save.rs`; `tests/other/config_editor_phase3_integration.rs`. Both environment-variable capture and injected candidate generation require `https://` and no `@`. Manual values must be empty or credential-free HTTPS, with instructions to remove userinfo or disable the proxy. Export redacts any proxy containing `@`. | `regression_proxy_candidates_require_credential_free_https` (U: “unsafe environment proxy was suggested”); `regression_proxy_validation` (U: “unsafe proxy accepted”); `config_editor_phase4_export_writes_effective_values_with_redaction` (E: missing redacted proxy line). Tests include credential-bearing HTTPS, HTTP, clean HTTPS, empty values, and absence of the dummy proxy password in export. | UP, EP, full suite. |
| **B2** | `src/events/modals/config_editor/mouse.rs`. Any non-None modal returns false before taking editor state, beside the existing top-row-menu guard. | `regression_alert_blocks_editor_mouse` (U: `!handle_config_editor_mouse(click(2, 1), &mut app)` failed). Exercises an Alert above tab and row rectangles; verifies no tab switch or popup. | UP, full suite. |
| **B3** | `src/events/modals/config_editor/popup.rs`, `tests.rs`. Integer nudges parse `effective_value()`, replace the buffer, and refresh the free-input picker, clearing its highlight. | `regression_integer_nudge_uses_highlight` (U: effective value `21`, expected `2`). Covers 20 → highlighted 1 → `+` → 2 and highlighted 5 → `-` → 4. | UP, full suite. |
| **B6** | `src/state/config_editor/values.rs`, `ux_tests.rs`. Keybind modified state compares complete current/default chord slices. | `regression_extra_chord_marks_modified` (U: `snapshot.is_modified(entry("keybind_recent_remove"))` failed with only an appended F9). | UP, full suite. |
| **A1** | `src/state/config_editor/mod.rs`, `tests.rs`, `ux_tests.rs`; `src/state/app_state/default_impl.rs`; `src/events/modals/config_editor/mod.rs`, `mouse.rs`; `src/app/persist.rs`; `tests/other/config_editor_phase1_integration.rs`. Default allocates only in-memory empty state. `load_persisted()` preserves the directory, JSON filenames, normalization and limits, and is used at startup and editor entry. Motion/drag return before the state take. Flushes skip empty paths and retain dirty flags. | `regression_default_state_is_in_memory_only` (U: nonempty recent path). Added edge/workflow coverage: `config_editor_explicit_persistence_loading` checks Default creates no directory, ignores seeded files, and startup/editor re-entry load normalized lists; `config_editor_flush_skips_empty_paths`; `config_editor_motion_preserves_state`. Updated the stale Default-loading test-helper documentation. | UP, EP, full suite. |
| **A2** | `src/events/modals/config_editor/popup.rs`, `tests.rs`. Bool controls ignore printable input except Space; hidden filters cannot be entered. | `regression_bool_printable_input_is_ignored` (U: 1 visible choice, expected 2 after `y`). Also checks unchanged buffer, effective value and filter flag. | UP, full suite. |
| **A3** | `src/events/modals/config_editor/mouse.rs`. While recents are open, any left click first applies a recent row or dismisses the dropdown and consumes the event. | `regression_recent_miss_consumes_click` (U: `recent_dropdown.is_none()` failed). Covers underlying row, tab and empty-space clicks with no selection or popup changes. | UP, full suite. |
| **A6** | `src/state/config_editor/mod.rs`, `src/state/mod.rs`, `src/events/modals/config_editor/mod.rs`, `popup.rs`. Removed the listed unused fields, enums, methods, re-exports, and popup handler boolean result. | Read-only grep across `src/` and `tests/` found only their definitions/internal assignments and imports; unrelated package-search focus APIs were retained. No behavioral change requiring a failing test. The specified ws4 compatibility fields/method remain. | Clippy/check/full suite, including ws4 tests. |
| **A7** | `src/ui/modals/config_editor/mod.rs`. Module docs now distinguish the renderer's one filtering call from the outer footer-position calculation's separate `selected_key()`/filtering call. | Documentation correction only; no runtime change. | Clippy/check/full suite. |
| **A8** | `src/state/config_editor/mod.rs`; `src/events/modals/config_editor/popup.rs`; `src/ui/modals/config_editor/popup.rs`, `tests.rs`. Shared `DROPDOWN_PAGE_ROWS = 8` controls highlight scrolling, PgUp/PgDn, and maximum suggestion rows. Indicators have their own row budget; renderer still clamps to actual available height each frame. | `regression_dropdown_uses_eight_row_viewport` (U: 9 suggestion rows, expected 8). Existing scroll-clamp/indicator tests also pass. | UP, full suite. |
| **A9** | `src/state/config_editor/mod.rs`, `ux_tests.rs`. Comma-list filtering excludes suggestions equal to trimmed completed-prefix tokens. | `regression_comma_list_excludes_completed_tokens` (U: Germany was still visible for `Germany, Ger`). Also verifies it remains available for `France, Ger`. | UP, EP, full suite. |
| **B5** | `src/ui/modals/config_editor/list.rs`, `tests.rs`. Compact labels remove `.conf` when needed; a strip start offset ensures the active tab fits. Active captions can truncate at extreme widths; hit rectangles cover drawn tabs only. | `regression_narrow_tab_strip_keeps_active_visible` (U: “active Theme missing at width 40”). Covers widths 40 and 60 with every active tab, bracket marker, nonempty hit text, and rectangle bounds. | UP, full suite. |
| **P1** | `src/ui/modals/config_editor/details.rs`, `tests.rs`. Current/default field values wrap into the value column with hanging indentation, preserve styled color/value spans, and retain a four-row limit with ellipsis. | `regression_details_values_have_hanging_indent` (U: continuation began at the pane's left edge). Checks rendered cells in a 46-column details pane. | UP, full suite. |
| **P2** | `config/locales/{en-US,de-DE,hu-HU}.yml`; `src/ui/modals/config_editor/popup.rs`, `tests.rs`. Details use natural values after “Applies”: automatically / when saved / after restart; German uses automatisch / beim Speichern / nach einem Neustart. Popup metadata has short, separate apply labels and a short chord type; full conflict scope remains in details. | `regression_apply_wording_and_compact_popup_header` (U: redundant “applies on save” remained); `regression_compact_keybind_popup_header` (U: `keybind_help · key chord · conflict scope: Global shortcuts · applies on` wrapped before “save”). Existing wording assertions were updated. | UP, full suite; keybind_help metadata fits one row in a 76-column popup. |
| **P3** | `src/ui/modals/config_editor/format.rs`, `tests.rs`. Explicit `EMPTY_AUTO_DETECT_KEYS` contains only `preferred_terminal`; existing enum-empty behavior remains. | `regression_empty_terminal_displays_auto_detect` (U: screen contained `(empty)` for terminal list/current/default). Requires Auto-detect in all three places. | UP, full suite. |
| **P4** | `src/state/config_editor/suggestions.rs`. Palette context carries `(key, value)` pairs; palette suggestions receive the source key's settings-label translation key. Deduplicated colors retain the first source label. | `regression_palette_candidates_name_source` (U: `label_key` was None, expected `app.modals.config_editor.settings.background_base.label`). Existing palette-kind test adapted to pairs. Final test uses an entirely injected palette. | UP, full suite. |
| **A4** | `config/locales/{de-DE,hu-HU}.yml`; `src/ui/modals/config_editor/tests.rs`. Added all missing settings labels/summaries: German translations and English Hungarian placeholders with `# TODO: translate to Hungarian`. Locale parity now requires settings labels and summaries in all three locales. | `renderer_locale_keys_exist_in_all_locales` (L: missing German settings labels/summaries). | UP, full suite; independent YAML duplicate-key validation passes for all three files. |

## Required final checks

Run from the worktree root, in the requested order:

| Command | Exit | Result |
|---|---:|---|
| `cargo fmt --all` | 0 | Formatted. |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Clean; no new lint allowances. |
| `cargo check` | 0 | Compiles. |
| `cargo test -- --test-threads=1` | 0 | 2,040 passed, 0 failed, 17 ignored across 15 result groups. |

The initial Clippy run exited 101 for `needless_pass_by_value` on the styled-field label and `redundant_closure_for_method_calls` in a render assertion. Both were fixed before rerunning the final sequence. An intermediate focused run had two existing assertions still expecting the old apply wording; those expectations were updated and UP then passed.

Additional checks: `git diff --check` exit 0; YAML validation with duplicate-key rejection exit 0; the index raw snapshot comparison exit 0. All protected `ws4_tui` tests passed, including `config_editor_uses_configured_pi_scan_setup_chord`, `pi_scan_help_renders_workspace_wizard_and_configured_chord`, and `all_locales_include_pi_scan_workspace_translations`. No ws4 file changes. No dependencies were added, so no dependency audit was needed.

## Deviations and residual risks

- The “144 keys” locale gap means 144 label/summary **strings across 72 settings rows**, rather than 144 distinct settings rows. Four other settings rows already had translations. All 76 settings rows now have labels and summaries in each locale; 16 theme rows also have both.
- A1 necessarily changes `src/app/persist.rs` in addition to the feature modules to guard empty persistence paths, as requested. Startup and editor-entry loading are covered through the public event dispatcher.
- A7 is strictly a documentation correction; repeated footer construction/filtering remains as explicitly deferred.
- German additions have not had native-speaker review. Hungarian additions intentionally remain English TODO placeholders.
- Rendering is validated with ratatui TestBackend; no live-terminal visual or mouse run was performed. This is a source fix in the worktree, not an installed or restarted application.
- The plan already acquired the parent's unstaged review-disposition update during this session. It was read, never edited by this worker, and its hash stayed unchanged during final verification. The only Markdown file written by this worker is this handoff.
