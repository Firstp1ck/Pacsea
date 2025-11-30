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
- **Direct install/remove integration**: Direct install/remove operations (bypassing preflight) now use integrated processes
- **Security scan integration**: Security scans (ClamAV, Trivy, Semgrep, ShellCheck, VirusTotal, custom patterns) now use integrated processes; aur-sleuth runs in separate terminal simultaneously
- **File database sync fallback**: File database sync fallback now uses integrated process with password prompt instead of terminal spawning
- **Optional deps improvements**: `semgrep-bin` uses AUR helper flow; `paru`/`yay` use temporary directories for safe cloning; pressing Enter on already installed dependencies shows reinstall confirmation
- **Downgrade functionality**: Full downgrade support with terminal spawning for interactive tools
- **Comprehensive tests**: Integration and UI tests for all terminal-spawning processes

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

**Key Changes:**
- New PTY executor worker (`src/app/runtime/workers/executor.rs`) streams command output in real-time
- Password prompt modal integrated into TUI for sudo authentication
- Loading modal shows progress during async post-summary computation
- Auto-scrolling log panel displays latest output with progress bar support
- Reinstall confirmation modal (`src/ui/modals/confirm.rs`) for packages already installed
- Preflight risk calculation enhanced to show dependent packages in summary and add +2 risk per dependent
- Dry-run commands properly quoted using `shell_single_quote` to prevent syntax errors
- System updates, custom commands, and optional deps now use executor pattern
- Direct install/remove operations (`src/install/direct.rs`) integrated into executor pattern, bypassing preflight when configured
- Security scans (`ExecutorRequest::Scan`) integrated into executor pattern; aur-sleuth runs in separate terminal simultaneously when enabled
- File database sync fallback (`src/events/preflight/keys/command_keys.rs`) integrated into executor pattern; attempts non-sudo sync first, then shows password prompt for `sudo pacman -Fy`
- Custom command handler enhanced to support any `sudo` command with password (not just `makepkg -si`)
- Optional deps: `semgrep-bin` converted to use standard AUR helper flow; `paru`/`yay` use temporary directories to prevent accidental deletion; pressing Enter on already installed dependencies shows reinstall confirmation with password prompt for pacman packages
- Executor worker refactored into helper functions for better code organization and maintainability
- Downgrade functionality with terminal spawning for interactive tools
- Comprehensive test suite covering all terminal-spawning processes

**Dependencies Added:**
- `portable_pty`: Cross-platform PTY support
- `strip-ansi-escapes`: ANSI code removal from output

## Breaking changes
None. This is a new feature that enhances the existing installation flow without breaking existing functionality.

## Additional context

**Key Files:**
- `src/app/runtime/workers/executor.rs`: PTY executor worker (refactored into helper functions)
- `src/app/runtime/tick_handler.rs`: File database sync result checking
- `src/install/executor.rs`: Executor request/output types and command builders
- `src/install/direct.rs`: Direct install/remove operations using integrated processes
- `src/install/scan/pkg.rs`: Scan command builders (with/without aur-sleuth)
- `src/install/scan/spawn.rs`: aur-sleuth terminal spawning
- `src/events/modals/scan.rs`: Scan modal handler using integrated process
- `src/events/preflight/keys/command_keys.rs`: File database sync with password prompt fallback
- `src/ui/modals/password.rs`: Password prompt modal (includes FileSync purpose)
- `src/ui/modals/misc.rs`: Loading modal
- `src/ui/modals/preflight_exec.rs`: Auto-scrolling log panel
- `src/events/modals/handlers.rs`: Reinstall confirmation and password handling (includes FileSync)
- `src/events/install/mod.rs`: Direct install handling with reinstall/batch update logic
- `src/events/search/preflight_helpers.rs`: Direct install handling with reinstall/batch update logic
- `src/events/modals/optional_deps.rs`: Optional deps installation with improved AUR helper usage and reinstall confirmation for installed dependencies
- `src/logic/preflight/mod.rs`: Enhanced risk calculation with dependent package display
- `src/logic/deps/reverse.rs`: Added `get_installed_required_by` function
- `src/ui/modals/preflight/tabs/summary.rs`: Display dependent packages in summary
- `src/state/app_state/mod.rs`: Added `FileSyncResult` type alias and `pending_file_sync_result` field
- `src/state/modal.rs`: Added `PasswordPurpose::FileSync` variant
- `tests/*_integration.rs`: Comprehensive integration tests
- `tests/*_ui.rs`: UI state transition tests

### Future Improvements

- Abort functionality (currently shows toast but doesn't actually abort)
- Better error recovery for PTY failures
- Support for interactive prompts beyond password
- Configurable log buffer size
- Scroll history for log panel (currently auto-scrolls to bottom)

