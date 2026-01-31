<!-- Thank you for contributing to Pacsea!

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary

- **Problem:** Numpad Enter did not submit the password prompt; it could insert a character or do nothing. On some terminals (e.g. Wayland/Alacritty), numpad Enter is reported as `KeyCode::Char('\r')` or `KeyCode::Char('\n')` instead of `KeyCode::Enter`, and only main Enter was handled as submit.
- **Fix:** Treat `KeyCode::Char('\n')` and `KeyCode::Char('\r')` as submit alongside `KeyCode::Enter` in the password prompt and extend the same behaviour for consistency across search, modals (Alert, Help, Confirm, optional deps, import, system update, etc.), recent panel, and install flow.
- **Implementation:** In `src/events/modals/password.rs`, submit arm is `KeyCode::Enter | KeyCode::Char('\n' | '\r')`. Same “Enter = submit” semantics applied in search (normal/insert), modal handlers, recent, and install. PostSummary excluded-keys includes `Char('\n')` and `Char('\r')` for restore logic.
- **Tests:** Unit tests for password handler (numpad Enter as `\r`/`\n`, main Enter, regression, edge cases) and modal numpad Enter tests (Alert, Help, Confirm, PostSummary, GnomeTerminalPrompt, ImportHelp, VirusTotalSetup, News, Scan).

- **Problem (Update system):** When using all selections in the "Update system" modal (mirrors + pacman + AUR + cache), AUR packages were not updated. The AUR command was stored separately and only run when pacman failed (ConfirmAurUpdate); it was never run when pacman succeeded.
- **Fix (Update system):** Store password and header_chips when submitting the Update password so they are available after the first batch. In the tick handler, when `PreflightExec` shows success with empty items (system update) and `pending_aur_update_command` is set, queue the AUR command; AUR then runs automatically after pacman succeeds.
- **Implementation (Update system):** Handlers: set `pending_executor_password` and `pending_exec_header_chips` on Update submit. Tick handler: new `maybe_queue_aur_after_system_update_success()`; call it from `handle_tick`. Executor: uses pending password and header_chips when running the queued AUR command after pacman succeeds.
- **Tests (Update system):** `handle_tick_queues_aur_after_system_update_success` verifies AUR is queued after system update success when pacman + AUR were selected.

## Type of change

- [ ] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [x] test (add/update tests)
- [ ] chore (build/infra/CI)
- [ ] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## Related issues

Closes #119

## How to test

1. Run quality checks:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

2. Manual: trigger a password prompt (e.g. install a package that prompts for sudo password), type password, press **numpad Enter** — password should submit and flow continue (same as main Enter).

3. Manual: in search, modals (confirm, alert, help, etc.), recent panel, and install flow, verify numpad Enter acts as submit/confirm where main Enter does.

## Screenshots / recordings (if UI changes)

N/A — key handling only; no visual change.

## Checklist

**Code Quality:**
- [ ] Code compiles locally (`cargo check`)
- [ ] `cargo fmt --all` ran without changes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test -- --test-threads=1` passes
- [ ] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [ ] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [ ] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [ ] Added or updated tests where it makes sense
- [ ] For bug fixes: created failing tests first, then fixed the issue
- [ ] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed
- [ ] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [ ] Changes respect `--dry-run` flag
- [ ] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] No breaking changes (or clearly documented if intentional)

**Other:**
- [ ] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

- Minimal behavioural change: only additional key codes (`Char('\n')`, `Char('\r')`) are treated as submit; main Enter and existing behaviour unchanged.
- `\n` and `\r` are control characters, so they were already not inserted into the password buffer; this change only makes them trigger submit.
- Consistency pass: same Enter semantics applied in search, modals, recent, and install so numpad Enter behaves uniformly.
- Design/analysis is documented in `dev/IMPROVEMENTS/ISSUE_119_NUMPAD_ENTER_PASSWORD_PROMPT.md`.

## Breaking changes

None.

## Additional context

- Issue: [Firstp1ck/Pacsea#119](https://github.com/Firstp1ck/Pacsea/issues/119)
- Crossterm has no `NumpadEnter`; numpad Enter is reported as `KeyCode::Enter` or `KeyCode::Char('\r')` / `KeyCode::Char('\n')` depending on terminal/OS.
