#[cfg(target_os = "windows")]
/// What: Resolve an executable on `PATH` (Windows).
///
/// Input:
/// - `cmd`: Executable name to probe.
///
/// Output:
/// - `Some(path)` when `which` resolves the command; otherwise `None`.
///
/// Details:
/// - Uses the `which` crate so `PATHEXT` and Windows search rules match `command_on_path`.
#[must_use]
pub fn resolve_command_on_path(cmd: &str) -> Option<std::path::PathBuf> {
    which::which(cmd).ok()
}

#[cfg(target_os = "windows")]
/// What: Determine whether a command is available on the Windows `PATH`.
///
/// Input:
/// - `cmd`: Executable name to probe.
///
/// Output:
/// - `true` when the command resolves via the `which` crate; otherwise `false`.
///
/// Details:
/// - Delegates to `resolve_command_on_path` so presence matches actual resolution.
#[must_use]
pub fn command_on_path(cmd: &str) -> bool {
    resolve_command_on_path(cmd).is_some()
}

#[cfg(target_os = "windows")]
/// What: Check if `PowerShell` is available on Windows.
///
/// Output:
/// - `true` when `PowerShell` can be found on PATH; otherwise `false`.
///
/// Details:
/// - Checks for `powershell.exe` or `pwsh.exe` (`PowerShell` Core) on the system.
pub fn is_powershell_available() -> bool {
    command_on_path("powershell.exe") || command_on_path("pwsh.exe")
}

#[cfg(target_os = "windows")]
/// What: Build a safe `cmd.exe` echo command for arbitrary text.
///
/// Inputs:
/// - `msg`: Message text displayed by `cmd /K`.
///
/// Output:
/// - A command string beginning with `echo(` and escaped for `cmd.exe`.
///
/// Details:
/// - Escapes command metacharacters so text cannot break out of the echo context.
/// - Doubles `%` to prevent environment-variable expansion.
/// - Escapes `!` to avoid delayed-expansion surprises.
/// - Converts newline boundaries into chained `echo(` calls.
#[must_use]
pub fn cmd_echo_command(msg: &str) -> String {
    let mut out = String::from("echo(");
    for ch in msg.chars() {
        match ch {
            '\r' => {}
            '\n' => out.push_str(" & echo("),
            '^' | '&' | '|' | '<' | '>' | '(' | ')' => {
                out.push('^');
                out.push(ch);
            }
            '%' => out.push_str("%%"),
            '!' => out.push_str("^!"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(not(target_os = "windows"))]
/// What: Return whether `p` is a regular file with at least one executable bit set (Unix).
///
/// Input:
/// - `p`: Filesystem path to inspect.
///
/// Output:
/// - `true` when the path is a file and mode includes any execute bit; otherwise `false`.
///
/// Details:
/// - Used by `resolve_command_on_path` and terminal discovery so “on PATH” matches the shell.
#[must_use]
fn path_is_executable(p: &std::path::Path) -> bool {
    if !p.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p)
            .map(|meta| meta.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Resolve an executable on `PATH` or by explicit path (Unix).
///
/// Input:
/// - `cmd`: Program basename or path containing `MAIN_SEPARATOR`.
///
/// Output:
/// - `Some(path)` for the first executable match; otherwise `None`.
///
/// Details:
/// - Honour Unix permission bits so a non-executable file on `PATH` is not treated as a tool.
#[must_use]
pub fn resolve_command_on_path(cmd: &str) -> Option<std::path::PathBuf> {
    use std::path::Path;

    if cmd.contains(std::path::MAIN_SEPARATOR) {
        let p = Path::new(cmd);
        return path_is_executable(p).then(|| p.to_path_buf());
    }

    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(cmd);
        if path_is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
/// What: Determine whether a command is available on the Unix `PATH`.
///
/// Input:
/// - `cmd`: Program name or explicit path to inspect.
///
/// Output:
/// - `true` when an executable file is found and marked executable.
///
/// Details:
/// - Same rules as `resolve_command_on_path`; kept as a convenience for boolean checks.
#[must_use]
pub fn command_on_path(cmd: &str) -> bool {
    resolve_command_on_path(cmd).is_some()
}

#[cfg(not(target_os = "windows"))]
/// What: Locate the first available terminal executable from a preference list.
///
/// Input:
/// - `terms`: Tuples of `(binary, args, needs_xfce_command)` ordered by preference.
///
/// Output:
/// - `Some(index)` pointing into `terms` when a binary is found; otherwise `None`.
///
/// Details:
/// - Iterates directories in `PATH`, favouring the earliest match respecting executable bits.
pub fn choose_terminal_index_prefer_path(terms: &[(&str, &[&str], bool)]) -> Option<usize> {
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for (i, (name, _args, _hold)) in terms.iter().enumerate() {
                let candidate = dir.join(name);
                if path_is_executable(&candidate) {
                    return Some(i);
                }
            }
        }
    }
    None
}

/// What: Safely single-quote an arbitrary string for POSIX shells.
///
/// Input:
/// - `s`: Text to quote.
///
/// Output:
/// - New string wrapped in single quotes, escaping embedded quotes via the `'
///   '"'"'` sequence.
///
/// Details:
/// - Returns `''` for empty input so the shell treats it as an empty argument.
#[must_use]
pub fn shell_single_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// What: Check whether a package name matches the strict allowlist used for shell-bound install commands.
///
/// Input:
/// - `name`: Candidate package name to validate.
///
/// Output:
/// - `true` when `name` is non-empty and every byte is one of `a-z`, `0-9`, `@`, `.`, `_`, `+`, `-`.
///
/// Details:
/// - This is a defense-in-depth gate before shell interpolation, matching security guidance from the audit.
/// - The validator is intentionally strict and accepts only lowercase ASCII letters.
#[must_use]
pub fn is_safe_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'@' | b'.' | b'_' | b'+' | b'-')
        })
}

/// What: Validate a list of package names against the strict install-command allowlist.
///
/// Inputs:
/// - `names`: Package names that will be interpolated into shell command strings.
/// - `context`: Human-readable operation context for actionable error messages.
///
/// Output:
/// - `Ok(())` when all names are valid, otherwise `Err` with the first invalid package.
///
/// Details:
/// - Centralises validation so all install builders apply the same safety policy.
/// - Call this before quoting/interpolation and abort command construction when validation fails.
pub fn validate_package_names(names: &[String], context: &str) -> Result<(), String> {
    if let Some(invalid) = names
        .iter()
        .find(|name| !is_safe_package_name(name.as_str()))
    {
        return Err(format!(
            "Invalid package name '{invalid}' for {context}. Allowed pattern: ^[a-z\\d@._+-]+$"
        ));
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
/// Fallback message when no terminal editor is found. Must not contain single quotes (used inside `echo '...'` in shell).
/// Reusable for i18n or logging if needed.
const EDITOR_FALLBACK_MESSAGE: &str = "No terminal editor found (nvim/vim/emacsclient/emacs/hx/helix/nano). Set VISUAL or EDITOR to use your preferred editor.";

#[cfg(not(target_os = "windows"))]
/// What: Build a shell command string that opens a config file in the user's preferred terminal editor.
///
/// Inputs:
/// - `path`: Path to the config file to open.
///
/// Output:
/// - A single shell expression that tries, in order: `$VISUAL`, `$EDITOR`, then the built-in
///   fallback chain (nvim, vim, hx, helix, emacsclient, emacs, nano), and finally a fallback
///   message with `read -rn1 -s _`.
///
/// Details:
/// - The script expects `VISUAL`/`EDITOR` to be runnable commands that accept a file path.
/// - The path is passed through `shell_single_quote` so paths with spaces or single quotes are safe.
/// - Order: VISUAL then EDITOR then nvim → vim → hx → helix → emacsclient -t → emacs -nw → nano.
#[must_use]
pub fn editor_open_config_command(path: &std::path::Path) -> String {
    let path_str = path.display().to_string();
    let path_quoted = shell_single_quote(&path_str);
    // path_quoted is already single-quoted, so the full argument to echo is one safe string.
    format!(
        "( [ -n \"${{VISUAL}}\" ] && command -v \"${{VISUAL%% *}}\" >/dev/null 2>&1 && eval \"${{VISUAL}}\" {path_quoted} ) || \
         ( [ -n \"${{EDITOR}}\" ] && command -v \"${{EDITOR%% *}}\" >/dev/null 2>&1 && eval \"${{EDITOR}}\" {path_quoted} ) || \
         ((command -v nvim >/dev/null 2>&1 || pacman -Qi neovim >/dev/null 2>&1) && nvim {path_quoted}) || \
         ((command -v vim >/dev/null 2>&1 || pacman -Qi vim >/dev/null 2>&1) && vim {path_quoted}) || \
         ((command -v hx >/dev/null 2>&1 || pacman -Qi helix >/dev/null 2>&1) && hx {path_quoted}) || \
         ((command -v helix >/dev/null 2>&1 || pacman -Qi helix >/dev/null 2>&1) && helix {path_quoted}) || \
         ((command -v emacsclient >/dev/null 2>&1 || pacman -Qi emacs >/dev/null 2>&1) && emacsclient -t {path_quoted}) || \
         ((command -v emacs >/dev/null 2>&1 || pacman -Qi emacs >/dev/null 2>&1) && emacs -nw {path_quoted}) || \
         ((command -v nano >/dev/null 2>&1 || pacman -Qi nano >/dev/null 2>&1) && nano {path_quoted}) || \
         (echo '{EDITOR_FALLBACK_MESSAGE}'; echo 'File: {path_quoted}'; read -rn1 -s _ || true)"
    )
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    /// What: Validate that `command_on_path` recognises executables present on the customised `PATH`.
    ///
    /// Inputs:
    /// - Temporary directory containing a shim `mycmd` script made executable.
    /// - Environment `PATH` overridden to reference only the temp directory.
    ///
    /// Output:
    /// - Returns `true` for `mycmd` and `false` for a missing binary, confirming detection logic.
    ///
    /// Details:
    /// - Restores the original `PATH` and cleans up the temporary directory after assertions.
    fn utils_command_on_path_detects_executable() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_utils_path_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut cmd_path = dir.clone();
        cmd_path.push("mycmd");
        fs::write(&cmd_path, b"#!/bin/sh\nexit 0\n").expect("Failed to write test command script");
        let mut perms = fs::metadata(&cmd_path)
            .expect("Failed to read test command script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&cmd_path, perms)
            .expect("Failed to set test command script permissions");

        let orig_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.display().to_string()) };
        assert!(super::command_on_path("mycmd"));
        assert_eq!(
            super::resolve_command_on_path("mycmd").as_deref(),
            Some(cmd_path.as_path())
        );
        assert!(!super::command_on_path("notexist"));
        assert!(super::resolve_command_on_path("notexist").is_none());
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Ensure a non-executable file on `PATH` is not resolved as a command.
    ///
    /// Inputs:
    /// - Temporary directory with a `stub` file mode `0o644` prepended to `PATH`.
    ///
    /// Output:
    /// - `command_on_path` is `false` and `resolve_command_on_path` is `None`.
    ///
    /// Details:
    /// - Guards parity with shell behaviour for PKGBUILD check tool discovery.
    fn utils_resolve_command_on_path_skips_non_executable_file() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_utils_notexec_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut stub = dir.clone();
        stub.push("stub");
        fs::write(&stub, b"not runnable\n").expect("Failed to write stub file");
        let mut perms = fs::metadata(&stub)
            .expect("Failed to read stub metadata")
            .permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&stub, perms).expect("Failed to set non-executable permissions");

        let orig_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.display().to_string()) };
        assert!(!super::command_on_path("stub"));
        assert!(super::resolve_command_on_path("stub").is_none());
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Ensure `choose_terminal_index_prefer_path` honours the preference ordering when multiple terminals exist.
    ///
    /// Inputs:
    /// - Temporary directory with an executable `kitty` shim placed on `PATH`.
    /// - Preference list where `gnome-terminal` precedes `kitty` but is absent.
    ///
    /// Output:
    /// - Function returns index `1`, selecting `kitty`, the first available terminal in the list.
    ///
    /// Details:
    /// - Saves and restores the `PATH` environment variable while ensuring the temp directory is removed.
    fn utils_choose_terminal_index_prefers_first_present_in_terms_order() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;

        let mut dir: PathBuf = std::env::temp_dir();
        dir.push(format!(
            "pacsea_test_utils_terms_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut kitty = dir.clone();
        kitty.push("kitty");
        fs::write(&kitty, b"#!/bin/sh\nexit 0\n").expect("Failed to write test kitty script");
        let mut perms = fs::metadata(&kitty)
            .expect("Failed to read test kitty script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&kitty, perms).expect("Failed to set test kitty script permissions");

        let terms: &[(&str, &[&str], bool)] =
            &[("gnome-terminal", &[], false), ("kitty", &[], false)];
        let orig_path = std::env::var_os("PATH");
        unsafe { std::env::set_var("PATH", dir.display().to_string()) };
        let idx = super::choose_terminal_index_prefer_path(terms).expect("index");
        assert_eq!(idx, 1);
        unsafe {
            if let Some(v) = orig_path {
                std::env::set_var("PATH", v);
            } else {
                std::env::remove_var("PATH");
            }
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    /// What: Check that `shell_single_quote` escapes edge cases safely.
    ///
    /// Inputs:
    /// - Three sample strings: empty, plain ASCII, and text containing a single quote.
    ///
    /// Output:
    /// - Returns properly quoted strings, using `''` for empty and the standard POSIX escape for embedded quotes.
    ///
    /// Details:
    /// - Covers representative cases without filesystem interaction to guard future regressions.
    fn utils_shell_single_quote_handles_edges() {
        assert_eq!(super::shell_single_quote(""), "''");
        assert_eq!(super::shell_single_quote("abc"), "'abc'");
        assert_eq!(super::shell_single_quote("a'b"), "'a'\"'\"'b'");
    }

    #[test]
    /// What: Verify strict package-name validator allows only the documented safe pattern.
    ///
    /// Inputs:
    /// - Representative valid and invalid package names.
    ///
    /// Output:
    /// - Returns `true` for valid lowercase names and `false` for disallowed characters/casing.
    ///
    /// Details:
    /// - Guards the defense-in-depth allowlist used before shell interpolation.
    fn utils_is_safe_package_name_strict_allowlist() {
        assert!(super::is_safe_package_name("ripgrep"));
        assert!(super::is_safe_package_name("lib32-foo+bar"));
        assert!(super::is_safe_package_name("qt6-base@beta.1"));
        assert!(!super::is_safe_package_name(""));
        assert!(!super::is_safe_package_name("Ripgrep"));
        assert!(!super::is_safe_package_name("bad;name"));
        assert!(!super::is_safe_package_name("bad name"));
    }

    #[test]
    /// What: Assert that `editor_open_config_command` builds a command with VISUAL then EDITOR then fallbacks.
    ///
    /// Inputs:
    /// - A dummy path (e.g. `/tmp/settings.conf`).
    ///
    /// Output:
    /// - The returned string contains VISUAL branch before EDITOR branch before nvim fallback.
    ///
    /// Details:
    /// - Shell-only implementation; order is fixed in the string regardless of env.
    fn utils_editor_open_config_command_order_visual_then_editor_then_fallbacks() {
        use std::path::Path;
        let path = Path::new("/tmp/settings.conf");
        let cmd = super::editor_open_config_command(path);
        let idx_visual = cmd.find("VISUAL").expect("command must mention VISUAL");
        let idx_editor = cmd.find("EDITOR").expect("command must mention EDITOR");
        let idx_nvim = cmd
            .find("nvim")
            .expect("command must mention nvim fallback");
        assert!(idx_visual < idx_editor, "VISUAL must appear before EDITOR");
        assert!(
            idx_editor < idx_nvim,
            "EDITOR must appear before nvim fallback"
        );
    }

    #[test]
    /// What: Assert that `editor_open_config_command` includes the full fallback chain and final message.
    ///
    /// Inputs:
    /// - A dummy path.
    ///
    /// Output:
    /// - The returned string contains nvim, vim, hx, helix, emacsclient, emacs, nano and "No terminal editor found".
    ///
    /// Details:
    /// - Validates the built-in fallback list and fallback message without executing shell.
    fn utils_editor_open_config_command_contains_fallback_chain_and_message() {
        use std::path::Path;
        let path = Path::new("/tmp/theme.conf");
        let cmd = super::editor_open_config_command(path);
        assert!(cmd.contains("nvim"), "fallback chain must include nvim");
        assert!(cmd.contains("vim"), "fallback chain must include vim");
        assert!(cmd.contains("hx"), "fallback chain must include hx");
        assert!(cmd.contains("helix"), "fallback chain must include helix");
        assert!(
            cmd.contains("emacsclient"),
            "fallback chain must include emacsclient"
        );
        assert!(cmd.contains("emacs"), "fallback chain must include emacs");
        assert!(cmd.contains("nano"), "fallback chain must include nano");
        assert!(
            cmd.contains("No terminal editor found"),
            "command must include fallback message"
        );
    }

    #[test]
    /// What: Assert that the path in `editor_open_config_command` is shell-single-quoted.
    ///
    /// Inputs:
    /// - A path containing a single quote (e.g. `/tmp/foo'bar.conf`).
    ///
    /// Output:
    /// - The returned string contains the safely quoted path (single-quote escape sequence), not raw path.
    ///
    /// Details:
    /// - Paths with single quotes must be quoted via `shell_single_quote` so the shell sees one argument.
    fn utils_editor_open_config_command_path_is_shell_single_quoted() {
        use std::path::Path;
        let path_with_quote = Path::new("/tmp/foo'bar.conf");
        let path_str = path_with_quote.display().to_string();
        let path_quoted = super::shell_single_quote(&path_str);
        let cmd = super::editor_open_config_command(path_with_quote);
        assert!(
            cmd.contains(&path_quoted),
            "command must contain shell-single-quoted path, got quoted: {path_quoted:?}"
        );
        // Raw unquoted path with single quote would break shell; must not appear as '/tmp/foo'bar.conf'
        assert!(
            !cmd.contains("/tmp/foo'bar.conf"),
            "command must not contain raw path with unescaped single quote"
        );
    }
}
