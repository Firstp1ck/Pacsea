//! Background worker for executing commands via PTY.

use tokio::sync::mpsc;

use crate::install::{
    ExecutorOutput, ExecutorRequest, build_install_command_for_executor,
    build_remove_command_for_executor,
};

/// What: Spawn background worker for command execution via PTY.
///
/// Inputs:
/// - `executor_req_rx`: Channel receiver for executor requests
/// - `executor_res_tx`: Channel sender for executor output
///
/// Details:
/// - Executes commands in a PTY to capture full terminal output
/// - Streams output line by line to the main event loop
/// - Handles both install and remove operations
#[cfg(not(target_os = "windows"))]
pub fn spawn_executor_worker(
    mut executor_req_rx: mpsc::UnboundedReceiver<ExecutorRequest>,
    executor_res_tx: mpsc::UnboundedSender<ExecutorOutput>,
) {
    let executor_res_tx_bg = executor_res_tx;
    tokio::spawn(async move {
        tracing::info!("[Runtime] Executor worker started, waiting for requests...");
        while let Some(request) = executor_req_rx.recv().await {
            let res_tx = executor_res_tx_bg.clone();
            match request {
                ExecutorRequest::Install {
                    items,
                    password,
                    dry_run,
                } => {
                    tracing::info!(
                        "[Runtime] Executor worker received install request: {} items, dry_run={}",
                        items.len(),
                        dry_run
                    );
                    let cmd =
                        build_install_command_for_executor(&items, password.as_deref(), dry_run);
                    let res_tx_clone = res_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        execute_command_pty(&cmd, res_tx_clone);
                    });
                }
                ExecutorRequest::Remove {
                    names,
                    password,
                    cascade,
                    dry_run,
                } => {
                    tracing::info!(
                        "[Runtime] Executor worker received remove request: {} packages, dry_run={}",
                        names.len(),
                        dry_run
                    );
                    let cmd = build_remove_command_for_executor(
                        &names,
                        password.as_deref(),
                        cascade,
                        dry_run,
                    );
                    let res_tx_clone = res_tx.clone();
                    tokio::task::spawn_blocking(move || {
                        execute_command_pty(&cmd, res_tx_clone);
                    });
                }
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
) {
    for ch in text.chars() {
        match ch {
            '\n' => {
                // Newline - send the current line and start a new one
                if !line_buffer.trim().is_empty() {
                    // Strip ANSI escape codes before sending
                    let cleaned = strip_ansi_escapes::strip_str(&line_buffer);
                    let _ = res_tx.send(ExecutorOutput::Line(cleaned));
                }
                line_buffer.clear();
            }
            '\r' => {
                // Carriage return - check if this looks like a progress bar update
                // Progress bars typically have patterns like [====>], percentages, or (X/Y) at start
                // If the buffer is empty or doesn't look like a progress bar, treat as new line
                if !line_buffer.trim().is_empty() {
                    let cleaned = strip_ansi_escapes::strip_str(&line_buffer);
                    // Check if this looks like a progress bar (has progress indicators)
                    // Be conservative: only replace if it has both brackets AND percentage
                    // This avoids replacing regular output like "(1/1) Arming ConditionNeedsUpdate..."
                    let has_progress_brackets = cleaned.contains('[') && cleaned.contains(']');
                    let has_percentage = cleaned.contains('%');
                    let looks_like_progress = has_progress_brackets && has_percentage;

                    if looks_like_progress {
                        // This is a progress bar update - replace the last line
                        let _ = res_tx.send(ExecutorOutput::ReplaceLastLine(cleaned));
                    } else {
                        // This is regular output with \r - send as new line
                        let _ = res_tx.send(ExecutorOutput::Line(cleaned));
                    }
                }
                line_buffer.clear();
            }
            _ => {
                line_buffer.push(ch);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
/// What: Execute command in PTY and stream output.
///
/// Inputs:
/// - `cmd`: Command string to execute
/// - `password`: Optional password to provide to sudo
/// - `res_tx`: Channel sender for output lines
///
/// Details:
/// - Creates a PTY, spawns bash to execute the command
/// - Reads output line by line and sends via channel
/// - Strips ANSI escape codes from output using `strip-ansi-escapes` crate
/// - Sends Finished message when command completes
#[allow(clippy::needless_pass_by_value)] // Pass by value needed for move into closure
#[allow(clippy::too_many_lines)] // Complex PTY handling requires many steps
fn execute_command_pty(cmd: &str, res_tx: mpsc::UnboundedSender<ExecutorOutput>) {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::Read;
    use std::sync::mpsc as std_mpsc;

    let pty_system = native_pty_system();

    let pty_size = PtySize {
        rows: 24,
        cols: 80,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty = match pty_system.openpty(pty_size) {
        Ok(pty) => pty,
        Err(e) => {
            let _ = res_tx.send(ExecutorOutput::Error(format!("Failed to open PTY: {e}")));
            return;
        }
    };

    // Command already uses sudo -S (password via stdin) - we'll write password to PTY when sudo prompts
    let mut cmd_builder = CommandBuilder::new("bash");
    cmd_builder.arg("-c");
    cmd_builder.arg(cmd);

    let mut child = match pty.slave.spawn_command(cmd_builder) {
        Ok(child) => child,
        Err(e) => {
            let _ = res_tx.send(ExecutorOutput::Error(format!(
                "Failed to spawn command: {e}"
            )));
            return;
        }
    };

    // Read output from master using a separate thread to avoid blocking
    let reader = pty
        .master
        .try_clone_reader()
        .expect("Failed to clone reader");

    // Keep master alive - dropping it can cause issues with some PTY implementations
    let _master = pty.master;

    // Spawn a thread to read from PTY - this avoids blocking the main thread
    let (data_tx, data_rx) = std_mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut reader = reader;
        loop {
            let mut buf = [0u8; 4096];
            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - send empty to signal completion
                    let _ = data_tx.send(Vec::new());
                    break;
                }
                Ok(n) => {
                    if data_tx.send(buf[..n].to_vec()).is_err() {
                        break; // Receiver dropped
                    }
                }
                Err(_) => {
                    break;
                }
            }
        }
    });

    // Buffer for accumulating bytes that might contain incomplete UTF-8 sequences
    let mut byte_buffer = Vec::new();
    let mut line_buffer = String::new();

    // Main loop: check child status and process data with timeout
    loop {
        // First check if child has exited
        match child.try_wait() {
            Ok(Some(status)) => {
                tracing::debug!("Child process exited with status: {:?}", status.exit_code());
                // Drain any remaining data from the channel
                while let Ok(data) = data_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    if data.is_empty() {
                        break; // EOF marker
                    }
                    byte_buffer.extend_from_slice(&data);
                    if let Ok(text) = String::from_utf8(byte_buffer.clone()) {
                        byte_buffer.clear();
                        process_text_chars(&text, &mut line_buffer, &res_tx);
                    }
                }
                // Send any remaining line
                if !line_buffer.trim().is_empty() {
                    let cleaned = strip_ansi_escapes::strip_str(&line_buffer);
                    let _ = res_tx.send(ExecutorOutput::Line(cleaned));
                }
                // Send finished
                let exit_code_u32 = status.exit_code();
                let success = exit_code_u32 == 0;
                let exit_code = i32::try_from(exit_code_u32).ok();
                tracing::info!("Process finished: success={success}, exit_code={exit_code:?}");
                let _ = res_tx.send(ExecutorOutput::Finished { success, exit_code });
                return;
            }
            Ok(None) => {
                // Child still running, continue
            }
            Err(e) => {
                tracing::error!("Error checking child status: {e}");
                let _ = res_tx.send(ExecutorOutput::Error(format!("Process error: {e}")));
                return;
            }
        }

        // Try to receive data with short timeout so we can check child status frequently
        match data_rx.recv_timeout(std::time::Duration::from_millis(50)) {
            Ok(data) => {
                if data.is_empty() {
                    tracing::debug!("Got EOF signal from read thread");
                    break; // EOF
                }
                byte_buffer.extend_from_slice(&data);

                // Process bytes, handling incomplete UTF-8 sequences at boundaries
                // UTF-8 sequences are 1-4 bytes, so we check the last few bytes
                loop {
                    // Try to decode the buffer
                    // Handle both valid and invalid UTF-8 cases
                    if let Ok(text) = String::from_utf8(byte_buffer.clone()) {
                        // Successfully decoded all bytes - process the text
                        byte_buffer.clear();
                        process_text_chars(&text, &mut line_buffer, &res_tx);
                        break; // Processed all text, continue reading
                    }
                    // Invalid UTF-8 - might be incomplete sequence at the end
                    // Check if we have enough bytes to determine if it's incomplete
                    // UTF-8 sequences are max 4 bytes, so if buffer is small, wait for more
                    if byte_buffer.len() < 4 {
                        // Likely incomplete sequence, wait for more bytes
                        break;
                    }

                    // Try to find valid UTF-8 by removing bytes from the end
                    // This handles the case where a multi-byte sequence is split
                    let mut found_valid = false;
                    for trim_len in 1..=4.min(byte_buffer.len()) {
                        let test_len = byte_buffer.len().saturating_sub(trim_len);
                        if test_len == 0 {
                            break;
                        }

                        if let Ok(text) = String::from_utf8(byte_buffer[..test_len].to_vec()) {
                            // Found valid UTF-8 - process it
                            process_text_chars(&text, &mut line_buffer, &res_tx);

                            // Keep remaining bytes for next iteration
                            byte_buffer.drain(..test_len);
                            found_valid = true;
                            break;
                        }
                    }

                    if !found_valid {
                        // Can't decode, use lossy conversion as fallback
                        let text = String::from_utf8_lossy(&byte_buffer);
                        process_text_chars(&text, &mut line_buffer, &res_tx);
                        byte_buffer.clear();
                        break;
                    }
                }
            }
            Err(std_mpsc::RecvTimeoutError::Timeout) => {
                // No data available yet, loop continues to check child status
            }
            Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                // Read thread ended unexpectedly
                tracing::debug!("Read thread disconnected");
                break;
            }
        }
    }

    // Send any remaining line (only reached if we got EOF without try_wait detecting exit)
    if !line_buffer.trim().is_empty() {
        let cleaned = strip_ansi_escapes::strip_str(&line_buffer);
        let _ = res_tx.send(ExecutorOutput::Line(cleaned));
    }

    // If we got here via EOF, wait for process (should return immediately)
    tracing::debug!("Reached post-loop, waiting for child process...");
    let exit_status = match child.wait() {
        Ok(status) => status,
        Err(e) => {
            tracing::error!("Child wait error: {e}");
            let _ = res_tx.send(ExecutorOutput::Error(format!("Wait error: {e}")));
            return;
        }
    };

    let exit_code_u32 = exit_status.exit_code();
    let success = exit_code_u32 == 0;
    let exit_code = i32::try_from(exit_code_u32).ok();
    tracing::info!("Process finished (post-loop): success={success}, exit_code={exit_code:?}");
    let _ = res_tx.send(ExecutorOutput::Finished { success, exit_code });
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
