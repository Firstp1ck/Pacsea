## Summary
- Changed AUR update command from `-Syu` to `-Sua` to only update AUR packages (official packages already updated by pacman)
- Added confirmation popup when pacman update fails but AUR update is pending, allowing users to continue with AUR update anyway
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
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed:
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

2. **Confirmation Popup for Failed Pacman Updates**:
   - New modal type: `Modal::ConfirmAurUpdate`
   - Triggered when pacman fails but AUR update is pending
   - Allows user to continue with AUR update despite pacman failure
   - Handled in `src/events/modals/handlers.rs`

3. **Enhanced Error Reporting**:
   - Added `failed_commands` tracking in `UpdateState`
   - Added failure summary display in CLI output
   - Better error messages in localization files

4. **Event Loop Improvements**:
   - Better handling of pacman failure scenarios
   - Preserves password and header chips for AUR update continuation
   - Improved state management for pending AUR commands

5. **Testing**:
   - Comprehensive tests added in `src/events/modals/system_update/tests.rs`
   - Tests cover confirmation popup, command execution, and error scenarios

### Areas to Review:
- Event loop logic for handling pacman failures and AUR update continuation
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
2. Pacman update fails
3. If AUR update is pending, show confirmation popup
4. User can choose to continue (Enter/Y) or cancel (Esc/Q/N)
5. If continued, AUR update proceeds with preserved password/state