# Config editor UX — Review A (claudeAgent / claude-fable-5-1), round 1

Reviewer run: taskId `node:delegated-task:command%3Amcp%3A59fbb81b-62ac-4473-b0b5-c803dd8d6ba5%3Adelegate-task%3Aconfig-editor-ux-review-fable-r1`, read-only, fresh context. Verbatim final output below.

---

**Overall assessment**

The feature is well structured and matches the plan's approved decisions. The state/events/ui split is clean, the save path (patch, dry-run, theme whole-file validation, stale guard, reload) is unchanged and still covered by the integration tests, capture mode and popup printable-key precedence are routed correctly ahead of global shortcuts, package-mode printable chords are gated off in editor mode, hit rects are rebuilt every frame and the renderer reads cached validation only. Locale files parse, have no placeholder-count mismatches, and every key the renderer uses exists in all three locales. I found no critical or high issues. The most notable new problem is filesystem I/O on every mouse event, followed by a few small UX and maintainability gaps.

**Findings**

- **A1, medium, performance.** `src/events/modals/config_editor/mouse.rs:26` does `std::mem::take(&mut app.config_editor_state)` for every mouse event. `ConfigEditorState::default()` (`src/state/config_editor/mod.rs:394-437`) calls `lists_dir()` (which runs `create_dir_all`) and reads and JSON-parses two files. Crossterm's `EnableMouseCapture` enables any-event tracking (`?1003h`), so this runs on every pointer movement over the editor, not just clicks. The old code only ran on left-down and borrowed the state without `take`. The key handler and `src/ui.rs:436` already had this pattern, so the change multiplies an existing cost rather than introducing it. Fix: give `ConfigEditorState` a cheap swap that does not touch disk, or return early for `Moved`/`Drag` before taking the state.
- **A2, low, correctness/UX.** Bool popup printable keys filter against the hidden canonical buffer (`true`/`false`), not the displayed on/off labels. The bool popup has no input box, so pressing e.g. `y` or `o` silently collapses the picker to the single fallback row. Fix: ignore `KeyCode::Char` other than space for `EditPopupKind::Bool`, or filter on the localized label.
- **A3, low, mouse routing.** With the recents dropdown open, a left click that misses a recent row falls through to `row_rects` and selects a list row drawn underneath the dropdown; the dropdown stays open. Fix: when `recent_dropdown.is_some()`, close it on any non-recent click and return `true`.
- **A4, low, localization coverage.** de-DE and hu-HU lack `app.modals.config_editor.settings.<key>.{label,summary}` for the settings.conf rows (pre-existing, zero newly missing). German users cannot search settings rows by localized label, and the parity test only requires labels for keybind/theme rows. Suggest extending the parity test to settings labels or recording the gap.
- **A5, low, security hygiene.** `SuggestionContext::capture` surfaces `HTTPS_PROXY` verbatim as a dropdown row. Proxy URLs commonly embed `user:password@`, and `pi_scan_https_proxy` is not `Sensitive`, so the credential is shown on screen and written in clear by Ctrl+E export (`fs::write` without 0600). Fix: strip userinfo from the suggested value, or mark the key sensitive for export/masking.
- **A6, low, maintainability.** Legacy fields and APIs are now dead: `focus`, `file_cursor`, `search_focus`, `recent_cursor`, `bookmark_cursor`, `ConfigEditorFocus`, `ConfigEditorSearchFocus`, `selected_recent_query`, `selected_bookmarked_key`, `clamp_recent_cursor`, `clamp_bookmark_cursor`. `handle_popup_key`'s `bool` return is never read. Remove in a follow-up.
- **A7, low, performance (minor).** The footer is built three times per frame, and `keybind_footer_top` calls `selected_key()` so `filtered_keys()` runs twice per frame despite the module doc claiming once.
- **A8, low, consistency.** `select_highlight` assumes an 8-row dropdown and PgUp/PgDn move by 8 while the renderer draws up to 10 rows and re-clamps. Harmless, but the constant should be shared.
- **A9, low, UX.** Comma-list completion does not skip tokens already present, so picking `Germany` with buffer `Germany, Ger` yields `Germany, Germany`.

Parent polish notes: P2 and P3 confirmed in code. P1, P4, P5 cosmetic, not disputed.

**Checks run**: `cargo fmt --all -- --check` 0; `cargo clippy --all-targets --all-features -- -D warnings` 0; `cargo check` 0; `cargo test -- --test-threads=1` 0 (2,021 passed, 0 failed, 17 ignored); locale flatten/parity script 0 missing renderer keys, 0 `{}` mismatches, 0 non-string values. `tests/pi_scan/ws4_tui.rs` not in the staged diff; its tests passed.

**Not verified**: no live terminal run (A1 event volume inferred from crossterm's `?1003h`); German translation quality; Windows PATH probing and chord capture; `Shift+<letter>` capture on real terminals.
