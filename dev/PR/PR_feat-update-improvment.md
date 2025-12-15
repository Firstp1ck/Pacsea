## Summary
- Changed AUR update command from `-Syu` to `-Sua` to only update AUR packages (official packages already updated by pacman)
- Added confirmation popup when update command fails but AUR update is pending, allowing users to continue with AUR update anyway
- Fixed confirmation popup to track and display the actual failed command name (pacman, paru, yay, reflector, etc.) instead of always assuming pacman failed
- Enhanced error reporting with failure summary and failed commands tracking
- Improved localization with new messages for AUR update confirmation and error reporting
- Added comprehensive tests for system update modal functionality

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [x] test (add/update tests)

## Related issues
Closes #105 

## How to test
List exact steps and commands to verify the change. Include flags like `--dry-run` when appropriate.

```bash
# Format and lint
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test -- --test-threads=1
cargo test system_update -- --test-threads=1

# Test CLI update with dry-run
RUST_LOG=pacsea=debug cargo run -- --update --dry-run

# Test TUI update flow
RUST_LOG=pacsea=debug cargo run -- --dry-run
# Navigate to system update modal, enable AUR update, and test the confirmation popup
```

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
- [x] Added or updated tests where it makes sense
- [x] For bug fixes: created failing tests first, then fixed the issue
- [x] Tests are meaningful and cover the functionality

**Documentation:**
- [x] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [x] Updated relevant wiki pages if needed:
  - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
  - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
- [ ] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

### Key Changes:

1. **AUR Update Command Change (`-Syu` → `-Sua`)**:
   - Changed in both CLI (`src/args/update.rs`) and TUI (event loop)
   - `-Sua` only updates AUR packages, avoiding redundant official package updates
   - This is more efficient since pacman already updated official packages

2. **Confirmation Popup for Failed Update Commands**:
   - New modal type: `Modal::ConfirmAurUpdate`
   - Triggered when any update command fails but AUR update is pending
   - Tracks which command actually failed (pacman, reflector, pacman-mirrors, etc.) and displays correct name
   - Allows user to continue with AUR update despite previous command failure
   - Handled in `src/events/modals/handlers.rs`

3. **Enhanced Error Reporting**:
   - Added `failed_commands` tracking in `UpdateState`
   - Added failure summary display in CLI output
   - Better error messages in localization files

4. **Event Loop Improvements**:
   - Better handling of update command failure scenarios
   - Extracts failed command name from command list to show accurate error message
   - Preserves password and header chips for AUR update continuation
   - Improved state management for pending AUR commands

5. **Command Failure Tracking**:
   - Added `failed_command` field to `ExecutorOutput::Finished` enum
   - Extracts command name from failed command string (pacman, paru, yay, reflector, etc.)
   - Added `t_fmt2` function to i18n module for two-parameter string formatting
   - Updated localization strings to support dynamic command names in error messages

6. **Testing**:
   - Comprehensive tests added in `src/events/modals/system_update/tests.rs`
   - Tests cover confirmation popup, command execution, and error scenarios
   - Simplified test assertion logic by removing double negative pattern
   - Updated all `ExecutorOutput::Finished` pattern matches to include `failed_command` field

### Areas to Review:
- Event loop logic for handling update command failures and AUR update continuation
- Command name extraction logic for accurate error messages
- Modal state transitions and password preservation
- Error message clarity and user experience

## Breaking changes
None. This is a backward-compatible enhancement.

## Additional context

### Technical Details:

**AUR Update Command Rationale:**
- `-Syu`: Updates both official and AUR packages (redundant after pacman update)
- `-Sua`: Updates only AUR packages (more efficient, avoids conflicts)

**Confirmation Popup Flow:**
1. User initiates system update with AUR enabled
2. Any update command fails (mirrors, pacman, etc.)
3. System determines which command failed by checking command list
4. If AUR update is pending, show confirmation popup with correct failed command name
5. User can choose to continue (Enter/Y) or cancel (Esc/Q/N)
6. If continued, AUR update proceeds with preserved password/state

**Command Failure Tracking:**
- Commands are chained with `&&`, so first failure stops execution
- System extracts command name from failed command string
- Supports: pacman, paru, yay, reflector, pacman-mirrors, eos-rankmirrors, cachyos-rate-mirrors
- Error message dynamically shows which command failed instead of always showing "pacman"