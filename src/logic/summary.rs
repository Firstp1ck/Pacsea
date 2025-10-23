use crate::state::PackageItem;

/// Minimal data required to populate the PostSummary modal.
#[derive(Debug, Clone)]
pub struct PostSummaryData {
    pub success: bool,
    pub changed_files: usize,
    pub pacnew_count: usize,
    pub pacsave_count: usize,
    pub services_pending: Vec<String>,
    pub snapshot_label: Option<String>,
}

fn run_pacman(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let out = std::process::Command::new("pacman").args(args).output()?;
    if !out.status.success() {
        return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn count_changed_files_and_services(names: &[String]) -> (usize, Vec<String>) {
    let mut total_files: usize = 0;
    let mut services: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for name in names {
        if let Ok(body) = run_pacman(&["-Fl", name]) {
            for line in body.lines() {
                // pacman -Fl format: "<pkg> <path>"
                if let Some((_pkg, path)) = line.split_once(' ') {
                    if !path.ends_with('/') {
                        total_files += 1;
                    }
                    if path.starts_with("/usr/lib/systemd/system/") && path.ends_with(".service") {
                        // take filename
                        if let Some(stem) = std::path::Path::new(path)
                            .file_name()
                            .and_then(|s| s.to_str())
                        {
                            services.insert(stem.to_string());
                        }
                    }
                }
            }
        }
    }
    (total_files, services.into_iter().collect())
}

fn count_pac_conflicts_in_etc() -> (usize, usize) {
    fn walk(dir: &std::path::Path, pacnew: &mut usize, pacsave: &mut usize) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    // Limit to reasonable depth to avoid cycles (symlinks ignored)
                    if let Some(stripped) = p.strip_prefix("/etc").ok() {
                        if stripped.components().count() > 12 { continue; }
                    }
                    walk(&p, pacnew, pacsave);
                } else if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.ends_with(".pacnew") { *pacnew += 1; }
                    if name.ends_with(".pacsave") { *pacsave += 1; }
                }
            }
        }
    }
    let mut pn = 0usize;
    let mut ps = 0usize;
    walk(std::path::Path::new("/etc"), &mut pn, &mut ps);
    (pn, ps)
}

/// Compute a best-effort post-transaction summary from pacman file lists and /etc scan.
///
/// This does not execute any changes; it only inspects the sync file DB and the filesystem.
pub fn compute_post_summary(items: &[PackageItem]) -> PostSummaryData {
    let names: Vec<String> = items.iter().map(|p| p.name.clone()).collect();
    let (changed_files, services_pending) = count_changed_files_and_services(&names);
    let (pacnew_count, pacsave_count) = count_pac_conflicts_in_etc();
    PostSummaryData {
        success: true,
        changed_files,
        pacnew_count,
        pacsave_count,
        services_pending,
        snapshot_label: None,
    }
}


