# Implementation Plan: Integrated Config Editing in the TUI

**Created:** 2026-04-03  
**Status:** Planning (potential design — not scheduled)  
**Scope:** Let users view and change Pacsea configuration from inside the terminal UI without leaving the app.

## Goals

- Reduce friction for common tweaks (toggles, enums, paths, sort mode) that today require an external editor.
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
| Settings path resolution | `src/theme/paths.rs` | `resolve_settings_config_path`, `resolve_keybinds_config_path`, `resolve_theme_config_path`; legacy `pacsea.conf` fallbacks. |
| Line-preserving updates | `src/theme/config/settings_save.rs` | `save_boolean_key*`, `save_string_key`, `save_sort_mode`, etc.; bootstraps from `SETTINGS_SKELETON_CONTENT` when missing. |
| Skeletons / defaults | `src/theme/config/skeletons.rs` | Shipped examples and seed content. |
| Keybind parsing | `src/theme/settings/parse_keybinds.rs` | Populates `Settings.keymap`; supports dedicated `keybinds.conf` and legacy lines in `settings.conf`. |
| Theme load / diagnostics | `src/theme/config/theme_loader.rs` | `try_load_theme_with_diagnostics`; required canonical keys in `THEME_REQUIRED_CANONICAL`. |
| Runtime settings model | `src/theme/types.rs` | `Settings` and nested keymap types. |

Today, some UI actions already persist individual keys (e.g. sort mode, pane visibility). Integrated editing generalizes that into a coherent surface and fills gaps (keybinds, theme, less-common settings).

## UX directions (pick one primary; others can be phases)

### Option A — Structured “settings center” (recommended first)

- Full-screen or large modal with **tabs** or **sections**: General · Keybinds · Theme · Advanced.
- Each row is a known key: label, current value, type-appropriate control (toggle, single-line text, enum cycle, numeric where applicable).
- **Save** writes only changed keys using the same line-replacement helpers (extend `settings_save.rs` or extract a small `config::patch` module to avoid duplication).
- **Apply theme** re-runs theme load; on failure, show diagnostics from `try_load_theme_with_diagnostics` and keep previous in-memory theme.

**Pros:** Safe, discoverable, keyboard-first lists match existing TUI patterns.  
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

### Phase 1 — Shell + settings tab

1. Add an app mode or modal state (e.g. `ConfigEditor { section, focus, dirty }`) wired from a global keybind and/or command palette entry.
2. Implement **General** section: boolean and string keys already covered by `save_*` helpers; add any missing high-traffic keys with the same patch semantics.
3. Show **read-only** paths to active config files (from `resolve_*` helpers) for transparency.
4. Gate all writes on `!dry_run` and propagate I/O errors to the status line or a small error modal.

**Exit criteria:** User can change several `settings.conf` fields and see them after restart (or live where the app already hot-reads).

### Phase 2 — Keybinds tab

1. Render keybinds from `Settings.keymap` with human-readable names (reuse help-overlay formatting where possible — `src/theme/types.rs` / help code).
2. Implement capture mode with a dedicated input layer: swallow keys until chord complete or Esc.
3. Persist to `keybinds.conf` (create from skeleton if missing); run parser on the result in tests to ensure round-trip.

**Exit criteria:** At least global + one pane’s binds editable without corrupting file layout.

### Phase 3 — Theme tab

1. Load current file path from `resolve_theme_config_path`; edit canonical keys only in MVP.
2. On save, validate with `try_load_theme_with_diagnostics`; reject save if errors (show list).
3. Optional: “revert to skeleton” with confirmation.

**Exit criteria:** User can fix a broken color and recover without external tools.

### Phase 4 — Polish

- Undo buffer for in-session changes (before save) or “reset row to disk”.
- Export / copy effective config snippet for support.
- Documentation in shipped `config/*.conf` comments only if keys change (per project policy for README/wiki).

## Technical tasks (cross-cutting)

1. **Deduplicate path bootstrap** — `settings_save.rs` repeats resolve + mkdir + skeleton; consider one `fn settings_file_state() -> Result<...>` used by UI and saves.
2. **Schema source** — Either a static table (`&'static str` key, type enum, aliases) or generate from a macro; must stay in sync with parsers in `src/theme/settings/` and tests in `src/theme/config/tests.rs`.
3. **Reload contract** — Define which changes apply immediately vs after restart (document in code comments / rustdoc). Some fields may already live only at startup.
4. **Testing** — Unit tests for patch helpers (comments preserved, aliases migrated). Integration test: open editor state, simulate save in temp `PACSEA_*` or existing test harness patterns from `tests/`.
5. **Accessibility / help** — New keybinds for the editor must appear in the in-app help overlay (`AGENTS.md` UX rule).

## Risks and mitigations

| Risk | Mitigation |
|------|------------|
| Capturing keybinds steals global shortcuts | Dedicated sub-state; disable other handlers until capture completes. |
| Invalid theme leaves UI unreadable | Validate before commit; keep last good `Theme` in memory. |
| Concurrent manual edit on disk | Optional: reload-from-disk on open; warn if file mtime changed while editing. |
| Legacy `pacsea.conf` users | Continue using existing resolution order; UI label shows which file is active. |

## Open questions

1. Should **theme** and **keybinds** edits apply instantly for the current session without restart? (Requires careful `Settings` / `Theme` mutation and keymap rebinding.)
2. Is a **password** or dangerous-setting gate needed for options that affect privilege escalation (`auth_mode`, etc.)?
3. Do we expose **export** of effective merged config, or only on-disk files?

## Progress checklist (for when work starts)

- [ ] Phase 1: Settings center shell + general keys + dry-run + tests
- [ ] Phase 2: Keybind capture + persistence + tests
- [ ] Phase 3: Theme editor + validation + tests
- [ ] Phase 4: Polish (undo row, mtime warning, help overlay updates)

---

*This document is internal planning only; it does not change product behavior until implemented.*
