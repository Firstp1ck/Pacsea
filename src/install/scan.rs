/*!
AUR package scan launcher.

Provides functions to open a terminal that:
- clones the AUR package repository
- runs `makepkg -o` to download sources only
- runs ClamAV (`clamscan`) if available
- runs Trivy filesystem scan if available
- runs Semgrep static analysis if available
- looks up file hashes on VirusTotal if the `VT_API_KEY` environment variable is set

Notes:
- Semgrep will be installed via AUR helper (paru/yay) if missing; otherwise you'll be prompted to set up an AUR helper. Other tools are not installed.
- VirusTotal lookups are hash-based; if a file has never been seen by VirusTotal, the API will report an error (no report found).
- The working directory is a temporary directory printed to the terminal and left behind for inspection.
*/

#[cfg(not(target_os = "windows"))]
use super::shell::spawn_shell_commands_in_terminal;

#[cfg(not(target_os = "windows"))]
fn build_scan_cmds_for_pkg(pkg: &str) -> Vec<String> {
    // All commands are joined with " && " and run in a single bash -lc invocation in a terminal.
    // Keep each step resilient so later steps still run where possible.
    let mut cmds: Vec<String> = Vec::new();

    // 0) Create and enter working directory; remember it for later messages
    cmds.push(format!("pkg='{}'", pkg));
    cmds.push("work=$(mktemp -d -t pacsea_scan_XXXXXXXX)".to_string());
    cmds.push("echo \"Pacsea: scanning AUR package '$pkg'\"".to_string());
    cmds.push("echo \"Working directory: $work\"".to_string());
    cmds.push("cd \"$work\"".to_string());
    cmds.push("if command -v git >/dev/null 2>&1; then :; else echo 'git not found. Cannot clone AUR repo.'; exit 1; fi".to_string());

    // 1) Clone AUR repo
    cmds.push("echo 'Cloning AUR repository…'".to_string());
    cmds.push("git clone --depth 1 \"https://aur.archlinux.org/${pkg}.git\" || { echo 'Clone failed'; exit 1; }".to_string());
    cmds.push("cd \"$pkg\"".to_string());
    cmds.push("echo \"PKGBUILD path: $(pwd)/PKGBUILD\"".to_string());

    // 2) Download sources only
    cmds.push("echo 'Running makepkg -o (download sources only)…'".to_string());
    // Do not abort the whole chain if makepkg fails (e.g., missing base-devel). Continue scanning.
    cmds.push("(makepkg -o --noconfirm && echo 'makepkg -o: sources downloaded.') || echo 'makepkg -o failed or partially completed; continuing'".to_string());

    // 3) ClamAV scan
    cmds.push("echo '--- ClamAV scan (optional) ---'".to_string());
    cmds.push("(command -v clamscan >/dev/null 2>&1 && { if find /var/lib/clamav -maxdepth 1 -type f \\( -name '*.cvd' -o -name '*.cld' \\) 2>/dev/null | grep -q .; then clamscan -r . | tee ./.pacsea_scan_clamav.txt; else echo 'ClamAV found but no signature database in /var/lib/clamav'; echo 'Tip: run: sudo freshclam  (or start the updater: sudo systemctl start clamav-freshclam)'; fi; } || echo 'ClamAV (clamscan) encountered an error; skipping') || echo 'ClamAV not found; skipping'".to_string());

    // 4) Trivy filesystem scan
    cmds.push("echo '--- Trivy filesystem scan (optional) ---'".to_string());
    cmds.push("(command -v trivy >/dev/null 2>&1 && (trivy fs --quiet --format json . > ./.pacsea_scan_trivy.json || trivy fs --quiet . | tee ./.pacsea_scan_trivy.txt) || echo 'Trivy not found or failed; skipping')".to_string());

    // 5) Semgrep static analysis
    cmds.push("echo '--- Semgrep static analysis (optional) ---'".to_string());
    cmds.push("(command -v semgrep >/dev/null 2>&1 && (semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt) || { echo 'Semgrep not found; attempting install via AUR helper (semgrep-bin)'; if command -v paru >/dev/null 2>&1; then paru -S --needed --noconfirm semgrep-bin || echo 'Failed to install semgrep-bin with paru'; elif command -v yay >/dev/null 2>&1; then yay -S --needed --noconfirm semgrep-bin || echo 'Failed to install semgrep-bin with yay'; else echo 'No AUR helper (paru/yay) found. Please set up an AUR helper first to enable Semgrep scanning.'; fi; if command -v semgrep >/dev/null 2>&1; then semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt; else echo 'Semgrep not available; skipping'; fi; })".to_string());

    // 6) VirusTotal hash lookups
    cmds.push("echo '--- VirusTotal hash lookups (requires VT_API_KEY env var) ---'".to_string());
    cmds.push(
        concat!(
            "if [ -z \"${VT_API_KEY:-}\" ]; then ",
            "  cfg=\"${XDG_CONFIG_HOME:-$HOME/.config}/pacsea/settings.conf\"; ",
            "  if [ -f \"$cfg\" ]; then ",
            "    VT_API_KEY=\"$(awk -F= '/^[[:space:]]*virustotal_api_key[[:space:]]*=/{print $2}' \"$cfg\" | sed 's/#.*//' | xargs)\"; ",
            "  fi; ",
            "fi; ",
            "if [ -n \"${VT_API_KEY:-}\" ]; then ",
            "  files=$(find . -type f \\( -name 'PKGBUILD' -o -path './src/*' -o -name '*.patch' -o -name '*.diff' \\) 2>/dev/null); ",
            "  vt_total=0; vt_known=0; vt_unknown=0; vt_mal_sum=0; vt_sus_sum=0; vt_har_sum=0; vt_und_sum=0; ",
            "  : > ./.pacsea_scan_vt.txt; ",
            "  if [ -z \"$files\" ]; then ",
            "    echo 'No files to hash (PKGBUILD/src)'; ",
            "  else ",
            "    for f in $files; do ",
            "      if [ -f \"$f\" ]; then ",
            "        h=$(sha256sum \"$f\" | awk '{print $1}'); ",
            "        echo \"File: $f\" | tee -a ./.pacsea_scan_vt.txt; ",
            "        echo \"SHA256: $h\" | tee -a ./.pacsea_scan_vt.txt; ",
            "        vt_total=$((vt_total+1)); ",
            "        resp=$(curl -s -H \"x-apikey: $VT_API_KEY\" \"https://www.virustotal.com/api/v3/files/$h\"); ",
            "        if echo \"$resp\" | grep -q '\"error\"'; then ",
            "          echo 'VT: No report found' | tee -a ./.pacsea_scan_vt.txt; ",
            "          vt_unknown=$((vt_unknown+1)); ",
            "        else ",
            "          mal=$(echo \"$resp\" | grep -o '\"malicious\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          sus=$(echo \"$resp\" | grep -o '\"suspicious\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          har=$(echo \"$resp\" | grep -o '\"harmless\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          und=$(echo \"$resp\" | grep -o '\"undetected\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          echo \"VT: malicious=${mal:-0} suspicious=${sus:-0} harmless=${har:-0} undetected=${und:-0}\" | tee -a ./.pacsea_scan_vt.txt; ",
            "          echo \"VT report: https://www.virustotal.com/gui/file/$h\" | tee -a ./.pacsea_scan_vt.txt; ",
            "          vt_known=$((vt_known+1)); ",
            "          vt_mal_sum=$((vt_mal_sum+${mal:-0})); ",
            "          vt_sus_sum=$((vt_sus_sum+${sus:-0})); ",
            "          vt_har_sum=$((vt_har_sum+${har:-0})); ",
            "          vt_und_sum=$((vt_und_sum+${und:-0})); ",
            "        fi; ",
            "        echo | tee -a ./.pacsea_scan_vt.txt >/dev/null; ",
            "      fi; ",
            "    done; ",
            "    { ",
            "      echo \"VT_TOTAL=$vt_total\"; ",
            "      echo \"VT_KNOWN=$vt_known\"; ",
            "      echo \"VT_UNKNOWN=$vt_unknown\"; ",
            "      echo \"VT_MAL=$vt_mal_sum\"; ",
            "      echo \"VT_SUS=$vt_sus_sum\"; ",
            "      echo \"VT_HAR=$vt_har_sum\"; ",
            "      echo \"VT_UND=$vt_und_sum\"; ",
            "    } > ./.pacsea_scan_vt_summary.txt; ",
            "  fi; ",
            "else ",
            "  echo 'VT_API_KEY not set; skipping VirusTotal lookups.'; ",
            "fi"
        )
        .to_string(),
    );

    // 7) Final note with working directory for manual inspection
    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("if [ -f ./.pacsea_scan_clamav.txt ]; then inf=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); if [ -n \"$inf\" ]; then if [ \"$inf\" -gt 0 ]; then echo \"ClamAV: infected files: $inf\"; else echo \"ClamAV: no infections detected\"; fi; else echo 'ClamAV: no infections detected'; fi; else echo 'ClamAV: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_trivy.json ]; then c=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); h=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); m=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); l=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); t=$((c+h+m+l)); if [ \"$t\" -gt 0 ]; then echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; elif [ -f ./.pacsea_scan_trivy.txt ]; then if grep -qiE 'CRITICAL|HIGH|MEDIUM|LOW' ./.pacsea_scan_trivy.txt; then c=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); h=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); m=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); l=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); t=$((c+h+m+l)); echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; else echo 'Trivy: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_semgrep.json ]; then n=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; elif [ -f ./.pacsea_scan_semgrep.txt ]; then n=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; else echo 'Semgrep: not run'; fi".to_string());
    cmds.push("vtf=./.pacsea_scan_vt_summary.txt; if [ -f \"$vtf\" ]; then VT_TOTAL=$(grep -E '^VT_TOTAL=' \"$vtf\" | cut -d= -f2); VT_KNOWN=$(grep -E '^VT_KNOWN=' \"$vtf\" | cut -d= -f2); VT_UNKNOWN=$(grep -E '^VT_UNKNOWN=' \"$vtf\" | cut -d= -f2); VT_MAL=$(grep -E '^VT_MAL=' \"$vtf\" | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' \"$vtf\" | cut -d= -f2); VT_HAR=$(grep -E '^VT_HAR=' \"$vtf\" | cut -d= -f2); VT_UND=$(grep -E '^VT_UND=' \"$vtf\" | cut -d= -f2); echo \"VirusTotal: files=$VT_TOTAL known=$VT_KNOWN malicious=$VT_MAL suspicious=$VT_SUS harmless=$VT_HAR undetected=$VT_UND unknown=$VT_UNKNOWN\"; else echo 'VirusTotal: not configured or no files'; fi".to_string());
    cmds.push("echo".to_string());
    cmds.push("echo \"Pacsea: scan finished. Working directory preserved: $work\"".to_string());

    cmds
}

/// Launch a terminal that performs an AUR package scan for the given package name.
///
/// Behavior (Unix):
/// - Uses `git clone https://aur.archlinux.org/<pkg>.git`
/// - `makepkg -o` to download sources
/// - Optionally runs `clamscan`, `trivy fs`, `semgrep --config=auto`
/// - If `VT_API_KEY` is set in the environment, performs hash lookups on PKGBUILD and downloaded sources
#[cfg(not(target_os = "windows"))]
pub fn spawn_aur_scan_for(pkg: &str) {
    let cmds = build_scan_cmds_for_pkg(pkg);
    spawn_shell_commands_in_terminal(&cmds);
}

/// Launch a terminal to perform a scan in an already-unpacked AUR package directory.
///
/// The current implementation simply runs the scanners in-place, without cloning or `makepkg -o`.
#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn spawn_aur_scan_in_dir(path: &str) {
    let mut cmds: Vec<String> = Vec::new();
    cmds.push(format!("target_dir='{}'", path));
    cmds.push("if [ -d \"$target_dir\" ]; then cd \"$target_dir\"; else echo \"Directory not found: $target_dir\"; exit 1; fi".to_string());

    // Optional scanners
    cmds.push("echo '--- ClamAV scan (optional) ---'".to_string());
    cmds.push("(command -v clamscan >/dev/null 2>&1 && { if find /var/lib/clamav -maxdepth 1 -type f \\( -name '*.cvd' -o -name '*.cld' \\) 2>/dev/null | grep -q .; then clamscan -r . | tee ./.pacsea_scan_clamav.txt; else echo 'ClamAV found but no signature database in /var/lib/clamav'; echo 'Tip: run: sudo freshclam  (or start the updater: sudo systemctl start clamav-freshclam)'; fi; } || echo 'ClamAV (clamscan) encountered an error; skipping') || echo 'ClamAV not found; skipping'".to_string());

    cmds.push("echo '--- Trivy filesystem scan (optional) ---'".to_string());
    cmds.push("(command -v trivy >/dev/null 2>&1 && (trivy fs --quiet --format json . > ./.pacsea_scan_trivy.json || trivy fs --quiet . | tee ./.pacsea_scan_trivy.txt) || echo 'Trivy not found or failed; skipping')".to_string());

    cmds.push("echo '--- Semgrep static analysis (optional) ---'".to_string());
    cmds.push("(command -v semgrep >/dev/null 2>&1 && (semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt) || { echo 'Semgrep not found; attempting install via AUR helper (semgrep-bin)'; if command -v paru >/dev/null 2>&1; then paru -S --needed --noconfirm semgrep-bin || echo 'Failed to install semgrep-bin with paru'; elif command -v yay >/dev/null 2>&1; then yay -S --needed --noconfirm semgrep-bin || echo 'Failed to install semgrep-bin with yay'; else echo 'No AUR helper (paru/yay) found. Please set up an AUR helper first to enable Semgrep scanning.'; fi; if command -v semgrep >/dev/null 2>&1; then semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt; else echo 'Semgrep not available; skipping'; fi; })".to_string());

    // VirusTotal hash lookups
    cmds.push("echo '--- VirusTotal hash lookups (requires VT_API_KEY env var) ---'".to_string());
    cmds.push(
        concat!(
            "if [ -z \"${VT_API_KEY:-}\" ]; then ",
            "  cfg=\"${XDG_CONFIG_HOME:-$HOME/.config}/pacsea/settings.conf\"; ",
            "  if [ -f \"$cfg\" ]; then ",
            "    VT_API_KEY=\"$(awk -F= '/^[[:space:]]*virustotal_api_key[[:space:]]*=/{print $2}' \"$cfg\" | sed 's/#.*//' | xargs)\"; ",
            "  fi; ",
            "fi; ",
            "if [ -n \"${VT_API_KEY:-}\" ]; then ",
            "  files=$(find . -type f \\( -name 'PKGBUILD' -o -path './src/*' -o -name '*.patch' -o -name '*.diff' \\) 2>/dev/null); ",
            "  vt_total=0; vt_known=0; vt_unknown=0; vt_mal_sum=0; vt_sus_sum=0; vt_har_sum=0; vt_und_sum=0; ",
            "  : > ./.pacsea_scan_vt.txt; ",
            "  if [ -z \"$files\" ]; then ",
            "    echo 'No files to hash (PKGBUILD/src)'; ",
            "  else ",
            "    for f in $files; do ",
            "      if [ -f \"$f\" ]; then ",
            "        h=$(sha256sum \"$f\" | awk '{print $1}'); ",
            "        echo \"File: $f\" | tee -a ./.pacsea_scan_vt.txt; ",
            "        echo \"SHA256: $h\" | tee -a ./.pacsea_scan_vt.txt; ",
            "        vt_total=$((vt_total+1)); ",
            "        resp=$(curl -s -H \"x-apikey: $VT_API_KEY\" \"https://www.virustotal.com/api/v3/files/$h\"); ",
            "        if echo \"$resp\" | grep -q '\"error\"'; then ",
            "          echo 'VT: No report found' | tee -a ./.pacsea_scan_vt.txt; ",
            "          vt_unknown=$((vt_unknown+1)); ",
            "        else ",
            "          mal=$(echo \"$resp\" | grep -o '\"malicious\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          sus=$(echo \"$resp\" | grep -o '\"suspicious\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          har=$(echo \"$resp\" | grep -o '\"harmless\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          und=$(echo \"$resp\" | grep -o '\"undetected\":[0-9]\\+' | head -n1 | cut -d: -f2); ",
            "          echo \"VT: malicious=${mal:-0} suspicious=${sus:-0} harmless=${har:-0} undetected=${und:-0}\" | tee -a ./.pacsea_scan_vt.txt; ",
            "          echo \"VT report: https://www.virustotal.com/gui/file/$h\" | tee -a ./.pacsea_scan_vt.txt; ",
            "          vt_known=$((vt_known+1)); ",
            "          vt_mal_sum=$((vt_mal_sum+${mal:-0})); ",
            "          vt_sus_sum=$((vt_sus_sum+${sus:-0})); ",
            "          vt_har_sum=$((vt_har_sum+${har:-0})); ",
            "          vt_und_sum=$((vt_und_sum+${und:-0})); ",
            "        fi; ",
            "        echo | tee -a ./.pacsea_scan_vt.txt >/dev/null; ",
            "      fi; ",
            "    done; ",
            "    { ",
            "      echo \"VT_TOTAL=$vt_total\"; ",
            "      echo \"VT_KNOWN=$vt_known\"; ",
            "      echo \"VT_UNKNOWN=$vt_unknown\"; ",
            "      echo \"VT_MAL=$vt_mal_sum\"; ",
            "      echo \"VT_SUS=$vt_sus_sum\"; ",
            "      echo \"VT_HAR=$vt_har_sum\"; ",
            "      echo \"VT_UND=$vt_und_sum\"; ",
            "    } > ./.pacsea_scan_vt_summary.txt; ",
            "  fi; ",
            "else ",
            "  echo 'VT_API_KEY not set; skipping VirusTotal lookups.'; ",
            "fi"
        )
        .to_string(),
    );

    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("if [ -f ./.pacsea_scan_clamav.txt ]; then inf=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); if [ -n \"$inf\" ]; then if [ \"$inf\" -gt 0 ]; then echo \"ClamAV: infected files: $inf\"; else echo \"ClamAV: no infections detected\"; fi; else echo 'ClamAV: no infections detected'; fi; else echo 'ClamAV: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_trivy.json ]; then c=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); h=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); m=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); l=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); t=$((c+h+m+l)); if [ \"$t\" -gt 0 ]; then echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; elif [ -f ./.pacsea_scan_trivy.txt ]; then if grep -qiE 'CRITICAL|HIGH|MEDIUM|LOW' ./.pacsea_scan_trivy.txt; then c=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); h=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); m=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); l=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); t=$((c+h+m+l)); echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; else echo 'Trivy: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_semgrep.json ]; then n=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; elif [ -f ./.pacsea_scan_semgrep.txt ]; then n=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; else echo 'Semgrep: not run'; fi".to_string());
    cmds.push("vtf=./.pacsea_scan_vt_summary.txt; if [ -f \"$vtf\" ]; then VT_TOTAL=$(grep -E '^VT_TOTAL=' \"$vtf\" | cut -d= -f2); VT_KNOWN=$(grep -E '^VT_KNOWN=' \"$vtf\" | cut -d= -f2); VT_UNKNOWN=$(grep -E '^VT_UNKNOWN=' \"$vtf\" | cut -d= -f2); VT_MAL=$(grep -E '^VT_MAL=' \"$vtf\" | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' \"$vtf\" | cut -d= -f2); VT_HAR=$(grep -E '^VT_HAR=' \"$vtf\" | cut -d= -f2); VT_UND=$(grep -E '^VT_UND=' \"$vtf\" | cut -d= -f2); echo \"VirusTotal: files=$VT_TOTAL known=$VT_KNOWN malicious=$VT_MAL suspicious=$VT_SUS harmless=$VT_HAR undetected=$VT_UND unknown=$VT_UNKNOWN\"; else echo 'VirusTotal: not configured or no files'; fi".to_string());
    cmds.push("echo".to_string());
    cmds.push("echo 'Pacsea: in-place scan finished.'".to_string());
    spawn_shell_commands_in_terminal(&cmds);
}

#[cfg(target_os = "windows")]
/// Windows stub: AUR scan is not supported on Windows; show a message in a terminal.
pub fn spawn_aur_scan_for(pkg: &str) {
    let msg = format!(
        "AUR scan is not supported on Windows. Intended to scan AUR package: {}",
        pkg
    );
    super::shell::spawn_shell_commands_in_terminal(&[format!("echo {}", msg)]);
}

#[cfg(target_os = "windows")]
/// Windows stub: In-place scan is not supported; show a message in a terminal.
pub fn spawn_aur_scan_in_dir(path: &str) {
    let msg = format!(
        "AUR scan is not supported on Windows. Intended to scan directory: {}",
        path
    );
    super::shell::spawn_shell_commands_in_terminal(&[format!("echo {}", msg)]);
}
