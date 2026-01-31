# PR Review: feat/edit-visual-picker

**Branch:** `feat/edit-visual-picker` → `main`  
**Scope:** Use `$VISUAL` / `$EDITOR` for opening config files (settings, theme, keybinds), with a shared helper and safe path quoting.

---

## 1. Summary of changes

- **Centralized editor command:** New `install::utils::editor_open_config_command(path)` builds one shell snippet that tries VISUAL → EDITOR → nvim → vim → hx → helix → emacsclient → emacs → nano, with a final fallback message.
- **Call sites:** All three entry points (config menu keyboard 1/2/3 in `normal_mode.rs`, global menu selection in `global.rs`, mouse click in `menus.rs`) use this helper on non-Windows; Windows uses `crate::util::open_file(&target)`.
- **Path safety:** Path is passed through `shell_single_quote()` so spaces and single quotes in paths are safe; VISUAL/EDITOR support values with options via `eval` in the shell.
- **normal_mode.rs:** Windows branch added so numeric config selection uses `open_file` on Windows instead of the Unix shell command.
- **Tests:** Unit tests in `src/install/utils.rs` for order (VISUAL → EDITOR → fallbacks), fallback chain content, and path quoting; integration tests in `tests/install/editor_config_integration.rs` with `VISUAL=echo` and `EDITOR=echo`.
- **Docs:** `dev/IMPROVEMENTS/EDITOR_VISUAL_IMPLEMENTATION_CONSIDERATIONS.md` added; PR description in `dev/PR/feat-edit-visual-picker.md`.

---

## 2. Code standards and quality (CONTRIBUTING / AGENTS.md)

### 2.1 Pre-commit checklist

| Check | Status |
|-------|--------|
| `cargo fmt --all` | ✓ (assumed from branch state) |
| `cargo clippy --all-targets --all-features -- -D warnings` | ✓ Clean |
| `cargo check` | ✓ Compiles |
| `cargo test -- --test-threads=1` | ✓ All tests pass |
| Complexity (cyclomatic/data flow < 25) | ✓ New code is simple |

### 2.2 Documentation

- **`editor_open_config_command`:** Rustdoc present with What, Inputs, Output, Details. Good.
- **Unit/integration tests:** Each has a doc comment (What/Inputs/Output/Details). Good.
- **`#[must_use]`** on `editor_open_config_command`: Appropriate for a function that returns a value that must be used.

### 2.3 Testing

- Unit tests cover: order of VISUAL/EDITOR/fallbacks, presence of full fallback chain and message, and shell-single-quoting of path (including path with single quote).
- Integration tests run the built command under `bash -lc` with `VISUAL=echo` and `EDITOR=echo` and assert the path appears on stdout. Deterministic, no real editor.

### 2.4 Conventions

- No `unwrap()` or `expect()` in non-test code. Integration tests use `.expect("bash -lc must run")`, which is acceptable in test code per CONTRIBUTING.
- Error handling: Helper returns `String`; no `Result` needed. Call sites unchanged in error-handling pattern (they already spawned a thread with the command).
- Platform: `#[cfg(not(target_os = "windows"))]` used consistently for the helper and its re-export; Windows path uses `open_file` in all three call sites.

---

## 3. Security

### 3.1 Path injection / shell injection

- **Path:** Always passed through `shell_single_quote()`, so user-controlled or arbitrary paths (e.g. config dir under user control) do not break out of quoting. The unit test `utils_editor_open_config_command_path_is_shell_single_quoted` explicitly checks a path containing a single quote.
- **VISUAL/EDITOR:** Expanded by the shell at runtime; the path is appended as a separate, quoted argument. We do not interpolate the path into the contents of VISUAL/EDITOR. A malicious VISUAL/EDITOR only affects the user’s own session (same as before).
- **Fallback message:** Literal Rust string with no single quotes, used inside `echo '...'` in the snippet. Safe as long as the literal is not changed to include single quotes; the comment in code already states the constraint.

### 3.2 Other

- No new privileged operations; editor is run in the same way as before (terminal, inherited environment).
- No new network or file writes; config paths are existing config file paths.

**Verdict:** No security issues identified; path quoting is correct and VISUAL/EDITOR use is consistent with common practice.

---

## 4. Suggestions for improvement

### 4.1 High priority

None. The change set is consistent, well-tested, and documented.

### 4.2 Medium / low priority

1. **Fallback message in a constant (optional)**  
   The fallback message is an inline literal in `editor_open_config_command`. If you ever need to reuse it (e.g. for i18n or logging), consider a `const` or a small helper. Not required for this PR.

2. **Integration test env isolation**  
   The integration tests set `VISUAL` or `EDITOR` and remove the other, but do not clear other env vars. If the test runner or CI ever sets VISUAL/EDITOR globally, behavior could change. You could use `.env_clear()` and then set only `PATH`, `VISUAL`/`EDITOR`, and any other vars required for `bash -lc` to run, for maximum isolation. Current approach is acceptable if CI and local env are known to be safe.

3. **`echo 'File: {path_quoted}'` in fallback**  
   The final fallback uses `echo 'File: {path_quoted}'`. Because `path_quoted` is already single-quoted, the full argument to `echo` is a single string; that’s correct. No change needed; just a note for future readers.

### 4.3 Branch hygiene

4. **Removed files: `dev/PR/PR_feat-passwordless-sudo*.md`**  
   Compared to `main`, this branch **deletes** `dev/PR/PR_feat-passwordless-sudo.md` and `dev/PR/PR_feat-passwordless-sudo-REVIEW.md`. If that’s intentional (e.g. those PRs are merged and the files were cleaned up), consider a brief note in the PR description. If not, restore them so this PR doesn’t remove unrelated PR artifacts.

---

## 5. File-by-file notes

| File | Notes |
|------|--------|
| `src/install/utils.rs` | New `editor_open_config_command` and unit tests. Doc and quoting logic are clear. |
| `src/install/mod.rs` | Conditional re-export of `editor_open_config_command` for non-Windows only. Correct. |
| `src/events/global.rs` | Replaced inline editor snippet with `editor_open_config_command(&target)`. Windows branch already used `open_file`. |
| `src/events/mouse/menus.rs` | Same as global: one helper call on non-Windows, `open_file` on Windows. |
| `src/events/search/normal_mode.rs` | Windows branch added so numeric config selection uses `open_file` on Windows; non-Windows uses the new helper. Aligns behavior with global and menus. |
| `tests/install.rs` | Registers `editor_config_integration` module. |
| `tests/install/mod.rs` | Adds `mod editor_config_integration`. |
| `tests/install/editor_config_integration.rs` | Two integration tests, both with clear docs and focused assertions. |
| `dev/IMPROVEMENTS/EDITOR_VISUAL_IMPLEMENTATION_CONSIDERATIONS.md` | Design doc; matches implemented behavior (Option A, shell-only). |
| `dev/PR/feat-edit-visual-picker.md` | PR description and checklist are complete. |

---

## 6. Conclusion

- **Merge recommendation:** **Approve**, with optional follow-ups as above.
- **Summary:** The branch achieves VISUAL → EDITOR → fallback order, centralizes the editor command in one helper, and keeps path quoting safe. It matches CONTRIBUTING and AGENTS.md (docs, tests, no unwrap in non-test code, complexity in check). The only actionable item to confirm is the intentional removal of the `PR_feat-passwordless-sudo*.md` files; the rest are optional refinements.
