<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
- Use `$VISUAL` then `$EDITOR` then the existing nvim→vim→hx→helix→emacsclient→emacs→nano fallback when opening config files (settings, theme, keybinds) in a terminal editor.
- Add a single helper `editor_open_config_command(path)` in `install::utils` so the editor command is built in one place; all three call sites (config menu keyboard 1/2/3, global menu selection, mouse click) use it.
- Path is passed through `shell_single_quote` so paths with spaces or single quotes are safe; VISUAL/EDITOR support values with options (e.g. `nvim -f`, `emacsclient -t`) via `eval` in the shell snippet.
- Add Windows branch in `normal_mode` (open config via `open_file`) for consistency with global and menus.
- Add unit tests for command order (VISUAL → EDITOR → fallbacks) and path quoting; add integration tests with `VISUAL=echo` and `EDITOR=echo` to confirm the config path is passed and printed.

## Type of change
- [x] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [x] test (add/update tests)
- [ ] chore (build/infra/CI)
- [ ] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## Related issues
Closes #117

## How to test
1. Run quality checks:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```
2. On a non-Windows system, set `VISUAL=echo` (or `EDITOR=echo`), open the config menu and choose Settings/Theme/Keybinds (keyboard 1/2/3 or menu/mouse). The path should be printed to the terminal (or your editor opens the file if VISUAL/EDITOR point to a real editor).
3. With VISUAL and EDITOR unset, opening config should still use the built-in fallback chain (nvim, vim, …).

## Screenshots / recordings (if UI changes)
N/A — no UI change; behavior is the same except editor selection respects VISUAL/EDITOR.

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
- [ ] For bug fixes: created failing tests first, then fixed the issue
- [x] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed
- [ ] Updated config examples in `config/` directory if config keys changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag (N/A — config open does not use dry-run)
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional)

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
- Editor selection is implemented entirely in the shell snippet (Option A from `dev/IMPROVEMENTS/EDITOR_VISUAL_IMPLEMENTATION_CONSIDERATIONS.md`): no Rust env reads, so the terminal’s environment is the source of truth.
- Optional-deps “Editor” modal rows are unchanged; they still suggest the fixed list of editors to install.

## Breaking changes
None. If neither VISUAL nor EDITOR is set, behavior matches the previous nvim→…→nano chain.

## Additional context
- Implementation follows [dev/IMPROVEMENTS/EDITOR_VISUAL_IMPLEMENTATION_CONSIDERATIONS.md](../IMPROVEMENTS/EDITOR_VISUAL_IMPLEMENTATION_CONSIDERATIONS.md).
- GitHub issue: [#117](https://github.com/Firstp1ck/Pacsea/issues/117) — Use `$EDITOR`/`$VISUAL` instead of a hard-coded list.
