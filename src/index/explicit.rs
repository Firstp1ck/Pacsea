use std::collections::HashSet;

use super::explicit_lock;

/// Refresh the process-wide cache of explicitly installed package names using
/// `pacman -Qqe` and store them in `EXPLICIT_SET`.
pub async fn refresh_explicit_cache() {
    fn run_pacman_qe() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qetq"]) // explicitly installed AND not required (leaf), names only
            .output()?;
        if !out.status.success() {
            return Err(format!("pacman -Qetq exited with {:?}", out.status).into());
        }
        Ok(String::from_utf8(out.stdout)?)
    }
    if let Ok(Ok(body)) = tokio::task::spawn_blocking(run_pacman_qe).await {
        let set: HashSet<String> = body.lines().map(|s| s.trim().to_string()).collect();
        if let Ok(mut g) = explicit_lock().write() {
            *g = set;
        }
    }
}

/// Return a cloned set of explicitly installed package names.
pub fn explicit_names() -> HashSet<String> {
    explicit_lock()
        .read()
        .map(|s| s.clone())
        .unwrap_or_default()
}
