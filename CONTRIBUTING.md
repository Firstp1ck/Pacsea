# Contributing to Pacsea

Thanks for your interest in contributing! Pacsea is a fast, keyboard-first TUI for discovering and installing Arch and AUR packages.

By participating, you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute
- Bug reports and fixes
- Feature requests and implementations
- Documentation and examples
- UI/UX polish and accessibility improvements

## Before you start
- Target platform: Pacsea focuses on Arch Linux and Arch-based distributions (e.g., EndeavourOS, Manjaro, CachyOS). End-to-end install/update features rely on Arch tools like `pacman` and an AUR helper (`paru` or `yay`).
- Safety: During development, prefer `--dry-run` and/or a disposable VM/container to avoid unintended changes.
- Security: If your report involves a security issue, use our [Security Policy](SECURITY.md).

## Development setup
1. Install Rust (stable):
   ```bash
   sudo pacman -S rustup && rustup default stable
   ```
2. Clone and run:
   ```bash
   git clone https://github.com/Firstp1ck/Pacsea
   cd Pacsea
   cargo run -- --dry-run
   ```
3. Optional debugging:
   ```bash
   RUST_LOG=pacsea=debug cargo run -- --dry-run
   ```
4. Run tests (single-threaded):
   ```bash
   cargo test -- --test-threads=1
   # or
   RUST_TEST_THREADS=1 cargo test
   ```

## Build, format, lint, test
- Format:
  ```bash
  cargo fmt --all
  ```
- Lint:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```
- Test:
  ```bash
  cargo test -- --test-threads=1
  ```

Please ensure all of the above pass before opening a PR.

## Commit and branch guidelines
- Branch naming: `feature/<short-description>` or `fix/<short-description>`
- Commit style: Prefer Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `perf:`, `test:`)
- Keep commits focused and reasonably small; add rationale in the body if non-obvious.

## Pull Request checklist
- Code compiles and tests pass
- `cargo fmt` and `cargo clippy` are clean
- Add/update tests where it makes sense
- Update docs (README, examples, config docs) if behavior or options change
- For UI changes: include a brief description and screenshots; update images in `Images/` if relevant
- Ensure examples respect `--dry-run` and do not trigger real installs

## Project conventions
- Language: Rust. Favor clear naming, early returns, and avoid unnecessary `unwrap`/`expect` in non-test code.
- Logging: Use `tracing` for diagnostics; avoid noisy logs at info level.
- UX: Keyboard-first, minimal keystrokes. Update the help overlay and keybind docs if shortcuts change.
- Config: If you add/change config keys, update README sections for `settings.conf`, `theme.conf`, and `keybinds.conf`.
- Platform behavior: Any invocation of `pacman`, `paru`, or `yay` must degrade gracefully if unavailable and respect `--dry-run`.

## Filing issues
- Bug reports: include Pacsea version (e.g., 0.4.x), Arch version, terminal, display server (Wayland/X11), AUR helper, steps to reproduce, expected vs. actual behavior, and logs if available (or run with `RUST_LOG=pacsea=debug`).
- Feature requests: describe the problem being solved and the desired UX. Mockups or keybind suggestions help.

Open issues in the [issue tracker](https://github.com/Firstp1ck/Pacsea/issues).

## Packaging notes
- AUR packages live in separate repos: `pacsea-bin` and `pacsea-git`. Propose packaging changes in those repos rather than here.

## License
By contributing, you agree that your contributions will be licensed under the MIT License (see [LICENSE](LICENSE)).

## Code of Conduct and Security
- Code of Conduct: see [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). For conduct issues, contact firstpick1992@proton.me.
- Security Policy: see [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

Thank you for helping improve Pacsea!

<details>
  <summary>Support indirectly</summary>
If Pacsea has been helpful, you can support ongoing development here ❤️ [Patreon](https://www.patreon.com/Firstp1ck)
</details>
