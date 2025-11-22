/*!
What: Common scan command builders shared between package and directory scans

Input:
- Command vector to append to

Output:
- Appends scan command strings to the provided vector

Details:
- Provides reusable functions for `ClamAV`, `Trivy`, `Semgrep`, `ShellCheck`, Custom patterns, `VirusTotal`, and `aur-sleuth` scans
*/

/// What: Add pattern environment variable exports to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends pattern export commands to the vector.
///
/// Details:
/// - Sets default pattern regexes for CRITICAL, HIGH, MEDIUM, and LOW severity levels if not already set.
#[cfg(not(target_os = "windows"))]
pub fn add_pattern_exports(cmds: &mut Vec<String>) {
    cmds.push("if [ -z \"${PACSEA_PATTERNS_CRIT:-}\" ]; then export PACSEA_PATTERNS_CRIT='/dev/(tcp|udp)/|bash -i *>& *[^ ]*/dev/(tcp|udp)/[0-9]+|exec [0-9]{2,}<>/dev/(tcp|udp)/|rm -rf[[:space:]]+/|dd if=/dev/zero of=/dev/sd[a-z]|[>]{1,2}[[:space:]]*/dev/sd[a-z]|: *\\(\\) *\\{ *: *\\| *: *& *\\};:|/etc/sudoers([[:space:]>]|$)|echo .*[>]{2}.*(/etc/sudoers|/root/.ssh/authorized_keys)|/etc/ld\\.so\\.preload|LD_PRELOAD=|authorized_keys.*[>]{2}|ssh-rsa [A-Za-z0-9+/=]+.*[>]{2}.*authorized_keys|curl .*(169\\.254\\.169\\.254)'; fi".to_string());
    cmds.push("if [ -z \"${PACSEA_PATTERNS_HIGH:-}\" ]; then export PACSEA_PATTERNS_HIGH='eval|base64 -d|wget .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|curl .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|sudo[[:space:]]|chattr[[:space:]]|useradd|adduser|groupadd|systemctl|service[[:space:]]|crontab|/etc/cron\\.|[>]{2}.*(\\.bashrc|\\.bash_profile|/etc/profile|\\.zshrc)|cat[[:space:]]+/etc/shadow|cat[[:space:]]+~/.ssh/id_rsa|cat[[:space:]]+~/.bash_history|systemctl stop (auditd|rsyslog)|service (auditd|rsyslog) stop|scp .*@|curl -F|nc[[:space:]].*<|tar -czv?f|zip -r'; fi".to_string());
    cmds.push("if [ -z \"${PACSEA_PATTERNS_MEDIUM:-}\" ]; then export PACSEA_PATTERNS_MEDIUM='whoami|uname -a|hostname|id|groups|nmap|netstat -anp|ss -anp|ifconfig|ip addr|arp -a|grep -ri .*secret|find .*-name.*(password|\\.key)|env[[:space:]]*\\|[[:space:]]*grep -i pass|wget https?://|curl https?://'; fi".to_string());
    cmds.push("if [ -z \"${PACSEA_PATTERNS_LOW:-}\" ]; then export PACSEA_PATTERNS_LOW='http_proxy=|https_proxy=|ALL_PROXY=|yes[[:space:]]+> */dev/null *&|ulimit -n [0-9]{5,}'; fi".to_string());
}

/// What: Add `ClamAV` scan commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `ClamAV` scan commands to the vector.
///
/// Details:
/// - Checks for `ClamAV` availability and signature database before running scan.
/// - Respects `PACSEA_SCAN_DO_CLAMAV` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_clamav_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- ClamAV scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🔍] ClamAV scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_CLAMAV:-1}\" = \"1\" ]; then ((command -v clamscan >/dev/null 2>&1 || sudo pacman -Qi clamav >/dev/null 2>&1) && { if find /var/lib/clamav -maxdepth 1 -type f \\( -name '*.cvd' -o -name '*.cld' \\) 2>/dev/null | grep -q .; then clamscan -r . | tee ./.pacsea_scan_clamav.txt; else echo 'ClamAV found but no signature database in /var/lib/clamav'; echo 'Tip: run: sudo freshclam  (or start the updater: sudo systemctl start clamav-freshclam)'; fi; } || echo 'ClamAV (clamscan) encountered an error; skipping') || echo 'ClamAV not found; skipping'; else echo 'ClamAV: skipped by config'; fi)".to_string());
}

/// What: Add Trivy filesystem scan commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends Trivy scan commands to the vector.
///
/// Details:
/// - Attempts JSON output first, falls back to text output.
/// - Respects `PACSEA_SCAN_DO_TRIVY` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_trivy_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- Trivy filesystem scan (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧰] Trivy filesystem scan (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_TRIVY:-1}\" = \"1\" ]; then ((command -v trivy >/dev/null 2>&1 || sudo pacman -Qi trivy >/dev/null 2>&1) && (trivy fs --quiet --format json . > ./.pacsea_scan_trivy.json || trivy fs --quiet . | tee ./.pacsea_scan_trivy.txt) || echo 'Trivy not found or failed; skipping'); else echo 'Trivy: skipped by config'; fi)".to_string());
}

/// What: Add Semgrep static analysis commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends Semgrep scan commands to the vector.
///
/// Details:
/// - Uses auto-config mode for `Semgrep`.
/// - Respects `PACSEA_SCAN_DO_SEMGREP` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_semgrep_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- Semgrep static analysis (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧪] Semgrep static analysis (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SEMGREP:-1}\" = \"1\" ]; then ((command -v semgrep >/dev/null 2>&1 || sudo pacman -Qi semgrep >/dev/null 2>&1) && (semgrep --config=auto --json . > ./.pacsea_scan_semgrep.json || semgrep --config=auto . | tee ./.pacsea_scan_semgrep.txt) || echo 'Semgrep not found; skipping'); else echo 'Semgrep: skipped by config'; fi)".to_string());
}

/// What: Add aur-sleuth audit commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends aur-sleuth audit commands to the vector.
///
/// Details:
/// - Searches for aur-sleuth in multiple locations.
/// - Loads proxy settings from Pacsea config if available.
/// - Respects `PACSEA_SCAN_DO_SLEUTH` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_sleuth_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- aur-sleuth audit (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🔎] aur-sleuth audit (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SLEUTH:-1}\" = \"1\" ]; then A_SLEUTH=\"$(command -v aur-sleuth 2>/dev/null || true)\"; if [ -z \"$A_SLEUTH\" ] && [ -x \"${HOME}/.local/bin/aur-sleuth\" ]; then A_SLEUTH=\"${HOME}/.local/bin/aur-sleuth\"; fi; if [ -z \"$A_SLEUTH\" ] && [ -x \"/usr/local/bin/aur-sleuth\" ]; then A_SLEUTH=\"/usr/local/bin/aur-sleuth\"; fi; if [ -z \"$A_SLEUTH\" ] && [ -x \"/usr/bin/aur-sleuth\" ]; then A_SLEUTH=\"/usr/bin/aur-sleuth\"; fi; if [ -n \"$A_SLEUTH\" ]; then cfg=\"${XDG_CONFIG_HOME:-$HOME/.config}/pacsea/settings.conf\"; if [ -f \"$cfg\" ]; then get_key() { awk -F= -v k=\"$1\" 'tolower($0) ~ \"^[[:space:]]*\"k\"[[:space:]]*=\" {sub(/#.*/,\"\",$2); gsub(/^[[:space:]]+|[[:space:]]+$/,\"\",$2); print $2; exit }' \"$cfg\"; }; HP=$(get_key http_proxy); [ -n \"$HP\" ] && export http_proxy=\"$HP\"; XP=$(get_key https_proxy); [ -n \"$XP\" ] && export https_proxy=\"$XP\"; AP=$(get_key all_proxy); [ -n \"$AP\" ] && export ALL_PROXY=\"$AP\"; NP=$(get_key no_proxy); [ -n \"$NP\" ] && export NO_PROXY=\"$NP\"; CAB=$(get_key requests_ca_bundle); [ -n \"$CAB\" ] && export REQUESTS_CA_BUNDLE=\"$CAB\"; SCF=$(get_key ssl_cert_file); [ -n \"$SCF\" ] && export SSL_CERT_FILE=\"$SCF\"; CCB=$(get_key curl_ca_bundle); [ -n \"$CCB\" ] && export CURL_CA_BUNDLE=\"$CCB\"; PIPIDX=$(get_key pip_index_url); [ -n \"$PIPIDX\" ] && export PIP_INDEX_URL=\"$PIPIDX\"; PIPEX=$(get_key pip_extra_index_url); [ -n \"$PIPEX\" ] && export PIP_EXTRA_INDEX_URL=\"$PIPEX\"; PIPTH=$(get_key pip_trusted_host); [ -n \"$PIPTH\" ] && export PIP_TRUSTED_HOST=\"$PIPTH\"; UVCA=$(get_key uv_http_ca_certs); [ -n \"$UVCA\" ] && export UV_HTTP_CA_CERTS=\"$UVCA\"; fi; \"$A_SLEUTH\" --output plain --pkgdir . | tee ./.pacsea_sleuth.txt || echo 'aur-sleuth failed; see output above'; else echo 'aur-sleuth not found (checked PATH, ~/.local/bin, /usr/local/bin, /usr/bin)'; fi; else echo 'aur-sleuth: skipped by config'; fi)".to_string());
}

/// What: Add `ShellCheck` lint commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `ShellCheck` lint commands to the vector.
///
/// Details:
/// - Analyzes `PKGBUILD` and `*.install` files.
/// - Respects `PACSEA_SCAN_DO_SHELLCHECK` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_shellcheck_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- ShellCheck lint (optional) ---'".to_string());
    cmds.push("echo -e '\\033[1;34m[🧹] ShellCheck lint (optional)\\033[0m'".to_string());
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SHELLCHECK:-1}\" = \"1\" ]; then if command -v shellcheck >/dev/null 2>&1 || sudo pacman -Qi shellcheck >/dev/null 2>&1; then if [ -f PKGBUILD ]; then echo \"[shellcheck] Analyzing: PKGBUILD (bash, -e SC2034)\"; (shellcheck -s bash -x -e SC2034 -f json PKGBUILD > ./.pacsea_shellcheck_pkgbuild.json || shellcheck -s bash -x -e SC2034 PKGBUILD | tee ./.pacsea_shellcheck_pkgbuild.txt || true); fi; inst_files=(); while IFS= read -r -d '' f; do inst_files+=(\"$f\"); done < <(find . -maxdepth 1 -type f -name \"*.install\" -print0); if [ \"${#inst_files[@]}\" -gt 0 ]; then echo \"[shellcheck] Analyzing: ${inst_files[*]} (bash)\"; (shellcheck -s bash -x -f json \"${inst_files[@]}\" > ./.pacsea_shellcheck_install.json || shellcheck -s bash -x \"${inst_files[@]}\" | tee ./.pacsea_shellcheck_install.txt || true); fi; else echo 'ShellCheck not found; skipping'; fi; else echo 'ShellCheck: skipped by config'; fi)".to_string());
}

/// What: Add ShellCheck risk evaluation commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends risk evaluation commands to the vector.
///
/// Details:
/// - Calculates risk score based on `ShellCheck` errors/warnings and `PKGBUILD` heuristics.
/// - Respects `PACSEA_SCAN_DO_SHELLCHECK` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_shellcheck_risk_eval(cmds: &mut Vec<String>) {
    cmds.push("(if [ \"${PACSEA_SCAN_DO_SHELLCHECK:-1}\" = \"1\" ]; then echo -e '\\033[1;33m[⚠️ ] Risk evaluation (PKGBUILD/.install)\\033[0m'; ({ sc_err=0; sc_warn=0; sc_info=0; sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"error\"' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"warning\"' | wc -l))); sc_info=$((sc_info + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '\"level\":\"info\"' | wc -l))); sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l))); sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l))); if [ -f PKGBUILD ]; then pkgrisk=$(grep -Eoi 'curl|wget|bash -c|sudo|chown|chmod|mktemp|systemctl|useradd|groupadd|nc\\s|socat|/tmp/' PKGBUILD | wc -l); else pkgrisk=0; fi; if ls ./*.install >/dev/null 2>&1; then inst_risk=$(grep -Eoi 'post_install|pre_install|post_upgrade|pre_upgrade|systemctl|useradd|groupadd|chown|chmod|sudo|service|adduser' ./*.install | wc -l); else inst_risk=0; fi; risk=$((sc_err*5 + sc_warn*2 + sc_info + pkgrisk*3 + inst_risk*4)); tier='LOW'; if [ \"$risk\" -ge 60 ]; then tier='CRITICAL'; elif [ \"$risk\" -ge 40 ]; then tier='HIGH'; elif [ \"$risk\" -ge 20 ]; then tier='MEDIUM'; fi; { echo \"SC_ERRORS=$sc_err\"; echo \"SC_WARNINGS=$sc_warn\"; echo \"SC_INFO=$sc_info\"; echo \"PKGBUILD_HEURISTICS=$pkgrisk\"; echo \"INSTALL_HEURISTICS=$inst_risk\"; echo \"RISK_SCORE=$risk\"; echo \"RISK_TIER=$tier\"; } > ./.pacsea_shellcheck_risk.txt; echo \"Risk score: $risk ($tier)\"; } || echo 'Risk evaluation encountered an error; skipping'); else echo 'Risk Evaluation: skipped (ShellCheck disabled)'; fi)".to_string());
}

/// What: Add custom suspicious patterns scan commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends custom pattern scan commands to the vector.
///
/// Details:
/// - Scans `PKGBUILD`, `*.install`, and shell files in `src/` for suspicious patterns.
/// - Calculates risk score based on pattern matches.
/// - Respects `PACSEA_SCAN_DO_CUSTOM` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_custom_pattern_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- Custom suspicious patterns scan (optional) ---'".to_string());
    cmds.push(
        "echo -e '\\033[1;34m[🕵️] Custom suspicious patterns scan (optional)\\033[0m'".to_string(),
    );
    cmds.push(r#"(if [ "${PACSEA_SCAN_DO_CUSTOM:-1}" = "1" ]; then
  files='';
  if [ -f PKGBUILD ]; then files='PKGBUILD'; fi;
  for f in ./*.install; do [ -f "$f" ] && files="$files $f"; done;
  # Include shell-like files under src/
  if [ -d ./src ]; then
    src_ext=$(find ./src -type f \( -name "*.sh" -o -name "*.bash" -o -name "*.zsh" -o -name "*.ksh" \) 2>/dev/null)
    src_shebang=$(grep -Ilr '^#!.*\b(sh|bash|zsh|ksh)\b' ./src 2>/dev/null)
    if [ -n "$src_ext$src_shebang" ]; then files="$files $src_ext $src_shebang"; fi;
  fi;
  if [ -z "$files" ]; then
    echo 'No PKGBUILD, .install, or src shell files to scan';
  else
    : > ./.pacsea_custom_scan.txt;
    # Critical indicators: reverse shells, destructive ops, sudoers/ld.so.preload tampering, FD sockets, authorized_keys backdoor, IMDS
    crit="$PACSEA_PATTERNS_CRIT";
    # High indicators: eval/obfuscation, download+execute, persistence, priv escalation, service control, data theft, log tamper
    high="$PACSEA_PATTERNS_HIGH";
    # Medium indicators: recon, network scan, sensitive search, proxies, generic downloads
    med="$PACSEA_PATTERNS_MEDIUM";
    # Low indicators: proxy vars, resource hints
    low="$PACSEA_PATTERNS_LOW";
    echo "[debug] Files to scan: $files"
    echo "[debug] Pattern (CRIT): $crit"
    echo "[debug] Pattern (HIGH): $high"
    echo "[debug] Pattern (MED):  $med"
    echo "[debug] Pattern (LOW):  $low"
    echo "[debug] Running grep for CRIT..."
    tmp=$(grep -Eo "$crit" $files 2>/dev/null); rc=$?; printf "%s\n" "$tmp" > ./.pacsea_custom_crit_hits.txt
    Ccrit=$(printf "%s" "$tmp" | wc -l); echo "[debug] grep CRIT rc=$rc count=$Ccrit"
    echo "[debug] Running grep for HIGH..."
    tmp=$(grep -Eo "$high" $files 2>/dev/null); rc=$?; printf "%s\n" "$tmp" > ./.pacsea_custom_high_hits.txt
    Chigh=$(printf "%s" "$tmp" | wc -l); echo "[debug] grep HIGH rc=$rc count=$Chigh"
    echo "[debug] Running grep for MED..."
    tmp=$(grep -Eo "$med" $files 2>/dev/null); rc=$?; printf "%s\n" "$tmp" > ./.pacsea_custom_med_hits.txt
    Cmed=$(printf "%s" "$tmp" | wc -l); echo "[debug] grep MED rc=$rc count=$Cmed"
    echo "[debug] Running grep for LOW..."
    tmp=$(grep -Eo "$low" $files 2>/dev/null); rc=$?; printf "%s\n" "$tmp" > ./.pacsea_custom_low_hits.txt
    Clow=$(printf "%s" "$tmp" | wc -l); echo "[debug] grep LOW rc=$rc count=$Clow"
    score=$((Ccrit*10 + Chigh*5 + Cmed*2 + Clow));
    if [ "$score" -gt 100 ]; then score=100; fi;
    tier='LOW';
    if [ "$score" -ge 75 ]; then tier='CRITICAL';
    elif [ "$score" -ge 50 ]; then tier='HIGH';
    elif [ "$score" -ge 25 ]; then tier='MEDIUM';
    fi;
    {
      echo "CUSTOM_CRIT=$Ccrit";
      echo "CUSTOM_HIGH=$Chigh";
      echo "CUSTOM_MED=$Cmed";
      echo "CUSTOM_LOW=$Clow";
      echo "CUSTOM_PERCENT=$score";
      echo "CUSTOM_TIER=$tier";
    } > ./.pacsea_custom_score.txt;
    echo "Custom suspicious patterns: crit=$Ccrit high=$Chigh med=$Cmed low=$Clow score=${score}% tier=$tier" | tee -a ./.pacsea_custom_scan.txt;
  fi;
else
  echo 'Custom scan: skipped by config';
fi)"#.to_string());
}

/// What: Add `VirusTotal` hash lookup commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `VirusTotal` lookup commands to the vector.
///
/// Details:
/// - Looks up SHA256 hashes of `PKGBUILD` and `src` files in `VirusTotal`.
/// - Requires `VT_API_KEY` environment variable or config setting.
/// - Respects `PACSEA_SCAN_DO_VIRUSTOTAL` environment variable.
#[cfg(not(target_os = "windows"))]
pub fn add_virustotal_scan(cmds: &mut Vec<String>) {
    cmds.push("echo '--- VirusTotal hash lookups (requires VT_API_KEY env var) ---'".to_string());
    cmds.push(
        "echo -e '\\033[1;34m[🔬] VirusTotal hash lookups (requires VT_API_KEY env var)\\033[0m'"
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
}
