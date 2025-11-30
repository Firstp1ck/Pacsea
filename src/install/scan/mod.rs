/*!
What: AUR package scan launcher

Input:
- Package name to scan (clone-and-scan), or a target directory to scan in-place

Output:
- Spawns a terminal that runs a sequence of shell commands to clone, download sources, and run optional scanners; results are printed/saved into files in a temp directory (or target dir)

Details:
- Steps: clone AUR repo; run `makepkg -o`; run optional scanners (`ClamAV`, `Trivy`, `Semgrep`); optional `VirusTotal` hash lookups when `VT_API_KEY` is present
- Semgrep is not installed automatically; if missing, a warning is printed and the scan is skipped
- `VirusTotal` lookups are hash-based; unknown files may report "no report found"
- Working directory is a temporary directory printed to the terminal and preserved for inspection
*/

mod common;
mod dir;
pub mod pkg;
pub mod spawn;
mod summary;

#[cfg(not(target_os = "windows"))]
pub use spawn::spawn_aur_scan_for_with_config;
