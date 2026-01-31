# PR Review: feat/passwordless-sudo (Passwordless Sudo)

**Branch:** `feat/passwordless-sudo`  
**PR file:** `PR_feat-passwordless-sudo.md`  
**Implementation plan:** `dev/IMPROVEMENTS/PASSWORDLESS_SUDO_IMPLEMENTATION_PLAN.md`

This document summarizes the review of the passwordless sudo pull request, including security considerations, alignment with project standards (CONTRIBUTING.md, AGENTS.md), and concrete suggestions for improvements. **No code changes were made; this is review-only.**

---

## 1. Summary of changes reviewed

- **`src/logic/password.rs`** — `check_passwordless_sudo_available()`, `should_use_passwordless_sudo()`, password validation; test env var handling.
- **`src/theme/types.rs`** — `Settings.use_passwordless_sudo`, `Default`.
- **`src/theme/settings/parse_settings.rs`** — Parsing of `use_passwordless_sudo` (and aliases).
- **`config/settings.conf`** — New option and comments.
- **`src/install/direct.rs`** — Install/remove flows; when to show `PasswordPrompt` vs proceed with `None` password.
- **`src/args/update.rs`** — CLI update path and its password/passwordless handling.
- **`tests/passwordless_sudo/`** — Helpers, integration tests (install, update, remove, downgrade, filesync).
- **`dev/PR/PR_feat-passwordless-sudo.md`** — PR description and checklist.

---

## 2. Security review

### 2.1 Critical: Test env var honored in production

**Location:** `src/logic/password.rs` — `check_passwordless_sudo_available()` and `should_use_passwordless_sudo()`.

**Finding:**  
`PACSEA_TEST_SUDO_PASSWORDLESS` is read in **all builds** (including release). If an attacker or script runs the application with `PACSEA_TEST_SUDO_PASSWORDLESS=1`, the app will treat passwordless sudo as enabled even when `use_passwordless_sudo` is `false` in settings. That bypasses the intended opt-in and weakens the safety barrier.

**Suggestion:**  
- Honor `PACSEA_TEST_SUDO_PASSWORDLESS` only when the process is clearly in a test context, for example:
  - Only when another env var set exclusively by the test harness is present (e.g. `PACSEA_INTEGRATION_TEST=1`), or
  - When compiled with a test-only cfg (if you introduce a dedicated test binary or build profile that sets such a cfg).
- In production code paths, ignore `PACSEA_TEST_SUDO_PASSWORDLESS` so that the only way to enable passwordless sudo is via `use_passwordless_sudo` in settings.

### 2.2 Password handling

**Location:** `src/logic/password.rs` — `validate_sudo_password()`.

**Finding:**  
- Password is escaped with `shell_single_quote()` before being passed to the shell command.  
- No password is written to logs (e.g. in `args/update.rs`, the log uses a shape that omits the password).

**Suggestion:**  
- Keep avoiding any logging or tracing of password content.  
- If you add more code paths that build shell commands with the password, consistently use the same escaping helper and avoid building commands via string concatenation with user input.

### 2.3 Remove always requires password

**Location:** `src/install/direct.rs` — `start_integrated_remove_all()`.

**Finding:**  
Remove operations always show `PasswordPrompt`; there is no branch that skips it based on passwordless sudo. This matches the design (remove always requires authentication).

**Suggestion:**  
None; behavior is correct. Optionally add a short comment in code that remove is intentionally excluded from passwordless sudo for safety.

### 2.4 CLI update vs settings

**Location:** `src/args/update.rs` — `prompt_and_validate_password()`.

**Finding:**  
The CLI update path checks passwordless sudo with `sudo -n true` and skips the password prompt if that succeeds. It does **not** read `use_passwordless_sudo` from settings. So:

- **TUI:** Respects `use_passwordless_sudo` (opt-in).
- **CLI update:** Uses passwordless sudo whenever the system allows it, regardless of config.

**Suggestion:**  
- Decide intended behavior: either (a) CLI update should also respect `use_passwordless_sudo` for consistency, or (b) document that CLI update always tries passwordless sudo when available.  
- If (a): load settings (or at least the flag) in the CLI update path and only skip the password prompt when both `sudo -n true` succeeds and `use_passwordless_sudo` is true.

---

## 3. Code quality and project standards

### 3.1 CONTRIBUTING.md / AGENTS.md

- **Rust docs (What, Inputs, Output, Details):** Present and consistent in `password.rs`, `direct.rs`, theme types, and parse_settings.  
- **No `unwrap()` / `expect()` in non-test code:** The reviewed code uses `Result` and `unwrap_or_else` for defaults (e.g. `USER`); no inappropriate unwraps.  
- **Clippy:** Not re-run as part of this review; the PR checklist states it was run.  
- **Tests:** Integration tests in `tests/passwordless_sudo/` cover install (single/multiple, official/AUR), remove (always password), update, downgrade, filesync, and env var behavior.  
- **Complexity:** New functions in `password.rs` and `direct.rs` are short and linear; no obvious cyclomatic/complexity concerns.

### 3.2 Configuration

- **Default:** `use_passwordless_sudo: false` in `Settings::default()` — correct.  
- **Parsing:** `use_passwordless_sudo` and aliases (`passwordless_sudo`, `allow_passwordless_sudo`) in `parse_misc_settings` — consistent.  
- **Example config:** `config/settings.conf` documents the option and safety implications.

### 3.3 Minor documentation improvements

- In `password.rs`, the **Testing** paragraphs for the env var could state explicitly that this override must not be honored in production builds (once you restrict it as in 2.1).  
- In `direct.rs`, the docstrings for install flows already mention passwordless sudo; you could add one sentence that remove is intentionally always gated by password prompt.

---

## 4. Suggestions for improvements (no code changes applied)

### 4.1 Security

| # | Suggestion | Priority |
|---|------------|----------|
| 1 | Restrict `PACSEA_TEST_SUDO_PASSWORDLESS` to test-only contexts (e.g. only when another test-only env var is set) so production builds never honor it. | High |
| 2 | Align CLI update with TUI: either make CLI update respect `use_passwordless_sudo`, or document that CLI update always uses passwordless sudo when available. | Medium |

### 4.2 Robustness and clarity

| # | Suggestion | Priority |
|---|------------|----------|
| 3 | Add a one-line comment in `start_integrated_remove_all` that remove is intentionally never passwordless for safety. | Low |
| 4 | In rustdoc for the test env var, clarify that the override is (or will be) disabled in production. | Low |

### 4.3 Tests

| # | Suggestion | Priority |
|---|------------|----------|
| 5 | Add an integration test that, when `PACSEA_TEST_SUDO_PASSWORDLESS` is unset (or not honored), install path shows password prompt when `use_passwordless_sudo` is false, even if the system has passwordless sudo. | Medium |
| 6 | If CLI update is changed to respect settings, add a test (or doc example) that CLI update still prompts when `use_passwordless_sudo` is false. | Low |

### 4.4 Documentation and PR

| # | Suggestion | Priority |
|---|------------|----------|
| 7 | In the PR description or wiki, briefly document that remove operations always require a password, regardless of `use_passwordless_sudo`. | Low |
| 8 | If you keep the test env var for integration tests, add a note in CONTRIBUTING or the test README that `PACSEA_TEST_SUDO_PASSWORDLESS` is for tests only and must not be set in production. | Low |

---

## 5. Checklist vs review

| PR checklist item | Review note |
|-------------------|------------|
| Code compiles, fmt, clippy, tests | Not re-run; PR states they pass. |
| Complexity checks | New code appears within limits. |
| Rust docs (What, Inputs, Output, Details) | Present on new/updated public API. |
| No `unwrap()`/`expect()` in non-test code | Confirmed in reviewed files. |
| Integration tests | 34 tests; cover main flows and remove-always-password. |
| Config examples updated | `config/settings.conf` updated. |
| Respects `--dry-run` | Not re-verified; PR states yes. |
| Graceful degradation | Passwordless check failures fall back to password prompt. |
| No breaking changes | Opt-in, default off; no breaking change. |

---

## 6. Conclusion

The feature is implemented in line with the plan: opt-in, default off, install/update can use passwordless sudo when enabled and available, remove always requires a password. The main improvement is to ensure the test-only env var cannot affect production behavior. Aligning CLI update with the TUI setting (or documenting the difference) will make behavior clearer and more consistent.

**Recommended before merge:** Address the high-priority item (restrict `PACSEA_TEST_SUDO_PASSWORDLESS` to test context). The rest can be done in this PR or in follow-ups.
