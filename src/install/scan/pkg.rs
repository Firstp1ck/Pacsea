/*!
What: Package scan command builder

Input:
- Package name to scan

Output:
- Vector of shell commands for scanning an AUR package

Details:
- Handles repository fetching, makepkg, and all scan stages
*/

#[cfg(not(target_os = "windows"))]
use super::{common, summary};

#[cfg(not(target_os = "windows"))]
/// What: Assemble the shell command sequence used to scan an AUR package in a temporary workspace.
///
/// Input:
/// - `pkg`: AUR package name to clone and analyse.
///
/// Output:
/// - Ordered vector of shell fragments executed sequentially in a spawned terminal.
///
/// Details:
/// - Handles repository fetching, `makepkg -o`, optional scanners, and summary reporting.
/// - Ensures each step logs progress and tolerates partial failures where possible.
pub fn build_scan_cmds_for_pkg(pkg: &str) -> Vec<String> {
    // All commands are joined with " && " and run in a single bash -lc invocation in a terminal.
    // Keep each step resilient so later steps still run where possible.
    let mut cmds: Vec<String> = Vec::new();

    add_setup_commands(&mut cmds, pkg);
    add_fetch_commands(&mut cmds, pkg);
    add_makepkg_commands(&mut cmds, pkg);
    add_all_scans(&mut cmds);
    add_summary_commands(&mut cmds, pkg);

    cmds
}

/// What: Add setup commands (working directory, logging) to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector.
/// - `pkg`: Package name.
///
/// Output:
/// - Appends setup commands to the vector.
#[cfg(not(target_os = "windows"))]
fn add_setup_commands(cmds: &mut Vec<String>, pkg: &str) {
    // 0) Create and enter working directory; remember it for later messages
    cmds.push(format!("pkg='{pkg}'"));
    cmds.push("echo \"[PACSEA] scan_start pkg='$pkg' ts=$(date -Ins) shell=$SHELL term=$TERM display=$DISPLAY\"".to_string());
    cmds.push("work=$(mktemp -d -t pacsea_scan_XXXXXXXX)".to_string());
    cmds.push("echo \"Pacsea: scanning AUR package '$pkg'\"".to_string());
    cmds.push("echo \"Working directory: $work\"".to_string());
    cmds.push("cd \"$work\" && { export PACSEA_DEBUG_LOG=\"$(pwd)/.pacsea_debug.log\"; exec > >(tee -a \"$PACSEA_DEBUG_LOG\") 2>&1; exec 9>>\"$PACSEA_DEBUG_LOG\"; export BASH_XTRACEFD=9; set -x; echo \"Pacsea debug: $(date) start scan for '$pkg' in $PWD\"; trap 'code=$?; echo; echo \"Pacsea debug: exit code=$code\"; echo \"Log: $PACSEA_DEBUG_LOG\"; echo \"Press any key to close...\"; read -rn1 -s _' EXIT; }".to_string());
    cmds.push("if command -v git >/dev/null 2>&1 || sudo pacman -Qi git >/dev/null 2>&1; then :; else echo 'git not found. Cannot clone AUR repo.'; false; fi".to_string());
}

/// What: Add repository fetching commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector.
/// - `_pkg`: Package name (unused, shell variable $pkg is set earlier).
///
/// Output:
/// - Appends fetch commands to the vector.
#[cfg(not(target_os = "windows"))]
fn add_fetch_commands(cmds: &mut Vec<String>, _pkg: &str) {
    // 1) Fetch PKGBUILD via AUR helper first; fallback to git clone
    cmds.push("echo 'Fetching PKGBUILD via AUR helper (-G)…'".to_string());
    cmds.push("echo \"[PACSEA] phase=fetch_helper ts=$(date -Ins)\"".to_string());
    cmds.push("(if command -v paru >/dev/null 2>&1; then paru -G \"$pkg\"; elif command -v yay >/dev/null 2>&1; then yay -G \"$pkg\"; else echo 'No AUR helper (paru/yay) found for -G'; false; fi) || (echo 'Falling back to git clone…'; git clone --depth 1 \"https://aur.archlinux.org/${pkg}.git\" || { echo 'Clone failed'; false; })".to_string());
    cmds.push("if [ -f \"$pkg/PKGBUILD\" ]; then cd \"$pkg\"; else f=$(find \"$pkg\" -maxdepth 3 -type f -name PKGBUILD 2>/dev/null | head -n1); if [ -n \"$f\" ]; then cd \"$(dirname \"$f\")\"; elif [ -d \"$pkg\" ]; then cd \"$pkg\"; fi; fi".to_string());
    cmds.push("echo \"PKGBUILD path: $(pwd)/PKGBUILD\"".to_string());
}

/// What: Add makepkg commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector.
/// - `_pkg`: Package name (unused, shell variable $pkg is set earlier).
///
/// Output:
/// - Appends makepkg commands to the vector.
///
/// Details:
/// - Handles PKGBUILD location fallbacks and helper cache population.
#[cfg(not(target_os = "windows"))]
fn add_makepkg_commands(cmds: &mut Vec<String>, _pkg: &str) {
    // 2) Download sources only
    cmds.push("echo 'Running makepkg -o (download sources only)…'".to_string());
    cmds.push("echo \"[PACSEA] phase=makepkg_download ts=$(date -Ins)\"".to_string());
    // Do not abort the whole chain if makepkg fails (e.g., missing base-devel). Continue scanning.
    cmds.push("({ \
        if [ ! -f PKGBUILD ]; then \
            echo 'PKGBUILD not found; fallback: re-clone via git…'; \
            cd .. || true; \
            rm -rf \"$pkg\" 2>/dev/null || true; \
            git clone --depth 1 \"https://aur.archlinux.org/${pkg}.git\" || true; \
            if [ -f \"$pkg/PKGBUILD\" ]; then \
                cd \"$pkg\"; \
            else \
                f=$(find \"$pkg\" -maxdepth 3 -type f -name PKGBUILD 2>/dev/null | head -n1); \
                if [ -n \"$f\" ]; then cd \"$(dirname \"$f\")\" || true; else echo 'PKGBUILD still missing after git fallback'; fi; \
            fi; \
        fi; \
        if [ ! -f PKGBUILD ]; then \
            echo 'Trying helper -S to populate cache and copy build files…'; \
            cdir=''; \
            if command -v paru >/dev/null 2>&1; then \
                echo 'Detecting paru buildDir…'; \
                bdir=$(paru -Pg 2>/dev/null | grep -m1 -o '\"buildDir\": *\"[^\"]*\"' | cut -d '\"' -f4); \
                bdir=${bdir:-\"$HOME/.cache/paru\"}; \
                echo \"Paru buildDir: $bdir\"; \
                echo 'Cleaning existing cached package directory…'; \
                find \"$bdir\" -maxdepth 5 -type d -name \"$pkg\" -exec rm -rf {} + 2>/dev/null || true; \
                echo 'Populating paru cache with -S (auto-abort, 20s timeout)…'; \
                timeout 20s bash -lc 'yes n | paru -S \"$pkg\"' >/dev/null 2>&1 || true; \
                cdir=$(find \"$bdir\" -maxdepth 6 -type f -name PKGBUILD -path \"*/$pkg/*\" 2>/dev/null | head -n1); \
                if [ -z \"$cdir\" ]; then cdir=$(find \"$bdir\" -maxdepth 6 -type f -name PKGBUILD 2>/dev/null | head -n1); fi; \
            elif command -v yay >/dev/null 2>&1; then \
                echo 'Detecting yay buildDir…'; \
                bdir=$(yay -Pg 2>/dev/null | grep -m1 -o '\"buildDir\": *\"[^\"]*\"' | cut -d '\"' -f4); \
                bdir=${bdir:-\"$HOME/.cache/yay\"}; \
                echo \"Yay buildDir: $bdir\"; \
                echo 'Cleaning existing cached package directory…'; \
                find \"$bdir\" -maxdepth 5 -type d -name \"$pkg\" -exec rm -rf {} + 2>/dev/null || true; \
                echo 'Populating yay cache with -S (auto-abort, 20s timeout)…'; \
                timeout 20s bash -lc 'yes n | yay -S --noconfirm \"$pkg\"' >/dev/null 2>&1 || true; \
                cdir=$(find \"$bdir\" -maxdepth 6 -type f -name PKGBUILD -path \"*/$pkg/*\" 2>/dev/null | head -n1); \
                if [ -z \"$cdir\" ]; then cdir=$(find \"$bdir\" -maxdepth 6 -type f -name PKGBUILD 2>/dev/null | head -n1); fi; \
            fi; \
            if [ -n \"$cdir\" ]; then \
                cd \"$(dirname \"$cdir\")\" || true; \
            else \
                echo 'Could not locate PKGBUILD in helper cache.'; \
            fi; \
        fi; \
        echo \"PKGBUILD path: $(pwd)/PKGBUILD\"; \
        if [ -f PKGBUILD ]; then \
            makepkg -o --noconfirm && echo 'makepkg -o: sources downloaded.'; \
        else \
            echo 'Skipping makepkg -o: PKGBUILD still missing.'; \
        fi; \
    }) || echo 'makepkg -o failed or partially completed; continuing'".to_string());
}

/// What: Add all scan commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector.
///
/// Output:
/// - Appends all scan commands to the vector.
#[cfg(not(target_os = "windows"))]
fn add_all_scans(cmds: &mut Vec<String>) {
    common::add_pattern_exports(cmds);
    common::add_clamav_scan(cmds);
    common::add_trivy_scan(cmds);
    common::add_semgrep_scan(cmds);
    common::add_sleuth_scan(cmds);
    common::add_shellcheck_scan(cmds);
    common::add_shellcheck_risk_eval(cmds);
    common::add_custom_pattern_scan(cmds);
    common::add_virustotal_scan(cmds);
}

/// What: Add summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector.
/// - `_pkg`: Package name (unused, shell variable $pkg is set earlier).
///
/// Output:
/// - Appends summary commands to the vector.
#[cfg(not(target_os = "windows"))]
fn add_summary_commands(cmds: &mut Vec<String>, _pkg: &str) {
    // Final note with working directory for manual inspection
    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("echo -e '\\033[1;36m[📊] Summary\\033[0m'".to_string());
    summary::add_overall_risk_calc(cmds);
    summary::add_clamav_summary(cmds);
    summary::add_trivy_summary(cmds);
    summary::add_semgrep_summary(cmds);
    summary::add_shellcheck_summary(cmds);
    summary::add_shellcheck_risk_summary(cmds);
    summary::add_sleuth_summary(cmds);
    summary::add_custom_and_vt_summary(cmds);
    cmds.push("echo".to_string());
    cmds.push("echo \"Pacsea: scan finished. Working directory preserved: $work\"".to_string());
    cmds.push("echo -e \"\\033[1;32m[✔] Pacsea: scan finished.\\033[0m Working directory preserved: $work\"".to_string());
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    #[test]
    /// What: Ensure scan command generation for AUR packages exports expected steps and annotations.
    ///
    /// Inputs:
    /// - Package name `foobar` supplied to `build_scan_cmds_for_pkg`.
    ///
    /// Output:
    /// - Command list includes environment exports, git clone, makepkg fetch, optional scan sections, and summary note.
    ///
    /// Details:
    /// - Joins the command list to assert presence of key substrings, catching regressions in the scripted pipeline.
    fn build_scan_cmds_for_pkg_has_core_steps() {
        let cmds = build_scan_cmds_for_pkg("foobar");
        let joined = cmds.join("\n");

        assert!(
            joined.contains("pkg='foobar'"),
            "should export pkg variable with provided name"
        );
        assert!(
            joined.contains("git clone --depth 1 \"https://aur.archlinux.org/${pkg}.git\""),
            "should clone the AUR repo using the pkg variable"
        );
        assert!(
            joined.contains("makepkg -o --noconfirm"),
            "should attempt to download sources with makepkg -o"
        );
        assert!(
            joined.contains("--- ClamAV scan (optional) ---"),
            "should include ClamAV scan section"
        );
        assert!(
            joined.contains("--- Trivy filesystem scan (optional) ---"),
            "should include Trivy FS scan section"
        );
        assert!(
            joined.contains("--- Semgrep static analysis (optional) ---"),
            "should include Semgrep scan section"
        );
        assert!(
            joined.contains("--- VirusTotal hash lookups (requires VT_API_KEY env var) ---"),
            "should include VirusTotal lookup section"
        );
        assert!(
            joined.contains("echo '--- Summary ---'"),
            "should include final summary section"
        );
        assert!(
            joined.contains("Pacsea: scan finished. Working directory preserved: $work"),
            "should print final working directory note"
        );
    }
}
