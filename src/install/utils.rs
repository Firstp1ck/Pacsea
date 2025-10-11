#[cfg(not(target_os = "windows"))]
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
