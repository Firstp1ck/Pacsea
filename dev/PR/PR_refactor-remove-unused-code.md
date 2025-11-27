<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
This PR removes unused code annotations and refactors comment handling logic for better maintainability. The changes include:
- Removal of unnecessary `#[allow(dead_code)]` annotations across the codebase
- Introduction of `CommentExtractionContext` struct to streamline HTML data extraction
- Refactoring of comment rendering functions for improved readability
- Better separation of concerns for loading, error, and empty state items
- Improved handling of pinned comments and their display logic

## Type of change
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
- This is a refactoring PR focused on code cleanup and improved structure
- The main changes are in comment handling logic (`src/sources/comments.rs`, `src/ui/details/comments.rs`)
- Removed `#[allow(dead_code)]` annotations from multiple files, indicating actual dead code was removed
- Added `dev/AGENTS.md` file for development documentation

## Breaking changes
None - this is a refactoring PR with no functional changes.

## Additional context

### Commits
1. **ea8b13ee** - added agents.md file for dev
   - Added development documentation file `dev/AGENTS.md`

2. **5c22df34** - refactor: clean up dead code and improve comment handling
   - Removed unnecessary `#[allow(dead_code)]` annotations from various functions and structs across the codebase
   - Enhanced the comment fetching logic by introducing a `CommentExtractionContext` struct to streamline data extraction from HTML elements
   - Refactored comment rendering functions for better readability and maintainability, including separating concerns for building loading, error, and empty state items
   - Improved the handling of pinned comments and their display logic in the comments viewer

### Files Changed
- `dev/AGENTS.md` (new file)
- `src/app/news.rs`
- `src/index/fetch.rs`
- `src/index/mirrors.rs`
- `src/install/scan/dir.rs`
- `src/logic/deps/parse.rs`
- `src/logic/preflight/mod.rs`
- `src/sources/comments.rs`
- `src/state/modal.rs`
- `src/theme/config/settings_save.rs`
- `src/ui/details/comments.rs`
- `src/ui/modals/preflight/header.rs`
- `src/ui/modals/preflight/helpers/extract.rs`

**Total:** 13 files changed, 588 insertions(+), 430 deletions(-)

