//! Command-line list installed packages functionality.

use crate::args::i18n;

/// What: Handle list installed packages flag by querying pacman and displaying results.
///
/// Inputs:
/// - `exp`: If true, list explicitly installed packages.
/// - `imp`: If true, list implicitly installed packages.
/// - `all`: If true, list all installed packages.
///
/// Output:
/// - Exits the process after displaying the package list.
///
/// Details:
/// - Uses `pacman -Qq` to get all installed packages.
/// - Uses `pacman -Qetq` to get explicitly installed packages.
/// - Calculates implicitly installed as all minus explicit.
/// - Defaults to `--exp` (explicitly installed) if no option is specified.
/// - Prints packages one per line to stdout.
/// - Exits immediately after listing (doesn't launch TUI).
pub fn handle_list(exp: bool, imp: bool, all: bool) -> ! {
    use std::process::{Command, Stdio};

    // Default to --exp if no option is specified
    let exp = if !exp && !imp && !all {
        tracing::info!("No list option specified, defaulting to --exp");
        true
    } else {
        exp
    };

    tracing::info!(
        exp = exp,
        imp = imp,
        all = all,
        "List installed packages requested from CLI"
    );

    // Get all installed packages
    let all_packages = match Command::new("pacman")
        .args(["-Qq"])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                eprintln!("{}", i18n::t("app.cli.list.query_failed"));
                tracing::error!("pacman -Qq failed");
                std::process::exit(1);
            }
            let packages: std::collections::HashSet<String> =
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            packages
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.list.pacman_exec_failed", &e));
            tracing::error!(error = %e, "Failed to execute pacman");
            std::process::exit(1);
        }
    };

    // Get explicitly installed packages
    let explicit_packages = match Command::new("pacman")
        .args(["-Qetq"])
        .stdin(Stdio::null())
        .output()
    {
        Ok(output) => {
            if !output.status.success() {
                eprintln!("{}", i18n::t("app.cli.list.query_explicit_failed"));
                tracing::error!("pacman -Qetq failed");
                std::process::exit(1);
            }
            let packages: std::collections::HashSet<String> =
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            packages
        }
        Err(e) => {
            eprintln!("{}", i18n::t_fmt1("app.cli.list.pacman_exec_failed", &e));
            tracing::error!(error = %e, "Failed to execute pacman");
            std::process::exit(1);
        }
    };

    // Calculate implicitly installed packages (all - explicit)
    let implicit_packages: std::collections::HashSet<String> = all_packages
        .difference(&explicit_packages)
        .cloned()
        .collect();

    // Collect and sort packages based on requested type
    let mut packages_to_list = Vec::new();

    if all {
        packages_to_list.extend(all_packages.iter().cloned());
    }
    if exp {
        packages_to_list.extend(explicit_packages.iter().cloned());
    }
    if imp {
        packages_to_list.extend(implicit_packages.iter().cloned());
    }

    // Remove duplicates and sort
    let mut unique_packages: std::collections::HashSet<String> =
        packages_to_list.into_iter().collect();
    let mut sorted_packages: Vec<String> = unique_packages.drain().collect();
    sorted_packages.sort();

    let count = sorted_packages.len();

    // Print packages one per line
    for pkg in sorted_packages {
        println!("{}", pkg);
    }

    tracing::info!(count = count, "Listed installed packages");
    std::process::exit(0);
}
