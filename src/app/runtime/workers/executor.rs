//! Background worker for executing commands via PTY.

use tokio::sync::mpsc;

use crate::install::{ExecutorOutput, ExecutorRequest};
#[cfg(not(target_os = "windows"))]
use crate::install::{
    build_downgrade_command_for_executor, build_install_command_for_executor,
    build_remove_command_for_executor, build_scan_command_for_executor,
    build_update_command_for_executor,
};

/// What: Handle install request by building command and executing via PTY.
///
/// Inputs:
/// - `items`: Packages to install
/// - `password`: Optional sudo password
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
///
/// Details:
/// - AUR helpers (paru/yay) need sudo for the final installation step
/// - Detects AUR commands and caches sudo credentials when password is provided
/// - Uses the same flow as custom commands for sudo passthrough
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Values are moved into spawn_blocking closure
fn handle_install_request(
    items: Vec<crate::state::PackageItem>,
    password: Option<String>,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    use crate::install::shell_single_quote;
    use crate::state::Source;

    tracing::info!(
        "[Runtime] Executor worker received install request: {} items, dry_run={}",
        items.len(),
        dry_run
    );

    // Check if there are AUR packages
    let has_aur = items.iter().any(|item| matches!(item.source, Source::Aur));

    // For official packages: password is piped to sudo (printf '%s\n' password | sudo -S command)
    // For AUR packages: cache sudo credentials first, then run paru/yay (same sudo prompt flow)
    let cmd = if has_aur {
        // Build AUR command without password embedded
        build_install_command_for_executor(&items, None, dry_run)
    } else {
        // Build official command with password piping
        build_install_command_for_executor(&items, password.as_deref(), dry_run)
    };

    // For AUR packages, cache sudo credentials first using the same password piping approach
    // This ensures paru/yay can use sudo without prompting (same flow as official packages)
    // Use `;` instead of `&&` so the AUR command runs regardless of sudo -v result
    // (paru/yay can handle their own sudo prompts if credential caching fails)
    let final_cmd = if has_aur && !dry_run && password.is_some() {
        if let Some(ref pass) = password {
            let escaped_pass = shell_single_quote(pass);
            // Cache sudo credentials first, then run the AUR command
            // sudo -v validates and caches credentials without running a command
            // Using `;` ensures AUR command runs even if sudo -v returns non-zero
            format!("printf '%s\\n' {escaped_pass} | sudo -S -v 2>/dev/null ; {cmd}")
        } else {
            cmd
        }
    } else {
        cmd
    };

    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        execute_command_pty(&final_cmd, None, res_tx_clone);
    });
}

/// What: Handle remove request by building command and executing via PTY.
///
/// Inputs:
/// - `names`: Package names to remove
/// - `password`: Optional sudo password
/// - `cascade`: Cascade removal mode
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Values are moved into spawn_blocking closure
fn handle_remove_request(
    names: Vec<String>,
    password: Option<String>,
    cascade: crate::state::modal::CascadeMode,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tracing::info!(
        "[Runtime] Executor worker received remove request: {} packages, dry_run={}",
        names.len(),
        dry_run
    );
    let cmd = build_remove_command_for_executor(&names, password.as_deref(), cascade, dry_run);
    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        execute_command_pty(&cmd, None, res_tx_clone);
    });
}

/// What: Handle downgrade request by building command and executing via PTY.
///
/// Inputs:
/// - `names`: Package names to downgrade
/// - `password`: Optional sudo password
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Values are moved into spawn_blocking closure
fn handle_downgrade_request(
    names: Vec<String>,
    password: Option<String>,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tracing::info!(
        "[Runtime] Executor worker received downgrade request: {} packages, dry_run={}",
        names.len(),
        dry_run
    );
    let cmd = build_downgrade_command_for_executor(&names, password.as_deref(), dry_run);
    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        execute_command_pty(&cmd, None, res_tx_clone);
    });
}

/// What: Handle update request by building command and executing via PTY.
///
/// Inputs:
/// - `commands`: Commands to execute
/// - `password`: Optional sudo password
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Values are moved into spawn_blocking closure
fn handle_update_request(
    commands: Vec<String>,
    password: Option<String>,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tracing::info!(
        "[Runtime] Executor worker received update request: {} commands, dry_run={}",
        commands.len(),
        dry_run
    );
    let cmd = build_update_command_for_executor(&commands, password.as_deref(), dry_run);
    tracing::debug!("[Runtime] Built update command (length={})", cmd.len());
    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        tracing::debug!("[Runtime] spawn_blocking started for update command");
        execute_command_pty(&cmd, None, res_tx_clone);
        tracing::debug!("[Runtime] spawn_blocking completed for update command");
    });
}

/// What: Handle scan request by building command and executing via PTY.
///
/// Inputs:
/// - `package`: Package name to scan
/// - `do_clamav`/`do_trivy`/`do_semgrep`/`do_shellcheck`/`do_virustotal`/`do_custom`: Scan flags
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
#[cfg(not(target_os = "windows"))]
#[allow(
    clippy::needless_pass_by_value, // Values are moved into spawn_blocking closure
    clippy::too_many_arguments, // Scan configuration requires multiple flags
    clippy::fn_params_excessive_bools // Scan configuration requires multiple bool flags
)]
fn handle_scan_request(
    package: String,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tracing::info!(
        "[Runtime] Executor worker received scan request: package={}, dry_run={}",
        package,
        dry_run
    );
    let cmd = build_scan_command_for_executor(
        &package,
        do_clamav,
        do_trivy,
        do_semgrep,
        do_shellcheck,
        do_virustotal,
        do_custom,
        dry_run,
    );
    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        execute_command_pty(&cmd, None, res_tx_clone);
    });
}

/// What: Handle custom command request by building command and executing via PTY.
///
/// Inputs:
/// - `command`: Command string to execute
/// - `password`: Optional sudo password
/// - `dry_run`: Whether to run in dry-run mode
/// - `res_tx`: Channel sender for output
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Values are moved into spawn_blocking closure
fn handle_custom_command_request(
    command: String,
    password: Option<String>,
    dry_run: bool,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tracing::info!(
        "[Runtime] Executor worker received custom command request, dry_run={}",
        dry_run
    );
    let cmd = if dry_run {
        // Properly quote the command to avoid syntax errors with complex shell constructs
        use crate::install::shell_single_quote;
        let quoted = shell_single_quote(&command);
        format!("echo DRY RUN: {quoted}")
    } else {
        // For commands that use sudo, we need to handle sudo password
        // Use SUDO_ASKPASS to provide password when sudo prompts
        if command.contains("sudo") && password.is_some() {
            if let Some(ref pass) = password {
                // Create a temporary script that outputs the password
                // Use printf instead of echo for better security
                use std::fs;
                use std::os::unix::fs::PermissionsExt;
                let temp_dir = std::env::temp_dir();
                #[allow(clippy::uninlined_format_args)]
                // process::id() needs formatting
                let askpass_script =
                    temp_dir.join(format!("pacsea_sudo_askpass_{}.sh", std::process::id()));
                // Use printf with %s to safely output password
                // Escape single quotes in password by replacing ' with '\''
                let escaped_pass = pass.replace('\'', "'\\''");
                #[allow(clippy::uninlined_format_args)] // Need to escape password
                let script_content = format!("#!/bin/sh\nprintf '%s\\n' '{}'\n", escaped_pass);
                if let Err(e) = fs::write(&askpass_script, script_content) {
                    let _ = res_tx.send(ExecutorOutput::Error(format!(
                        "Failed to create sudo askpass script: {e}"
                    )));
                    return;
                }
                // Make script executable
                if let Err(e) =
                    fs::set_permissions(&askpass_script, fs::Permissions::from_mode(0o755))
                {
                    let _ = res_tx.send(ExecutorOutput::Error(format!(
                        "Failed to make askpass script executable: {e}"
                    )));
                    return;
                }
                let askpass_path = askpass_script.to_string_lossy().to_string();
                // Escape the path for shell
                let escaped_path = askpass_path.replace('\'', "'\\''");
                // Need to escape path and use command variable, so can't use inline format
                let final_cmd = format!(
                    "export SUDO_ASKPASS='{escaped_path}'; {command}; rm -f '{escaped_path}'"
                );
                final_cmd
            } else {
                // No password provided, try without SUDO_ASKPASS
                // (might work if passwordless sudo is configured)
                command
            }
        } else {
            command
        }
    };
    let res_tx_clone = res_tx;
    tokio::task::spawn_blocking(move || {
        execute_command_pty(&cmd, password, res_tx_clone);
    });
}

/// What: Spawn background worker for command execution via PTY.
///
/// Inputs:
/// - `executor_req_rx`: Channel receiver for executor requests
/// - `executor_res_tx`: Channel sender for executor output
///
/// Details:
/// - Executes commands in a PTY to capture full terminal output
/// - Streams output line by line to the main event loop
/// - Handles install, remove, downgrade, update, scan, and custom command operations
#[cfg(not(target_os = "windows"))]
pub fn spawn_executor_worker(
    executor_req_rx: mpsc::UnboundedReceiver<ExecutorRequest>,
    executor_res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    let executor_res_tx_bg = executor_res_tx;
    tokio::spawn(async move {
        let mut executor_req_rx = executor_req_rx;
        tracing::info!("[Runtime] Executor worker started, waiting for requests...");
        while let Some(request) = executor_req_rx.recv().await {
            let res_tx = executor_res_tx_bg.clone();
            match request {
                ExecutorRequest::Install {
                    items,
                    password,
                    dry_run,
                } => handle_install_request(items, password, dry_run, res_tx),
                ExecutorRequest::Remove {
                    names,
                    password,
                    cascade,
                    dry_run,
                } => handle_remove_request(names, password, cascade, dry_run, res_tx),
                ExecutorRequest::Downgrade {
                    names,
                    password,
                    dry_run,
                } => handle_downgrade_request(names, password, dry_run, res_tx),
                ExecutorRequest::Update {
                    commands,
                    password,
                    dry_run,
                } => handle_update_request(commands, password, dry_run, res_tx),
                #[cfg(not(target_os = "windows"))]
                ExecutorRequest::Scan {
                    package,
                    do_clamav,
                    do_trivy,
                    do_semgrep,
                    do_shellcheck,
                    do_virustotal,
                    do_custom,
                    dry_run,
                } => handle_scan_request(
                    package,
                    do_clamav,
                    do_trivy,
                    do_semgrep,
                    do_shellcheck,
                    do_virustotal,
                    do_custom,
                    dry_run,
                    res_tx,
                ),
                ExecutorRequest::CustomCommand {
                    command,
                    password,
                    dry_run,
                } => handle_custom_command_request(command, password, dry_run, res_tx),
            }
        }
        tracing::debug!("[Runtime] Executor worker exiting (channel closed)");
    });
}

#[cfg(not(target_os = "windows"))]
/// What: Process text characters and send lines via channel.
///
/// Inputs:
/// - `text`: Text to process
/// - `line_buffer`: Mutable reference to current line buffer
/// - `res_tx`: Channel sender for output lines
///
/// Details:
/// - Handles newlines and carriage returns, strips ANSI codes, and sends complete lines.
fn process_text_chars(
    text: &str,
    line_buffer: &mut String,
    res_tx: &mpsc::UnboundedSender<ExecutorOutput>,
) -> usize {
    let mut lines_sent = 0;
    for ch in text.chars() {
        match ch {
            '\n' => {
                // Newline - send the current line and start a new one
                if !line_buffer.trim().is_empty() {
                    // Strip ANSI escape codes before sending
                    let cleaned = strip_ansi_escapes::strip_str(&*line_buffer);
                    tracing::trace!(
                        "[PTY] Sending line: {}...",
                        &cleaned[..cleaned.len().min(50)]
                    );
                    if res_tx.send(ExecutorOutput::Line(cleaned)).is_ok() {
                        lines_sent += 1;
                    } else {
                        tracing::warn!("[PTY] Failed to send line - channel closed");
                    }
                }
                line_buffer.clear();
            }
            '\r' => {
                // Carriage return - check if this looks like a progress bar update
                // Progress bars typically have patterns like [====>], percentages, or (X/Y) at start
                // If the buffer is empty or doesn't look like a progress bar, treat as new line
                if !line_buffer.trim().is_empty() {
                    let cleaned = strip_ansi_escapes::strip_str(&*line_buffer);
                    // Check if this looks like a progress bar (has progress indicators)
                    // Be conservative: only replace if it has both brackets AND percentage
                    // This avoids replacing regular output like "(1/1) Arming ConditionNeedsUpdate..."
                    let has_progress_brackets = cleaned.contains('[') && cleaned.contains(']');
                    let has_percentage = cleaned.contains('%');
                    let looks_like_progress = has_progress_brackets && has_percentage;

                    if looks_like_progress {
                        // This is a progress bar update - replace the last line
                        if res_tx
                            .send(ExecutorOutput::ReplaceLastLine(cleaned))
                            .is_ok()
                        {
                            lines_sent += 1;
                        }
                    } else {
                        // This is regular output with \r - send as new line
                        if res_tx.send(ExecutorOutput::Line(cleaned)).is_ok() {
                            lines_sent += 1;
                        }
                    }
                }
                line_buffer.clear();
            }
            _ => {
                line_buffer.push(ch);
            }
        }
    }
    lines_sent
}

#[cfg(not(target_os = "windows"))]
/// What: Spawns a reader thread that reads from PTY and sends data to channel.
///
/// Inputs:
/// - `reader`: PTY reader to read from
/// - `data_tx`: Channel sender to send read bytes
///
/// Details:
/// - Reads in 4KB chunks and sends to channel
/// - Sends empty vec on EOF to signal completion
#[cfg(not(target_os = "windows"))]
fn spawn_pty_reader_thread(
    reader: Box<dyn std::io::Read + Send>,
    data_tx: std::sync::mpsc::Sender<Vec<u8>>,
) {
    std::thread::spawn(move || {
        tracing::debug!("[PTY Reader] Reader thread started");
        let mut reader = reader;
        let mut total_bytes_read: usize = 0;
        loop {
            let mut buf = [0u8; 4096];
            match reader.read(&mut buf) {
                Ok(0) => {
                    tracing::debug!(
                        "[PTY Reader] EOF received, total bytes read: {}",
                        total_bytes_read
                    );
                    let _ = data_tx.send(Vec::new());
                    break;
                }
                Ok(n) => {
                    total_bytes_read += n;
                    tracing::trace!(
                        "[PTY Reader] Read {} bytes (total: {})",
                        n,
                        total_bytes_read
                    );
                    if data_tx.send(buf[..n].to_vec()).is_err() {
                        tracing::debug!("[PTY Reader] Receiver dropped, exiting");
                        break;
                    }
                }
                Err(e) => {
                    tracing::debug!(
                        "[PTY Reader] Read error: {}, total bytes: {}",
                        e,
                        total_bytes_read
                    );
                    break;
                }
            }
        }
        tracing::debug!("[PTY Reader] Reader thread exiting");
    });
}

/// What: Process byte buffer handling incomplete UTF-8 sequences.
///
/// Inputs:
/// - `byte_buffer`: Buffer containing raw bytes to process
/// - `line_buffer`: Buffer for accumulating line text
/// - `res_tx`: Channel to send processed output
///
/// Output:
/// - Number of lines sent to channel
///
/// Details:
/// - Handles split UTF-8 sequences at buffer boundaries
/// - Falls back to lossy conversion if needed
#[cfg(not(target_os = "windows"))]
fn process_byte_buffer_utf8(
    byte_buffer: &mut Vec<u8>,
    line_buffer: &mut String,
    res_tx: &mpsc::UnboundedSender<ExecutorOutput>,
) -> usize {
    let mut lines_sent = 0;

    loop {
        // Try to decode the full buffer
        if let Ok(text) = String::from_utf8(byte_buffer.clone()) {
            byte_buffer.clear();
            lines_sent += process_text_chars(&text, line_buffer, res_tx);
            break;
        }

        // Buffer is small - might be incomplete UTF-8 sequence, wait for more
        if byte_buffer.len() < 4 {
            break;
        }

        // Try to find valid UTF-8 by trimming bytes from the end
        let mut found_valid = false;
        for trim_len in 1..=4.min(byte_buffer.len()) {
            let test_len = byte_buffer.len().saturating_sub(trim_len);
            if test_len == 0 {
                break;
            }

            if let Ok(text) = String::from_utf8(byte_buffer[..test_len].to_vec()) {
                lines_sent += process_text_chars(&text, line_buffer, res_tx);
                byte_buffer.drain(..test_len);
                found_valid = true;
                break;
            }
        }

        if !found_valid {
            // Fall back to lossy conversion
            let text = String::from_utf8_lossy(byte_buffer);
            lines_sent += process_text_chars(&text, line_buffer, res_tx);
            byte_buffer.clear();
            break;
        }
    }

    lines_sent
}

/// What: Send remaining line content and finished message.
///
/// Inputs:
/// - `line_buffer`: Any remaining line content to send
/// - `lines_sent`: Current count of lines sent
/// - `exit_code_u32`: Exit code from process
/// - `res_tx`: Channel to send output
/// - `context`: String describing the context (for logging)
#[cfg(not(target_os = "windows"))]
fn send_finish_message(
    line_buffer: &str,
    lines_sent: &mut usize,
    exit_code_u32: u32,
    res_tx: &mpsc::UnboundedSender<ExecutorOutput>,
    context: &str,
) {
    // Send any remaining line
    if !line_buffer.trim().is_empty() {
        let cleaned = strip_ansi_escapes::strip_str(line_buffer);
        if res_tx.send(ExecutorOutput::Line(cleaned)).is_ok() {
            *lines_sent += 1;
        }
    }

    let success = exit_code_u32 == 0;
    let exit_code = i32::try_from(exit_code_u32).ok();
    tracing::info!(
        "[PTY] Process finished{}: success={}, exit_code={:?}, total_lines_sent={}",
        context,
        success,
        exit_code,
        lines_sent
    );
    let _ = res_tx.send(ExecutorOutput::Finished {
        success,
        exit_code,
        failed_command: None,
    });
}

/// What: Execute command in PTY and stream output.
///
/// Inputs:
/// - `cmd`: Command string to execute
/// - `_password`: Optional password (currently unused - password is handled in command builder)
/// - `res_tx`: Channel sender for output lines
///
/// Details:
/// - Creates a PTY, spawns bash to execute the command
/// - Reads output line by line and sends via channel
/// - Strips ANSI escape codes from output using `strip-ansi-escapes` crate
/// - Sends Finished message when command completes
/// - Note: Password is handled in command builder (piped for official packages, credential caching for AUR)
#[cfg(not(target_os = "windows"))]
#[allow(clippy::needless_pass_by_value)] // Pass by value needed for move into closure
fn execute_command_pty(
    cmd: &str,
    _password: Option<String>,
    res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::sync::mpsc as std_mpsc;

    tracing::debug!("[PTY] Starting execute_command_pty");
    tracing::debug!("[PTY] Command length: {} chars", cmd.len());

    // Open PTY
    let pty_system = native_pty_system();
    let pty_size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    tracing::debug!("[PTY] Opening PTY");
    let pty = match pty_system.openpty(pty_size) {
        Ok(pty) => pty,
        Err(e) => {
            tracing::error!("[PTY] Failed to open PTY: {e}");
            let _ = res_tx.send(ExecutorOutput::Error(format!("Failed to open PTY: {e}")));
            return;
        }
    };

    // Spawn command
    let mut cmd_builder = CommandBuilder::new("bash");
    cmd_builder.arg("-c");
    cmd_builder.arg(cmd);

    tracing::debug!("[PTY] Spawning bash command");
    let mut child = match pty.slave.spawn_command(cmd_builder) {
        Ok(child) => child,
        Err(e) => {
            tracing::error!("[PTY] Failed to spawn command: {e}");
            let _ = res_tx.send(ExecutorOutput::Error(format!("Failed to spawn: {e}")));
            return;
        }
    };

    // Setup reader thread
    let reader = pty
        .master
        .try_clone_reader()
        .expect("Failed to clone reader");
    let _master = pty.master; // Keep master alive
    let (data_tx, data_rx) = std_mpsc::channel::<Vec<u8>>();
    spawn_pty_reader_thread(reader, data_tx);

    // Process output
    let mut byte_buffer = Vec::new();
    let mut line_buffer = String::new();
    let mut lines_sent: usize = 0;

    tracing::debug!("[PTY] Entering main processing loop");
    loop {
        // Check if child has exited
        match child.try_wait() {
            Ok(Some(status)) => {
                tracing::debug!("[PTY] Child exited with code: {:?}", status.exit_code());
                drain_remaining_data(
                    &data_rx,
                    &mut byte_buffer,
                    &mut line_buffer,
                    &mut lines_sent,
                    &res_tx,
                );
                send_finish_message(
                    &line_buffer,
                    &mut lines_sent,
                    status.exit_code(),
                    &res_tx,
                    "",
                );
                return;
            }
            Ok(None) => {} // Still running
            Err(e) => {
                tracing::error!("[PTY] Error checking child status: {e}");
                let _ = res_tx.send(ExecutorOutput::Error(format!("Process error: {e}")));
                return;
            }
        }

        // Receive data with timeout
        match data_rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(data) if data.is_empty() => {
                tracing::debug!("[PTY] Got EOF signal");
                break;
            }
            Ok(data) => {
                byte_buffer.extend_from_slice(&data);
                lines_sent += process_byte_buffer_utf8(&mut byte_buffer, &mut line_buffer, &res_tx);
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {}
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                tracing::debug!("[PTY] Read thread disconnected");
                break;
            }
        }
    }

    // Wait for process and send finish (EOF path)
    tracing::debug!("[PTY] Waiting for child process after EOF");
    match child.wait() {
        Ok(status) => {
            send_finish_message(
                &line_buffer,
                &mut lines_sent,
                status.exit_code(),
                &res_tx,
                " (post-loop)",
            );
        }
        Err(e) => {
            tracing::error!("[PTY] Child wait error: {e}");
            let _ = res_tx.send(ExecutorOutput::Error(format!("Wait error: {e}")));
        }
    }
}

/// What: Drain remaining data from channel after child exits.
///
/// Inputs:
/// - `data_rx`: Channel receiver
/// - `byte_buffer`: Buffer to accumulate bytes
/// - `line_buffer`: Buffer for line text
/// - `lines_sent`: Counter for lines sent
/// - `res_tx`: Channel to send output
#[cfg(not(target_os = "windows"))]
fn drain_remaining_data(
    data_rx: &std::sync::mpsc::Receiver<Vec<u8>>,
    byte_buffer: &mut Vec<u8>,
    line_buffer: &mut String,
    lines_sent: &mut usize,
    res_tx: &mpsc::UnboundedSender<ExecutorOutput>,
) {
    while let Ok(data) = data_rx.recv_timeout(std::time::Duration::from_millis(100)) {
        if data.is_empty() {
            break;
        }
        byte_buffer.extend_from_slice(&data);
        if let Ok(text) = String::from_utf8(byte_buffer.clone()) {
            byte_buffer.clear();
            *lines_sent += process_text_chars(&text, line_buffer, res_tx);
        }
    }
}

#[cfg(target_os = "windows")]
/// What: Placeholder executor worker for Windows (unsupported).
///
/// Inputs:
/// - `executor_req_rx`: Channel receiver for executor requests
/// - `executor_res_tx`: Channel sender for executor output
///
/// Details:
/// - Windows is not supported for PTY-based execution
pub fn spawn_executor_worker(
    mut executor_req_rx: mpsc::UnboundedReceiver<ExecutorRequest>,
    executor_res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    tokio::spawn(async move {
        while let Some(_request) = executor_req_rx.recv().await {
            let _ = executor_res_tx.send(ExecutorOutput::Error(
                "PTY execution is not supported on Windows".to_string(),
            ));
        }
    });
}
