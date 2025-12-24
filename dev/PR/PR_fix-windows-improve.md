## Summary

**Windows Compatibility Fixes:**
- Fixed Windows-specific compilation issues in updates worker (conditional imports and unused variables)
- Fixed error handling in mirrors index refresh to properly handle async task results

**News Search Input Separation:**
- Separated news search input state from regular search input to prevent shared state issues
- News mode now uses dedicated fields independent from Package mode search

**Toast Improvements:**
- Improved toast clearing logic to only block news loading toast (not all toasts during news loading)
- Added toast title detection for news, clipboard, and notification types
- Added `title_notification` translation key

**News Modal Behavior:**
- Mark read actions now only work in normal mode (prevents accidental marking when typing 'r' in insert mode)
- Added tests for insert mode behavior

**UI Improvements:**
- Removed sort menu auto-close functionality
- Added change_sort keybind to help footer in news mode
- Fixed help text punctuation (comma to colon)

## Type of change
- [x] fix (bug fix)
- [x] feat (new feature)
- [x] test (add/update tests)
- [x] change (behavioral change)

## Related issues
Closes #112, #111, #103

## How to test

**Basic Tests:**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```

**Windows Compatibility:**
1. On Windows, verify the application compiles without errors
2. Test package updates functionality works correctly
3. Verify mirror index refresh works without task join errors

**News Search Input Separation:**
1. Launch Pacsea and switch to News mode
2. Type in the search box - verify it only affects news search, not package search
3. Switch between News and Package modes - verify search inputs are independent
4. Test text selection and deletion in News mode search box
5. Test clearing search input (Shift+Del) works correctly in News mode

**Toast Improvements:**
1. Trigger various toasts (news loading, clipboard operations, notifications)
2. Verify toast titles are correctly detected and displayed
3. Verify news loading toast doesn't get cleared prematurely while news are loading
4. Verify other toasts can be cleared normally even when news are loading

**News Modal Mark Read:**
1. Open News modal and switch to insert mode (Esc)
2. Try pressing 'r' to mark as read - should NOT mark as read (should just type 'r')
3. Switch back to normal mode (Esc again)
4. Press 'r' - should mark current item as read
5. Press Ctrl+R - should mark all items as read
6. Switch to insert mode again - verify Ctrl+R doesn't mark as read

**Sort Menu:**
1. Click sort button or use Shift+Tab
2. Verify sort menu stays open (doesn't auto-close after 2 seconds)
3. Verify menu closes when clicking sort button again or selecting an option

**UI/Help Text:**
1. Check help footer in News mode insert mode section
2. Verify "Insert Mode:" uses colon instead of comma
3. Verify change_sort keybind is shown in insert mode help

## Screenshots / recordings (if UI changes)
N/A - Internal behavior improvements, no visible UI changes

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [x] Complexity checks pass for new code
- [x] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [x] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added or updated tests where it makes sense
- [x] For bug fixes: created failing tests first, then fixed the issue
- [x] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed
- [x] Updated config examples in `config/` directory if config keys changed (locale files updated)
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers

**Windows Compatibility:**
- `check_aur_helper` is conditionally imported only on non-Windows platforms
- Unused `is_checkupdates_tool` variable prefixed with `_` on Windows
- mirrors.rs: Removed nested `Result` handling that was losing task join errors

**News Search Input Separation:**
- Significant refactoring: News mode now uses dedicated `news_search_input`, `news_search_caret`, `news_search_select_anchor` fields
- Previously shared `app.input`, `app.search_caret`, `app.search_select_anchor` caused bugs when switching modes
- All search functions now check `app.app_mode` to determine which fields to use

**Toast Clearing Logic:**
- Changed from blocking all toasts when `app.news_loading` is true to specifically checking if current toast matches news loading message
- Allows other toasts to clear normally during news loading

**Mark Read Mode Restriction:**
- Prevents mark read actions in insert mode (users can now type 'r' without marking as read)
- More consistent with vim-like behavior

## Breaking changes
None. All changes are backward compatible.