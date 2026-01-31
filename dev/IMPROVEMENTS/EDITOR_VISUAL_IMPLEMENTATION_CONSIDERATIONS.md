# Implementation Considerations: Use `$EDITOR` / `$VISUAL` Instead of Hard-Coded Editor List

**Reference:** [GitHub Issue #117](https://github.com/Firstp1ck/Pacsea/issues/117) — Use `$EDITOR`/`$VISUAL` instead of a hard-coded list.

**Scope:** This document describes implementation considerations only.

---

## 1. Summary of the Feature Request

Many TUI tools (e.g. `git`, `yazi`, `gitui`) use environment variables to determine the user’s preferred text editor. The proposal is to pick the editor in this order:

1. **`$VISUAL`** — full-screen/visual editor (preferred for interactive use)
2. **`$EDITOR`** — line or visual editor
3. **Current fallback list** — nvim → vim → hx/helix → emacsclient → emacs → nano (unchanged as last resort)

This keeps Pacsea consistent with common Unix conventions and respects the user’s existing editor configuration.

---

## 2. Current Behaviour (Codebase Summary)

### 2.1 Where Config Files Are Opened in an Editor

Config file editing (settings, theme, keybinds) is triggered from three places. In each case, a **single long shell command** is built and executed via `crate::install::spawn_shell_commands_in_terminal(&[editor_cmd])`:

| Location | Function / context | Platform |
|----------|--------------------|----------|
| `src/events/search/normal_mode.rs` | `handle_config_menu_numeric_selection` (keyboard 1/2/3) | Non-Windows only (Windows uses `open_file`) |
| `src/events/global.rs` | `handle_config_menu_selection` (menu index 0/1/2) | Non-Windows only |
| `src/events/mouse/menus.rs` | `handle_config_menu_click` (mouse click on config menu row) | Non-Windows only |

On Windows, the same actions use `crate::util::open_file(&target)`, which uses `Invoke-Item` / `cmd start` (default OS handler). The considerations below focus on the **non-Windows** path where the hard-coded editor chain is used.

### 2.2 Current Editor Command Shape

The editor command is a single shell expression: a chain of `(condition && editor 'path')` joined by `||`, ending with a fallback message and `read -rn1 -s _`. Example (conceptually):

- Try: `(command -v nvim … || pacman -Qi neovim …) && nvim 'path'`
- Else: same for vim, hx, helix, emacsclient, emacs, nano
- Else: `echo 'No terminal editor found (nvim/vim/...).'; echo 'File: path'; read -rn1 -s _ || true`

Each candidate is checked with `command -v <bin>` and/or `sudo pacman -Qi <pkg>`. The **same literal string** (with different `path_str` interpolation) appears in all three call sites; there is no shared helper that returns the editor command string.

### 2.3 How the Command Is Run

- `spawn_shell_commands_in_terminal` writes the command into a **temporary script** and runs it with `bash -lc <script_path>` (or via a preferred terminal emulator that runs the same).
- The child process is created with Rust’s `std::process::Command`, which **inherits the environment** by default. So when the script runs, it sees the same `VISUAL`/`EDITOR` (and `PATH`) as the user’s session, **provided** the terminal that runs the script was started with that environment (e.g. from a login shell or the same desktop session). No Rust-side env stripping was found.

### 2.4 Optional Deps “Editor” Category

- **`src/events/mouse/menu_options.rs`** defines `build_editor_rows`, which builds the optional-deps modal rows for the **“Editor”** category.
- It uses a **fixed list** of candidates: `(nvim, neovim)`, `(vim, vim)`, `(hx, helix)`, `(helix, helix)`, `(emacsclient, emacs)`, `(emacs, emacs)`, `(nano, nano)`.
- Logic: if any candidate is “installed” (PATH or pacman), show that one; otherwise show all unique packages. This is **only for the UI** (suggesting an editor to install). It does **not** drive which editor is used when opening config files; that is entirely the shell chain above.

### 2.5 Other Editor-Related Code

- **`src/util/mod.rs` — `open_file(path)`:** Used on Windows for config files; on Unix it uses `xdg-open` / `open`. It does **not** use `EDITOR`/`VISUAL` or the terminal-editor chain. Out of scope for “open config in terminal editor” but worth a note if we ever want desktop-open to respect editor prefs.
- **Tests:** `src/events/mod.rs` has optional-deps tests that expect an “Editor: nvim” row and that `nvim` is marked installed when a fake `nvim` is on `PATH`. No tests were found that assert the exact shell string used for opening config files.

---

## 3. Design Considerations

### 3.1 Order of Preference: VISUAL vs EDITOR

- **Convention:** `VISUAL` is typically the full-screen editor, `EDITOR` the line editor. Many tools (e.g. `git`) use `VISUAL` when available, then `EDITOR`. The issue explicitly asks for that order: **VISUAL → EDITOR → fallbacks**.
- **Implication:** The shell snippet (or any Rust-side logic) must try `$VISUAL` first, then `$EDITOR`, then the existing nvim/vim/… chain.

### 3.2 Where to Implement the Logic

**Option A — In the shell command (recommended):**

- Prepend to the existing chain: e.g. “if `$VISUAL` is set and executable, run it with the path; else if `$EDITOR` is set and executable, run it; else current chain.”
- **Pros:** No Rust env handling, no new dependencies; behaviour is consistent with the environment the script runs in; single source of “editor selection” in the script.
- **Cons:** Shell string becomes slightly more complex; must be careful with quoting (path, and possibly `$VISUAL`/`$EDITOR` if they contain arguments).

**Option B — In Rust:**

- Read `std::env::var("VISUAL")` and `std::env::var("EDITOR")` at runtime, validate (e.g. first word is an executable), then build a shell command that runs that executable with the path.
- **Pros:** Full control and testability in Rust; can normalize and log.
- **Cons:** Must define “executable” (PATH at process start? at spawn time?); env in the Rust process might differ from the terminal’s (e.g. if Pacsea is launched from a different env); need to safely pass possibly multi-word `VISUAL`/`EDITOR` into the shell.

**Recommendation:** Prefer **Option A** (shell-only) for consistency with how the rest of the editor chain is implemented and to avoid env differences between Pacsea and the terminal. If we later add a Rust helper that **generates** the shell string (see “DRY” below), that helper can still embed a shell fragment that uses `$VISUAL` and `$EDITOR` first.

### 3.3 Handling `VISUAL` / `EDITOR` Content

- Values may be:
  - A single binary name: `nvim`
  - A path: `/usr/bin/nvim`
  - A command with options: `nvim -f`, `emacsclient -t`, `code --wait`
- The script must **pass the config file path** to the editor. Convention is to append the path as an argument. So the shell must run something like `$VISUAL '$path'` or `$EDITOR '$path'` in a way that:
  - Preserves path with spaces/special chars (single-quote the path).
  - Does not break if `VISUAL`/`EDITOR` contain options (e.g. run with `eval` or a wrapper that appends the path). Many tools use `eval` or a small wrapper; care is needed with quoting.
- **Security:** Avoid `eval` on unvalidated user input if possible; if we use `eval`, the only user-controlled part should be the path (and we control path). `VISUAL`/`EDITOR` are already user-controlled in the same way as on other tools.

### 3.4 “Executable” Check for VISUAL/EDITOR

- For the **current** fallbacks we check `command -v <bin>` and/or `pacman -Qi <pkg>`. For `$VISUAL`/`$EDITOR` we only have a string; there may be no package name.
- Possible approaches in shell:
  - Use the first word of `$VISUAL`/`$EDITOR` and check `command -v <first_word>`; if present, run the full value with the path appended.
  - Or: try running `$VISUAL '$path'` (and then `$EDITOR '$path'`) and fall back to the next option on failure (e.g. `(...) || (...)` chain). That matches “try and see” behaviour of many programs.
- Document in the script or in comments that we expect `VISUAL`/`EDITOR` to be a runnable command that accepts a file path.

### 3.5 Fallback Message and i18n

- Current fallback message is a fixed English string: `'No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano).'`. If we add `$VISUAL`/`$EDITOR` to the chain, we might:
  - Keep the message as-is and optionally add “or set VISUAL/EDITOR” in the message or in docs.
  - Or add an i18n key (e.g. under `app.toasts` or a dedicated key) and have the Rust side pass the translated message into the script (script would receive it as a single argument or env var). That would require a small Rust change (one string) and possibly escaping for shell.

### 3.6 DRY: Single Place for the Editor Command

- The **same** long editor command string is duplicated in three files: `normal_mode.rs`, `global.rs`, `menus.rs`. Any change (including adding VISUAL/EDITOR) should be done in **one** place to avoid drift.
- **Suggestion:** Introduce a small helper (e.g. in `crate::util` or `crate::install`) such as `fn editor_open_config_command(path: &Path) -> String` that returns the full shell command string. All three call sites then call this and pass the result to `spawn_shell_commands_in_terminal`. The helper can take an optional “fallback message” or “locale” if we add i18n later.
- **Location:** `src/util/mod.rs` already has `open_file`; a sibling `editor_open_config_command` (or a dedicated small module under `util` or `install`) keeps “how we open config in an editor” in one place. Alternatively, under `install` next to `spawn_shell_commands_in_terminal` if we consider this “install/run” behaviour.

### 3.7 Optional Deps Modal (“Editor” Rows)

- The optional deps “Editor” list is for **suggesting** an editor to install, not for choosing the running editor. It can stay as-is (fixed list) so the modal continues to suggest common terminal editors.
- **Optional enhancement:** If we want the modal to reflect the user’s choice, we could add a row like “Editor: $VISUAL” or “Editor: $EDITOR” when set, and/or show “Using: $VISUAL” in the UI. This is optional and not required by issue #117.

### 3.8 Windows and `open_file`

- On Windows, config open uses `open_file`, which uses the OS default handler. Issue #117 does not mention Windows; behaviour can remain unchanged unless we explicitly extend “editor” to mean “honour EDITOR/VISUAL on Windows” (e.g. via WSL or a separate setting). Left as future work.

### 3.9 Dry-Run and Tests

- **Dry-run:** Config editing spawns a real terminal/editor; there is no `--dry-run` branch that skips spawning. If we add a dry-run mode for “open config,” we’d only log or substitute the command. No change required for the VISUAL/EDITOR feature itself.
- **Tests:** Current tests that spawn the editor are no-ops unless `PACSEA_TEST_OUT` is set. To test “editor selection” we could:
  - Unit-test a **Rust** helper that builds the command string (e.g. assert it contains `$VISUAL` and `$EDITOR` in the right order when we add that).
  - Integration test with a fake terminal and env (e.g. set `VISUAL=echo`, run the script, check log or output). This would require the script to be deterministic and the test to set env before calling the code that builds the command.

### 3.10 Backward Compatibility and Documentation

- **Behaviour:** If neither `VISUAL` nor `EDITOR` is set, behaviour should match current: use the nvim → … → nano chain. So existing users are unchanged.
- **Docs:** Consider a short note in the repo (e.g. in a “Config” or “Environment” section) that Pacsea uses `VISUAL`, then `EDITOR`, then a built-in list when opening config files. No need to change wiki/README unless explicitly requested.

---

## 4. Files and Call Sites to Touch (Checklist)

When implementing, the following should be kept in mind:

| Item | Location | Note |
|------|----------|------|
| Editor command string (add VISUAL/EDITOR + DRY) | New or existing helper used by all three below | Single source of truth for the shell command |
| Config menu (keyboard 1/2/3) | `src/events/search/normal_mode.rs` — `handle_config_menu_numeric_selection` | Replace inline `editor_cmd` with call to helper |
| Config menu (global menu index) | `src/events/global.rs` — `handle_config_menu_selection` | Same |
| Config menu (mouse click) | `src/events/mouse/menus.rs` — `handle_config_menu_click` | Same |
| Fallback message / i18n (optional) | Same helper or call sites | If we add a key for “no editor found” |
| Unit tests for command builder | e.g. next to the helper or in `util`/`install` tests | Assert order: VISUAL → EDITOR → fallbacks |
| Optional deps “Editor” (optional) | `src/events/mouse/menu_options.rs` — `build_editor_rows` | Only if we want to show VISUAL/EDITOR in the modal |

---

## 5. Example Shell Snippet (Reference Only)

The following is a **sketch** of how the start of the editor chain could look in shell. It is for discussion only; actual implementation must match project style and quoting rules.

```sh
# Prefer VISUAL, then EDITOR, then existing chain.
( [ -n "${VISUAL}" ] && command -v "${VISUAL%% *}" >/dev/null 2>&1 && eval "${VISUAL}" '"'"'path_here'"'"' ) || \
( [ -n "${EDITOR}" ] && command -v "${EDITOR%% *}" >/dev/null 2>&1 && eval "${EDITOR}" '"'"'path_here'"'"' ) || \
( (command -v nvim ...) && nvim 'path_here' ) || ...
```

Notes:

- `"${VISUAL%% *}"` takes the first word (binary); `command -v` checks it. We then run the full `VISUAL`/`EDITOR` with the path appended.
- Quoting of `path_here` must be done so that the path is one argument and spaces/special chars are safe. The snippet uses single-quoted path; Rust would inject the path via a variable or replace `path_here` with a properly quoted value.
- `eval` is one option to support `VISUAL="nvim -f"`; alternatives are a small wrapper or documenting that we only support a single binary (simpler but less flexible).

---

## 6. Summary of Recommendations

1. **Order:** Use `$VISUAL` first, then `$EDITOR`, then the existing nvim → … → nano fallback list.
2. **Implement in shell:** Add VISUAL/EDITOR at the beginning of the existing shell chain; avoid relying on Rust env if possible so the terminal’s environment is the source of truth.
3. **DRY:** Introduce one helper (e.g. `editor_open_config_command(path)`) and use it from `normal_mode.rs`, `global.rs`, and `menus.rs`.
4. **Quoting and safety:** Ensure the config file path is passed as a single, safely quoted argument; handle `VISUAL`/`EDITOR` containing options (e.g. via `eval` or documented convention).
5. **Optional deps:** Leave “Editor” rows as-is unless we explicitly want to show VISUAL/EDITOR in the modal.
6. **Tests:** Add unit tests for the command string (order of VISUAL/EDITOR/fallbacks); consider integration test with fake env if useful.
7. **Docs:** Optionally document in the repo that Pacsea respects VISUAL and EDITOR when opening config files.

This document does not prescribe exact code changes; it is intended as a reference for whoever implements the feature.
