# Rust Development Rules for AI Agents

## When Creating New Code (Files, Functions, Methods, Enums)
- Keep **cognitive complexity** below the threshold of **25** (see **Complexity and linting** below).
- Keep functions under **150 lines** (enforced by Clippy `too_many_lines`).
- Prefer straightforward **data flow** (fewer threaded parameters, clearer state boundaries). This is a **design guideline**—there is no compiler lint for it; match patterns used in existing modules.
- Add `///` rustdoc to all new public **and** private items (`missing_docs` + `missing_docs_in_private_items` are both enforced; see **Lint configuration**).
- Use the **What / Inputs / Output / Details** rustdoc layout for non-trivial APIs (see **Documentation** for the template).
- Add focused **unit** tests for new logic.
- Add **integration** tests when behavior crosses modules or the CLI boundary.

## When Fixing Bugs/Issues
1. Identify the root cause before writing code.
2. Write or adjust a test that **fails** on the bug.
3. Run the test — it must fail. If it passes, the test does not reproduce the issue; adjust it.
4. Fix the bug.
5. Run the test again — it must pass. If not, iterate on the fix.
6. Add edge-case tests when they reduce future regressions.

## Always Run After Changes
Run from the repository root, in this order:
1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo check`
4. `cargo test -- --test-threads=1`

**CI note:** `.github/workflows/rust.yml` only runs `cargo build` and `cargo test` (with `--test-threads=1`). It does **not** run `fmt`, `clippy`, or `check`. Those are still required locally before considering work done.

## Lint configuration (source of truth)

**`Cargo.toml` — `[lints.clippy]`** (excerpt; see file for full list):
```toml
[lints.clippy]
cognitive_complexity = "warn"
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
unwrap_used = "deny"
missing_docs_in_private_items = "warn"
```

**`Cargo.toml` — `[lints.rust]`:**
```toml
[lints.rust]
missing_docs = "warn"
```

**`clippy.toml`:**
- `cognitive-complexity-threshold = 25` — used by Clippy's **`cognitive_complexity`** lint (not cyclomatic complexity; that is a different metric).
- `too-many-lines-threshold = 150` — used by Clippy's **`too_many_lines`** lint.

With `cargo clippy ... -- -D warnings`, **all warnings become errors**. That means `cognitive_complexity`, `missing_docs`, and `missing_docs_in_private_items` violations will **fail** the Clippy run.

## Code Quality Requirements

### Pre-commit checklist
Before completing any task, ensure all of the following pass:
1. **Format:** `cargo fmt --all` produces no diff.
2. **Clippy:** `cargo clippy --all-targets --all-features -- -D warnings` is clean.
3. **Compile:** `cargo check` succeeds.
4. **Tests:** `cargo test -- --test-threads=1` — all tests pass.
5. **Complexity:** New functions stay under cognitive-complexity threshold (25).
6. **Length:** New functions stay under too-many-lines threshold (150).
7. **Exceptions:** If a threshold cannot be met, add a **documented** `#[allow(...)]` with a justification comment. Use sparingly.

### Documentation
- **All** new public and private functions, methods, structs, enums, traits, and modules must have `///` rustdoc comments.
  - `missing_docs` fires on public items.
  - `missing_docs_in_private_items` fires on private items.
  - Both are **warn**, promoted to **error** by `-D warnings`.
- For non-trivial APIs, use the structured rustdoc layout with **What**, **Inputs**, **Output**, and **Details** sections:
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
- Documentation must include all four sections: **What**, **Inputs**, **Output**, and **Details**.

### Testing

**For bug fixes:**
1. Create a failing test that reproduces the issue.
2. Fix the bug.
3. Verify the test passes.
4. Add additional edge-case tests if applicable.

**For new features:**
1. Add unit tests for new functions/methods.
2. Add integration tests for new workflows.
3. Test error cases and edge conditions.
4. Ensure tests are meaningful and cover the functionality.

**Test guidelines:**
- Tests must be deterministic and not rely on undeclared external machine state.
- For code paths that would mutate the system, exercise **dry-run** behavior via the `dry_run` bool field (wired from the CLI `--dry-run` flag through the app), or use equivalent test doubles. Do **not** blindly pass `--dry-run` to every shell command.
- Always run tests with `--test-threads=1` to avoid parallel interference.

## Code style conventions
- **Edition:** Rust 2024 (see `Cargo.toml`).
- **Naming:** Clear and descriptive; clarity over brevity.
- **Errors:** Prefer `Result`; never use `unwrap()` or `expect()` outside tests (enforced by `unwrap_used = "deny"`).
- **Control flow:** Prefer early returns over deep nesting.
- **Logging:** Use `tracing`; avoid noisy `info!` in hot paths.

## Platform behavior

### Dry-run
- All code paths that modify the system must respect the application's **dry-run** mode.
- The CLI `--dry-run` flag wires into a `dry_run` bool through the app.
- In dry-run mode, do not execute mutating commands; simulate or no-op as existing executors do.
- When implementing new features that execute system commands, always check the `dry_run` flag first.

### Graceful degradation
- Do not assume `pacman`, AUR helpers (`paru`, `yay`, etc.), or other external tools are installed.
- Handle their absence with clear, actionable error messages.
- Never crash or panic when a tool is missing.

### Error messages
- User-facing errors must say **what** failed and **what the user can do** next.
- Provide clear, actionable guidance — not raw error codes or stack traces.

## Configuration updates
If config keys or schema change:
- Update shipped examples under `config/` (`settings.conf`, `theme.conf`, `keybinds.conf`, and related files as needed).
- Ensure backward compatibility when possible.
- Do **not** edit wiki or `README.md` unless the user explicitly asks (see **Documentation policy**).

## UX guidelines
- **Keyboard-first:** Design for minimal keystrokes, Vim-friendly navigation.
- **Help overlay:** If default keybinds change, update the in-app help overlay.
- **Keybind consistency:** Maintain consistency with existing keybind patterns.

## Documentation policy
- Do **not** create or edit `*.md` files (including `README.md`) unless explicitly requested.
- Do **not** edit wiki content unless explicitly requested.
- Prefer rustdoc for code documentation.
- Use internal `dev/` docs only when the user explicitly asks.

## Pull request files (`dev/PR/`)
- **Template:** `.github/PULL_REQUEST_TEMPLATE.md`.
- **Creating:** If no PR file exists in `dev/PR/` for the current branch, create one based on the template. Name it `PR_<branch-name>.md` or `PR_<short-description>.md`.
- **Updating:** If a PR file already exists, **always update it** when changes are made to the codebase.
- **Content:** Document only changes that differ from the **main** branch. Remove entries for changes that were reverted. Focus on the final state, not intermediate iterations.

## Complexity and linting (summary)
| Concern | Enforcement | Threshold |
|---------|-------------|-----------|
| Cognitive complexity | Clippy `cognitive_complexity` + `clippy.toml` | 25 |
| Function length | Clippy `too_many_lines` + `clippy.toml` | 150 lines |
| Data flow / coupling | Manual design review; no lint — match existing module patterns | N/A |

## General rules
- No unsolicited `*.md` / wiki / README edits.
- Preserve dry-run semantics and graceful handling of missing external tools.
- Keep PR description files in `dev/PR/` in sync with the branch when that workflow applies.
