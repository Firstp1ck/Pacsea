### Why: current code is Arch/pacman-centric
- **Install/remove hardcoded to pacman/paru/yay** (needs adapter):
```12:23:src/install/command.rs
        Source::Official { .. } => {
            let base_cmd = format!("pacman -S --needed --noconfirm {}", item.name);
            let hold_tail = "; echo; echo 'Finished.'; echo 'Press any key to close...'; read -rn1 -s _ || (echo; echo 'Press Ctrl+C to close'; sleep infinity)";
            if dry_run {
                let bash = format!("echo DRY RUN: sudo {base_cmd}{hold_tail}");
                return (bash, true);
            }
            let pass = password.unwrap_or("");
            if pass.is_empty() {
                let bash = format!("sudo {base_cmd}{hold_tail}");
                (bash, true)
```
- **Official index built via pacman -Sl** (no direct apt equivalent):
```3:12:src/index/fetch.rs
/// Fetch a minimal list of official packages using `pacman -Sl`.
pub async fn fetch_official_pkg_names()
-> Result<Vec<OfficialPkg>, Box<dyn std::error::Error + Send + Sync>> {
    fn run_pacman(args: &[&str]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman").args(args).output()?;
```
- **Installed/explicit packages via pacman -Qq / -Qetq**:
```7:12:src/index/installed.rs
    fn run_pacman_q() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let out = std::process::Command::new("pacman")
            .args(["-Qq"])
            .output()?;
```
- **Details via pacman -Si / archlinux.org API**:
```14:21:src/sources/details.rs
    let out = std::process::Command::new("pacman")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .args(["-Si", &spec])
        .output()?;
    if !out.status.success() {
        return Err(format!("pacman -Si failed: {:?}", out.status).into());
```
- AUR search/details are Arch-specific (no Debian equivalent).

### What changes are needed
- **Introduce a package manager abstraction** (trait like `PkgManager`) with methods: `search`, `fetch_details`, `install`, `remove`, `list_installed`, `list_explicit`. Provide `PacmanBackend` and `AptBackend`.
- **AptBackend mappings**
  - Search: `apt-cache search <query>` (parse `name - description`); optional improvement later.
  - Details: `apt-cache show <pkg>` (parse fields).
  - Install: `sudo apt-get install -y <pkg>`.
  - Remove: `sudo apt-get purge -y <pkg>` (and optionally `sudo apt-get autoremove -y`).
  - Installed: `dpkg-query -W -f=${binary:Package}\n`.
  - Explicit (“manual”): `apt-mark showmanual`.
  - Replace Arch repo/arch fields with Debian concepts (component/suite/arch) or keep generic.
- **Index strategy**
  - Minimal (fast): drop the prebuilt offline index for apt and use on-demand `apt-cache search` (good first pass).
  - Parity (slower to build): index from `apt-cache dumpavail` or `apt list` to build an on-disk index similar to current model.
- **UI/state tweaks**
  - Generalize `Source` (e.g., keep `Official`, disable `Aur` when on apt).
  - Sorting labels that mention pacman/AUR.

### Effort estimate
- **Phase 1: Abstraction + apt basic support** (install/remove/search/details/installed): 1–2 days
- **Phase 2: Replace/disable Arch-only features** (AUR, Arch status/news, repo ordering): 0.5–1 day
- **Phase 3: Optional offline index for apt** (parse `dumpavail`/`apt list`, enrich): 2–4 days
- **Phase 4: Testing across Debian/Ubuntu variants + docs**: 0.5–1 day

Totals:
- Basic apt support (no offline index, AUR disabled): ~2–4 days
- Near-parity (offline index + richer details): ~4–7 days

### Risks/nuance
- apt outputs vary by distro/locale; enforce `LC_ALL=C`.
- Search performance with `apt-cache search` on large repos can be slower than the current in-memory index.
- “Explicit” package logic differs (`apt-mark showmanual` isn’t a perfect analog to pacman `-Qetq`).
- Handling recommends/suggests and purge/autoremove semantics.

If you want, I can sketch the `PkgManager` trait and a minimal `AptBackend` implementation next.