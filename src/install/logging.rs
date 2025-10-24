use std::io::Write;

pub fn log_installed(names: &[String]) -> std::io::Result<()> {
    let mut path = crate::theme::logs_dir();
    path.push("install_log.txt");
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

pub fn log_removed(names: &[String]) -> std::io::Result<()> {
    let mut path = crate::theme::logs_dir();
    path.push("remove_log.txt");
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
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::create_dir_all(&home);
        unsafe { std::env::set_var("HOME", home.display().to_string()) };

        // Write install log
        let names = vec!["a".to_string(), "b".to_string()];
        super::log_installed(&names).unwrap();
        let mut p = crate::theme::logs_dir();
        p.push("install_log.txt");
        let body = fs::read_to_string(&p).unwrap();
        assert!(body.contains(" a\n") || body.contains(" a\r\n"));

        // Write remove log
        super::log_removed(&names).unwrap();
        let mut pr = crate::theme::logs_dir();
        pr.push("remove_log.txt");
        let body_r = fs::read_to_string(&pr).unwrap();
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
