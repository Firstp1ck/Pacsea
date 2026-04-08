# Security Practices Guide

General guidance for preventing and reducing security vulnerabilities in Pacsea. For the specific findings that motivated these practices, see `dev/SECURITY_AUDIT_REPORT.md`.

---

## 1. Shell Command Construction

Pacsea's core function is building and executing shell commands with privilege escalation. This is the highest-risk surface area.

### Treat all external data as hostile

Any value that originates outside the compiled binary — package names from APIs, config file values, file paths, user input — must be treated as potentially containing shell metacharacters. This applies even when the upstream source (AUR, pacman) enforces its own validation, because:

- Data can be corrupted in transit.
- A future code path might introduce values from a less-validated source.
- Defense-in-depth means not relying on a single trust boundary.

### Quoting strategy

Use `shell_single_quote()` from `src/install/utils.rs` for every variable value interpolated into a shell string. Single-quoting is the safest POSIX shell quoting — only the `'` character itself needs escaping, and the function handles that correctly.

When building commands with multiple values (batch package names), quote each value individually before joining:

```rust
let safe = names.iter().map(|n| shell_single_quote(n)).collect::<Vec<_>>().join(" ");
```

Never join first, then quote — that would treat all names as a single argument.

### Prefer `Command::new().arg()` over string interpolation

Rust's `std::process::Command` passes arguments directly to the kernel (no shell interpretation). When you don't need shell features (pipes, redirects, `&&` chains), always use `.arg()`:

```rust
// Good: no shell interpretation
Command::new("pacman").arg("-Qi").arg(&package_name).output()

// Risky: shell interprets the string
Command::new("sh").arg("-c").arg(format!("pacman -Qi {package_name}")).output()
```

The second form is only necessary when the command requires shell features. In those cases, quote every variable fragment.

### Input validation as a second layer

For known formats (package names, repo names, key fingerprints), validate the character set before using the value. This catches bugs early and provides clear error messages. Validation complements quoting — use both.

---

## 2. Credential Handling

### Minimize credential lifetime

Credentials (passwords, tokens) should exist in memory for the shortest possible time. Acquire → use → destroy. Don't cache them in application state longer than needed for the immediate operation.

### Zero memory on drop

Standard Rust `String` and `Vec<u8>` do not clear their contents when deallocated. Use a zeroizing wrapper (e.g., the `zeroize` or `secrecy` crate) for any type that holds a password, token, or private key material. This prevents credentials from lingering on the heap or being swapped to disk.

### Never log credentials

Any function that writes to disk (log files, temp scripts, debug output) must redact credential material before writing. Common patterns to watch for:

- Password pipe commands: `printf '%s\n' '<password>' | sudo -S ...`
- Authentication headers: `Authorization: Bearer ...`
- SSH key paths: `/home/user/.ssh/id_ed25519`

When logging that a credential was used, log a boolean (`password_provided = true`), never the value.

### Restrict file permissions on sensitive outputs

Any file that could contain credential traces — log files, temp scripts, state persistence files — should be created with `0o600` permissions on Unix. Use `OpenOptions::mode(0o600)` at creation time rather than writing first and tightening permissions later (that creates a TOCTOU window).

---

## 3. Network Security

### Always validate TLS

Never disable certificate verification (`-k`, `--insecure`, `danger_accept_invalid_certs`). If TLS doesn't work, fix the root cause (missing CA bundle, expired cert, misconfigured system trust store) rather than disabling verification entirely.

### Bound response sizes

Set `--max-filesize` (curl) or equivalent response body limits on all HTTP requests. A compromised or misconfigured server returning gigabytes of data can exhaust memory. Choose a limit that's generous for legitimate responses but prevents OOM — 10 MB is reasonable for API responses.

### Validate URLs before use

When constructing URLs from config values, API responses, or user input, verify the scheme is `http://` or `https://`. Reject `file://`, `ftp://`, `data:`, and other schemes that could leak local files or bypass network controls.

### Centralize HTTP configuration

Route all HTTP requests through a single function (`curl_args`, reqwest client builder, etc.) so timeouts, headers, TLS settings, and size limits are applied uniformly. Avoid constructing one-off curl commands that bypass the central configuration.

### Keep TLS dependencies current

TLS libraries (rustls, aws-lc-sys, openssl-sys) are frequent advisory targets. Run `cargo audit` after every dependency update and before every release. Treat high-severity TLS advisories as release-blockers.

---

## 4. File System Safety

### Validate paths before privileged writes

Before writing to system paths (anything under `/etc/`, `/usr/`, etc.), validate:

- The path is absolute (starts with `/`).
- It contains no `..` segments.
- It only uses expected characters (alphanumeric, `/`, `-`, `.`, `_`).
- It's within the expected directory tree.

### Create temp files atomically

Use `OpenOptions::create_new(true)` when creating temporary files. The `create_new` flag fails if the file already exists, preventing symlink-following attacks where an attacker pre-creates a symlink at the expected temp path pointing to a sensitive file.

Combine with `.mode(0o700)` (or `0o600` for non-executable files) so the file is created with correct permissions from the start — no window where it's world-readable.

### Clean up temp files

Temp files containing command strings or credential material should be removed after use. Schedule cleanup on the success path and handle the failure path (process crash leaves orphaned files). Consider a startup cleanup pass that removes stale `pacsea_*` files from `/tmp`.

### Guard `.gitignore`

Include patterns for common secret file types (`.env`, `*.pem`, `*.key`, `credentials.json`) even if the project doesn't currently use them. This prevents accidental commits by contributors who create local test configs.

---

## 5. Test vs. Production Boundaries

### Compile out test overrides in release builds

Environment-variable-based test overrides that bypass authentication, privilege checks, or input validation are dangerous if they're reachable in production binaries. Gate them with `#[cfg(debug_assertions)]` or `#[cfg(test)]` so they compile to no-ops in release mode.

```rust
fn is_test_override_active() -> bool {
    #[cfg(debug_assertions)]
    { std::env::var("MY_TEST_OVERRIDE").is_ok_and(|v| v == "1") }
    #[cfg(not(debug_assertions))]
    { false }
}
```

### Don't trust environment variables for security decisions

If an attacker can set environment variables for the process, they can already do significant damage (PATH hijacking, LD_PRELOAD, etc.). But don't make it easier — don't let env vars skip privilege checks, disable TLS, or bypass input validation in production builds.

---

## 6. Dependency Hygiene

### Audit regularly

```bash
cargo audit              # Check for known advisories
cargo outdated           # Check for available updates
cargo tree --duplicates  # Check for duplicate dependency versions
```

Run `cargo audit` as part of the pre-release checklist and ideally in CI. High/critical advisories should block releases.

### Evaluate transitive dependencies

Security vulnerabilities in transitive dependencies (deps of deps) are common and easy to miss. When `cargo audit` reports a transitive advisory, check:

1. Can the parent crate be updated to pull the fix? (`cargo update -p <parent>`)
2. Is the vulnerable code path actually reachable from Pacsea's usage?
3. Is the parent crate maintained? If not, consider alternatives.

### Minimize the dependency tree

Fewer dependencies means fewer attack surfaces. Before adding a new crate:

- Check its maintenance status (last commit, issue response time).
- Check its dependency count (large trees increase transitive risk).
- Check for existing advisories.
- Consider whether the functionality can be implemented in a few dozen lines instead.

---

## 7. Error Messages and Information Disclosure

### Scrub sensitive data from error outputs

Error messages shown to users or written to logs should not contain:

- File system paths to SSH keys, credentials, or home directories.
- Raw API responses that might contain tokens.
- Full stack traces that reveal internal structure.

Filter or truncate external error content before displaying it. The existing `sanitize_stderr` pattern (filtering lines containing `/.ssh/` or `identity file`) is a good model.

### Be specific about what failed, vague about internals

Good: "SSH auth failed. Ensure your SSH key is uploaded to your AUR account."
Bad: "Permission denied (publickey) for /home/user/.ssh/id_ed25519 connecting to aur@aur.archlinux.org:22"

The user needs to know what to do, not the exact internal state.

---

## 8. Ongoing Maintenance

### Periodic re-audits

Security is not a one-time activity. Re-audit when:

- Major features are added (new shell command paths, new network endpoints, new auth flows).
- Dependencies are significantly updated.
- The threat model changes (e.g., adding multi-user support, adding a network-facing API).

### Regression tests for security fixes

When fixing a security issue, add a test that would have caught the vulnerability. This ensures the same class of bug doesn't reappear. For shell injection, test with metacharacters in package names. For credential leaks, assert that log output contains `[REDACTED]` not plaintext passwords.

### Document security-relevant design decisions

When a function handles credentials, constructs privileged commands, or makes trust decisions, document the security rationale in rustdoc. Future contributors (human or AI) need to understand why a particular approach was chosen so they don't accidentally weaken it during refactoring.
