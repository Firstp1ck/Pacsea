## Summary
- Ensure dependency, file, service, and sandbox cache files are created on startup if missing.
- When the install list is empty, flushers now write empty cache payloads instead of deleting the files, preventing noisy "failed to remove" logs.
- Added startup helper plus unit tests to cover empty cache persistence for all cache types.

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
Closes #N/A

## How to test
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
cargo test complexity -- --nocapture --test-threads=1
```

## Screenshots / recordings (if UI changes)
N/A

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
- Cache files are now created eagerly at startup and rewritten to empty JSON when the install list is empty to eliminate repeated "Failed to remove ... cache" debug logs.
- Added coverage for empty-cache writes across all cache types and a startup helper that ensures parent directories exist before writing.

## Breaking changes
None.

## Additional context
Logs showed repeated `[Persist] Failed to remove ... cache` messages when cache files were absent. This change replaces removal with empty writes to keep paths stable and quiet.

