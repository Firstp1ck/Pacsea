<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
This PR adds PTY-based command execution with live output streaming, enabling real-time display of package installation progress directly within the TUI. The implementation includes:

- **Live output streaming**: See installation progress in real-time as commands execute
- **Password prompt modal**: Secure sudo authentication without leaving the TUI
- **Loading modal**: Shows progress during async post-summary computation
- **Auto-scrolling logs**: Log panel automatically scrolls to show latest output with progress bar support
- **Reinstall confirmation**: Modal for confirming reinstallation of already installed packages
- **Enhanced preflight risk calculation**: Shows dependent packages and adds +2 risk per dependent
- **System update integration**: System updates now use executor pattern with PTY
- **Custom command support**: Special packages (paru/yay/semgrep-bin) handled via executor
- **Downgrade functionality**: Full downgrade support with terminal spawning for interactive tools
- **Comprehensive tests**: Integration and UI tests for all terminal-spawning processes

## Type of change
- [x] feat (new feature)
- [x] ui (visual/interaction changes)

## How to test

```bash
# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test -- --test-threads=1

# Test installation flow:
# 1. Select packages to install
# 2. Press Enter to open preflight modal
# 3. Review preflight summary
# 4. Press Enter to execute installation
# 5. Observe live output streaming in PreflightExec modal
# 6. If sudo password is required, password prompt modal should appear
# 7. Enter password and observe command execution continues
# 8. After completion, press Enter to view post-summary
# 9. Loading modal should appear briefly while computing summary
# 10. Post-summary modal should display results
```

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

**Key Changes:**
- New PTY executor worker (`src/app/runtime/workers/executor.rs`) streams command output in real-time
- Password prompt modal integrated into TUI for sudo authentication
- Loading modal shows progress during async post-summary computation
- Auto-scrolling log panel displays latest output with progress bar support
- Reinstall confirmation modal (`src/ui/modals/confirm.rs`) for packages already installed
- Preflight risk calculation enhanced to show dependent packages in summary and add +2 risk per dependent
- Dry-run commands properly quoted using `shell_single_quote` to prevent syntax errors
- System updates, custom commands, and optional deps now use executor pattern
- Downgrade functionality with terminal spawning for interactive tools
- Comprehensive test suite covering all terminal-spawning processes

**Dependencies Added:**
- `portable_pty`: Cross-platform PTY support
- `strip-ansi-escapes`: ANSI code removal from output

## Breaking changes
None. This is a new feature that enhances the existing installation flow without breaking existing functionality.

## Additional context

**Files Changed:** 93 files (+8951 insertions, -468 deletions)

**Key Files:**
- `src/app/runtime/workers/executor.rs`: PTY executor worker
- `src/install/executor.rs`: Executor request/output types
- `src/ui/modals/password.rs`: Password prompt modal
- `src/ui/modals/misc.rs`: Loading modal
- `src/ui/modals/preflight_exec.rs`: Auto-scrolling log panel
- `src/events/modals/handlers.rs`: Reinstall confirmation and password handling
- `src/events/preflight/keys/command_keys.rs`: Reinstall check and batch update logic
- `src/logic/preflight/mod.rs`: Enhanced risk calculation with dependent package display
- `src/logic/deps/reverse.rs`: Added `get_installed_required_by` function
- `src/ui/modals/preflight/tabs/summary.rs`: Display dependent packages in summary
- `tests/*_integration.rs`: Comprehensive integration tests
- `tests/*_ui.rs`: UI state transition tests

### Future Improvements

- Abort functionality (currently shows toast but doesn't actually abort)
- Better error recovery for PTY failures
- Support for interactive prompts beyond password
- Configurable log buffer size
- Scroll history for log panel (currently auto-scrolls to bottom)

