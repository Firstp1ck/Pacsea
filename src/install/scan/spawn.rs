/*!
What: Scan spawn launcher

Input:
- Package name and scan configuration flags

Output:
- Spawns a terminal running the scan pipeline

Details:
- Configures environment variables and launches the scan terminal
*/

/// What: Launch a terminal that performs an AUR package scan for a given package name
///
/// Input:
/// - `pkg`: AUR package identifier to analyse.
/// - `do_clamav`/`do_trivy`/`do_semgrep`/`do_shellcheck`/`do_virustotal`/`do_custom`/`do_sleuth`: Toggles enabling optional scan stages.
///
/// Output:
/// - Spawns a terminal that runs the scan pipeline and writes artifacts under a temporary working directory.
///
/// Details:
/// - Clones `https://aur.archlinux.org/<pkg>.git` and runs `makepkg -o` (download sources only).
/// - Optionally runs `ClamAV`, `Trivy` filesystem, and `Semgrep` scans.
/// - Performs `VirusTotal` hash lookups for `PKGBUILD`/`src` files when `VT_API_KEY` is provided via environment or Pacsea settings.
#[cfg(not(target_os = "windows"))]
#[allow(clippy::too_many_arguments)]
pub fn spawn_aur_scan_for_with_config(
    pkg: &str,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    do_sleuth: bool,
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
    cmds.push(format!(
        "export PACSEA_SCAN_DO_CUSTOM={}",
        if do_custom { "1" } else { "0" }
    ));
    // Export aur-sleuth toggle from UI/config
    cmds.push(format!(
        "export PACSEA_SCAN_DO_SLEUTH={}",
        if do_sleuth { "1" } else { "0" }
    ));
    // Export default pattern sets (can be overridden by PACSEA_PATTERNS_* env or pattern.conf in future)
    cmds.push("export PACSEA_PATTERNS_CRIT='/dev/(tcp|udp)/|bash -i *>& *[^ ]*/dev/(tcp|udp)/[0-9]+|exec [0-9]{2,}<>/dev/(tcp|udp)/|rm -rf[[:space:]]+/|dd if=/dev/zero of=/dev/sd[a-z]|[>]{1,2}[[:space:]]*/dev/sd[a-z]|: *\\(\\) *\\{ *: *\\| *: *& *\\};:|/etc/sudoers([[:space:]>]|$)|echo .*[>]{2}.*(/etc/sudoers|/root/.ssh/authorized_keys)|/etc/ld\\.so\\.preload|LD_PRELOAD=|authorized_keys.*[>]{2}|ssh-rsa [A-Za-z0-9+/=]+.*[>]{2}.*authorized_keys|curl .*(169\\.254\\.169\\.254)'".to_string());
    cmds.push("export PACSEA_PATTERNS_HIGH='eval|base64 -d|wget .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|curl .*(sh|bash|dash|ksh|zsh)([^A-Za-z]|$)|sudo[[:space:]]|chattr[[:space:]]|useradd|adduser|groupadd|systemctl|service[[:space:]]|crontab|/etc/cron\\.|[>]{2}.*(\\.bashrc|\\.bash_profile|/etc/profile|\\.zshrc)|cat[[:space:]]+/etc/shadow|cat[[:space:]]+~/.ssh/id_rsa|cat[[:space:]]+~/.bash_history|systemctl stop (auditd|rsyslog)|service (auditd|rsyslog) stop|scp .*@|curl -F|nc[[:space:]].*<|tar -czv?f|zip -r'".to_string());
    cmds.push("export PACSEA_PATTERNS_MEDIUM='whoami|uname -a|hostname|id|groups|nmap|netstat -anp|ss -anp|ifconfig|ip addr|arp -a|grep -ri .*secret|find .*-name.*(password|\\.key)|env[[:space:]]*\\|[[:space:]]*grep -i pass|wget https?://|curl https?://'".to_string());
    cmds.push("export PACSEA_PATTERNS_LOW='http_proxy=|https_proxy=|ALL_PROXY=|yes[[:space:]]+> */dev/null *&|ulimit -n [0-9]{5,}'".to_string());
    // Append the scan pipeline commands
    cmds.extend(super::pkg::build_scan_cmds_for_pkg(pkg));
    super::super::shell::spawn_shell_commands_in_terminal_with_hold(&cmds, false);
}
