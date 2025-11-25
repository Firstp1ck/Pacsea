# Contributing to Pacsea

Thanks for your interest in contributing! Pacsea is a fast, keyboard-first TUI for discovering and installing Arch and AUR packages.

By participating, you agree to follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## Ways to contribute
- Bug reports and fixes
- Feature requests and implementations
- Documentation and examples
- UI/UX polish and accessibility improvements
- Translations and localization

## Before you start
- **Target platform**: Pacsea focuses on Arch Linux and Arch-based distributions (e.g., EndeavourOS, Manjaro, CachyOS, Artix). End-to-end install/update features rely on Arch tools like `pacman` and an AUR helper (`paru` or `yay`).
- **Safety**: During development, prefer `--dry-run` and/or a disposable VM/container to avoid unintended changes.
- **Security**: If your report involves a security issue, use our [Security Policy](SECURITY.md).

## Development setup

### Prerequisites
1. Install Rust (stable):
   ```bash
   sudo pacman -S rustup && rustup default stable
   ```
2. Clone the repository:
   ```bash
   git clone https://github.com/Firstp1ck/Pacsea
   cd Pacsea
   ```

### Running the application
```bash
# Basic run (dry-run mode recommended for development)
cargo run -- --dry-run

# With debug logging
RUST_LOG=pacsea=debug cargo run -- --dry-run

# Or use verbose flag
cargo run -- --dry-run --verbose
```

### Running tests
Tests must be run single-threaded to avoid race conditions:
```bash
cargo test -- --test-threads=1
# or
RUST_TEST_THREADS=1 cargo test
```

## Code quality requirements

### Pre-commit checklist
Before committing, ensure all of the following pass:

1. **Format code:**
   ```bash
   cargo fmt --all
   ```

2. **Lint with Clippy:**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```
   
   The project uses strict Clippy settings configured in `Cargo.toml`:
   ```toml
   [lints.clippy]
   cognitive_complexity = "warn"
   pedantic = { level = "deny", priority = -1 }
   nursery = { level = "deny", priority = -1 }
   unwrap_used = "deny"
   ```
   
   Additional settings in `clippy.toml`:
   - `cognitive-complexity-threshold = 25`
   - `too-many-lines-threshold = 150`

3. **Check compilation:**
   ```bash
   cargo check
   ```

4. **Run tests:**
   ```bash
   cargo test -- --test-threads=1
   ```

5. **Check complexity (for new code):**
   ```bash
   # Run complexity tests to ensure new functions meet thresholds
   cargo test complexity -- --nocapture
   ```
   
   **Complexity thresholds:**
   - **Cyclomatic complexity**: Should be < 25 for new functions
   - **Data flow complexity**: Should be < 25 for new functions
   
   See [Development wiki](https://github.com/Firstp1ck/Pacsea/wiki/Development) for detailed complexity analysis.

### Code documentation requirements

**For all new code (functions, methods, structs, enums):**

1. **Rust documentation comments** are required:
   ```rust
   /// What: Brief description of what the function does.
   ///
   /// Inputs:
   /// - `param1`: Description of parameter 1
   /// - `param2`: Description of parameter 2
   ///
   /// Output:
   /// - Description of return value or side effects
   ///
   /// Details:
   /// - Additional context, edge cases, or important notes
   pub fn example_function(param1: Type1, param2: Type2) -> Result<Type3> {
       // implementation
   }
   ```

2. **Documentation should include:**
   - **What**: What the function/method does
   - **Inputs**: All parameters with descriptions
   - **Output**: Return value, side effects, or state changes
   - **Details**: Important implementation details, edge cases, or usage notes

### Testing requirements

**For bug fixes:**
1. **Create failing tests first** that reproduce the issue
2. Fix the bug
3. Verify tests pass
4. Add additional tests for edge cases if applicable

**For new features:**
1. Add unit tests for new functions/methods
2. Add integration tests for new workflows
3. Test error cases and edge conditions
4. Ensure tests are meaningful and cover the functionality

**Test guidelines:**
- Use `#[allow(clippy::cognitive_complexity, clippy::too_many_lines)]` for complex integration tests if needed
- Tests should be deterministic and not rely on external state
- Use `--dry-run` in tests that would modify the system

## Commit and branch guidelines

### Branch naming
- `feat/<short-description>` — New features
- `fix/<short-description>` — Bug fixes
- `docs/<short-description>` — Documentation only
- `refactor/<short-description>` — Code refactoring
- `test/<short-description>` — Test additions/updates
- `chore/<short-description>` — Build/infrastructure changes

### Commit messages
Prefer [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>: <short summary>

<optional longer description>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `refactor`: Code refactoring (no functional change)
- `perf`: Performance improvements
- `test`: Test additions or updates
- `chore`: Build/infrastructure changes
- `ui`: Visual/interaction changes
- `breaking change`: Incompatible behavior changes

**Examples:**
```
feat: add fuzzy search functionality

- Implemented fzf-style matching for package searches
- Added CTRL+F toggle for fuzzy search mode
- Updated localization files for new feature
```

```
fix: resolve reverse dependencies in preflight modal

Fixes issue where reverse dependencies were not shown when
removing packages. Now correctly displays packages that depend
on the package being removed.
```

**Guidelines:**
- Keep commits focused and reasonably small
- Add rationale in the body if the change is non-obvious
- Reference issue numbers if applicable: `Closes #123` or `Fixes #456`

## Pull Request process

### Before opening a PR

1. **Ensure all quality checks pass:**
   - [ ] `cargo fmt --all` (no changes needed)
   - [ ] `cargo clippy --all-targets --all-features -- -D warnings` (clean)
   - [ ] `cargo check` (compiles successfully)
   - [ ] `cargo test -- --test-threads=1` (all tests pass)
   - [ ] Complexity checks pass for new code

2. **Code requirements:**
   - [ ] All new functions/methods have rustdoc comments (What, Inputs, Output, Details)
   - [ ] Code follows project conventions (see below)
   - [ ] No `unwrap()` or `expect()` in non-test code (use proper error handling)
   - [ ] Complexity thresholds met (cyclomatic < 25, data flow < 25)

3. **Testing:**
   - [ ] Added/updated tests where it makes sense
   - [ ] For bug fixes: created failing tests first, then fixed the issue
   - [ ] Tests are meaningful and cover the functionality

4. **Documentation:**
   - [ ] Updated README if behavior, options, or keybinds changed
   - [ ] Updated wiki pages if needed (see [Wiki](https://github.com/Firstp1ck/Pacsea/wiki))
   - [ ] Updated config examples if config keys changed
   - [ ] For UI changes: included screenshots and updated `Images/` if applicable

5. **Compatibility:**
   - [ ] Changes respect `--dry-run` flag
   - [ ] Code degrades gracefully if `pacman`/`paru`/`yay` are unavailable
   - [ ] No breaking changes (or clearly documented if intentional)

### PR description template

Use the following structure for your PR description (see `Documents/PR_DESCRIPTION.md` for a template):

```markdown
## Summary
Brief description of what this PR does.

**Bug Fixes:**
1. Description of bug fix 1
2. Description of bug fix 2

**New Features:**
1. Description of new feature 1
2. Description of new feature 2

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
Closes #123

## How to test
Step-by-step testing instructions.

## Checklist
- [ ] Code compiles locally
- [ ] `cargo fmt --all` ran without changes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is clean
- [ ] `cargo test -- --test-threads=1` passes
- [ ] Added or updated tests where it makes sense
- [ ] Updated docs if behavior, options, or keybinds changed
- [ ] For UI changes: included screenshots and updated `Images/` if applicable
- [ ] Changes respect `--dry-run` and degrade gracefully if `pacman`/`paru`/`yay` are unavailable
- [ ] If config keys changed: updated README/wiki sections for `settings.conf`, `theme.conf`, and `keybinds.conf`
- [ ] Not a packaging change for AUR (otherwise propose in `pacsea-bin` or `pacsea-git` repos)

## Notes for reviewers
Any additional context, implementation details, or decisions that reviewers should know.

## Breaking changes
None (or description of breaking changes if applicable)

## Configuration
If new config keys were added, document them here.
```

## Project conventions

### Code style
- **Language**: Rust (edition 2024)
- **Naming**: Clear, descriptive names. Favor clarity over brevity.
- **Error handling**: Use `Result` types. Avoid `unwrap()`/`expect()` in non-test code.
- **Early returns**: Prefer early returns over deep nesting.
- **Logging**: Use `tracing` for diagnostics. Avoid noisy logs at info level.

### UX guidelines
- **Keyboard-first**: Minimal keystrokes, Vim-friendly navigation
- **Help overlay**: Update help overlay if shortcuts change
- **Keybind docs**: Update [Keyboard Shortcuts wiki](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts) if keybinds change

### Configuration
- **Config keys**: If you add/change config keys:
  - Update `config/settings.conf`, `config/theme.conf`, or `config/keybinds.conf` examples
  - Update [Configuration wiki](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
  - Update README if it's a major feature
  - Ensure backward compatibility when possible

### Platform behavior
- **Dry-run**: All commands must respect `--dry-run` flag
- **Graceful degradation**: Commands must degrade gracefully if `pacman`/`paru`/`yay` are unavailable
- **Error messages**: Provide clear, actionable error messages

### Documentation updates
When updating documentation:

1. **README.md**: Keep high-level, reference wiki for details
2. **Wiki pages**: Update relevant wiki pages with detailed information:
   - [How to use Pacsea](https://github.com/Firstp1ck/Pacsea/wiki/How-to-use-Pacsea)
   - [Configuration](https://github.com/Firstp1ck/Pacsea/wiki/Configuration)
   - [Keyboard Shortcuts](https://github.com/Firstp1ck/Pacsea/wiki/Keyboard-Shortcuts)
   - [Installation](https://github.com/Firstp1ck/Pacsea/wiki/Installation)
   - [Troubleshooting](https://github.com/Firstp1ck/Pacsea/wiki/Troubleshooting)

3. **Config examples**: Update example config files in `config/` directory

## Filing issues

### Bug reports
Include the following information:
- Pacsea version (e.g., `0.5.2` or commit hash)
- Arch Linux version or distribution
- Terminal emulator and version
- Display server (Wayland/X11)
- AUR helper (`paru`/`yay`) and version
- Steps to reproduce
- Expected vs. actual behavior
- Logs (run with `RUST_LOG=pacsea=debug` or `--verbose`)

**Example:**
```markdown
**Version**: 0.5.2
**Distribution**: Arch Linux (kernel 6.x)
**Terminal**: alacritty 0.13.x
**Display**: Wayland
**AUR Helper**: paru 1.11.x

**Steps to reproduce:**
1. Launch pacsea
2. Search for "firefox"
3. Press Space to add to install list
4. Press Enter

**Expected**: Preflight modal opens
**Actual**: Application crashes

**Logs:**
[Include relevant log output]
```

### Feature requests
- Describe the problem being solved
- Describe the desired UX/behavior
- Include mockups or keybind suggestions if applicable
- Consider edge cases and compatibility

Open issues in the [issue tracker](https://github.com/Firstp1ck/Pacsea/issues).

## Packaging notes
- AUR packages live in separate repos: `pacsea-bin` and `pacsea-git`
- Propose packaging changes in those repos rather than here
- Version bumps should be coordinated with AUR package maintainers

## Code of Conduct and Security
- **Code of Conduct**: See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). For conduct issues, contact firstpick1992@proton.me.
- **Security Policy**: See [SECURITY.md](SECURITY.md) for how to report vulnerabilities.

## Getting help
- Check the [Wiki](https://github.com/Firstp1ck/Pacsea/wiki) for documentation
- Review existing issues and PRs
- Ask questions in [Discussions](https://github.com/Firstp1ck/Pacsea/discussions)

Thank you for helping improve Pacsea!

<details>
  <summary>Support indirectly</summary>
   
If you want to support the project, please vote on the AUR: [Pacsea](https://aur.archlinux.org/packages/pacsea-bin)
   
If Pacsea has been helpful, you can support ongoing development here ❤️ [Patreon](https://www.patreon.com/Firstp1ck)
</details>
