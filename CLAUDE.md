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

**CI note:** `.github/workflows/rust.yml` runs `cargo build` and `cargo test` (with `--test-threads=1`). `.github/workflows/lint.yml` runs `cargo fmt --check` and `cargo clippy ... -D warnings` on pushes/PRs to `main`. `.github/workflows/security.yml` runs supply-chain and secret scans (no clippy). `cargo check` is still required locally before considering work done.

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

## Security rules

These rules exist to prevent specific vulnerability classes identified in `dev/SECURITY_AUDIT_REPORT.md`. They are **mandatory** — not suggestions.

### Shell command construction

- **Never interpolate package names, file paths, or user input directly into shell command strings.** Always pass them through `shell_single_quote()` from `src/install/utils.rs` first.
- When building shell commands with multiple package names, quote **each name individually** before joining:
  ```rust
  let quoted: Vec<String> = names.iter().map(|n| shell_single_quote(n)).collect();
  let names_str = quoted.join(" ");
  ```
- When adding a new function in `src/install/` that constructs shell commands, verify that **every** variable fragment interpolated into `format!()` is either:
  1. A constant/static string, **or**
  2. Passed through `shell_single_quote()`, **or**
  3. Validated against `^[a-z\d@._+-]+$` before use.
- **Never use `format!()` with `sh -c` and unsanitized user data.** Prefer `Command::new().arg()` argument passing over string interpolation when possible — `Command` arguments are not interpreted by a shell.
- On Windows, never pass unescaped strings into `cmd /C` or `cmd /K`. Use PowerShell with proper quoting or escape `&`, `|`, `>`, `<`, `^` for cmd.exe.

### Password and credential handling

- **Never log passwords.** Any function that writes command strings to disk (log files, temp scripts) must redact password pipe patterns (`printf '%s\n' '...' | sudo -S`) before writing. Replace the password with `[REDACTED]`.
- **Never store passwords in plain `String`.** Use `SecureString` (zeroize-backed) from `src/state/types.rs` for all password fields. If `SecureString` does not exist yet, use `String` but add a `// TODO: migrate to SecureString` comment and a `zeroize` call on the containing struct's `Drop`.
- When creating files that might contain sensitive data (log files, temp scripts), use `OpenOptions::mode(0o600)` on Unix to prevent world-readable permissions.
- Never include passwords or credentials in `tracing::info!`, `tracing::debug!`, or `tracing::warn!` output. If you need to log that a password was provided, log `password_provided = true` — never the value.

### Network and HTTP

- All curl invocations must go through `curl_args()` in `src/util/mod.rs`. Do not construct raw curl commands.
- `curl_args()` must include `--max-filesize 10485760` (10 MB) to prevent memory exhaustion from oversized responses. If this is not yet present, add it.
- Never disable TLS certificate verification (`-k` / `--insecure`) on Linux builds. The Windows `-k` flag is a known issue tracked for removal.
- URLs constructed from user input or API data must be validated to start with `http://` or `https://` before passing to curl. See `looks_like_http_url()` in `src/logic/repos/apply_plan.rs` for the pattern.

### File system safety

- **Validate all paths before writing.** Use or extend `is_safe_abs_path()` from `src/logic/repos/apply_plan.rs` for any privileged file write. Reject paths containing `..`.
- **Create temp files atomically.** Use `OpenOptions::create_new(true).mode(0o700)` instead of `fs::write()` followed by `set_permissions()`. The `create_new` flag prevents symlink-following TOCTOU races.
- When writing files under `~/.config/pacsea/`, create parent directories with `create_dir_all` but do not set overly permissive modes. Default umask-derived permissions are acceptable for non-sensitive files; use `0o600` for anything that could contain credentials.

### Test environment overrides

- Functions that check `PACSEA_INTEGRATION_TEST` or `PACSEA_TEST_*` environment variables must **not** bypass security checks in release builds. Gate them with `#[cfg(debug_assertions)]` or `#[cfg(test)]` so they compile to no-ops in release mode.
- Never add new environment-variable-based test overrides that skip authentication, privilege checks, or input validation without a `#[cfg(debug_assertions)]` guard.

### Dependency management

- Run `cargo audit` after adding or updating dependencies. Resolve all **critical** and **high** advisories before merging.
- Prefer direct dependencies over transitive ones for security-sensitive functionality. If a transitive dependency has an advisory, check if the parent crate can be updated to pull a fixed version.
- Do not add dependencies that require `unsafe` for their core functionality unless there is no safe alternative and the crate is well-maintained.

### Audit reference

Full findings and remediation steps: `dev/SECURITY_AUDIT_REPORT.md` and `dev/SECURITY_REMEDIATION_GUIDE.md`.

## General rules
- No unsolicited `*.md` / wiki / README edits.
- Preserve dry-run semantics and graceful handling of missing external tools.
- Keep PR description files in `dev/PR/` in sync with the branch when that workflow applies.
