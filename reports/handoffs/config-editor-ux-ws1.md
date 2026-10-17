# Config editor UX — WS1 handoff

Status: COMPLETE — all required checks pass. Source changes only, not installed. No staging, commits, stash, branch changes, plan edits, or locale edits.

## Changed files

- `src/state/config_editor.rs` → `src/state/config_editor/mod.rs`: shared state, Settings-first tab navigation, cross-file Starred, grouped AND filtering, lazy localized search, popup suggestions/highlight/effective input, cached validation, UTF-8 caret editing; existing re-export paths retained.
- `src/state/config_editor/values.rs`: one captured settings/theme pair, shared settings-to-string mapping, cached codebase defaults, modified state, extra chords, existing color/keybind helpers.
- `src/state/config_editor/groups.rs`: stable file/group sorting, reused keybind scopes, group translation keys.
- `src/state/config_editor/suggestions.rs`: injected suggestion facts, shared terminal candidates, per-kind/per-key presets, token completion data, provenance dedupe, durations, swatches, choice descriptions.
- `src/state/config_editor/validate.rs`: existing schema/semantic/color/chord/conflict validators moved from events; public schema validation API.
- `src/state/config_editor/{tests,ux_tests}.rs`: preserved original tests and added state/search/tab/caret/popup regression coverage.
- `src/state/mod.rs`: additionally re-export `ConfigEditorTab`.
- `src/events/modals/config_editor.rs` → `src/events/modals/config_editor/mod.rs`: new list keys, recents, Esc ladder, opening-time suggestion context and localized search, unchanged configured Pi Scan wizard chord.
- `src/events/modals/config_editor/popup.rs`: Enter/Ctrl+S save effective input, manual-first chords, Ctrl+K recording, reset/reveal, dropdown navigation/completion, bool selection, integer nudges.
- `src/events/modals/config_editor/save.rs`: existing patch/dry-run/theme whole-file validation/stale-file guard/export/reload flow; restart note; preserve editor mode across settings application.
- `src/events/modals/config_editor/mouse.rs`: tab/row/recent/Record/suggestion hit testing and list/popup wheel selection, with mouse regression tests.
- `src/events/modals/config_editor/tests.rs`: preserved validation and capture tests, replaced obsolete file-list tests with navigation/recents/Esc/popup checks.
- `src/events/modals/mod.rs`: mouse handler re-export.
- `src/events/mod.rs`: popup printable-key precedence before global shortcuts, after capture interception.
- `src/events/mouse/mod.rs`: route all editor mouse input before menus without forwarding handled=true as exit=true.
- `src/events/{global.rs,mouse/menus.rs,search/normal_mode.rs}`: pass application translations into `build_config_editor_state(app)`.
- `src/events/mouse/menu_options.rs`: consume the shared terminal candidate list rather than duplicating it.
- `src/theme/config/schema.rs`: `pi_scan_thinking` is the accepted seven-choice enum.
- `src/theme/{resolve.rs,mod.rs}`: expose/re-export the codebase `default_theme()`.
- `src/ui/modals/config_editor.rs`, `src/ui/details/footer.rs`: compile adapters removing FileList and accepting Starred; WS2 replaces rendering/hints.
- `tests/other/config_editor_phase{1,2,3}_integration.rs`: tabs replace file-list navigation; retained file-content/dry-run/conflict/theme rejection/stale-file/export assertions; added workflows below.

## Exact WS2 API

All types/functions below are under `pacsea::state::config_editor` unless otherwise specified. The existing root `state` re-exports remain, with `ConfigEditorTab` added. `ValueSnapshot` fields are private. `suggestions` and `validate` submodules are public; groups and values are consumed through re-exports.

```rust
pub enum ConfigEditorView { KeyList, Starred }
pub enum ConfigEditorTab { Settings, Keybinds, Theme, Starred }
impl ConfigEditorTab { pub const ALL: [Self; 4]; }

impl ConfigEditorState {
    pub const fn active_tab(&self) -> ConfigEditorTab;
    pub fn set_tab(&mut self, tab: ConfigEditorTab);
    pub fn cycle_tab(&mut self, forward: bool);
    pub fn filtered_keys(&self) -> Vec<&'static EditableSetting>;
    pub fn selected_key(&self) -> Option<&'static EditableSetting>;
    pub fn is_bookmarked(&self, key: &str) -> bool;
    pub fn clamp_key_cursor(&mut self);
    pub fn build_search_text(&mut self, app: &AppState);
}
```

`selected_file: ConfigFile`, `view`, `query: String`, and `clamp_key_cursor()` remain compatible with the untouched Pi Scan integration tests. `active_tab()` derives from the view/file; there is no duplicate active-tab field. `set_tab()` preserves query and clamps the existing cursor. Starred retains the last regular file in `selected_file` (used by export), while `selected_key().file` identifies the actual bookmarked entry.

New state fields exactly match the plan:

```rust
pub search_text: HashMap<&'static str, String>,
pub recent_dropdown: Option<usize>,
pub list_page_rows: u16, // default 10; WS2 sets actual selectable rows
pub tab_rects: Vec<(ConfigEditorTab, (u16, u16, u16, u16))>,
pub row_rects: Vec<(usize, (u16, u16, u16, u16))>,
pub popup_choice_rects: Vec<(u16, u16, u16, u16)>,
pub recent_rects: Vec<(usize, (u16, u16, u16, u16))>,
pub record_button_rect: Option<(u16, u16, u16, u16)>,
pub list_rect: Option<(u16, u16, u16, u16)>,
pub popup_rect: Option<(u16, u16, u16, u16)>,
```

Rectangles are `(x,y,width,height)`, half-open. `row_rects` indices address `filtered_keys()` excluding group-heading rows. `popup_choice_rects[i]` addresses `popup.visible[popup.scroll + i]`. `recent_rects` indices address `recent_queries`; these are read only while `recent_dropdown` is open. Clicks select, and clicking an already-selected key opens its popup. Popup clicks highlight without saving.

Legacy state fields (`focus`, `file_cursor`, `search_focus`, `recent_cursor`, `bookmark_cursor` and persistence fields) are retained so the old renderer still compiles; new handlers use `recent_dropdown` and `key_cursor`, not those old side-panel focus controls.

```rust
pub enum SettingGroup {
    General, Layout, Search, Privileges, Mirrors, Updates, Scans, Pkgbuild,
    News, StartupNews, AurVoting, PiScan,
    KeybindGlobal, KeybindSearch, KeybindSearchNormal, KeybindRecent,
    KeybindInstall, KeybindNews,
    Backgrounds, SurfacesOverlays, Text, Accents, Semantic,
}
impl SettingGroup { pub const fn i18n_key(self) -> &'static str; }
pub fn setting_group(entry: &EditableSetting) -> SettingGroup;
```

Group keys are `app.modals.config_editor.groups.` plus respectively `general`, `layout`, `search`, `privileges`, `mirrors`, `updates`, `scans`, `pkgbuild`, `news`, `startup_news`, `aur_voting`, `pi_scan`, `keybind_global`, `keybind_search`, `keybind_search_normal`, `keybind_recent`, `keybind_install`, `keybind_news`, `backgrounds`, `surfaces_overlays`, `text`, `accents`, `semantic`. Sorting is stable by Settings/Keybinds/Theme, then enum group order, preserving schema order inside each group. Starred uses the same order.

```rust
pub struct ValueSnapshot { /* private captured Settings and Theme */ }
impl ValueSnapshot {
    pub fn capture() -> Self;
    pub fn value_for(&self, entry: &EditableSetting) -> String;
    pub fn default_for(&self, entry: &EditableSetting) -> String;
    pub fn is_modified(&self, entry: &EditableSetting) -> bool;
    pub fn extra_chords(&self, entry: &EditableSetting) -> Vec<String>;
}
pub fn current_value_string(entry: &EditableSetting) -> String;
// Also retained: theme_color_value_string, color_to_config_string,
// keybind_chords_for_key, chord_to_canonical_string,
// key_event_to_canonical_chord_string, lookup_setting, bool_to_canonical.
```

Capture one `ValueSnapshot` per render and reuse it for every row/details field. `current_value_string` captures a fresh snapshot and is for one-off callers. Defaults use a `OnceLock`, `Settings::default()` (including `KeyMap::default()`), and `crate::theme::default_theme()`; never effective terminal colors.

```rust
pub enum SuggestionTag {
    Current, Default, AutoDetect, Unlimited, Installed, Detected, Minimum, Maximum,
}
pub struct Suggestion {
    pub value: String,
    pub tags: Vec<SuggestionTag>,
    pub duration_secs: Option<u64>,
    pub swatch: Option<(u8, u8, u8)>,
    pub label_key: Option<String>,
}
pub struct SuggestionContext {
    pub current: String,
    pub default: String,
    pub installed_terminals: Vec<String>,
    pub pi_routes: Vec<(String, String)>,
    pub env_proxy: Option<String>,
    pub palette: Vec<String>,
    pub pi_binary: Option<String>,
}
impl SuggestionContext {
    pub fn capture(app: &AppState, entry: &EditableSetting, values: &ValueSnapshot) -> Self;
}
pub fn suggestions_for(entry: &EditableSetting, ctx: &SuggestionContext) -> Vec<Suggestion>;
// suggestions submodule also exports:
pub fn comma_list(key: &str) -> bool;
pub fn main_pane_choices() -> Vec<String>;
```

SuggestionContext derives `Default` for injection tests. Runtime capture uses existing `install::{command_on_path,resolve_command_on_path}` (filesystem/executable bits), wizard `verified.routes`, `HTTPS_PROXY` then lowercase fallback, and other theme keys from the shared snapshot. No process or network is invoked. Secrets return an empty suggestion list. Current/default comma-list tokens carry their own provenance; values are deduplicated with merged tags.

Enum/bool/pane-order descriptions use `app.modals.config_editor.choices.<key>.<choice>`, with `_empty` for empty choice. `<choice>` is the raw canonical string, including commas/spaces in pane orders. Render descriptions only when translated; missing translations return their identifier. Labels and summaries retain `app.modals.config_editor.settings.<key>.label` and `.summary`.

```rust
pub enum EditPopupKind {
    Bool(bool),
    Enum { choices: Vec<String>, index: usize },
    Int { min: i64, max: i64 },
    Text,
    Secret { revealed: bool },
    KeyChord { capturing: bool },
}
pub struct EditPopupState {
    pub setting: &'static EditableSetting,
    pub kind: EditPopupKind,
    pub buffer: String,
    pub caret: usize, // byte offset, always kept on UTF-8 boundaries by edits
    pub original: String,
    pub suggestions: Vec<Suggestion>,
    pub visible: Vec<usize>, // indices into suggestions
    pub highlight: Option<usize>, // position within visible, NOT suggestion index
    pub scroll: usize,
    pub filter_active: bool,
    pub validation: Result<(), String>,
    pub opened_mtime: Option<std::time::SystemTime>,
    pub stale_overwrite_armed: bool,
}
impl EditPopupState {
    pub fn from_current(setting: &'static EditableSetting, current: &str) -> Self;
    pub fn select_enum_index(&mut self, index: usize) -> bool;
    pub fn canonical_value(&self) -> String;
    pub fn effective_value(&self) -> String;
    pub fn caret_column(&self) -> usize; // Unicode scalar count before caret
    pub const fn is_strict(&self) -> bool;
    pub fn comma_list(&self) -> bool;
    pub fn set_suggestions(&mut self, suggestions: Vec<Suggestion>);
    pub fn refresh_visible(&mut self);
    pub fn refresh_validation(&mut self);
    pub fn select_highlight(&mut self, position: usize) -> bool;
    pub fn move_highlight(&mut self, delta: isize);
    pub fn complete_suggestion(&mut self);
    pub fn edit_buffer(&mut self, code: crossterm::event::KeyCode);
}
pub fn validate_value(setting: &EditableSetting, value: &str) -> Result<(), String>;
```

Use `effective_value()` for the save-preview line, and `validation` directly for live feedback. `canonical_value()` alone excludes free-input suggestion highlighting. `validate_value` checks schema/semantic syntax; cached popup validation also checks current keymap conflicts. Do not validate per frame. Mutating via the popup methods refreshes the cache. `main_pane_order` is `EditPopupKind::Enum` even though its schema retains `ValueKind::MainPaneOrder`; all six choices use `format_main_pane_order`. `pi_scan_thinking` is a schema enum. Strict typing filters but keeps the current canonical choice visible when no match exists, maintaining a visible highlighted effective save value. Free typing clears highlight. Picker navigation and bool toggles synchronize selected kind state.

## Verification

Final required sequence (from this worktree root):

| Command | Exit | Result |
|---|---:|---|
| `cargo fmt --all` | 0 | Formatted |
| `cargo clippy --all-targets --all-features -- -D warnings` | 0 | Clean; no new lint suppressions |
| `cargo check` | 0 | Compiles |
| `cargo test -- --test-threads=1` | 0 | 2,004 passed; 0 failed; 17 ignored across unit, integration, and doc-test suites |

Earlier development runs:

- `cargo test config_editor_popup_printable_precedence -- --test-threads=1`: **101 before fix**, actual buffer empty vs expected `?COP`, proving global shortcuts stole popup characters.
- Initial `cargo check`: **101** for split-module import/mouse reference/new-builder call-site compile errors; corrected.
- Initial `cargo test config_editor -- --test-threads=1`: **101** for a moved `KeyMap` test import; corrected.
- Subsequent focused run: **101** for two new test expectations (partial opening token counted as current; substring `ma` also matches `minimal`); tests adjusted to represent actual editing and an unambiguous `max` filter.
- First Clippy pass: **101** for style, docs, and routing complexity; corrected without new lint allowances by extracting the printable-key predicate.
- Focused integration run: **101** for sequential edits after save; root cause was `apply_settings_to_app_state` applying startup mode. Editor reload now restores its mode.
- Focused `cargo test config_editor -- --test-threads=1`: **0**, 77 matching library tests, 20 config-editor integration tests, plus existing matching install/Pi Scan tests. Final full run additionally includes the last numeric-bounds test.
- Additional `cargo fmt --all`, Clippy, and focused reruns: **0** after fixes.
- `git diff --check`: **0**.
- No validation failures remain; no unrelated failure was suppressed. No dependencies changed, so `cargo audit` was not required.

## Added tests

- Stable file/group/declaration order; AND matching across keys, aliases, localized cache; untranslated-key exclusion; tabs wrapping with preserved query; cross-file Starred.
- Snapshot default/effective equality, modified values, default stability, theme defaults, extra chords.
- Suggestions: installed terminal/default/current dedupe, secret exclusion, countries and ShellCheck token lists, durations, numeric bounds/tags, optional days, color swatches, Pi provider/model/fallback routes, binary path, caps tagged Unlimited, detected proxy, default keybind chord, thinking levels, six pane permutations.
- Strict/free highlight rules, no-match strict fallback, effective save value, token completion, UTF-8 mid-buffer caret movement and deletion.
- Existing validators and scope conflict tests preserved after moving validation; live conflict cache exercised.
- List page movement, Home/End, Ctrl+B, Ctrl+R dropdown selection/dismissal, Esc ladder; popup Enter save, int nudge/clamp, Ctrl+R reveal; existing Ctrl+K/record/cancel/reset cases retained.
- Mouse tab/row/select-again-open/list wheel/recent/suggestion-scroll/Record hit testing; existing mouse router enum test preserved.
- Public integration flows: `?COP` input precedence, tabs/Starred editing, bool picker + Enter save, text suggestion + save, comma completion + save, Record-button capture + Enter save, restart-required status suffix. Existing saved-content, dry-run, conflicts, invalid-theme rejection, stale-overwrite, export-redaction assertions retained.

## Deviations and resolved decisions

- One extra builder call site exists in `src/events/search/normal_mode.rs`; it now passes `app` too. This is necessary for compilation and initializes translations consistently for every entry path.
- Shared terminal candidate tuples moved into `state::config_editor::suggestions::TERMINAL_CANDIDATES` and the optional-dependencies menu consumes them. This touches `events/mouse/menu_options.rs` to fulfill the plan's reuse requirement without maintaining two candidate lists.
- Save reload now preserves editor mode. This was required by the new multi-edit integration flow and fixes an existing startup-mode side effect locally, without editing Pi Scan/application initialization code.
- When a strict filter has zero matches, retain the selected canonical choice as the one visible highlighted fallback. This satisfies the strict-highlight invariant and prevents a hidden effective save choice.
- Extra test modules keep the state implementation separate from regression tests. Event validation tests remain in the event regression module but exercise the moved state validators.

Unresolved decisions: none blocking WS2. Popup page movement and automatic scroll use eight rows; the renderer may adjust `scroll` to fit its actual dropdown height.

## WS2 integration notes and residual risks

- Rendering is intentionally still the previous three-band implementation with compile adapters. No new locale YAML or help-overlay text was changed. Tabs, grouped headers, current/default/modified columns, suggestion dropdowns, live validation, actual caret placement, Record action, and recents dropdown are WS2 work.
- New hit rectangles default empty/None; only the existing enum-choice rectangles are currently populated. WS2 must clear/rebuild every contract rectangle each frame and set `list_page_rows`. Existing enum rendering still assumes the full choices list; replace it with `visible` + `scroll` so filtered picker hit rows align correctly.
- The old footer/search hints still advertise the previous interactions; WS2 owns replacing them. The configured Pi Scan setup chord and its help/footer rendering are preserved and tested.
- Free popups never auto-highlight after typing; showing “Enter saves” via `effective_value()` is essential. Keybinds open in manual text mode; `capturing` starts only through Ctrl+K/Record. Esc first stops capture.
- Search text is built at entry and lazily on the first event if absent. If WS2 changes locale while leaving the editor alive, explicitly rebuild the index.
- Snapshot/validation caching is provided; WS2 must use one snapshot per frame and avoid the one-off current-value wrapper for each row.
- No renderer visual acceptance was performed in WS1. Final layout, localization, and compact-terminal tests belong to WS2.
