## Summary
This PR removes dead code and unused modules while improving code organization and documentation. The changes include:
- **Bug fix**: Fixed preflight tabs not resolving when opening a second package directly from results pane without searching in between
- **Dead code removal**: Deleted unused `news.rs` module (date parsing functions that were no longer used)
- **Unused code cleanup**: Removed `#[allow(dead_code)]` and `#[allow(clippy::needless_borrow)]` annotations across the codebase
- **Code refactoring**: Improved comment handling with `CommentExtractionContext` struct for HTML data extraction
- **UI improvements**: Introduced `TabHeaderContext` struct to consolidate tab header rendering parameters
- **Function optimizations**: Updated `build_header_chips` to be a `const fn`, improving performance
- **Code quality**: Removed unused functions, streamlined comments, and improved documentation
- **Test updates**: Comprehensive test refactoring to align with code changes (4,354 insertions, 3,112 deletions)

## Type of change
- [x] bugfix (non-breaking change which fixes an issue)
- [x] refactor (no functional change)

## How to test
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
RUST_LOG=pacsea=debug cargo run -- --dry-run
```

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
- [ ] Tests are meaningful and cover the functionality

**Compatibility:**
- [ ] Changes respect `--dry-run` flag
- [ ] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] No breaking changes (or clearly documented if intentional)

## Notes for reviewers
- This is a comprehensive refactoring PR focused on removing dead code and improving maintainability
- **Bug fix**: Fixed `preflight_cancelled` flag not being reset when opening preflight with `use_cache = true` in `open_preflight_modal()`. This caused preflight tabs to not resolve when opening a second package directly from results pane without searching in between.
- **Significant changes**: Deletion of unused `news.rs` module (178 lines removed) which contained date parsing functions
- **Main refactoring areas**:
  - Comment handling: `src/sources/comments.rs` and `src/ui/details/comments.rs` (291 and 488 line files refactored)
  - Preflight logic: `src/logic/preflight/mod.rs` (183 lines changed)
  - Arguments handling: `src/args/update.rs` (326 lines refactored)
  - Directory scanning: `src/install/scan/dir.rs` (95 lines removed - dead code)
- **Code cleanup**: Removed `#[allow(dead_code)]` annotations from multiple files, indicating actual dead code was removed
- **Documentation improvements**: Added `dev/AGENTS.md` and improved code comments throughout
- **Test updates**: All 14 test files refactored to align with code changes (totaling ~1,242 net additions)

## Breaking changes
None - this is a refactoring PR with a bug fix but no breaking changes. The `news.rs` module deletion is safe as those functions were not used anywhere in the codebase. The bug fix for `preflight_cancelled` flag is backwards compatible.

### Files Changed
**Bug Fix:**
- `src/events/search/preflight_helpers.rs` - Added `preflight_cancelled` flag reset in `use_cache` branch + 5 new unit tests

**Deleted/Removed:**
- `src/app/news.rs` (178 lines) - unused date parsing functions

**Test Files (14 refactored):**
- All test files in `tests/` and `tests/preflight_integration/` updated to align with codebase changes


