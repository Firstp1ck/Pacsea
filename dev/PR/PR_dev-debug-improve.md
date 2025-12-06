## Summary
- Add `ChangeLogger` helper and wire it into preflight files/summary tabs to drop duplicate UI debug logs.
- Clamp verbose HTML parser/service tracing to `warn` by default and add structured logging around pacman/update command execution.
- Harden cache persistence/cleanup (details, recents, news, announcements, deps/files/services/sandbox) with explicit tracing and error handling.
- Rename the Recent pane to “Search history” across locales and UI (contribution from community member @Summoner001).
- Migrate config key to `show_search_history_pane` with legacy alias for `show_recent_pane` and coverage.

## Type of change
- [ ] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [ ] test (add/update tests)
- [ ] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test
List exact steps and commands to verify the change. Include flags like `--dry-run` when appropriate.

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
RUST_LOG=pacsea=debug cargo run -- --dry-run
```

## Checklist

**Code Quality:**
- [x] Code compiles locally (`cargo check`)
- [x] `cargo fmt --all` ran without changes
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [x] `cargo test -- --test-threads=1` passes
- [ ] Complexity checks pass for new code (`cargo test complexity -- --nocapture`)
- [ ] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
- [ ] No `unwrap()` or `expect()` in non-test code

**Testing:**
- [x] Added or updated tests where it makes sense
- [ ] For bug fixes: created failing tests first, then fixed the issue
- [ ] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed:
  - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
  - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
- [x] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [ ] Changes respect `--dry-run` flag
- [ ] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] No breaking changes (or clearly documented if intentional)

**Other:**
- [ ] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
- Focus areas: preflight logging noise reduction, cache persistence tracing, and search-history naming/config key migration (legacy `show_recent_pane` remains supported).

## Breaking changes
None.

## Additional context
None.

