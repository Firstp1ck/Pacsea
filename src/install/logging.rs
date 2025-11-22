use std::io::Write;

/// What: Append installed package names to an audit log under the logs directory.
///
/// Input: `names` slice of package names to log; each line is timestamped.
///
/// Output: `Ok(())` on success; otherwise an I/O error.
///
/// Details: Writes to `logs_dir/install_log.log`, prefixing each name with a UTC timestamp.
pub fn log_installed(names: &[String]) -> std::io::Result<()> {
    let mut path = crate::theme::logs_dir();
    path.push("install_log.log");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .ok();
    let when = crate::util::ts_to_date(now);
    for n in names {
        writeln!(f, "{when} {n}")?;
    }
    Ok(())
}

/// What: Append removed package names to an audit log under the logs directory.
///
/// Input:
/// - `names` slice of package names to append (one per line).
///
/// Output:
/// - `Ok(())` on success; otherwise an I/O error.
///
/// # Errors
/// - Returns `Err` when the logs directory cannot be accessed or created
/// - Returns `Err` when the log file cannot be opened or written to
///
/// Details:
/// - Appends to `logs_dir/remove_log.log` without timestamps.
pub fn log_removed(names: &[String]) -> std::io::Result<()> {
    let mut path = crate::theme::logs_dir();
    path.push("remove_log.log");
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for n in names {
        writeln!(f, "{n}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    /// What: Ensure install/remove logging helpers write files beneath the configured logs directory.
    ///
    /// Inputs:
    /// - `names`: Sample package list written to both install and remove logs with HOME redirected.
    ///
    /// Output:
    /// - Generated log files contain the package names (with timestamp for installs) under `logs_dir`.
    ///
    /// Details:
    /// - Temporarily overrides `HOME`, calls both logging functions, then verifies file contents before
    ///   restoring the environment.
    fn logging_writes_install_and_remove_logs_under_logs_dir() {
        use std::fs;
        use std::path::PathBuf;
        // Shim HOME to temp so logs_dir resolves within it
        let orig_home = std::env::var_os("HOME");
        let mut home: PathBuf = std::env::temp_dir();
        home.push(format!(
            "pacsea_test_logs_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("System time is before UNIX epoch")
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&home);
        unsafe { std::env::set_var("HOME", home.display().to_string()) };

        // Write install log
        let names = vec!["a".to_string(), "b".to_string()];
        super::log_installed(&names).expect("Failed to write install log in test");
        let mut p = crate::theme::logs_dir();
        p.push("install_log.log");
        let body = fs::read_to_string(&p).expect("Failed to read install log in test");
        assert!(body.contains(" a\n") || body.contains(" a\r\n"));

        // Write remove log
        super::log_removed(&names).expect("Failed to write remove log in test");
        let mut pr = crate::theme::logs_dir();
        pr.push("remove_log.log");
        let body_r = fs::read_to_string(&pr).expect("Failed to read remove log in test");
        assert!(body_r.contains("a\n") || body_r.contains("a\r\n"));

        // Cleanup env; not removing files so test artifacts may remain in tmp
        unsafe {
            if let Some(v) = orig_home {
                std::env::set_var("HOME", v);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }
}
