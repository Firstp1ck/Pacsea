# Config editor UX — WS2 handoff (rendering, footer, help, locales)

Status: COMPLETE. All required checks pass from the worktree root. Changes are unstaged. I did not stage, commit, stash, switch branches, or touch the plan. This is a source change only; nothing was installed or run in a live terminal.

## Changed files

- `src/ui/modals/config_editor.rs` → **deleted**, replaced by the module directory below.
- `src/ui/modals/config_editor/mod.rs`: entry point `render_config_editor_window` (same signature), `keybind_footer_top`, `EditorLayout` (tabs / search / list / details / footer), wide vs. stacked split, per-frame hit-rect reset, `clear_stale_main_rects` (unchanged), footer band, and one `ValueSnapshot` per frame. `render_frame` takes an injected snapshot so tests are deterministic.
- `src/ui/modals/config_editor/list.rs`: tab strip with `tab_rects`, search line (real cursor, placeholder, `N of M`, Ctrl+R hint), grouped key list (`row_rects`, `list_rect`, `list_page_rows`), empty states, recent-search dropdown (`recent_rects`).
- `src/ui/modals/config_editor/details.rs`: details pane with aligned fields and hanging-indent wrapping.
- `src/ui/modals/config_editor/popup.rs`: popup with adaptive height, input box with real cursor, swatch, Record button, recording state, dropdown with tags/durations/swatches/descriptions and ↑/↓ indicators, validation line, "Enter saves" line, and per-kind hint. Clamps `popup.scroll`; fills `popup_rect`, `popup_choice_rects`, `record_button_rect`.
- `src/ui/modals/config_editor/format.rs`: i18n helpers (`ctx.t`, `t_args`, `translated`), labels, group labels, apply behavior, type/range text, duration humanizing, tag labels/colors, and type-aware value spans.
- `src/ui/modals/config_editor/text.rs`: display-width helpers (`truncate`, `pad_right`, `fit_spans`, `wrap` with an exact row count, `input_window` for caret-following horizontal scroll).
- `src/ui/modals/config_editor/tests.rs`: 21 render and locale tests (listed below).
- `src/ui/details/footer.rs`: config-editor footer rewritten as packable items (`EditorFooterItem`, `pack_editor_footer`) with list / recent dropdown / popup / recording contexts. Changed signatures: `build_config_editor_footer_hint_lines(app, state, selected, th, width)` and `config_editor_footer_reserved_rows(show, app, state, selected, width)`. Removed `add_literal_key_entry`, `build_config_editor_globals_menus_line`, `append_config_editor_popup_hints`, `build_config_editor_editor_line_spans`. Its tests were updated and extended.
- `src/state/config_editor/suggestions.rs` (minimal, additive): `parse_swatch` is now `pub` with full rustdoc. The input preview's swatch reuses the same parser as the dropdown swatches, so the two can't drift apart.
- `src/state/config_editor/values.rs` (minimal, additive): `#[cfg(test)] pub(crate) const fn ValueSnapshot::from_parts(settings, theme)`. Render tests use it so assertions don't depend on the developer's `~/.config/pacsea`. It does not exist in non-test builds.
- `config/locales/{en-US,de-DE,hu-HU}.yml`: `app.modals.config_editor.*` and `app.modals.help.config_editor_lines` (details below).

`src/ui/modals/help.rs` and `src/ui.rs` were not changed. Help still renders the configured Pi Scan setup chord from `key_labels.pi_scan_setup` in the Pi Scan section.

## Layout

Wide (editor ≥ 100 columns): list 58 % | details 42 %.

```text
╭ Config editor ───────────────────────────────────────────────────────────────╮
│ [ settings.conf ]   keybinds.conf     theme.conf     ★ Starred (1)            │
│ Search: mirror▏                                     2 of 76 · Ctrl+R recent   │
│╭ Keys ───────────────────────────────────────╮╭ Details ─────────────────────╮│
││ Mirrors                                     ││Mirror count  ★ starred       ││
││▶ ★ Mirror count                     27 ●    ││Key         mirror_count      ││
││    Mirror countries          Worldwide      ││Current     27                ││
││ General                                     ││Default     20  ● modified    ││
││    Dry-run by default              off  ↻   ││Type        integer 1–200     ││
│╰─────────────────────────────────────────────╯╰──────────────────────────────╯│
│GLOBALS:  [Ctrl+C] Exit  |  [?] Help  Menus  [Shift+C] Config / lists  |  …    │
│Editor  [↑↓ PgUp/PgDn] Move selection  |  [Enter] Edit value  |  …            │
╰──────────────────────────────────────────────────────────────────────────────╯
```

Narrow (< 100 columns): the list sits above the details pane. The details pane is 40 % of the body height (7–12 rows) and is dropped when the body is shorter than 14 rows. The raw key column appears only when the list's inner width is ≥ 84.

```text
╭ Config editor ────────────────────────────────────────────╮
│ [ settings.conf ]  keybinds.conf  theme.conf  ★ Starred   │
│ Search: ▏type to filter …              76 of 76 · Ctrl+R… │
│╭ Keys ───────────────────────────────────────────────────╮│
││ General                                                 ││
││▶   Locale                           Auto-detect  ↻      ││
│╰─────────────────────────────────────────────────────────╯│
│╭ Details ────────────────────────────────────────────────╮│
││Locale                                                   ││
││Key         locale                                       ││
│╰─────────────────────────────────────────────────────────╯│
│ footer rows (packed to width)                             │
╰───────────────────────────────────────────────────────────╯
```

Popup (centered, height adapts; the dropdown shrinks first):

```text
╭ Edit · Open help ───────────────────────────────────────────────╮
│keybind_help · key chord · conflict scope: Global shortcuts · …  │
│╭───────────────────────────────────────────────────────────────╮│
││?▏                                                             ││
│╰───────────────────────────────────────────────────────────────╯│
│ ● Record (Ctrl+K)   or type a chord such as Ctrl+R              │  ← record_button_rect
│▶ ? [Current] [Default]                                          │  ← popup_choice_rects[0]
│  ↓ 3 more                                                       │  (only when clipped)
│✓ Valid                                                          │
│Enter saves: ?                                                   │
│Type a chord or press Ctrl+K to record one · ↑↓ pick · Enter save│
╰─────────────────────────────────────────────────────────────────╯
```

While recording, the input box turns red with the title `● Recording…` and shows "Press the new key combination · Esc stops recording". The button inverts, and the footer lists only `[Esc] Stop recording`. Bool popups have no input box: they show a two-row on/off picker and are at most 48 columns wide.

Row and value conventions:
- Row layout: `▶`, `★`, label, right-aligned value, `●` (modified, via `ValueSnapshot::is_modified`), `↻` (`RequiresRestart`).
- Bools show localized on/off. Colors show a `██` swatch plus hex. Secrets show `••••••••` or "(not set)".
- Keybinds show the chord plus `+N` for extra chords. An empty choice in an enum that allows it shows "Auto-detect"; an unbound keybind shows "(unbound)"; any other empty value shows "(empty)".
- The active tab is bracketed and inverted, so it is identifiable without color.

## Hit-rect contract (written every frame)

`reset_hit_rects` clears `tab_rects`, `row_rects`, `popup_choice_rects`, `recent_rects`, `record_button_rect`, `list_rect`, and `popup_rect` before drawing.

- `row_rects`: `(index into filtered_keys(), rect)`. Group headings have no rect.
- `popup_choice_rects[i]` addresses `visible[scroll + i]`, using the scroll value after the renderer clamps it.
- `recent_rects`: indices into `recent_queries`.
- `list_page_rows`: the number of selectable rows visible in the list (at least 1).
- `clear_stale_main_rects` is unchanged.

## Performance

`render_frame` calls `filtered_keys()` once. It passes the selection into the details pane and footer, which previously called `selected_key()`. It captures one `ValueSnapshot`. It never calls `current_value_string` and never validates; it reads the cached `popup.validation`.

There is one more `selected_key()` call per frame, in `keybind_footer_top`. `src/ui.rs` calls that function before the renderer to place toasts, and `src/ui.rs` is outside my scope.

## Footer and Help

Footer contexts:
- **List:** `[↑↓ PgUp/PgDn] Move selection`, `[Enter] Edit value`, `[Tab ← →] Switch tab`, `[Ctrl+B] Star / unstar`, `[Ctrl+R] Recent searches`, `[Ctrl+E] Export`, `[Esc]` (label is "Clear search" or "Leave editor" depending on the query), and the configured Pi Scan setup chord on `pi_scan_*` rows.
- **Recent dropdown:** `↑↓` / Enter / Esc.
- **Popup:** `[Enter/Ctrl+S] Save`, `[Esc] Cancel`, `[Tab]` (Complete suggestion, or Next choice for strict pickers), `[↑↓] Pick`, `[Ctrl+Z] Reset`, plus `Ctrl+R` reveal for secrets, `Ctrl+K` record for keybinds, `+/-` for integers, `Space` toggle for bools.
- **Recording:** only `[Esc] Stop recording`.

Rows are greedily packed to the footer width and drawn without wrapping, so `config_editor_footer_reserved_rows` equals the drawn row count. `keybind_footer_top` uses the same width, so toast placement stays correct. `app.modals.help.config_editor_lines` was rewritten in all three locales (8 lines).

## Locale keys

All of the following live under `app.modals.config_editor.`.

**Added** (en-US; de-DE has real German; hu-HU has the English text plus `# TODO: translate to Hungarian`):
- `tabs.{settings,keybinds,theme,starred}`
- `search.{label,placeholder,count,recent_hint}`
- `recent.{title,empty}`
- `list.{title,no_matches,starred_empty}`
- `details.{title,no_key,key,current,default,modified,type,applies,file,extra_chords,extra_chords_note,pi_scan_setup,pi_scan_setup_hint,starred}`
- `types.{bool,enum,int,text,list,path,secret,days,color,pane_order,chord}`
- `applies.{auto_reload,on_save,restart}`
- `values.{on,off,empty,unbound,secret_unset}`
- `tags.{current,default,auto_detect,unlimited,installed,detected,minimum,maximum}`
- `duration.{seconds,minutes,hours,day,days}`
- `popup.{title,placeholder_filter,placeholder_text,record,record_alt,recording,recording_prompt,valid,invalid,saves,more_above,more_below,no_matches,hint_bool,hint_enum,hint_int,hint_text,hint_list,hint_secret,hint_chord,hint_recording}`
- `footer.{heading_popup,heading_recording,heading_recent,switch_tab,star,recent,esc_leave,esc_clear,recent_choose,recent_apply,recent_close,popup_complete,popup_next_choice,popup_pick,popup_adjust,popup_toggle}`
- `settings.<key>.label` for all 58 keybind rows and all 16 theme rows, plus `.summary` for the 16 theme rows.
- `choices.<key>.<choice>` for `sort_mode`, `privilege_mode`, `auth_mode`, `installed_packages_mode`, `search_startup_mode`, `package_marker`, `locale` (`_empty` included), and `pi_scan_thinking`, plus the six `main_pane_order` permutations. Pane-order keys use the raw canonical string as a quoted YAML key (e.g. `"results, search, package_info"`). The loader only joins on `.`, and a test confirms these keys resolve, so they were not skipped.
- `groups.<all 23 SettingGroup keys>`.

**Removed** (grep found no remaining references):
- `title_files`, `files_pane_title`, `files_heading`, `keys_pane_title`, `keys_heading`, `keys_heading_filtered`, `no_matches`, `details_pane_title`, `hint_pick_file`, `hint_no_key`
- `search_pane_title`, `search_recent_title`, `search_recent_empty`, `search_input_title`, `search_input_placeholder`, `search_hint`, `search_bookmarks_title`, `search_bookmarks_empty`
- `footer.{move_file,open_file,bookmark,switch_panel,esc_hint,widen_search}`

**Kept:** `title`, `popup_empty_choice`, and the remaining `footer.*` keys. Some English footer texts were refreshed: `popup_cancel` is now "Cancel", `popup_reset` is "Reset to opening value", `popup_capture_cancel` is "Stop recording". hu-HU keeps its existing Hungarian for keys whose meaning did not change, and the two unchanged help lines (theme validation, Ctrl+E export) keep their Hungarian text.

Choice keys such as `"on"`/`"off"` are quoted so no YAML 1.1 parser reads them as booleans. All three files load with `pacsea::i18n::load_locale_file` and with PyYAML.

## Commands (final run, in order, from the worktree root)

| Command | Exit | Result |
|---|---:|---|
| `cargo fmt --all` | 0 | formatted (`--check` also 0) |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | clean, no new `#[allow]` |
| `cargo check` | 0 | compiles |
| `cargo test -- --test-threads=1` | 0 | 2,021 passed, 0 failed, 17 ignored across 15 result groups |
| `git diff --check` | 0 | — |

The read-only protected tests passed: `ws4_tui::pi_scan_help_renders_workspace_wizard_and_configured_chord`, `ws4_tui::config_editor_uses_configured_pi_scan_setup_chord`, and `ws4_tui::all_locales_include_pi_scan_workspace_translations`.

Failures during development, all fixed:
- First clippy run: `single_option_map`, `option_if_let_else`.
- First render-test run: 4 failures, all wrong test expectations. The wrapped text needed a shorter needle, the en heading is "GLOBALS:", the wide-list width had to be 160 so the key column appears, and a heading row was checked across both panes.
- One footer test needed translations loaded, because untranslated keys are wider than 40 columns.

## Tests added

`src/ui/modals/config_editor/tests.rs`:
- Tab strip active marker and rects.
- Grouped headings with values, `●`, `↻`, on/off text, and the raw key column on wide lists.
- `row_rects` text matches the labels; `list_page_rows` equals the visible selectable rows; row rects lie inside `list_rect`.
- The selection stays visible at the end of the list.
- All details fields, including starred and modified.
- Configured Pi Scan setup chord in details.
- Extra chords and conflict scope.
- Search cursor position and `N of M`.
- Empty states (no matches, empty Starred).
- Narrow stacked layout without the key column.
- Footer reserved rows equal the rendered footer, using `keybind_footer_top` and the status row.
- Popup dropdown, tags, validation ✓/✗, "Enter saves" line; choice rects match the suggestion values and sit inside the popup; cursor inside the popup.
- Record button rect inside the popup, recording state, and the recording-only footer.
- Bool on/off picker.
- Enum placeholder, translated description, and Auto-detect label.
- Narrow enum popup rows match their rects.
- Secret masking and reveal.
- Scroll clamp follows the highlight; indicators and tag badges render.
- Recent dropdown with rects and its empty state.
- Hit rects cleared between frames.
- Locale parity: every renderer key, the help lines, every keybind/theme label, theme summaries, every group key, and the choice descriptions of the 9 enums exist in en-US, de-DE, and hu-HU.

`src/ui/details/footer.rs`:
- Configured Pi Scan chord (ported to the new API).
- Packed rows fit widths 40/80/120/200 and equal the reserved rows.
- Recording lists only Esc.

## Validation omissions

- No live terminal or real-app run. Everything was verified with `TestBackend` frames, and I visually inspected dumps at 130×34 and 90×34 (list, int popup, keybind popup, theme tab, color popup).
- Mouse interaction was not re-tested end to end against the new rects. WS1's mouse tests use their own rects; my tests check that rects align with the rendered text.
- The German strings are my own translations and were not reviewed by a native speaker.

## Deviations and why

- **Footer API signatures changed**: added `selected` and `width` parameters. The footer has to be packed to the width so the reserved rows match what is drawn, and the selection is passed in so `filtered_keys()` is not recomputed per footer build. Both functions are crate-internal (`ui::details` is private).
- **Two minimal state edits** (`parse_swatch` made public; test-only `ValueSnapshot::from_parts`), justified above. No event semantics changed.
- **Tab labels**: Starred shows its bookmark count, e.g. `★ Starred (2)`.
- **Status line** stays green and single-row as before. Long status messages are clipped, not wrapped, to keep the reserved height exact.
- **Keybind `.summary` keys** were not added; the brief asked only for labels.

## Residual risks

- Input horizontal scrolling uses character display widths. Text mixing wide and combining characters is approximated.
- `resolved_config_path` is evaluated once per frame for the details pane (same as before).
- With very small terminals (fewer than about 12 rows), the popup clips its bottom sections. No hit rect is created for a row outside the popup body.
- hu-HU now has about 200 more `# TODO: translate` placeholders.

## Integration notes

- Nothing outside the listed paths changed. The plan file shows an unstaged modification made by the parent, not by WS2.
- `render_config_editor_window` and `keybind_footer_top` keep their signatures, so `src/ui.rs` needs no change.
- The renderer clamps `popup.scroll` to the real dropdown height every frame. The event handler's fixed 8-row assumption therefore only affects PgUp/PgDn step size.
- If a later change adds a new enum with descriptions, add it to `DESCRIBED_ENUMS` in the tests so locale parity covers it.
