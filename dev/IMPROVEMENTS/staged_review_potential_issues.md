# Staged Changes Review: Potential Issues & Problems

**Date:** 2026-04-03
**Branch:** `feat/custom-repos`
**Scope:** Custom repository management via `repos.conf` (~4,300 lines across 47 files)

---

## 1. Security Concerns

### 1.1 Path traversal in `is_safe_abs_path` is too permissive

**File:** `src/logic/repos/apply_plan.rs`
**Severity:** High

The path validator allows `.` characters, which means `..` path traversal sequences like `/etc/pacman.d/../../tmp/evil` pass validation. The check should reject paths containing `..` segments explicitly.

```rust
fn is_safe_abs_path(p: &str) -> bool {
    p.starts_with('/')
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '-' | '.' | '_'))
}
```

**Fix:** Add `&& !p.contains("..")` or validate resolved canonical path.

### 1.2 `append_managed_include_command` — main_path not shell-quoted in `>>`

**File:** `src/logic/repos/apply_plan.rs`
**Severity:** Medium

In the `printf ... >> {main_path}` command, `main_path` is embedded unquoted in the shell string. While `is_safe_abs_path` restricts the character set, the path should still be `shell_single_quote`d for defense in depth, consistent with how other variable fragments are treated.

### 1.3 `SigLevel = Optional TrustAll` default is insecure

**File:** `src/logic/repos/apply_plan.rs` — `render_dropin_body`
**Severity:** Medium

When a `[[repo]]` row omits `sig_level`, the drop-in defaults to `SigLevel = Optional TrustAll`, which disables signature verification entirely. This silently trusts unsigned packages from third-party repos. A safer default would be `Required DatabaseOptional` (the standard for custom repos like Chaotic-AUR, EndeavourOS, etc.), or at minimum the user should be warned.

### 1.4 `CARGO_MANIFEST_DIR` baked into binary at compile time

**File:** `src/events/modals/repositories.rs` — `open_repos_conf_example_in_editor`
**Severity:** Low

```rust
let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/examples/repos.conf.example");
```

This embeds the build machine's absolute path into the binary. For installed/packaged builds, this path will not exist. The fallback toast already handles this, but users on installed builds will always see "example not found" with no alternative.

---

## 2. Correctness Issues

### 2.1 `DefaultHasher` is not stable across Rust versions

**File:** `src/logic/repos/apply_plan.rs` — `mirror_url_dest_path`
**Severity:** Medium

The mirrorlist destination path includes a hash suffix from `DefaultHasher`. Rust explicitly does not guarantee `DefaultHasher` stability across compiler versions. This means upgrading rustc could change the hash, leaving orphaned mirrorlist files at the old path and generating a new file at a different path. The drop-in would reference the new path, but the old file would persist.

**Fix:** Use a fixed hash algorithm (e.g., FNV, CRC32, or a simple custom hash).

### 2.2 Options menu index shift may break existing keybind muscle-memory

**Files:** `src/events/global.rs`, `src/events/mouse/menus.rs`
**Severity:** Low

The "Repositories" entry is inserted at index 3 in package mode and index 2 in news mode, pushing "News management" / "Package mode" toggle down by one position. Users relying on numeric muscle memory for menu navigation will hit the wrong option.

### 2.3 `scroll` field uses `u16` but `selected` uses `usize`

**File:** `src/state/modal.rs` — `Modal::Repositories`
**Severity:** Low

Mixing `u16` for scroll and `usize` for selected creates friction throughout the codebase with frequent `u16::try_from()` / `.unwrap_or()` calls. For lists with >65535 repos this would break, though practically this is unlikely. It does add noise and potential for off-by-one bugs in the many conversion sites.

### 2.4 Enter key handled in two places for Repositories modal

**Files:** `src/events/modals/handlers.rs` (line ~208), `src/events/modals/repositories.rs` (line ~468)
**Severity:** Low

`handle_repositories_modal` checks for Enter before delegating to `handle_repositories_modal_keys`, which also has its own `KeyCode::Enter` arm that returns `Some(false)`. The outer handler catches Enter first, so the inner match arm is dead code that could confuse future maintainers.

---

## 3. Robustness Concerns

### 3.1 No check for Linux platform in keyboard-triggered `handle_options_repositories`

**File:** `src/events/global.rs` — `handle_options_repositories`
**Severity:** Medium

The keyboard-triggered path (`handle_options_repositories` in `global.rs`) does NOT have the `#[cfg(target_os = "linux")]` / unsupported-platform guard that the mouse-triggered path (`handle_repositories_option` in `menus.rs`) has. On non-Linux platforms, this will attempt to scan `/etc/pacman.conf`, fail silently or show broken state.

### 3.2 `pending_repo_apply_commands` not cleared on all error paths

**File:** `src/events/modals/repositories.rs` — `queue_repo_apply_execution`
**Severity:** Low

When the interactive auth handoff fails (`Ok(false)` or `Err`), `pending_repo_apply_summary` is cleared but `pending_repo_apply_commands` is not explicitly cleared (it was already moved into `cmds`). However, in the password prompt flow, if the user cancels the password dialog, `pending_repo_apply_commands` may linger in `AppState` until the next apply attempt. This is mostly harmless but could leak stale commands.

### 3.3 `PACSEA_TEST_OUT` env var short-circuits without privilege escalation

**File:** `src/events/modals/repositories.rs` — `queue_repo_apply_execution`
**Severity:** Low

When `PACSEA_TEST_OUT` is set, repo apply commands are spawned directly in a terminal without any password/auth flow. If a user accidentally sets this env var in production, privileged commands would be executed without confirmation.

---

## 4. Locale / i18n Issues

### 4.1 Hungarian locale has ~30 untranslated English placeholders

**File:** `config/locales/hu-HU.yml`
**Severity:** Medium

The entire repositories modal section, custom repo filter labels, password prompt heading, and help modal lines are English with `# TODO: translate to hungarian` comments. Hungarian users will see mixed-language UI.

### 4.2 German locale has no missing translations

**File:** `config/locales/de-DE.yml`
**Severity:** None

All new keys appear fully translated.

---

## 5. Architecture / Design Concerns

### 5.1 Modal state cloned on every key event

**File:** `src/events/modals/handlers.rs` — `handle_repositories_modal`
**Severity:** Low (performance)

The `restore_if_not_closed_with_option_result` pattern clones the entire `Modal::Repositories` (including `Vec<RepositoryModalRow>` and `Vec<String>`) on every key press to restore it after the event handler. For large repo lists this creates unnecessary allocations.

### 5.2 Config menu width may not account for the new "Repositories" entry

**File:** `src/ui/results/dropdowns.rs`
**Severity:** Low

The config menu calculates `widest` from option labels. The new "Repositories -> repos.conf" string is the longest entry and should be handled, but since `widest` is dynamically computed this works. However, the menu positioning may overflow on very narrow terminals since no max-width clamping is applied to the config menu (unlike options menu).

### 5.3 `toml` crate added as full dependency

**File:** `Cargo.toml`
**Severity:** Low

The `toml = "1.1.2"` dependency is added for parsing `repos.conf`. This pulls in `serde`, `toml_edit`, etc. The project already uses `serde` so the incremental cost is mainly `toml`/`toml_edit`, but worth noting for binary size consideration.

### 5.4 Implementation plan committed to repo

**File:** `dev/IMPROVEMENTS/IMPLEMENTATION_PLAN_custom_repos.md`
**Severity:** Low

A 275-line implementation plan is staged for commit. Depending on project conventions, design documents may not belong in the source tree. This is purely a project hygiene concern.

---

## 6. Missing Functionality / Edge Cases

### 6.1 No repos.conf file watcher / auto-reload

After the user edits `repos.conf` via the `o` shortcut and returns to the TUI, the Repositories modal still shows stale data. The user must close and reopen the modal to see changes.

### 6.2 No validation that `server` URLs use safe schemes

**File:** `src/logic/repos/apply_plan.rs`

While `mirrorlist_url` is validated for `http://`/`https://`, the `server` field is not validated at all. A malicious or malformed `server = "file:///etc/shadow"` would be written directly into the pacman drop-in. Pacman itself would likely reject it, but it still gets written to `/etc/pacman.d/pacsea-repos.conf`.

### 6.3 No rollback mechanism if apply partially fails

If the apply bundle's chained commands fail midway (e.g., key import succeeds but drop-in write fails), there is no rollback. The system is left in a partially-applied state. The summary lines hint at what was done, but there's no undo flow.

### 6.4 Filter dropdown sort order uses `String::sort` (lexicographic)

**File:** `src/events/mouse/filters.rs`, `src/ui/results/dropdowns.rs`

Dynamic filter keys are sorted lexicographically. If users expect display order to match `repos.conf` file order, this could be confusing.

---

## Summary

| Category | Critical | Medium | Low |
|----------|----------|--------|-----|
| Security | 1 (path traversal) | 2 (unquoted path, insecure SigLevel default) | 1 (CARGO_MANIFEST_DIR) |
| Correctness | 0 | 1 (unstable hash) | 3 |
| Robustness | 0 | 1 (missing platform guard) | 2 |
| i18n | 0 | 1 (Hungarian) | 0 |
| Architecture | 0 | 0 | 4 |
| Missing features | 0 | 0 | 4 |

**Highest priority fixes before merge:**
1. Add `..` traversal check to `is_safe_abs_path`
2. Add `#[cfg(target_os = "linux")]` guard to `handle_options_repositories` in `global.rs`
3. Change default `SigLevel` from `Optional TrustAll` to `Required DatabaseOptional`
4. Replace `DefaultHasher` with a stable hash for mirrorlist paths
