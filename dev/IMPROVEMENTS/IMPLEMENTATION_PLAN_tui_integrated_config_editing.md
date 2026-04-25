# Implementation Plan: Integrated Config Editing in the TUI

**Created:** 2026-04-03  
**Status:** Planning (potential design — not scheduled)  
**Scope:** Let users view and change Pacsea configuration from inside the terminal UI without leaving the app.

## Goals

- Reduce friction for common tweaks (toggles, enums, paths, sort mode) that today require an external editor.
- Treat the integrated editor as a safer, simpler front-end over the existing config files: it edits `*.conf` files in the background, but users interact with typed rows and focused popups instead of raw text.
- Keep behavior aligned with existing config layout: `settings.conf`, `keybinds.conf`, `theme.conf`, plus legacy `pacsea.conf` resolution (`src/theme/paths.rs`).
- Preserve user comments and unknown keys when rewriting files (match the line-oriented strategy in `src/theme/config/settings_save.rs`).
- Respect **dry-run**: do not write config files when the app is in dry-run mode; surface a clear message instead (same principle as other mutating paths).

## Non-goals (initially)

- Full in-TUI clone of Vim/Helix for arbitrary free-form editing of huge custom snippets (optional later phase).
- Remote or sync’d config; single-machine files only.
- Changing the on-disk key=value grammar or splitting files further without a dedicated migration story.

## Current state (relevant code)

| Area | Location | Notes |
|------|----------|--------|
| Settings path resolution | `src/theme/paths.rs` | `resolve_settings_config_path`, `resolve_keybinds_config_path`, `resolve_theme_config_path`; legacy `pacsea.conf` fallbacks. Settings/keybind resolvers are currently `pub(super)`, so editor-facing code needs a public wrapper or must live inside `theme`. |
| Line-preserving updates | `src/theme/config/settings_save.rs` | `save_boolean_key*`, `save_string_key`, `save_sort_mode`, etc.; bootstraps from `SETTINGS_SKELETON_CONTENT` when missing. Current helpers are fire-and-forget: no `Result`, no dry-run gate, and no atomic write contract. |
| Skeletons / defaults | `src/theme/config/skeletons.rs` | Shipped examples and seed content. |
| Keybind parsing | `src/theme/settings/parse_keybinds.rs` | Populates `Settings.keymap`; supports dedicated `keybinds.conf` and legacy lines in `settings.conf`. There is no serializer or editable keybind schema yet, and some actions intentionally allow multiple chords. |
| Theme load / diagnostics | `src/theme/config/theme_loader.rs` | `try_load_theme_with_diagnostics`; required canonical keys in `THEME_REQUIRED_CANONICAL`. Validation currently reads from a path, so pre-commit validation needs a temp-file or in-memory equivalent. |
| Runtime settings model | `src/theme/types.rs` | `Settings` and nested keymap types. |

Today, some UI actions already persist individual keys (e.g. sort mode, pane visibility). Integrated editing generalizes that into a coherent surface and fills gaps (keybinds, theme, less-common settings).

## UX directions (pick one primary; others can be phases)

### Option A — Dedicated pane-based config editor window (recommended first)

- Add a dedicated config editor window/mode, comparable to package install mode, installed-only package list, and news mode.
- Keep the familiar Pacsea layout, but change the content:
  - **Top pane:** initially lists config files (`settings.conf`, `keybinds.conf`, `theme.conf`, `repos.conf` if included) with short explanations. Selecting/clicking a file changes the top pane to that file’s editable config keys.
  - **Middle pane:** fuzzy search for config keys. While searching, the top pane shows matching config keys instead of config files.
  - **Bottom/details pane:** for the selected key, show the current value and the explanation derived from the corresponding config-file comment/schema documentation.
- Selecting a marked key in the top pane opens a small harmonized edit popup. Most fields use toggles or finite-choice selectors; string/path/API-key fields use explicit text input flows.
- `Ctrl+S` inside the popup saves the value to the relevant config file through the patch layer; Esc cancels without writing.
- **Save** writes only changed keys through a shared `Result`-returning config patch layer, not direct fire-and-forget writes.
- **Apply theme** re-runs theme load; on failure, show diagnostics from `try_load_theme_with_diagnostics` and keep previous in-memory theme.

**Pros:** Safe, discoverable, keyboard-first lists match existing TUI patterns and avoid a raw text editor.  
**Cons:** Every new setting needs a schema row (or a generic fallback).

### Option B — Keybind capture mode

- Focus row → **Record** → next key chord replaces binding; validate against duplicate/conflicting chords across contexts (search vs global, etc.).
- Write `keybinds.conf` lines in canonical form; document that users on legacy `pacsea.conf` may need migration messaging.

**Pros:** High value for Vim-style users.  
**Cons:** Input routing is subtle (must not trigger global actions while capturing); needs explicit “cancel” and conflict UI.

### Option C — Theme editor

- Grid or list of the 16 required canonical colors with hex input or incremental adjust (optional).
- Preview strip using current `Theme` before save; **Save** writes `theme.conf` while preserving unrelated lines if using the same line-patch approach.

**Pros:** Visual feedback loop without restarting the app.  
**Cons:** Hex validation and contrast accessibility; terminal color limits.

### Option D — Raw buffer editor (later / optional)

- Scrollable multiline buffer with the full file content; save replaces file atomically (write temp + rename).

**Pros:** Power users; no schema lag.  
**Cons:** Easy to break syntax; weaker validation; larger test surface.

## Suggested phased rollout

### Phase 0 — Config patch foundation

Before adding the UI, extract a small config-editing foundation so every later tab has the same safety and test story:

1. Add a line-preserving patch API, likely under `src/theme/config/`, that returns `Result<PatchOutcome, ConfigWriteError>` instead of silently swallowing failures.
2. Centralize target resolution + bootstrap:
   - Use existing resolution order for active files, including legacy `pacsea.conf`.
   - Expose only the path helpers needed by the editor, or keep the editor-side IO module inside `theme` so it can use `pub(super)` resolvers.
   - Bootstrap missing or empty files from the matching skeleton.
3. Support dry-run at the patch layer: compute the proposed change, but do not create directories or write files; return a clear `DryRun` outcome for the UI.
4. Use atomic writes for config persistence (`create_new` temp file in the same directory, then rename) where practical, while preserving comments and unknown keys.
5. Define a static editable schema:
   - key name, target file, value kind, aliases, live-apply behavior, sensitivity, and display label.
   - Mark sensitive fields such as `virustotal_api_key` as redacted by default and editable only through an explicit input flow.
6. Add focused tests for comments preserved, aliases migrated, missing files bootstrapped, dry-run skipped writes, write errors returned, and unknown keys left untouched.

**Exit criteria:** Config changes can be represented, patched, and tested without any TUI code.

### Phase 1 — Config editor window + settings file flow

1. Add an app mode/state for the config editor (e.g. `AppMode::ConfigEditor` plus `ConfigEditorState { view, selected_file, query, selected_key, popup }`) wired from a global keybind and/or command palette entry.
2. Reuse the existing three-pane mental model:
   - top pane lists config files, then keys for the selected file or search results;
   - middle pane owns fuzzy key search;
   - bottom/details pane shows the selected key’s current value and explanatory comment/schema text.
3. Implement the `settings.conf` flow first from the schema: booleans, enums, numeric values, and non-sensitive strings only.
4. Add a harmonized edit popup with type-specific controls:
   - toggle/finite choices for booleans and enums;
   - bounded numeric controls for numeric values;
   - explicit text input for string/path values;
   - `Ctrl+S` saves, Esc cancels.
5. Show read-only active config paths (from `resolve_*` helpers) for transparency.
6. Route all saves through the Phase 0 patch API; propagate I/O, validation, and dry-run outcomes to the status line or a small error modal.
7. Apply live changes only where a reload contract exists; otherwise show “takes effect after reload/restart”.

**Exit criteria:** User can change several `settings.conf` fields and see them after restart (or live where the app already hot-reads).

### Phase 2 — Keybinds tab

1. Render keybinds from `Settings.keymap` with human-readable names (reuse help-overlay formatting where possible — `src/theme/types.rs` / help code).
2. Implement capture mode with a dedicated input layer: swallow keys until chord complete or Esc.
3. Add keybind serialization before persistence:
   - Convert captured chords back to canonical config strings.
   - Preserve whether an action is single-binding or multi-binding (`recent_remove`, `install_remove`, etc.).
   - Decide per action whether capture replaces the primary binding or appends another chord.
4. Persist to `keybinds.conf` (create from skeleton if missing); run parser on the result in tests to ensure round-trip.
5. Detect conflicts across relevant contexts and show a confirmation or rejection path instead of silently producing ambiguous shortcuts.

**Exit criteria:** At least global + one pane’s binds editable without corrupting file layout.

### Phase 3 — Theme tab

1. Load current file path from `resolve_theme_config_path`; edit canonical keys only in MVP.
2. Add a theme patch helper that can validate the proposed full content before committing:
   - Prefer an in-memory validation function extracted from `theme_loader`.
   - If keeping path-based validation, write proposed content to a temp file and validate that temp file before renaming over the real config.
3. On save, validate with diagnostics; reject save if errors and leave the existing on-disk config untouched.
4. Optional: “revert to skeleton” with confirmation.

**Exit criteria:** User can fix a broken color and recover without external tools.

### Phase 4 — Polish

- Undo buffer for in-session changes (before save) or “reset row to disk”.
- Export / copy effective config snippet for support.
- Documentation in shipped `config/*.conf` comments only if keys change (per project policy for README/wiki).

## Technical tasks (cross-cutting)

1. **Deduplicate path bootstrap** — `settings_save.rs` repeats resolve + mkdir + skeleton; consider one `fn settings_file_state() -> Result<...>` used by UI and saves.
2. **Result-returning save contract** — New editor writes should not use the existing `save_*` helpers directly until they are refactored or wrapped with explicit errors and dry-run behavior.
3. **Schema source** — Either a static table (`&'static str` key, type enum, aliases, sensitivity, live-apply behavior) or generate from a macro; must stay in sync with parsers in `src/theme/settings/` and tests in `src/theme/config/tests.rs`.
4. **Reload contract** — Define which changes apply immediately vs after restart (document in code comments / rustdoc). Some fields may already live only at startup.
5. **Comment-to-help mapping** — Preserve or extract the nearby comment that explains each key so the bottom pane can show useful help even when the schema is sparse.
6. **Popup consistency** — Use one shared popup model/renderer for config value editing so booleans, enums, strings, numbers, keybinds, and colors feel consistent.
7. **Testing** — Unit tests for patch helpers (comments preserved, aliases migrated, dry-run leaves disk untouched). Integration test: open editor state, fuzzy-search a key, open the popup, save in temp HOME/XDG config using existing `theme::test_mutex` patterns where process-wide env vars are involved.
8. **Accessibility / help** — New keybinds for the editor must appear in the in-app help overlay (`AGENTS.md` UX rule), including `Ctrl+S` save behavior in the popup.
9. **Sensitive values** — Redact API keys/secrets in list views, logs, diagnostics, and tests. Do not display existing secret values unless the user explicitly enters edit mode.

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Capturing keybinds steals global shortcuts | Dedicated sub-state; disable other handlers until capture completes. |
| Invalid theme leaves UI unreadable | Validate before commit; keep last good `Theme` in memory. |
| Concurrent manual edit on disk | Optional: reload-from-disk on open; warn if file mtime changed while editing. |
| Legacy `pacsea.conf` users | Continue using existing resolution order; UI label shows which file is active. |
| Config search and file browsing fight for the top pane | Make editor state explicit: file list view, file key list view, and search results view. Clear query returns to the selected file’s key list. |
| Popup save key conflicts with existing global bindings | While the edit popup is active, route `Ctrl+S`, Esc, arrows, and text input to the popup before global handlers. |

## Open questions

1. Should **theme** and **keybinds** edits apply instantly for the current session without restart? (Requires careful `Settings` / `Theme` mutation and keymap rebinding.)
2. Is a **password** or dangerous-setting gate needed for options that affect privilege escalation (`auth_mode`, etc.)?
3. Do we expose **export** of effective merged config, or only on-disk files?

## Progress checklist (for when work starts)

- [x] Phase 0: Config patch foundation + schema + dry-run/write tests
- [ ] Phase 1: Config editor window + settings file keys + popup save flow
- [ ] Phase 2: Keybind capture + persistence + tests
- [ ] Phase 3: Theme editor + validation + tests
- [ ] Phase 4: Polish (undo row, mtime warning, help overlay updates)

---

*This document is internal planning only; it does not change product behavior until implemented.*
