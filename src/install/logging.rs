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
