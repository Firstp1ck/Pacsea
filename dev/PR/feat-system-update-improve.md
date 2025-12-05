<!-- Thank you for contributing to Pacsea! 

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
- Add sync mode selector to System Update modal: Normal (`-Syu`) / Force Sync (`-Syyu`)
- Toggle with Left/Right/Tab keys on the pacman row
- Auto-refresh updates count after install/remove/downgrade operations
- Add missing locale keys for de-DE and hu-HU

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [x] refactor (no functional change)
- [ ] perf (performance)
- [x] test (add/update tests)
- [x] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## How to test

### Test Force Sync Option:
1. Launch Pacsea: `cargo run`
2. Open System Update modal (Options → Update System or press the update keybind)
3. Navigate to "Update Pacman" row (second row)
4. Press Left/Right/Tab to toggle between "Normal (-Syu)" and "Force Sync (-Syyu)"
5. Enable the checkbox with Space
6. Press Enter and enter password
7. Verify the command uses `-Syyu` when Force Sync is selected

### Test Updates Auto-Refresh:
1. Launch Pacsea with updates available
2. Perform a system update via the System Update modal
3. After completion, verify the updates count in the UI refreshes to show the new count

### Run Tests:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
cargo test system_update -- --test-threads=1
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
- The `force_sync` field was added to `Modal::SystemUpdate` variant
- Row indices changed: AUR is now row 2, Cache is row 3, Country is row 4
- Left/Right on row 1 toggles sync mode; on row 4 cycles countries
- Tab key also toggles sync mode for accessibility
- Helper functions `row_style()` and `checkbox_line()` were extracted to reduce code complexity

## Breaking changes
None. The `force_sync` option defaults to `false`, maintaining backward-compatible behavior.

## Additional context

### Changes Summary:
1. **New Feature**: Force Sync option in System Update modal
2. **Bug Fix**: Auto-refresh updates count after operations complete
3. **Bug Fix**: Missing locale keys added to de-DE.yml and hu-HU.yml
4. **Refactor**: Extracted UI helper functions to reduce complexity
5. **Chore**: Added debug logging to executor worker for troubleshooting
6. **Chore**: Removed obsolete `tests.rs.bak` file

### New Locale Keys:
- `app.modals.system_update.sync_mode.normal`
- `app.modals.system_update.sync_mode.force`
- `app.toasts.preflight_downgrade_list`
- `app.toasts.downgrade_started`
- `app.modals.preflight_exec.title_downgrade`
- `app.modals.preflight.title_install/remove/downgrade`

