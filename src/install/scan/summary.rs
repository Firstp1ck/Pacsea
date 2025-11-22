/*!
What: Summary command builders for scan results

Input:
- Command vector to append to

Output:
- Appends summary command strings to the provided vector

Details:
- Provides functions to generate overall risk assessment and per-scan summaries
*/

/// What: Add overall risk calculation commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends overall risk calculation commands to the vector.
///
/// Details:
/// - Aggregates scores from `ShellCheck`, `ClamAV`, `Trivy`, `Semgrep`, `VirusTotal`, and Custom scans.
/// - Calculates overall percentage and tier (LOW/MEDIUM/HIGH/CRITICAL).
#[cfg(not(target_os = "windows"))]
pub fn add_overall_risk_calc(cmds: &mut Vec<String>) {
    cmds.push(
        r#"(
  overall_score=0; overall_max=0;
  rf=./.pacsea_shellcheck_risk.txt;
  if [ -f "$rf" ]; then
    RS=$(grep -E '^RISK_SCORE=' "$rf" | cut -d= -f2); RS=${RS:-0};
    if [ "$RS" -gt 100 ]; then RS=100; fi;
    overall_score=$((overall_score+RS)); overall_max=$((overall_max+100));
  fi;

  if [ -f ./.pacsea_scan_clamav.txt ]; then
    INF=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs);
    INF=${INF:-0};
    CV=$([ "$INF" -gt 0 ] && echo 100 || echo 0);
    overall_score=$((overall_score+CV)); overall_max=$((overall_max+100));
  fi;

  TRI=0;
  if [ -f ./.pacsea_scan_trivy.json ]; then
    C=$(grep -o '"Severity":"CRITICAL"' ./.pacsea_scan_trivy.json | wc -l);
    H=$(grep -o '"Severity":"HIGH"' ./.pacsea_scan_trivy.json | wc -l);
    M=$(grep -o '"Severity":"MEDIUM"' ./.pacsea_scan_trivy.json | wc -l);
    L=$(grep -o '"Severity":"LOW"' ./.pacsea_scan_trivy.json | wc -l);
    TRI=$((C*10 + H*5 + M*2 + L));
  elif [ -f ./.pacsea_scan_trivy.txt ]; then
    C=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l);
    H=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l);
    M=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l);
    L=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l);
    TRI=$((C*10 + H*5 + M*2 + L));
  fi;
  if [ -f ./.pacsea_scan_trivy.json ] || [ -f ./.pacsea_scan_trivy.txt ]; then
    if [ "$TRI" -gt 100 ]; then TRI=100; fi;
    overall_score=$((overall_score+TRI)); overall_max=$((overall_max+100));
  fi;

  SG=0;
  if [ -f ./.pacsea_scan_semgrep.json ]; then
    SG=$(grep -o '"check_id"' ./.pacsea_scan_semgrep.json | wc -l);
  elif [ -f ./.pacsea_scan_semgrep.txt ]; then
    SG=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l);
  fi;
  if [ -f ./.pacsea_scan_semgrep.json ] || [ -f ./.pacsea_scan_semgrep.txt ]; then
    SG=$((SG*3)); if [ "$SG" -gt 100 ]; then SG=100; fi;
    overall_score=$((overall_score+SG)); overall_max=$((overall_max+100));
  fi;

  VT=0;
  if [ -f ./.pacsea_scan_vt_summary.txt ]; then
    VT_MAL=$(grep -E '^VT_MAL=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2);
    VT_SUS=$(grep -E '^VT_SUS=' ./.pacsea_scan_vt_summary.txt | cut -d= -f2);
    VT_MAL=${VT_MAL:-0}; VT_SUS=${VT_SUS:-0};
    VT=$((VT_MAL*10 + VT_SUS*3));
    if [ "$VT" -gt 100 ]; then VT=100; fi;
    overall_score=$((overall_score+VT)); overall_max=$((overall_max+100));
  fi;

  CS=0;
  if [ -f ./.pacsea_custom_score.txt ]; then
    CS=$(grep -E '^CUSTOM_PERCENT=' ./.pacsea_custom_score.txt | cut -d= -f2); CS=${CS:-0};
    if [ "$CS" -gt 100 ]; then CS=100; fi;
    overall_score=$((overall_score+CS)); overall_max=$((overall_max+100));
  fi;

  PCT=0; if [ "$overall_max" -gt 0 ]; then PCT=$((overall_score*100/overall_max)); fi;
  TIER='LOW'; COLOR='\033[1;32m'; ICON='[✔]';
  if [ "$PCT" -ge 75 ]; then TIER='CRITICAL'; COLOR='\033[1;31m'; ICON='[❌]';
  elif [ "$PCT" -ge 50 ]; then TIER='HIGH'; COLOR='\033[1;33m'; ICON='[❗]';
  elif [ "$PCT" -ge 25 ]; then TIER='MEDIUM'; COLOR='\033[1;34m'; ICON='[⚠️ ]';
  fi;

  echo -e "$COLOR$ICON Overall risk: ${PCT}% ($TIER)\033[0m";
  {
    echo "OVERALL_PERCENT=$PCT";
    echo "OVERALL_TIER=$TIER";
    echo "COMPONENT_MAX=$overall_max";
    echo "COMPONENT_SCORE=$overall_score";
  } > ./.pacsea_overall_risk.txt;
)"#
            .to_string(),
    );
}

/// What: Add `ClamAV` summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `ClamAV` summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_clamav_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"if [ -f ./.pacsea_scan_clamav.txt ]; then
  inf=$(grep -E 'Infected files:[[:space:]]*[0-9]+' ./.pacsea_scan_clamav.txt | tail -n1 | awk -F: '{print $2}' | xargs);
  if [ -n "$inf" ]; then
    if [ "$inf" -gt 0 ]; then echo "ClamAV: infected files: $inf";
    else echo "ClamAV: no infections detected"; fi;
  else
    echo 'ClamAV: no infections detected';
  fi;
else
  echo 'ClamAV: not run';
fi"#
            .to_string(),
    );
}

/// What: Add Trivy summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends Trivy summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_trivy_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"if [ -f ./.pacsea_scan_trivy.json ]; then
  c=$(grep -o '"Severity":"CRITICAL"' ./.pacsea_scan_trivy.json | wc -l);
  h=$(grep -o '"Severity":"HIGH"' ./.pacsea_scan_trivy.json | wc -l);
  m=$(grep -o '"Severity":"MEDIUM"' ./.pacsea_scan_trivy.json | wc -l);
  l=$(grep -o '"Severity":"LOW"' ./.pacsea_scan_trivy.json | wc -l);
  t=$((c+h+m+l));
  if [ "$t" -gt 0 ]; then
    echo "Trivy findings: critical=$c high=$h medium=$m low=$l total=$t";
  else
    echo 'Trivy: no vulnerabilities found';
  fi;
elif [ -f ./.pacsea_scan_trivy.txt ]; then
  if grep -qiE 'CRITICAL|HIGH|MEDIUM|LOW' ./.pacsea_scan_trivy.txt; then
    c=$(grep -oi 'CRITICAL' ./.pacsea_scan_trivy.txt | wc -l);
    h=$(grep -oi 'HIGH' ./.pacsea_scan_trivy.txt | wc -l);
    m=$(grep -oi 'MEDIUM' ./.pacsea_scan_trivy.txt | wc -l);
    l=$(grep -oi 'LOW' ./.pacsea_scan_trivy.txt | wc -l);
    t=$((c+h+m+l));
    echo "Trivy findings: critical=$c high=$h medium=$m low=$l total=$t";
  else
    echo 'Trivy: no vulnerabilities found';
  fi;
else
  echo 'Trivy: not run';
fi"#
        .to_string(),
    );
}

/// What: Add Semgrep summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends Semgrep summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_semgrep_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"if [ -f ./.pacsea_scan_semgrep.json ]; then
  n=$(grep -o '"check_id"' ./.pacsea_scan_semgrep.json | wc -l);
  if [ "$n" -gt 0 ]; then echo "Semgrep findings: $n";
  else echo 'Semgrep: no findings'; fi;
elif [ -f ./.pacsea_scan_semgrep.txt ]; then
  n=$(grep -E '^[^:]+:[0-9]+:[0-9]+:' ./.pacsea_scan_semgrep.txt | wc -l);
  if [ "$n" -gt 0 ]; then echo "Semgrep findings: $n";
  else echo 'Semgrep: no findings'; fi;
else
  echo 'Semgrep: not run';
fi"#
        .to_string(),
    );
}

/// What: Add `ShellCheck` summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `ShellCheck` summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_shellcheck_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"if [ -f ./.pacsea_shellcheck_pkgbuild.json ] || [ -f ./.pacsea_shellcheck_pkgbuild.txt ] || [ -f ./.pacsea_shellcheck_install.json ] || [ -f ./.pacsea_shellcheck_install.txt ]; then
  sc_err=0; sc_warn=0;
  sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '"level":"error"' | wc -l)));
  sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.json ./.pacsea_shellcheck_install.json 2>/dev/null | grep -o '"level":"warning"' | wc -l)));
  sc_err=$((sc_err + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'error:' | wc -l)));
  sc_warn=$((sc_warn + $(cat ./.pacsea_shellcheck_pkgbuild.txt ./.pacsea_shellcheck_install.txt 2>/dev/null | grep -oi 'warning:' | wc -l)));
  echo "ShellCheck: errors=$sc_err warnings=$sc_warn";
else
  echo 'ShellCheck: not run';
fi"#
            .to_string(),
    );
}

/// What: Add `ShellCheck` risk evaluation summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends `ShellCheck` risk evaluation summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_shellcheck_risk_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"rf=./.pacsea_shellcheck_risk.txt; if [ -f "$rf" ] && { [ -f ./.pacsea_shellcheck_pkgbuild.json ] || [ -f ./.pacsea_shellcheck_pkgbuild.txt ] || [ -f ./.pacsea_shellcheck_install.json ] || [ -f ./.pacsea_shellcheck_install.txt ]; }; then
  RS=$(grep -E '^RISK_SCORE=' "$rf" | cut -d= -f2);
  RT=$(grep -E '^RISK_TIER=' "$rf" | cut -d= -f2);
  echo "ShellCheck Risk Evaluation: score=$RS tier=$RT";
fi"#
            .to_string(),
    );
}

/// What: Add custom scan and `VirusTotal` summary commands to command vector.
///
/// Input:
/// - `cmds`: Mutable reference to command vector to append to.
///
/// Output:
/// - Appends custom scan and `VirusTotal` summary commands to the vector.
#[cfg(not(target_os = "windows"))]
pub fn add_custom_and_vt_summary(cmds: &mut Vec<String>) {
    cmds.push(
        r#"csf=./.pacsea_custom_score.txt; if [ -f "$csf" ]; then
  CP=$(grep -E '^CUSTOM_PERCENT=' "$csf" | cut -d= -f2);
  CT=$(grep -E '^CUSTOM_TIER=' "$csf" | cut -d= -f2);
  CC=$(grep -E '^CUSTOM_CRIT=' "$csf" | cut -d= -f2);
  CH=$(grep -E '^CUSTOM_HIGH=' "$csf" | cut -d= -f2);
  CM=$(grep -E '^CUSTOM_MED=' "$csf" | cut -d= -f2);
  CL=$(grep -E '^CUSTOM_LOW=' "$csf" | cut -d= -f2);
  echo "Custom scan: score=${CP}% tier=$CT crit=$CC high=$CH med=$CM low=$CL";
else
  echo 'Custom scan: not run';
fi
vtf=./.pacsea_scan_vt_summary.txt; if [ -f "$vtf" ]; then
  VT_TOTAL=$(grep -E '^VT_TOTAL=' "$vtf" | cut -d= -f2);
  VT_KNOWN=$(grep -E '^VT_KNOWN=' "$vtf" | cut -d= -f2);
  VT_UNKNOWN=$(grep -E '^VT_UNKNOWN=' "$vtf" | cut -d= -f2);
  VT_MAL=$(grep -E '^VT_MAL=' "$vtf" | cut -d= -f2);
  VT_SUS=$(grep -E '^VT_SUS=' "$vtf" | cut -d= -f2);
  VT_HAR=$(grep -E '^VT_HAR=' "$vtf" | cut -d= -f2);
  VT_UND=$(grep -E '^VT_UND=' "$vtf" | cut -d= -f2);
  echo "VirusTotal: files=$VT_TOTAL known=$VT_KNOWN malicious=$VT_MAL suspicious=$VT_SUS harmless=$VT_HAR undetected=$VT_UND unknown=$VT_UNKNOWN";
else
  echo 'VirusTotal: not configured or no files';
fi"#
            .to_string(),
    );
}
