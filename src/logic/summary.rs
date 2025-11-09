use crate::state::PackageItem;

/// What: Minimal data required to populate the PostSummary modal.
///
/// Inputs:
/// - Populated by `compute_post_summary` after pacman inspections.
///
/// Output:
/// - Supplies boolean outcome, counts, and auxiliary labels for post-transaction display.
///
/// Details:
/// - Designed to be serializable/clonable so the UI can render snapshots outside the logic module.
#[derive(Debug, Clone)]
pub struct PostSummaryData {
    pub success: bool,
    pub changed_files: usize,
    pub pacnew_count: usize,
    pub pacsave_count: usize,
    pub services_pending: Vec<String>,
    pub snapshot_label: Option<String>,
}

/// What: Execute `pacman` with the provided arguments and capture stdout.
///
/// Inputs:
/// - `args`: Slice of CLI arguments passed directly to the pacman binary.
///
/// Output:
/// - Returns the command's stdout as a UTF-8 string or propagates execution/parsing errors.
///
/// Details:
/// - Used internally by summary helpers to keep command invocation boilerplate centralized.
fn run_pacman(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let out = std::process::Command::new("pacman").args(args).output()?;
    if !out.status.success() {
        return Err(format!("pacman {:?} exited with {:?}", args, out.status).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// What: Count changed files and collect affected systemd services for given packages.
///
/// Inputs:
/// - `names`: Package names whose remote file lists should be inspected.
///
/// Output:
/// - Returns a tuple with the number of file entries and a sorted list of service unit filenames.
///
/// Details:
/// - Queries `pacman -Fl` per package, ignoring directory entries, and extracts `.service` paths.
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

/// What: Scan `/etc` for outstanding `.pacnew` and `.pacsave` files.
///
/// Inputs:
/// - (none): Walks the filesystem directly with a depth guard.
///
/// Output:
/// - Returns counts of `.pacnew` and `.pacsave` files found beneath `/etc`.
///
/// Details:
/// - Ignores very deep directory structures to avoid pathological traversal scenarios.
fn count_pac_conflicts_in_etc() -> (usize, usize) {
    fn walk(dir: &std::path::Path, pacnew: &mut usize, pacsave: &mut usize) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    // Limit to reasonable depth to avoid cycles (symlinks ignored)
                    if p.strip_prefix("/etc")
                        .is_ok_and(|stripped| stripped.components().count() > 12)
                    {
                        continue;
                    }
                    walk(&p, pacnew, pacsave);
                } else if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                    if name.ends_with(".pacnew") {
                        *pacnew += 1;
                    }
                    if name.ends_with(".pacsave") {
                        *pacsave += 1;
                    }
                }
            }
        }
    }
    let mut pn = 0usize;
    let mut ps = 0usize;
    walk(std::path::Path::new("/etc"), &mut pn, &mut ps);
    (pn, ps)
}

/// What: Produce a best-effort summary of potential post-transaction tasks.
///
/// Inputs:
/// - `items`: Packages that were part of the transaction and should inform the summary.
///
/// Output:
/// - Returns a `PostSummaryData` structure with file counts, service hints, and conflict tallies.
///
/// Details:
/// - Combines sync database lookups with an `/etc` scan without performing system modifications.
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
