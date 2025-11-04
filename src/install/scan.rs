/*!
What: AUR package scan launcher

Input:
- Package name to scan (clone-and-scan), or a target directory to scan in-place

Output:
- Spawns a terminal that runs a sequence of shell commands to clone, download sources, and run optional scanners; results are printed/saved into files in a temp directory (or target dir)

Details:
- Steps: clone AUR repo; run `makepkg -o`; run optional scanners (ClamAV, Trivy, Semgrep); optional VirusTotal hash lookups when VT_API_KEY is present
- Semgrep is not installed automatically; if missing, a warning is printed and the scan is skipped
- VirusTotal lookups are hash-based; unknown files may report "no report found"
- Working directory is a temporary directory printed to the terminal and preserved for inspection
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
    cmds.push("if command -v git >/dev/null 2>&1 || sudo pacman -Qi git >/dev/null 2>&1; then :; else echo 'git not found. Cannot clone AUR repo.'; exit 1; fi".to_string());

    // 1) Fetch PKGBUILD via AUR helper first; fallback to git clone
    cmds.push("echo 'Fetching PKGBUILD via AUR helper (-G)…'".to_string());
    cmds.push("(if command -v paru >/dev/null 2>&1; then paru -G \"$pkg\"; elif command -v yay >/dev/null 2>&1; then yay -G \"$pkg\"; else echo 'No AUR helper (paru/yay) found for -G'; false; fi) || (echo 'Falling back to git clone…'; git clone --depth 1 \"https://aur.archlinux.org/${pkg}.git\" || { echo 'Clone failed'; exit 1; })".to_string());
    cmds.push("if [ -f \"$pkg/PKGBUILD\" ]; then cd \"$pkg\"; else f=$(find \"$pkg\" -maxdepth 3 -type f -name PKGBUILD 2>/dev/null | head -n1); if [ -n \"$f\" ]; then cd \"$(dirname \"$f\")\"; elif [ -d \"$pkg\" ]; then cd \"$pkg\"; fi; fi".to_string());
    cmds.push("echo \"PKGBUILD path: $(pwd)/PKGBUILD\"".to_string());

    // 2) Download sources only
    cmds.push("echo 'Running makepkg -o (download sources only)…'".to_string());
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

    // 3) ClamAV scan
    cmds.push("echo '--- ClamAV scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🔍] ClamAV scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_CLAMAV:-1}\" = \"1\" ]; then ((command -v clamscan >/dev/null 2>&1 || sudo pacman -Qi clamav >/dev/null 2>&1) && { if find /var/lib/clamav -maxdepth 1 -type f \\( -name '*.cvd' -o -name '*.cld' \\) 2>/dev/null | grep -q .; then clamscan -r . | tee ./.pacsea_scan_clamav.txt; else echo 'ClamAV found but no signature database in /var/lib/clamav'; echo 'Tip: run: sudo freshclam  (or start the updater: sudo systemctl start clamav-freshclam)'; fi; } || echo 'ClamAV (clamscan) encountered an error; skipping') || echo 'ClamAV not found; skipping'; else echo 'ClamAV: skipped by config'; fi)".to_string());

    // 4) Trivy filesystem scan
    cmds.push("echo '--- Trivy filesystem scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧰] Trivy filesystem scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_TRIVY:-1}\" = \"1\" ]; then ((command -v trivy >/dev/null 2>&1 || sudo pacman -Qi trivy >/dev/null 2>&1) && (trivy fs --quiet --format json . > ./.pacsea_scan_trivy.json || trivy fs --quiet . | tee ./.pacsea_scan_trivy.txt) || echo 'Trivy not found or failed; skipping'); else echo 'Trivy: skipped by config'; fi)".to_string());

    // 5) Semgrep static analysis
    cmds.push("echo '--- Semgrep static analysis (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧪] Semgrep static analysis (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SEMGREP:-1}\" = \"1\" ]; then ((command -v semgrep >/dev/null 2>&1 || sudo pacman -Qi semgrep >/dev/null 2>&1) && (semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt) || echo 'Semgrep not found; skipping'); else echo 'Semgrep: skipped by config'; fi)".to_string());

    // 6) VirusTotal hash lookups
    // 6) ShellCheck lint (PKGBUILD and *.install) and Risk evaluation
    cmds.push("echo '--- ShellCheck lint (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧹] ShellCheck lint (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SHELLCHECK:-1}\" = \"1\" ]; then if command -v shellcheck >/dev/null 2>&1 || sudo pacman -Qi shellcheck >/dev/null 2>&1; then if [ -f PKGBUILD ]; then echo \"[shellcheck] Analyzing: PKGBUILD (bash, -e SC2034)\"; (shellcheck -s bash -x -e SC2034 -f json PKGBUILD > ./.pacsea_shellcheck_pkgbuild.json || shellcheck -s bash -x -e SC2034 PKGBUILD | tee ./.pacsea_shellcheck_pkgbuild.txt || true); fi; inst_files=(); while IFS= read -r -d '' f; do inst_files+=(\"$f\"); done < <(find . -maxdepth 1 -type f -name \"*.install\" -print0); if [ \"${#inst_files[@]}\" -gt 0 ]; then echo \"[shellcheck] Analyzing: ${inst_files[*]} (bash)\"; (shellcheck -s bash -x -f json \"${inst_files[@]}\" > ./.pacsea_shellcheck_install.json || shellcheck -s bash -x \"${inst_files[@]}\" | tee ./.pacsea_shellcheck_install.txt || true); fi; else echo 'ShellCheck not found; skipping'; fi; else echo 'ShellCheck: skipped by config'; fi)".to_string());
    cmds.push("echo -e '\\033[1;33m[⚠️ ] Risk evaluation (PKGBUILD/.install)\\033[0m'".to_string());
    cmds.push("({ sc_err=0; sc_warn=0; sc_info=0; sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"error\"' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"warning\"' | wc -l))); sc_info=$((sc_info + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"info\"' | wc -l))); sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l))); if [ -f PKGBUILD ]; then pkgrisk=$(grep -Eoi 'curl|wget|bash -c|sudo|chown|chmod|mktemp|systemctl|useradd|groupadd|nc\\s|socat|/tmp/' PKGBUILD | wc -l); else pkgrisk=0; fi; if ls ./*.install >/dev/null 2>&1; then inst_risk=$(grep -Eoi 'post_install|pre_install|post_upgrade|pre_upgrade|systemctl|useradd|groupadd|chown|chmod|sudo|service|adduser' ./*.install | wc -l); else inst_risk=0; fi; risk=$((sc_err*5 + sc_warn*2 + sc_info + pkgrisk*3 + inst_risk*4)); tier='LOW'; if [ \"$risk\" -ge 60 ]; then tier='CRITICAL'; elif [ \"$risk\" -ge 40 ]; then tier='HIGH'; elif [ \"$risk\" -ge 20 ]; then tier='MEDIUM'; fi; { echo \"SC_ERRORS=$sc_err\"; echo \"SC_WARNINGS=$sc_warn\"; echo \"SC_INFO=$sc_info\"; echo \"PKGBUILD_HEURISTICS=$pkgrisk\"; echo \"INSTALL_HEURISTICS=$inst_risk\"; echo \"RISK_SCORE=$risk\"; echo \"RISK_TIER=$tier\"; } > ./.pacsea_shellcheck_risk.txt; echo \"Risk score: $risk ($tier)\"; } || echo 'Risk evaluation encountered an error; skipping')".to_string());
    // 7) VirusTotal hash lookups
    cmds.push("echo '--- VirusTotal hash lookups (requires VT_API_KEY env var) ---'".to_string());
    cmds.push(
        "echo -e '\\033[1;33m[🔬] VirusTotal hash lookups (requires VT_API_KEY env var)\\033[0m'"
            .to_string(),
    );
    cmds.push(
        concat!(
            "if [ \"${PACSEA_SCAN_DO_VIRUSTOTAL:-1}\" = \"1\" ]; then ",
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
            "fi; ",
            "else ",
            "  echo 'VirusTotal: skipped by config.'; ",
            "fi"
        )
        .to_string(),
    );

    // 7) Final note with working directory for manual inspection
    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("echo -e '\\033[1;36m[📊] Summary\\033[0m'".to_string());
    cmds.push("({ overall_score=0; overall_max=0; rf=./.pacsea_shellcheck_risk.txt; if [ -f \"$rf\" ]; then RS=$(grep -E '^RISK_SCORE=' \"$rf\" | cut -d= -f2); RS=${RS:-0}; if [ \"$RS\" -gt 100 ]; then RS=100; fi; overall_score=$((overall_score+RS)); overall_max=$((overall_max+100)); fi; if [ -f ./.pacsea_scan_clamav.txt ]; then INF=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); INF=${INF:-0}; CV=$([ \"$INF\" -gt 0 ] && echo 100 || echo 0); overall_score=$((overall_score+CV)); overall_max=$((overall_max+100)); fi; TRI=0; if [ -f ./.pacsea_scan_trivy.json ]; then C=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); H=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); M=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); L=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); TRI=$((C*10 + H*5 + M*2 + L)); elif [ -f ./.pacsea_scan_trivy.txt ]; then C=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); H=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); M=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); L=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); TRI=$((C*10 + H*5 + M*2 + L)); fi; if [ -f ./.pacsea_scan_trivy.json ] || [ -f ./.pacsea_scan_trivy.txt ]; then if [ \"$TRI\" -gt 100 ]; then TRI=100; fi; overall_score=$((overall_score+TRI)); overall_max=$((overall_max+100)); fi; SG=0; if [ -f ./.pacsea_scan_semgrep.json ]; then SG=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); elif [ -f ./.pacsea_scan_semgrep.txt ]; then SG=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); fi; if [ -f ./.pacsea_scan_semgrep.json ] || [ -f ./.pacsea_scan_semgrep.txt ]; then SG=$((SG*3)); if [ \"$SG\" -gt 100 ]; then SG=100; fi; overall_score=$((overall_score+SG)); overall_max=$((overall_max+100)); fi; VT=0; if [ -f ./.pacsea_scan_vt_summary.txt ]; then VT_MAL=$(grep -E '^VT_MAL=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2); VT_MAL=${VT_MAL:-0}; VT_SUS=${VT_SUS:-0}; VT=$((VT_MAL*10 + VT_SUS*3)); if [ \"$VT\" -gt 100 ]; then VT=100; fi; overall_score=$((overall_score+VT)); overall_max=$((overall_max+100)); fi; PCT=0; if [ \"$overall_max\" -gt 0 ]; then PCT=$((overall_score*100/overall_max)); fi; TIER='LOW'; COLOR='\\033[1;32m'; ICON='[✔]'; if [ \"$PCT\" -ge 75 ]; then TIER='CRITICAL'; COLOR='\\033[1;31m'; ICON='[❌]'; elif [ \"$PCT\" -ge 50 ]; then TIER='HIGH'; COLOR='\\033[1;33m'; ICON='[❗]'; elif [ \"$PCT\" -ge 25 ]; then TIER='MEDIUM'; COLOR='\\033[1;34m'; ICON='[⚠️ ]'; fi; echo -e \"$COLOR$ICON Overall risk: ${PCT}% ($TIER)\\033[0m\"; { echo \"OVERALL_PERCENT=$PCT\"; echo \"OVERALL_TIER=$TIER\"; echo \"COMPONENT_MAX=$overall_max\"; echo \"COMPONENT_SCORE=$overall_score\"; } > ./.pacsea_overall_risk.txt; })".to_string());
    cmds.push("if [ -f ./.pacsea_scan_clamav.txt ]; then inf=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); if [ -n \"$inf\" ]; then if [ \"$inf\" -gt 0 ]; then echo \"ClamAV: infected files: $inf\"; else echo \"ClamAV: no infections detected\"; fi; else echo 'ClamAV: no infections detected'; fi; else echo 'ClamAV: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_trivy.json ]; then c=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); h=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); m=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); l=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); t=$((c+h+m+l)); if [ \"$t\" -gt 0 ]; then echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; elif [ -f ./.pacsea_scan_trivy.txt ]; then if grep -qiE 'CRITICAL|HIGH|MEDIUM|LOW' ./.pacsea_scan_trivy.txt; then c=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); h=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); m=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); l=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); t=$((c+h+m+l)); echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; else echo 'Trivy: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_semgrep.json ]; then n=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; elif [ -f ./.pacsea_scan_semgrep.txt ]; then n=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; else echo 'Semgrep: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_shellcheck_pkgbuild.json ] || [ -f ./.pacsea_shellcheck_pkgbuild.txt ] || [ -f ./.pacsea_shellcheck_install.json ] || [ -f ./.pacsea_shellcheck_install.txt ]; then sc_err=0; sc_warn=0; sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"error\"' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"warning\"' | wc -l))); sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l))); echo \"ShellCheck: errors=$sc_err warnings=$sc_warn\"; else echo 'ShellCheck: not run'; fi".to_string());
    cmds.push("rf=./.pacsea_shellcheck_risk.txt; if [ -f \"$rf\" ]; then RS=$(grep -E '^RISK_SCORE=' \"$rf\" | cut -d= -f2); RT=$(grep -E '^RISK_TIER=' \"$rf\" | cut -d= -f2); echo \"Shellcheck Risk Evaluation: score=$RS tier=$RT\"; else echo 'Shellcheck Risk Evaluation: not evaluated'; fi".to_string());
    cmds.push("vtf=./.pacsea_scan_vt_summary.txt; if [ -f \"$vtf\" ]; then VT_TOTAL=$(grep -E '^VT_TOTAL=' \"$vtf\" | cut -d= -f2); VT_KNOWN=$(grep -E '^VT_KNOWN=' \"$vtf\" | cut -d= -f2); VT_UNKNOWN=$(grep -E '^VT_UNKNOWN=' \"$vtf\" | cut -d= -f2); VT_MAL=$(grep -E '^VT_MAL=' \"$vtf\" | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' \"$vtf\" | cut -d= -f2); VT_HAR=$(grep -E '^VT_HAR=' \"$vtf\" | cut -d= -f2); VT_UND=$(grep -E '^VT_UND=' \"$vtf\" | cut -d= -f2); echo \"VirusTotal: files=$VT_TOTAL known=$VT_KNOWN malicious=$VT_MAL suspicious=$VT_SUS harmless=$VT_HAR undetected=$VT_UND unknown=$VT_UNKNOWN\"; else echo 'VirusTotal: not configured or no files'; fi".to_string());
    cmds.push("echo".to_string());
    cmds.push("echo \"Pacsea: scan finished. Working directory preserved: $work\"".to_string());
    cmds.push("echo -e \"\\033[1;32m[✔] Pacsea: scan finished.\\033[0m Working directory preserved: $work\"".to_string());

    cmds
}

/// What: Launch a terminal that performs an AUR package scan for a given package name
///
/// Input: `pkg` AUR package name to scan
/// Output: Spawns a terminal that runs the scan pipeline; artifacts are written under a temp working directory
///
/// Details:
/// - Clones `https://aur.archlinux.org/<pkg>.git` and runs `makepkg -o` (download sources only)
/// - Optionally runs ClamAV, Trivy (fs), and Semgrep scans
/// - If `VT_API_KEY` is available (or configured in Pacsea settings), performs VirusTotal hash lookups for PKGBUILD/src files
#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn spawn_aur_scan_for(pkg: &str) {
    let cmds = build_scan_cmds_for_pkg(pkg);
    spawn_shell_commands_in_terminal(&cmds);
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_aur_scan_for_with_config(
    pkg: &str,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
) {
    // Prepend environment exports so subsequent steps honor the selection
    let mut cmds: Vec<String> = Vec::new();
    cmds.push(format!(
        "export PACSEA_SCAN_DO_CLAMAV={}",
        if do_clamav { "1" } else { "0" }
    ));
    cmds.push(format!(
        "export PACSEA_SCAN_DO_TRIVY={}",
        if do_trivy { "1" } else { "0" }
    ));
    cmds.push(format!(
        "export PACSEA_SCAN_DO_SEMGREP={}",
        if do_semgrep { "1" } else { "0" }
    ));
    cmds.push(format!(
        "export PACSEA_SCAN_DO_SHELLCHECK={}",
        if do_shellcheck { "1" } else { "0" }
    ));
    cmds.push(format!(
        "export PACSEA_SCAN_DO_VIRUSTOTAL={}",
        if do_virustotal { "1" } else { "0" }
    ));
    // Append the scan pipeline commands
    cmds.extend(build_scan_cmds_for_pkg(pkg));
    spawn_shell_commands_in_terminal(&cmds);
}

/// What: Launch a terminal to perform an in-place scan of an unpacked AUR package directory
///
/// Input: `path` directory containing PKGBUILD/src files
/// Output: Spawns a terminal that runs optional scanners in-place; results are written to files within the directory
///
/// Details: Does not clone or run `makepkg -o`; only executes the scanners and optional VirusTotal lookups.
#[cfg(not(target_os = "windows"))]
fn build_scan_cmds_in_dir(path: &str) -> Vec<String> {
    let mut cmds: Vec<String> = Vec::new();
    cmds.push(format!("target_dir='{}'", path));
    cmds.push("if [ -d \"$target_dir\" ]; then cd \"$target_dir\"; else echo \"Directory not found: $target_dir\"; exit 1; fi".to_string());

    // Optional scanners
    cmds.push("echo '--- ClamAV scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🔍] ClamAV scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_CLAMAV:-1}\" = \"1\" ]; then ((command -v clamscan >/dev/null 2>&1 || sudo pacman -Qi clamav >/dev/null 2>&1) && { if find /var/lib/clamav -maxdepth 1 -type f \\( -name '*.cvd' -o -name '*.cld' \\) 2>/dev/null | grep -q .; then clamscan -r . | tee ./.pacsea_scan_clamav.txt; else echo 'ClamAV found but no signature database in /var/lib/clamav'; echo 'Tip: run: sudo freshclam  (or start the updater: sudo systemctl start clamav-freshclam)'; fi; } || echo 'ClamAV (clamscan) encountered an error; skipping') || echo 'ClamAV not found; skipping'; else echo 'ClamAV: skipped by config'; fi)".to_string());

    cmds.push("echo '--- Trivy filesystem scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧰] Trivy filesystem scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_TRIVY:-1}\" = \"1\" ]; then ((command -v trivy >/dev/null 2>&1 || sudo pacman -Qi trivy >/dev/null 2>&1) && (trivy fs --quiet --format json . > ./.pacsea_scan_trivy.json || trivy fs --quiet . | tee ./.pacsea_scan_trivy.txt) || echo 'Trivy not found or failed; skipping'); else echo 'Trivy: skipped by config'; fi)".to_string());

    cmds.push("echo '--- Semgrep static analysis (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧪] Semgrep static analysis (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SEMGREP:-1}\" = \"1\" ]; then ((command -v semgrep >/dev/null 2>&1 || sudo pacman -Qi semgrep >/dev/null 2>&1) && (semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt) || echo 'Semgrep not found; skipping'); else echo 'Semgrep: skipped by config'; fi)".to_string());

    // VirusTotal hash lookups
    // ShellCheck lint (PKGBUILD and *.install) and Risk evaluation
    cmds.push("echo '--- ShellCheck lint (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧹] ShellCheck lint (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SHELLCHECK:-1}\" = \"1\" ]; then if command -v shellcheck >/dev/null 2>&1 || sudo pacman -Qi shellcheck >/dev/null 2>&1; then if [ -f PKGBUILD ]; then echo \"[shellcheck] Analyzing: PKGBUILD (bash, -e SC2034)\"; (shellcheck -s bash -x -e SC2034 -f json PKGBUILD > ./.pacsea_shellcheck_pkgbuild.json || shellcheck -s bash -x -e SC2034 PKGBUILD | tee ./.pacsea_shellcheck_pkgbuild.txt || true); fi; inst_files=(); while IFS= read -r -d '' f; do inst_files+=(\"$f\"); done < <(find . -maxdepth 1 -type f -name \"*.install\" -print0); if [ \"${#inst_files[@]}\" -gt 0 ]; then echo \"[shellcheck] Analyzing: ${inst_files[*]} (bash)\"; (shellcheck -s bash -x -f json \"${inst_files[@]}\" > ./.pacsea_shellcheck_install.json || shellcheck -s bash -x \"${inst_files[@]}\" | tee ./.pacsea_shellcheck_install.txt || true); fi; else echo 'ShellCheck not found; skipping'; fi; else echo 'ShellCheck: skipped by config'; fi)".to_string());
    cmds.push("echo -e '\\033[1;33m[⚠️ ] Risk evaluation (PKGBUILD/.install)\\033[0m'".to_string());
    cmds.push("({ sc_err=0; sc_warn=0; sc_info=0; sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"error\"' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"warning\"' | wc -l))); sc_info=$((sc_info + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"info\"' | wc -l))); sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l))); if [ -f PKGBUILD ]; then pkgrisk=$(grep -Eoi 'curl|wget|bash -c|sudo|chown|chmod|mktemp|systemctl|useradd|groupadd|nc\\s|socat|/tmp/' PKGBUILD | wc -l); else pkgrisk=0; fi; if ls ./*.install >/dev/null 2>&1; then inst_risk=$(grep -Eoi 'post_install|pre_install|post_upgrade|pre_upgrade|systemctl|useradd|groupadd|chown|chmod|sudo|service|adduser' ./*.install | wc -l); else inst_risk=0; fi; risk=$((sc_err*5 + sc_warn*2 + sc_info + pkgrisk*3 + inst_risk*4)); tier='LOW'; if [ \"$risk\" -ge 60 ]; then tier='CRITICAL'; elif [ \"$risk\" -ge 40 ]; then tier='HIGH'; elif [ \"$risk\" -ge 20 ]; then tier='MEDIUM'; fi; { echo \"SC_ERRORS=$sc_err\"; echo \"SC_WARNINGS=$sc_warn\"; echo \"SC_INFO=$sc_info\"; echo \"PKGBUILD_HEURISTICS=$pkgrisk\"; echo \"INSTALL_HEURISTICS=$inst_risk\"; echo \"RISK_SCORE=$risk\"; echo \"RISK_TIER=$tier\"; } > ./.pacsea_shellcheck_risk.txt; echo \"Risk score: $risk ($tier)\"; } || echo 'Risk evaluation encountered an error; skipping')".to_string());
    cmds.push("echo '--- VirusTotal hash lookups (requires VT_API_KEY env var) ---'".to_string());
    cmds.push(
        "echo -e '\\033[1;33m[🔬] VirusTotal hash lookups (requires VT_API_KEY env var)\\033[0m'"
            .to_string(),
    );
    cmds.push(
        concat!(
            "if [ \"${PACSEA_SCAN_DO_VIRUSTOTAL:-1}\" = \"1\" ]; then ",
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
            "fi; ",
            "else ",
            "  echo 'VirusTotal: skipped by config.'; ",
            "fi"
        )
        .to_string(),
    );

    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("echo -e '\\033[1;36m[📊] Summary\\033[0m'".to_string());
    cmds.push("({ overall_score=0; overall_max=0; rf=./.pacsea_shellcheck_risk.txt; if [ -f \"$rf\" ]; then RS=$(grep -E '^RISK_SCORE=' \"$rf\" | cut -d= -f2); RS=${RS:-0}; if [ \"$RS\" -gt 100 ]; then RS=100; fi; overall_score=$((overall_score+RS)); overall_max=$((overall_max+100)); fi; if [ -f ./.pacsea_scan_clamav.txt ]; then INF=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); INF=${INF:-0}; CV=$([ \"$INF\" -gt 0 ] && echo 100 || echo 0); overall_score=$((overall_score+CV)); overall_max=$((overall_max+100)); fi; TRI=0; if [ -f ./.pacsea_scan_trivy.json ]; then C=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); H=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); M=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); L=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); TRI=$((C*10 + H*5 + M*2 + L)); elif [ -f ./.pacsea_scan_trivy.txt ]; then C=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); H=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); M=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); L=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); TRI=$((C*10 + H*5 + M*2 + L)); fi; if [ -f ./.pacsea_scan_trivy.json ] || [ -f ./.pacsea_scan_trivy.txt ]; then if [ \"$TRI\" -gt 100 ]; then TRI=100; fi; overall_score=$((overall_score+TRI)); overall_max=$((overall_max+100)); fi; SG=0; if [ -f ./.pacsea_scan_semgrep.json ]; then SG=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); elif [ -f ./.pacsea_scan_semgrep.txt ]; then SG=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); fi; if [ -f ./.pacsea_scan_semgrep.json ] || [ -f ./.pacsea_scan_semgrep.txt ]; then SG=$((SG*3)); if [ \"$SG\" -gt 100 ]; then SG=100; fi; overall_score=$((overall_score+SG)); overall_max=$((overall_max+100)); fi; VT=0; if [ -f ./.pacsea_scan_vt_summary.txt ]; then VT_MAL=$(grep -E '^VT_MAL=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2); VT_MAL=${VT_MAL:-0}; VT_SUS=${VT_SUS:-0}; VT=$((VT_MAL*10 + VT_SUS*3)); if [ \"$VT\" -gt 100 ]; then VT=100; fi; overall_score=$((overall_score+VT)); overall_max=$((overall_max+100)); fi; PCT=0; if [ \"$overall_max\" -gt 0 ]; then PCT=$((overall_score*100/overall_max)); fi; TIER='LOW'; COLOR='\\033[1;32m'; ICON='[✔]'; if [ \"$PCT\" -ge 75 ]; then TIER='CRITICAL'; COLOR='\\033[1;31m'; ICON='[❌]'; elif [ \"$PCT\" -ge 50 ]; then TIER='HIGH'; COLOR='\\033[1;33m'; ICON='[❗]'; elif [ \"$PCT\" -ge 25 ]; then TIER='MEDIUM'; COLOR='\\033[1;34m'; ICON='[⚠️ ]'; fi; echo -e \"$COLOR$ICON Overall risk: ${PCT}% ($TIER)\\033[0m\"; { echo \"OVERALL_PERCENT=$PCT\"; echo \"OVERALL_TIER=$TIER\"; echo \"COMPONENT_MAX=$overall_max\"; echo \"COMPONENT_SCORE=$overall_score\"; } > ./.pacsea_overall_risk.txt; })".to_string());
    cmds.push("if [ -f ./.pacsea_scan_clamav.txt ]; then inf=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs); if [ -n \"$inf\" ]; then if [ \"$inf\" -gt 0 ]; then echo \"ClamAV: infected files: $inf\"; else echo \"ClamAV: no infections detected\"; fi; else echo 'ClamAV: no infections detected'; fi; else echo 'ClamAV: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_trivy.json ]; then c=$(grep -o '\"Severity\":\"CRITICAL\"' ./.pacsea_scan_trivy.json | wc -l); h=$(grep -o '\"Severity\":\"HIGH\"' ./.pacsea_scan_trivy.json | wc -l); m=$(grep -o '\"Severity\":\"MEDIUM\"' ./.pacsea_scan_trivy.json | wc -l); l=$(grep -o '\"Severity\":\"LOW\"' ./.pacsea_scan_trivy.json | wc -l); t=$((c+h+m+l)); if [ \"$t\" -gt 0 ]; then echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; elif [ -f ./.pacsea_scan_trivy.txt ]; then if grep -qiE 'CRITICAL|HIGH|MEDIUM|LOW' ./.pacsea_scan_trivy.txt; then c=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l); h=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l); m=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l); l=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l); t=$((c+h+m+l)); echo \"Trivy findings: critical=$c high=$h medium=$m low=$l total=$t\"; else echo 'Trivy: no vulnerabilities found'; fi; else echo 'Trivy: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_scan_semgrep.json ]; then n=$(grep -o '\"check_id\"' ./.pacsea_scan_semgrep.json | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; elif [ -f ./.pacsea_scan_semgrep.txt ]; then n=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l); if [ \"$n\" -gt 0 ]; then echo \"Semgrep findings: $n\"; else echo 'Semgrep: no findings'; fi; else echo 'Semgrep: not run'; fi".to_string());
    cmds.push("if [ -f ./.pacsea_shellcheck_pkgbuild.json ] || [ -f ./.pacsea_shellcheck_pkgbuild.txt ] || [ -f ./.pacsea_shellcheck_install.json ] || [ -f ./.pacsea_shellcheck_install.txt ]; then sc_err=0; sc_warn=0; sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"error\"' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"warning\"' | wc -l))); sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l))); echo \"ShellCheck: errors=$sc_err warnings=$sc_warn\"; else echo 'ShellCheck: not run'; fi".to_string());
    cmds.push("rf=./.pacsea_shellcheck_risk.txt; if [ -f \"$rf\" ]; then RS=$(grep -E '^RISK_SCORE=' \"$rf\" | cut -d= -f2); RT=$(grep -E '^RISK_TIER=' \"$rf\" | cut -d= -f2); echo \"Shellcheck Risk Evaluation: score=$RS tier=$RT\"; else echo 'Shellcheck Risk Evaluation: not evaluated'; fi".to_string());
    cmds.push("vtf=./.pacsea_scan_vt_summary.txt; if [ -f \"$vtf\" ]; then VT_TOTAL=$(grep -E '^VT_TOTAL=' \"$vtf\" | cut -d= -f2); VT_KNOWN=$(grep -E '^VT_KNOWN=' \"$vtf\" | cut -d= -f2); VT_UNKNOWN=$(grep -E '^VT_UNKNOWN=' \"$vtf\" | cut -d= -f2); VT_MAL=$(grep -E '^VT_MAL=' \"$vtf\" | cut -d= -f2); VT_SUS=$(grep -E '^VT_SUS=' \"$vtf\" | cut -d= -f2); VT_HAR=$(grep -E '^VT_HAR=' \"$vtf\" | cut -d= -f2); VT_UND=$(grep -E '^VT_UND=' \"$vtf\" | cut -d= -f2); echo \"VirusTotal: files=$VT_TOTAL known=$VT_KNOWN malicious=$VT_MAL suspicious=$VT_SUS harmless=$VT_HAR undetected=$VT_UND unknown=$VT_UNKNOWN\"; else echo 'VirusTotal: not configured or no files'; fi".to_string());
    cmds.push("echo".to_string());
    cmds.push("echo 'Pacsea: in-place scan finished.'".to_string());
    cmds.push("echo -e '\\033[1;32m[✔] Pacsea: in-place scan finished.\\033[0m'".to_string());
    cmds
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn spawn_aur_scan_in_dir(path: &str) {
    let cmds = build_scan_cmds_in_dir(path);
    spawn_shell_commands_in_terminal(&cmds);
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    #[test]
    fn build_scan_cmds_for_pkg_has_core_steps() {
        let cmds = super::build_scan_cmds_for_pkg("foobar");
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

    #[test]
    fn build_scan_cmds_in_dir_has_core_steps() {
        let cmds = super::build_scan_cmds_in_dir("/tmp/example");
        let joined = cmds.join("\n");

        assert!(
            joined.contains("target_dir='/tmp/example'"),
            "should define target_dir variable"
        );
        assert!(
            joined.contains("cd \"$target_dir\""),
            "should cd into target_dir"
        );
        assert!(
            joined.contains("--- ClamAV scan (optional) ---"),
            "should include ClamAV section"
        );
        assert!(
            joined.contains("--- Trivy filesystem scan (optional) ---"),
            "should include Trivy section"
        );
        assert!(
            joined.contains("--- Semgrep static analysis (optional) ---"),
            "should include Semgrep section"
        );
        assert!(
            joined.contains("--- VirusTotal hash lookups (requires VT_API_KEY env var) ---"),
            "should include VirusTotal section"
        );
        assert!(
            joined.contains("echo '--- Summary ---'"),
            "should include summary section"
        );
        assert!(
            joined.contains("Pacsea: in-place scan finished."),
            "should include final completion echo"
        );
    }
}

#[cfg(target_os = "windows")]
/// What: Windows stub for AUR package scan
///
/// Input: `pkg` AUR package name
/// Output: Opens a terminal echoing that AUR scan is not supported on Windows
///
/// Details: No scanning is performed.
pub fn spawn_aur_scan_for(pkg: &str) {
    let msg = format!(
        "AUR scan is not supported on Windows. Intended to scan AUR package: {}",
        pkg
    );
    super::shell::spawn_shell_commands_in_terminal(&[format!("echo {}", msg)]);
}

#[cfg(target_os = "windows")]
/// What: Windows stub for in-place scan
///
/// Input: `path` target directory
/// Output: Opens a terminal echoing that AUR scan is unsupported on Windows
///
/// Details: No scanning is performed.
pub fn spawn_aur_scan_in_dir(path: &str) {
    let msg = format!(
        "AUR scan is not supported on Windows. Intended to scan directory: {}",
        path
    );
    super::shell::spawn_shell_commands_in_terminal(&[format!("echo {}", msg)]);
}
