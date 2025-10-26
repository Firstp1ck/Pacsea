<!-- Thank you for contributing to Pacsea! Please read CONTRIBUTING.md before submitting. -->

## Summary
Briefly describe the problem and how your change solves it.

## Type of change
- [ ] feat (new feature)
- [ ] fix (bug fix)
- [ ] docs (documentation only)
- [ ] refactor (no functional change)
- [ ] perf (performance)
- [ ] test (add/update tests)
- [ ] chore (build/infra/CI)
- [ ] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## Related issues
Closes #

## How to test
List exact steps and commands to verify the change. Include flags like `--dry-run` when appropriate.

```bash
# examples
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test -- --test-threads=1
RUST_LOG=pacsea=debug cargo run -- --dry-run
```

## Screenshots / recordings (if UI changes)
Include before/after images or a short GIF. Update files in `Images/` if relevant.

## Checklist
- [ ] Code compiles locally
- [ ] `cargo fmt --all` ran without changes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test -- --test-threads=1` passes
- [ ] Added or updated tests where it makes sense
- [ ] Updated docs if behavior, options, or keybinds changed (README, config examples)
- [ ] For UI changes: included screenshots and updated `Images/` if applicable
- [ ] Changes respect `--dry-run` and degrade gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] If config keys changed: updated README sections for `settings.conf`, `theme.conf`, and `keybinds.conf`
- [ ] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
Call out tricky areas, assumptions, edge cases, or follow-ups.

## Breaking changes
Describe any breaking changes and migration steps (e.g., config key renames).

## Additional context
Logs, links to discussions, design notes, or prior art.


