<!-- Thank you for contributing to Pacsea!

**Important references:**
- [CONTRIBUTING.md](../CONTRIBUTING.md) — Full contribution guidelines and PR process
- [PR_DESCRIPTION.md](../Documents/PR_DESCRIPTION.md) — Detailed PR description template
- [Development Wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) — Development tools and debugging

Please ensure you've reviewed these before submitting your PR.
-->

## Summary
- **Problem**: When no theme is set (or user wants to match the terminal), Pacsea always wrote a default skeleton (Catppuccin Mocha) to `theme.conf` and used it. There was no way to use the terminal’s foreground/background colors so the UI could blend with the terminal palette.
- **Solution**:
  - Add `use_terminal_theme` setting in `settings.conf` (default: `false`). When `true`, theme resolution tries to query the terminal via OSC 10/11 (foreground/background), parses the reply, builds Pacsea’s 16-color theme from fg/bg and derivation rules, and uses it. If the terminal does not support query or the query fails (e.g. non-TTY, timeout), resolution falls back to `theme.conf` or the default skeleton.
  - Add theme resolution order: prefer terminal theme when `use_terminal_theme=true` and terminal is supported; otherwise use file theme or skeleton.
  - Add OSC 10/11 query implementation with timeout and robust parsing (`terminal_query.rs`), terminal support detection (`terminal_detect.rs`), and resolution logic (`resolve.rs`).
  - Improve robustness of OSC query (handling reply format, ST terminator, non-TTY).
  - On Unix: query terminal colors via `/dev/tty` with nix `poll` so stdin is not used and crossterm is not raced.
  - Add WezTerm and wezterm-gui to supported terminals (unit test added).
  - On Windows: join reader thread on timeout to avoid detached thread draining stdin.
  - In headless/test (`PACSEA_TEST_HEADLESS=1`): skip mouse capture disable/enable for clean output.
  - Remove dead `load_theme_from_file` from theme_loader; add rustdoc and error logging in `ensure_theme_file_exists`.
  - Fix AUR cache handling for long filenames in updates feed.
  - Build: add nix (Unix) dependency with poll and fs features.
- **Doc**: Add `dev/IMPROVEMENTS/TERMINAL_THEME_FALLBACK.md` describing design, OSC sequences, and implementation choices.

## Type of change
- [x] feat (new feature)
- [x] fix (bug fix)
- [ ] docs (documentation only)
- [x] refactor (no functional change)
- [ ] perf (performance)
- [ ] test (add/update tests)
- [x] chore (build/infra/CI)
- [x] ui (visual/interaction changes)
- [ ] breaking change (incompatible behavior)

## Related issues
Closes #122

## How to test
1. Format, lint, check, and test:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check
cargo test -- --test-threads=1
```
2. Terminal theme on supported terminal (e.g. Alacritty, Kitty, WezTerm): set `use_terminal_theme = true` in `settings.conf`, run Pacsea; UI should match terminal fg/bg–derived colors. Change terminal theme and reload theme in Pacsea to see updates. On Unix, query uses `/dev/tty` (no stdin).
3. Unsupported / non-TTY: run with `use_terminal_theme = true` in a pipe or unsupported terminal; should fall back to theme.conf or default skeleton without hanging.
4. AUR updates: run update flow on a system with AUR packages; long cache filenames should not cause failures.

```bash
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
- [ ] For bug fixes: created failing tests first, then fixed the issue
- [ ] Tests are meaningful and cover the functionality

**Documentation:**
- [ ] Updated README if behavior, options, or keybinds changed (keep high-level, reference wiki)
- [ ] Updated relevant wiki pages if needed:
  - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
  - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
- [x] Updated config examples in `config/` directory if config keys changed (`settings.conf`: `use_terminal_theme`)
- [ ] For UI changes: included screenshots and updated `Images/` if applicable

**Compatibility:**
- [x] Changes respect `--dry-run` flag
- [x] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
- [x] No breaking changes (or clearly documented if intentional): new setting is opt-in, default `false`

**Other:**
- [x] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
- **Theme resolution** (`src/theme/resolve.rs`): Order is (1) if `use_terminal_theme` and terminal supported → query OSC, use terminal theme; (2) else use theme from file or skeleton. Fallback on query failure avoids hanging in pipes/CI/unsupported terminals.
- **OSC query** (`src/theme/terminal_query.rs`): Uses blocking read with timeout; parses `rgb:rrrr/gggg/bbbb` and optional `rgba`; handles both `\033\\` and BEL as ST. Non-TTY or no reply → returns error so resolution can fall back.
- **Terminal detection** (`src/theme/terminal_detect.rs`): Heuristic (e.g. `COLORTERM`, `TERM`) to decide if we try OSC query; WezTerm/wezterm-gui added; not all terminals that set these support *query* (some only support *set*), so we still rely on query success/failure.
- **Unix query** (`src/theme/terminal_query.rs`): Colors are read from `/dev/tty` via nix `poll` so stdin is untouched and there is no race with crossterm.
- **Windows**: Reader thread is always joined on timeout to avoid a detached thread draining stdin.
- **Headless tests**: When `PACSEA_TEST_HEADLESS=1`, mouse capture disable/enable is skipped for clean test output.
- **AUR updates** (`src/sources/feeds/updates.rs`): Long filename fix is a small, separate change in the same branch; included in this PR.

## Breaking changes
None. New setting `use_terminal_theme` defaults to `false`; existing configs unchanged.

## Additional context
- Design and OSC details: `dev/IMPROVEMENTS/TERMINAL_THEME_FALLBACK.md`
- Terminals that support OSC 10/11 *query* (e.g. xterm, Alacritty, Kitty, WezTerm) will use terminal theme when `use_terminal_theme = true`; others fall back to file/skeleton.
