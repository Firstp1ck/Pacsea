<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
This PR adds PTY-based command execution with live output streaming, enabling real-time display of package operations directly within the TUI. Key features:

- **Live output streaming**: Real-time progress display for all package operations
- **TUI modals**: Password prompt, loading indicator, reinstall confirmation, and faillock lockout alerts
- **Auto-scrolling logs**: Log panel with progress bar support
- **Integrated processes**: All operations (install, remove, update, scan, downgrade, file sync, optional deps) now use PTY executor pattern
- **Security enhancements**: Password validation with attempt tracking, faillock lockout detection with periodic status checks
- **Enhanced preflight**: Shows dependent packages and adds +2 risk per dependent
- **Comprehensive test suite**: Feature-based test organization covering all workflows

## Type of change
- [x] feat (new feature)
- [x] ui (visual/interaction changes)

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added or updated tests where it makes sense
- [x] Tests are meaningful and cover the functionality

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes

## Notes for reviewers

**Core Implementation:**
- PTY executor worker (`src/app/runtime/workers/executor.rs`) streams command output in real-time (refactored into helper functions)
- All operations integrated: install/remove, updates, scans, downgrades, file sync, optional deps
- Security scans: aur-sleuth runs in separate terminal simultaneously when enabled
- Dry-run commands properly quoted using `shell_single_quote`

**Security & Authentication:**
- Password validation: `sudo -k ; sudo -S -v` (invalidates cached credentials first); language-independent (exit codes only); shows remaining attempts; clears input on failure
- Faillock lockout detection: Parses `/etc/security/faillock.conf`; checks at startup and every minute; displays status in top-right corner; shows alert modal when locked
- Password prompt supports all sudo operations (not just `makepkg -si`)

**UI Enhancements:**
- Auto-scrolling log panel with progress bar support
- Reinstall confirmation modal (installs all selected packages using `all_items` field)
- Loading modal for async post-summary computation
- Preflight risk calculation: shows dependent packages, adds +2 risk per dependent

**Optional Deps:**
- `semgrep-bin` uses standard AUR helper flow
- `paru`/`yay` use temporary directories for safe cloning
- Reinstall confirmation for already installed dependencies

**Testing:**
- Tests organized into feature-based subdirectories (install, scan, update, downgrade, etc.)
- Comprehensive integration and UI tests covering all workflows
- Password validation tests marked as ignored (run with `--ignored` to prevent lockout)

**Dependencies Added:**
- `portable_pty`: Cross-platform PTY support
- `strip-ansi-escapes`: ANSI code removal from output

## Breaking changes
None. This is a new feature that enhances the existing installation flow without breaking existing functionality.

## Additional context

**Key Files by Category:**

**Core Executor:**
- `src/app/runtime/workers/executor.rs`: PTY executor worker
- `src/install/executor.rs`: Executor request/output types and command builders
- `src/app/runtime/tick_handler.rs`: File sync result checking, periodic faillock checks
- `src/app/runtime/init.rs`: Initial faillock status check

**Operations:**
- `src/install/direct.rs`: Direct install/remove operations
- `src/install/scan/pkg.rs`, `src/install/scan/spawn.rs`: Scan command builders and aur-sleuth spawning
- `src/events/modals/scan.rs`, `src/events/install/mod.rs`, `src/events/search/preflight_helpers.rs`: Operation handlers
- `src/events/preflight/keys/command_keys.rs`: File database sync with password fallback
- `src/events/modals/optional_deps.rs`: Optional deps installation

**UI Components:**
- `src/ui/modals/password.rs`, `src/ui/modals/misc.rs`, `src/ui/modals/preflight_exec.rs`: Password, loading, log panel modals
- `src/ui/modals/preflight/tabs/summary.rs`: Dependent packages display
- `src/ui/updates.rs`: Lockout status display and alert modal

**Logic & State:**
- `src/logic/password.rs`, `src/logic/faillock.rs`: Password validation and faillock detection
- `src/logic/preflight/mod.rs`, `src/logic/deps/reverse.rs`: Risk calculation and dependency tracking
- `src/state/app_state/mod.rs`, `src/state/modal.rs`: State management additions
- `src/events/modals/handlers.rs`: Modal handlers with password validation

**Tests:**
- `tests/install/`, `tests/update/`, `tests/scan/`, `tests/downgrade/`, `tests/other/`, `tests/preflight_integration/`: Feature-based test organization

**UI Improvements:**
- Improved modal sizing: password, reinstall confirmation, and post-transaction summary windows are now more appropriately sized
- Enhanced password prompt: better formatting, clearer instructions, improved package list display
- Better keybind visibility: keybind hints are now always visible and prominently displayed
- Account locked alert: proper title ("Account Locked") and command highlighting in messages
- Plan section improvements: better header chips formatting with descriptive labels
- Translation support: added i18n keys for password prompts, reinstall confirmation, and account locked alerts (en/de/hu)

### Future Improvements

- Abort functionality (currently shows toast but doesn't actually abort)
- Better error recovery for PTY failures
- Support for interactive prompts beyond password
- Configurable log buffer size
- Scroll history for log panel (currently auto-scrolls to bottom)

