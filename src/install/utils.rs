#[cfg(target_os = "windows")]
pub fn command_on_path(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

#[cfg(not(target_os = "windows"))]
/// Return `true` if an executable named `cmd` can be found in the current `PATH`.
///
/// Inputs: `cmd` program name or absolute/relative path.
///
/// Output: `true` when an executable file is found (Unix executable bit respected).
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
                        let candidate = dir.join(format!("{}{}", cmd, ext));
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
/// Return the index of the first available terminal from `terms` as found in `PATH`.
///
/// Inputs: `terms` list of (binary name, args, needs_xfce_command).
///
/// Output: `Some(index)` of the first present terminal; `None` if none found.
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
/// Safely single-quote an arbitrary string for POSIX shells.
///
/// Inputs: `s` string to quote.
///
/// Output: New string wrapped in single quotes, with inner quotes escaped via `'
/// '"'"'` pattern.
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
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut cmd_path = dir.clone();
        cmd_path.push("mycmd");
        fs::write(&cmd_path, b"#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&cmd_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&cmd_path, perms).unwrap();

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
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&dir);
        let mut kitty = dir.clone();
        kitty.push("kitty");
        fs::write(&kitty, b"#!/bin/sh\nexit 0\n").unwrap();
        let mut perms = fs::metadata(&kitty).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&kitty, perms).unwrap();

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
    fn utils_shell_single_quote_handles_edges() {
        assert_eq!(super::shell_single_quote(""), "''");
        assert_eq!(super::shell_single_quote("abc"), "'abc'");
        assert_eq!(super::shell_single_quote("a'b"), "'a'\"'\"'b'");
    }
}
