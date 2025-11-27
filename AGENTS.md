# Rust Development Rules for AI Agents

## When Creating New Code (Files, Functions, Methods, Enums)
- Always check cyclomatic complexity < 25
- Always check data flow complexity < 25
- Always add rust docs to classes/methods/functions (What, Inputs, Output, Details)
- Always add logical important unit tests
- Always add logical important integration tests

## When Fixing Bugs/Issues
- Check deeply what the issue is
- Create tests that fail for the specific issue
- Run the created tests: it should fail, if not adjust the test
- Solve the issue
- Check if test passes, if not continue trying other solutions

## Always Run After Changes
- cargo fmt --all
- cargo clippy --all-targets --all-features -- -D warnings
- cargo check
- cargo test -- --test-threads=1

## Cargo Clippy Configuration
Check with cargo clippy after adding a new feature and fix clippy errors with the following settings:
```toml
[lints.clippy]
# Enable cognitive complexity lint to catch overly complex functions
cognitive_complexity = "warn"
pedantic = { level = "deny", priority = -1 }
nursery = { level = "deny", priority = -1 }
unwrap_used = "deny"
```

Additional settings in `clippy.toml`:
- `cognitive-complexity-threshold = 25`
- `too-many-lines-threshold = 150`

## Code Quality Requirements

### Pre-commit Checklist
Before completing any task, ensure all of the following pass:
1. **Format code**: `cargo fmt --all` (must produce no changes)
2. **Lint with Clippy**: `cargo clippy --all-targets --all-features -- -D warnings` (must be clean)
3. **Check compilation**: `cargo check` (must compile successfully)
4. **Run tests**: `cargo test -- --test-threads=1` (all tests must pass)
5. **Check complexity**: For new code, verify cyclomatic complexity < 25 and data flow complexity < 25

### Code Documentation Requirements
For all new code (functions, methods, structs, enums):
- **Always add Rust documentation comments** with the following format:
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
- Documentation must include: **What**, **Inputs**, **Output**, and **Details** sections

### Testing Requirements

**For bug fixes:**
1. Create failing tests first that reproduce the issue
2. Fix the bug
3. Verify tests pass
4. Add additional tests for edge cases if applicable

**For new features:**
1. Add unit tests for new functions/methods
2. Add integration tests for new workflows
3. Test error cases and edge conditions
4. Ensure tests are meaningful and cover the functionality

**Test guidelines:**
- Tests must be deterministic and not rely on external state
- Use `--dry-run` in tests that would modify the system
- Tests must be run with `--test-threads=1` to avoid race conditions

## Code Style Conventions
- **Language**: Rust (edition 2024)
- **Naming**: Clear, descriptive names. Favor clarity over brevity.
- **Error handling**: Use `Result` types. Never use `unwrap()` or `expect()` in non-test code.
- **Early returns**: Prefer early returns over deep nesting.
- **Logging**: Use `tracing` for diagnostics. Avoid noisy logs at info level.

## Platform Behavior Requirements

### Dry-run Support
- All commands that modify the system MUST respect the `--dry-run` flag
- When implementing new features that execute system commands, always check for `--dry-run` flag first
- In dry-run mode, simulate the operation without actually executing it

### Graceful Degradation
- Code MUST degrade gracefully if `pacman`/`paru`/`yay` are unavailable
- Never assume these tools are always present
- Provide clear, actionable error messages when tools are missing

### Error Messages
- Always provide clear, actionable error messages
- Error messages should help users understand what went wrong and how to fix it

## Configuration Updates
If you add or change config keys:
- Update `config/settings.conf`, `config/theme.conf`, or `config/keybinds.conf` examples
- Ensure backward compatibility when possible
- Do NOT update wiki pages or README unless explicitly asked (see General Rules)

## UX Guidelines
- **Keyboard-first**: Design for minimal keystrokes, Vim-friendly navigation
- **Help overlay**: If shortcuts change, update help overlay
- **Keybind consistency**: Maintain consistency with existing keybind patterns

## Documentation Updates
- **Do NOT create or update *.md files, unless explicitly asked**
- **Do NOT update wiki pages, unless explicitly asked**
- **Do NOT update README, unless explicitly asked**
- Focus on code implementation and inline documentation (rustdoc comments)

## Pull Request File Management
- **PR file location**: PR files are stored in `dev/PR/` directory
- **PR file template**: Use `.github/PULL_REQUEST_TEMPLATE.md` as the template when creating new PR files
- **Creating PR files**:
  - If no PR file exists in `dev/PR/` for the current branch, create one based on the template
  - Name the file following the pattern `PR_<branch-name>.md` or `PR_<description>.md`
  - Fill in all relevant sections from the template
- **Updating PR files**:
  - If a PR file already exists, **always update it** when changes are made to the codebase
  - Document changes relative to the main branch only (not intermediate changes within the current branch)
  - When a change is reversed, **remove it from the PR file** (do not document reversals)
  - Keep the PR file synchronized with the actual state of changes in the branch
- **Change documentation**:
  - Only document changes that differ from the main branch
  - Do not document intermediate iterations or changes that were later reverted
  - Focus on the final state of changes that will be merged

## Complexity Thresholds
- **Cyclomatic complexity**: Must be < 25 for all new functions
- **Data flow complexity**: Must be < 25 for all new functions
- **Function length**: Should not exceed 150 lines (too-many-lines-threshold)

## General Rules
- Do not create *.md files, unless explicitly asked
- Do not update wiki pages, unless explicitly asked
- Do not update README, unless explicitly asked
- Focus on code quality, tests, and inline documentation
- All changes must respect `--dry-run` flag
- All changes must degrade gracefully if system tools are unavailable
