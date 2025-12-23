/*!
What: Scan spawn launcher

Input:
- Package name and scan configuration flags

Output:
- Uses integrated process for scans (excluding aur-sleuth)
- Spawns terminal for aur-sleuth if enabled

Details:
- Configures environment variables and launches scans via executor
- aur-sleuth runs in separate terminal simultaneously
*/

/// What: Build aur-sleuth command for terminal execution.
///
/// Input:
/// - `pkg`: AUR package identifier to analyse.
///
/// Output:
/// - Command string for terminal execution.
///
/// Details:
/// - Sets up working directory, finds aur-sleuth binary, loads proxy settings, and runs aur-sleuth.
#[cfg(not(target_os = "windows"))]
#[must_use]
pub fn build_sleuth_command_for_terminal(pkg: &str) -> String {
    // This command will be run in a separate terminal
    // It sets up the working directory, finds aur-sleuth, loads config, and runs the scan
    format!(
        r#"pkg='{pkg}'; work=$(mktemp -d -t pacsea_scan_XXXXXXXX); cd "$work" && \
(if command -v paru >/dev/null 2>&1; then paru -G "$pkg"; elif command -v yay >/dev/null 2>&1; then yay -G "$pkg"; else git clone --depth 1 "https://aur.archlinux.org/${{pkg}}.git" || exit 1; fi) && \
if [ -f "$pkg/PKGBUILD" ]; then cd "$pkg"; else f=$(find "$pkg" -maxdepth 3 -type f -name PKGBUILD 2>/dev/null | head -n1); if [ -n "$f" ]; then cd "$(dirname "$f")"; elif [ -d "$pkg" ]; then cd "$pkg"; fi; fi && \
A_SLEUTH="$(command -v aur-sleuth 2>/dev/null || true)"; \
if [ -z "$A_SLEUTH" ] && [ -x "$HOME/.local/bin/aur-sleuth" ]; then A_SLEUTH="$HOME/.local/bin/aur-sleuth"; fi; \
if [ -z "$A_SLEUTH" ] && [ -x "/usr/local/bin/aur-sleuth" ]; then A_SLEUTH="/usr/local/bin/aur-sleuth"; fi; \
if [ -z "$A_SLEUTH" ] && [ -x "/usr/bin/aur-sleuth" ]; then A_SLEUTH="/usr/bin/aur-sleuth"; fi; \
if [ -n "$A_SLEUTH" ]; then \
  cfg="${{XDG_CONFIG_HOME:-$HOME/.config}}/pacsea/settings.conf"; \
  if [ -f "$cfg" ]; then \
    get_key() {{ awk -F= -v k="$1" 'tolower($0) ~ "^[[:space:]]*"k"[[:space:]]*=" {{ sub(/#.*/,"",$2); gsub(/^[[:space:]]+|[[:space:]]+$/,"",$2); print $2; exit }}' "$cfg"; }}; \
    HP=$(get_key http_proxy); [ -n "$HP" ] && export http_proxy="$HP"; \
    XP=$(get_key https_proxy); [ -n "$XP" ] && export https_proxy="$XP"; \
    AP=$(get_key all_proxy); [ -n "$AP" ] && export ALL_PROXY="$AP"; \
    NP=$(get_key no_proxy); [ -n "$NP" ] && export NO_PROXY="$NP"; \
    CAB=$(get_key requests_ca_bundle); [ -n "$CAB" ] && export REQUESTS_CA_BUNDLE="$CAB"; \
    SCF=$(get_key ssl_cert_file); [ -n "$SCF" ] && export SSL_CERT_FILE="$SCF"; \
    CCB=$(get_key curl_ca_bundle); [ -n "$CCB" ] && export CURL_CA_BUNDLE="$CCB"; \
    PIPIDX=$(get_key pip_index_url); [ -n "$PIPIDX" ] && export PIP_INDEX_URL="$PIPIDX"; \
    PIPEX=$(get_key pip_extra_index_url); [ -n "$PIPEX" ] && export PIP_EXTRA_INDEX_URL="$PIPEX"; \
    PIPTH=$(get_key pip_trusted_host); [ -n "$PIPTH" ] && export PIP_TRUSTED_HOST="$PIPTH"; \
    UVCA=$(get_key uv_http_ca_certs); [ -n "$UVCA" ] && export UV_HTTP_CA_CERTS="$UVCA"; \
  fi; \
  WORK_DIR=$(pwd); \
  SLEUTH_OUTPUT_FILE="./.pacsea_sleuth.txt"; \
  if command -v script >/dev/null 2>&1; then \
    SLEUTH_CMD="cd $(printf '%q' "$WORK_DIR") && script -f -q $(printf '%q' "$SLEUTH_OUTPUT_FILE") -c \"$(printf '%q' "$A_SLEUTH") --pkgdir .\"; echo ''; echo 'Press Enter to close this window...'; read -r _;"; \
  else \
    SLEUTH_CMD="cd $(printf '%q' "$WORK_DIR") && $(printf '%q' "$A_SLEUTH") --pkgdir .; echo ''; echo 'Press Enter to close this window...'; read -r _;"; \
  fi; \
  TERM_FOUND=false; \
  if command -v gnome-terminal >/dev/null 2>&1; then \
    gnome-terminal -- bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v alacritty >/dev/null 2>&1; then \
    alacritty -e bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v kitty >/dev/null 2>&1; then \
    kitty bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v xterm >/dev/null 2>&1; then \
    xterm -hold -e bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v konsole >/dev/null 2>&1; then \
    konsole -e bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v tilix >/dev/null 2>&1; then \
    tilix -e bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v mate-terminal >/dev/null 2>&1; then \
    mate-terminal -- bash -lc "$SLEUTH_CMD" 2>&1 && TERM_FOUND=true; \
  elif command -v xfce4-terminal >/dev/null 2>&1; then \
    SLEUTH_CMD_QUOTED=$(printf '%q' "$SLEUTH_CMD"); \
    xfce4-terminal --command "bash -lc $SLEUTH_CMD_QUOTED" 2>&1 && TERM_FOUND=true; \
  fi; \
  if [ "$TERM_FOUND" = "true" ]; then \
    echo "aur-sleuth launched in separate terminal window."; \
    echo "The scan will continue in the background. You can close the terminal when done."; \
  else \
    echo "No suitable terminal found. Running aur-sleuth in current terminal..."; \
    ("$A_SLEUTH" --pkgdir . 2>&1 | tee ./.pacsea_sleuth.txt) || echo 'aur-sleuth failed; see output above'; \
  fi; \
else \
  echo 'aur-sleuth not found (checked PATH, ~/.local/bin, /usr/local/bin, /usr/bin)'; \
fi"#
    )
}

/// What: Launch integrated scan process for AUR package (excluding aur-sleuth).
///
/// Input:
/// - `pkg`: AUR package identifier to analyse.
/// - `_do_clamav`/`_do_trivy`/`_do_semgrep`/`_do_shellcheck`/`_do_virustotal`/`_do_custom`/`do_sleuth`: Toggles enabling optional scan stages.
///
/// Output:
/// - Uses integrated process for scans (excluding aur-sleuth).
/// - Spawns terminal for aur-sleuth if enabled (runs simultaneously).
///
/// Details:
/// - Clones `https://aur.archlinux.org/<pkg>.git` and runs `makepkg -o` (download sources only).
/// - Optionally runs `ClamAV`, `Trivy` filesystem, and `Semgrep` scans via integrated process.
/// - Performs `VirusTotal` hash lookups for `PKGBUILD`/`src` files when `VT_API_KEY` is provided.
/// - aur-sleuth runs in separate terminal simultaneously if enabled.
/// - Note: This function is kept for backward compatibility; actual execution should use `ExecutorRequest::Scan`.
/// - `_do_clamav`, `_do_trivy`, `_do_semgrep`, `_do_shellcheck`, `_do_virustotal`, and `_do_custom` parameters are kept for API consistency but unused in this function.
/// - The actual scan configuration is handled via `ExecutorRequest::Scan` which reads from the application state.
/// - The underscore prefix suppresses Rust/clippy warnings for intentionally unused parameters.
#[cfg(not(target_os = "windows"))]
#[allow(
    clippy::too_many_arguments,
    clippy::fn_params_excessive_bools,
    clippy::must_use_candidate
)]
pub fn spawn_aur_scan_for_with_config(
    pkg: &str,
    _do_clamav: bool,
    _do_trivy: bool,
    _do_semgrep: bool,
    _do_shellcheck: bool,
    _do_virustotal: bool,
    _do_custom: bool,
    do_sleuth: bool,
) {
    // Note: _do_clamav, _do_trivy, _do_semgrep, _do_shellcheck, _do_virustotal, and _do_custom
    // are unused in this function. They are kept for API consistency, but the actual scan
    // configuration is handled via ExecutorRequest::Scan which reads from application state.
    // The underscore prefix suppresses Rust/clippy warnings for intentionally unused parameters.
    // If sleuth is enabled, spawn it in a separate terminal
    if do_sleuth {
        let sleuth_cmd = build_sleuth_command_for_terminal(pkg);
        super::super::shell::spawn_shell_commands_in_terminal(&[sleuth_cmd]);
    }

    // Note: The integrated scan process is triggered via ExecutorRequest::Scan
    // This function is kept for backward compatibility but the actual execution
    // should be done through the executor pattern (see events/modals/scan.rs)
}
