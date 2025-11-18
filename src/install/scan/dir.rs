/*!
What: Directory scan command builder

Input:
- Target directory to scan

Output:
- Vector of shell commands for scanning a directory in-place

Details:
- Mirrors package scan but omits clone steps, operating on existing directory
*/

use super::common;

#[cfg_attr(not(test), allow(dead_code))]
#[cfg(not(target_os = "windows"))]
/// What: Build the in-place scan pipeline for an existing directory.
///
/// Input:
/// - `target_dir`: Directory containing AUR-like sources to analyse.
///
/// Output:
/// - Vector of shell commands executed in order when launching the scan terminal.
///
/// Details:
/// - Mirrors the package flow but omits clone steps, operating on the provided directory.
/// - Respects optional toggles for ClamAV, Trivy, Semgrep, aur-sleuth, ShellCheck, custom patterns, and VirusTotal.
pub fn build_scan_cmds_in_dir(target_dir: &str) -> Vec<String> {
    let mut cmds: Vec<String> = Vec::new();

    // Basic context and logging
    cmds.push(format!("target_dir='{}'", target_dir));
    cmds.push("echo \"[PACSEA] scan_start dir='$target_dir' ts=$(date -Ins) shell=$SHELL term=$TERM display=$DISPLAY\"".to_string());
    cmds.push("echo \"Pacsea: scanning directory in-place: '$target_dir'\"".to_string());
    cmds.push("cd \"$target_dir\" && { export PACSEA_DEBUG_LOG=\"$(pwd)/.pacsea_debug.log\"; exec > >(tee -a \"$PACSEA_DEBUG_LOG\") 2>&1; exec 9>>\"$PACSEA_DEBUG_LOG\"; export BASH_XTRACEFD=9; set -x; echo \"Pacsea debug: $(date) start in-place scan for '$target_dir' in $PWD\"; trap 'code=$?; echo; echo \"Pacsea debug: exit code=$code\"; echo \"Log: $PACSEA_DEBUG_LOG\"; echo \"Press any key to close...\"; read -rn1 -s _' EXIT; }".to_string());

    // Add all scan stages
    common::add_clamav_scan(&mut cmds);
    common::add_trivy_scan(&mut cmds);
    common::add_semgrep_scan(&mut cmds);
    common::add_sleuth_scan(&mut cmds);
    common::add_shellcheck_scan(&mut cmds);
    common::add_shellcheck_risk_eval(&mut cmds);
    common::add_custom_pattern_scan(&mut cmds);
    common::add_virustotal_scan(&mut cmds);

    // Summary and completion
    cmds.push("echo".to_string());
    cmds.push("echo '--- Summary ---'".to_string());
    cmds.push("echo -e '\\033[1;36m[📊] Summary\\033[0m'".to_string());
    cmds.push("echo 'Pacsea: in-place scan finished.'".to_string());

    cmds
}

#[cfg(all(test, not(target_os = "windows")))]
mod tests {
    use super::*;

    #[test]
    /// What: Verify directory-based scan commands include all optional scans and final messaging.
    ///
    /// Inputs:
    /// - Directory path `/tmp/example` passed to `build_scan_cmds_in_dir`.
    ///
    /// Output:
    /// - Returned command list contains environment setup, `cd`, optional scan sections, and completion echo.
    ///
    /// Details:
    /// - Uses substring checks on the joined script to ensure future edits keep the documented steps intact.
    fn build_scan_cmds_in_dir_has_core_steps() {
        let cmds = build_scan_cmds_in_dir("/tmp/example");
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
