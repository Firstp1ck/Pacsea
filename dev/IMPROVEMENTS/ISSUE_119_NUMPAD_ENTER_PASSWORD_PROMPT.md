# Issue Fix: Numpad Enter Inserts Character Instead of Submitting Password Prompt

**Reference:** [GitHub Issue #119](https://github.com/Firstp1ck/Pacsea/issues/119) — Pressing Enter from the numpad inserts a character instead of submitting.

**Scope:** This document describes the issue, codebase findings, and implementation considerations to fix it.

---

## 1. Issue Summary

### 1.1 Reported Behaviour

- **Bug:** Using the numpad Enter key does not submit the password prompt; an extra character gets inserted instead of submitting.
- **Steps:** Install a package that prompts for password → enter password → press **numpad Enter**.
- **Expected:** Numpad Enter should submit the password and proceed (same as main Enter).
- **Actual:** An extra character is inserted instead of submitting.

### 1.2 Environment (from issue)

- Pacsea: `0.7.2` (AUR `pacsea-bin`)
- System: Arch Linux, kernel 6.18.6-arch1-1, **Wayland**, terminal **Alacritty** 0.16.1
- AUR helper: paru v2.1.0

---

## 2. Root Cause (Technical)

On many terminals and platforms:

- **Main Enter** is reported by crossterm as `KeyCode::Enter`.
- **Numpad Enter** can be reported as:
  - `KeyCode::Char('\r')` (carriage return), or
  - `KeyCode::Char('\n')` (newline),

depending on terminal and OS. The password prompt handler currently treats **only** `KeyCode::Enter` as “submit”. Any `KeyCode::Char(...)` is passed to the character-handling branch: control characters are not inserted (`char::is_control()`), but the event is still **not** treated as submit, so the user sees no submit. On some setups the terminal may send a different representation that does get inserted, leading to “extra character” behaviour. In all cases, numpad Enter should be treated as submit when it is semantically “Enter”.

---

## 3. Codebase Findings

### 3.1 Password Prompt Key Handling (Primary Fix Location)

| File | Relevant code | Finding |
|------|----------------|--------|
| `src/events/modals/password.rs` | `handle_password_prompt` | **Only** `KeyCode::Enter` is treated as submit (line 30–33). No handling of `KeyCode::Char('\n')` or `KeyCode::Char('\r')`. |
| `src/events/modals/handlers.rs` | `handle_password_prompt_modal` | Delegates to `super::password::handle_password_prompt`; no key normalization. |

So the **only** place that decides “submit on Enter” for the password prompt is the `match ke.code` in `password.rs`. Adding `KeyCode::Char('\n')` and `KeyCode::Char('\r')` there will fix numpad Enter for the password prompt.

### 3.2 Other Enter Handling in the Codebase (Consistency)

These already treat both main Enter and character forms where relevant:

| File | Pattern | Note |
|------|---------|------|
| `src/events/search/normal_mode.rs` | `matches!(ke.code, KeyCode::Char('\n') \| KeyCode::Enter)` | Preflight open: already accepts `\n` and Enter. |
| `src/events/search/insert_mode.rs` | `(KeyCode::Char('\n') \| KeyCode::Enter, m)` | Same for insert-mode Enter. |

These do **not** currently include `KeyCode::Char('\r')`. For consistency and robustness (e.g. other terminals/Wayland), consider treating `\r` as Enter in those paths too, or introducing a shared “is submit/enter key” helper.

Places that **only** check `KeyCode::Enter` (no `Char('\n')` / `Char('\r')`) and might benefit from the same normalization for consistency (not required for issue #119):

- `src/events/modals/handlers.rs` — multiple modal branches (confirm, alert, install, etc.)
- `src/events/modals/common.rs` — various modal key handlers
- `src/events/modals/optional_deps.rs`, `import.rs`, `system_update.rs`
- `src/events/install/mod.rs` — install flow Enter
- `src/events/recent.rs` — recent panel Enter

Auditing these is optional; the **minimal fix** for #119 is the password prompt only.

### 3.3 Crossterm and KeyCode

- **Crate:** `crossterm = "0.29.0"` (from `Cargo.toml`).
- **KeyCode:** Has `Enter` and `Char(char)`; no separate `NumpadEnter`. So numpad Enter, when not reported as `KeyCode::Enter`, will appear as `KeyCode::Char('\r')` or `KeyCode::Char('\n')`.

---

## 4. Implementation Considerations

### 4.1 Minimal Fix (Password Prompt Only)

**File:** `src/events/modals/password.rs`

- In `handle_password_prompt`, treat Enter as submit for:
  - `KeyCode::Enter`
  - `KeyCode::Char('\n')`
  - `KeyCode::Char('\r')`
- **Implementation:** Replace the single arm `KeyCode::Enter => { ... }` with a condition that is true for all three, e.g. add a preceding arm:
  - `KeyCode::Char('\n') | KeyCode::Char('\r') => { true }`  
  or combine with the existing Enter arm, e.g.:
  - `KeyCode::Enter | KeyCode::Char('\n') | KeyCode::Char('\r') => { true }`
- **No** change to `KeyCode::Char(ch)` general branch: `\n` and `\r` are control characters (`ch.is_control()` is true), so they are already not inserted into the buffer. The fix only ensures they trigger submit.

**Testing:**

- Add a unit test in `src/events/modals/password.rs` (or adjacent test module) that:
  - Calls `handle_password_prompt` with `KeyEvent { code: KeyCode::Char('\r'), ... }` and asserts that the return value is `true` (submit).
  - Optionally same for `KeyCode::Char('\n')`.
- Manually: run Pacsea, trigger password prompt, press numpad Enter and confirm password is submitted.

### 4.2 Optional: Shared “Enter = submit” Helper

To avoid duplication and to align behaviour across modals:

- **Place:** e.g. `src/events/global.rs` or a small shared module used by events.
- **Helper:** e.g. `fn is_enter_submit(ke: &KeyEvent) -> bool` returning true for `KeyCode::Enter`, `KeyCode::Char('\n')`, `KeyCode::Char('\r')` (and optionally ignoring or allowing certain modifiers, consistent with existing Enter handling).
- **Use:** In `handle_password_prompt` and, if desired, in other modal/install handlers that currently only check `KeyCode::Enter`.
- **Benefit:** Single place to document and extend “Enter” semantics (e.g. future key codes or modifiers).

### 4.3 Optional: Broader Consistency Pass

- Review all `KeyCode::Enter`-only branches listed in §3.2 and decide whether they should also accept `Char('\n')` and `Char('\r')` for consistency (e.g. confirm dialogs, install flow, recent panel). Not required to close #119 but improves behaviour on terminals that send numpad Enter as `\r`/`\n`.

### 4.4 Edge Cases

- **Ctrl+Enter / modifiers:** Current password handler does not special-case modifiers for Enter. Existing code in `normal_mode.rs` / `insert_mode.rs` ignores Enter when Ctrl is held (to avoid Ctrl+M being treated as Enter). If desired, the same rule can be applied in the password prompt (e.g. only treat as submit when modifiers are empty or only shift). Issue #119 does not mention modifiers; minimal fix can leave modifier behaviour as-is.
- **No double-insert:** `\n` and `\r` are not inserted into the password buffer because of `if !ch.is_control()` in the `KeyCode::Char(ch)` branch. So adding them as submit does not introduce double insertion.

---

## 5. Checklist for Implementing the Fix

1. [ ] In `src/events/modals/password.rs`, treat `KeyCode::Char('\n')` and `KeyCode::Char('\r')` as submit (together with `KeyCode::Enter`).
2. [ ] Add unit test(s) for numpad Enter (e.g. `KeyCode::Char('\r')` and optionally `KeyCode::Char('\n')` returning submit).
3. [ ] Run `cargo fmt`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check`, `cargo test -- --test-threads=1`.
4. [ ] Optionally: introduce `is_enter_submit(ke)` and use it in the password prompt (and elsewhere if desired).
5. [ ] Optionally: extend other Enter-handling paths to accept `\n`/`\r` for consistency.

---

## 6. References

- Issue: [Firstp1ck/Pacsea#119](https://github.com/Firstp1ck/Pacsea/issues/119)
- Crossterm `KeyCode`: e.g. [docs.rs/crossterm/event/enum.KeyCode](https://docs.rs/crossterm/latest/crossterm/event/enum.KeyCode.html)
- Existing pattern in codebase: `src/events/search/normal_mode.rs` and `insert_mode.rs` (Enter or `Char('\n')` for preflight open).
