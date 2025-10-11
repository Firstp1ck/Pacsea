use super::installed_lock;

/// Refresh the process-wide cache of installed package names using
/// `pacman -Qq` and store them in `INSTALLED_SET`.
pub async fn refresh_installed_cache() {
    // pacman -Qq to list installed names
    fn run_pacman_q() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qq"])
            .output()?;
        if !out.status.success() {
            return Err(format!("pacman -Qq exited with {:?}", out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(run_pacman_q).await {
        let set: std::collections::HashSet<String> =
            body.lines().map(|s| s.trim().to_string()).collect();
        if let Ok(mut g) = installed_lock().write() {
            *g = set;
        }
    }
}

/// Query whether `name` appears in the cached set of installed packages.
///
/// Returns `false` if the cache is unavailable.
pub fn is_installed(name: &str) -> bool {
    installed_lock()
        .read()
        .ok()
        .map(|s| s.contains(name))
        .unwrap_or(false)
}
