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
/// - Leverages `which::which`, inheriting its support for PATHEXT resolution.
#[must_use]
pub fn command_on_path(cmd: &str) -> bool {
    which::which(cmd).is_ok()
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
/// - Accepts explicit paths (containing path separators) and honours Unix permission bits.
/// - Falls back to scanning `PATH`, and on Windows builds respects `PATHEXT` as well.
#[must_use]
pub fn command_on_path(cmd: &str) -> bool {
    use std::path::Path;

    fn is_exec(p: &std::path::Path) -> bool {
        if !p.is_file() {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(p) {
                return meta.permissions().mode() & 0o111 != 0;
            }
            false
        }
        #[cfg(not(unix))]
        {
            true
        }
    }

    if cmd.contains(std::path::MAIN_SEPARATOR) {
        return is_exec(Path::new(cmd));
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(cmd);
            if is_exec(&candidate) {
                return true;
            }
            #[cfg(windows)]
            {
                if let Some(pathext) = std::env::var_os("PATHEXT") {
                    for ext in pathext.to_string_lossy().split(';') {
                        let candidate = dir.join(format!("{cmd}{ext}"));
                        if candidate.is_file() {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
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
    use std::os::unix::fs::PermissionsExt;
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for (i, (name, _args, _hold)) in terms.iter().enumerate() {
                let candidate = dir.join(name);
                if candidate.is_file()
                    && let Ok(meta) = std::fs::metadata(&candidate)
                    && meta.permissions().mode() & 0o111 != 0
                {
                    return Some(i);
                }
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
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
        assert!(!super::command_on_path("notexist"));
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
}
