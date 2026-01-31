<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
Adds passwordless sudo support for install/update operations. Users can opt-in via `use_passwordless_sudo = true` in settings to skip password prompts when passwordless sudo is configured on their system.

- New config option: `use_passwordless_sudo` (default: `false`)
- Applies to install/update operations only; remove operations always require password
- Includes 35 integration tests with environment variable support for testing
- Test env var `PACSEA_TEST_SUDO_PASSWORDLESS` is honored only when `PACSEA_INTEGRATION_TEST=1` is set (production never honors it)
- CLI update respects `use_passwordless_sudo` from settings (aligned with TUI)

## Type of change
- [x] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [x] test (add/update tests)
- [ ] chore (build/infra/CI)
- [ ] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## Related issues
Closes #115

## How to test

```bash
# Standard checks
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1

# Integration tests (with simulated passwordless sudo; test harness sets PACSEA_INTEGRATION_TEST=1)
cargo test passwordless_sudo -- --test-threads=1
```

**Manual testing:**
1. Set `use_passwordless_sudo = true` in `config/settings.conf`
2. Ensure passwordless sudo is configured on your system
3. Install a package - should skip password prompt if enabled
4. Remove a package - should always require password (security measure)

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [x] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added 35 integration tests covering install/update/remove operations and edge cases (including test-var-not-honored when not in integration test context)

**Documentation:**
- [x] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed:
  - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
  - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
- [x] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Respects `--dry-run` flag
- [x] Graceful degradation if sudo unavailable or passwordless sudo not configured
- [x] No breaking changes

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

**Key points:**
- Opt-in feature: requires `use_passwordless_sudo = true` in settings (safety barrier)
- Detection: uses `sudo -n true` to check availability
- Scope: install/update operations only; remove operations always require password
- Graceful degradation: falls back to password prompt if sudo unavailable or check fails

**Main changes:**
- `src/logic/password.rs`: Detection functions; test env var honored only when `PACSEA_INTEGRATION_TEST=1`
- `src/install/direct.rs`, `src/events/modals/handlers.rs`: Skip password prompt when enabled; remove intentionally never passwordless (comment)
- `src/args/update.rs`: CLI update respects `use_passwordless_sudo` from settings
- `config/settings.conf`, `src/theme/`: New config option
- `tests/passwordless_sudo/`: Integration test suite (35 tests); helpers set `PACSEA_INTEGRATION_TEST=1`

## Breaking changes
None. Opt-in feature, defaults to `false`.

## Additional context
- Requires passwordless sudo to be configured on the system (e.g., `/etc/sudoers`)
- Implementation follows `dev/IMPROVEMENTS/PASSWORDLESS_SUDO_IMPLEMENTATION_PLAN.md`
