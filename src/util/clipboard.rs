//! Clipboard helpers for copying plain text via `wl-copy` or `xclip`.

/// What: Copy plain text to the system clipboard without extra suffixes.
///
/// Inputs:
/// - `text`: Exact bytes to place on the clipboard.
///
/// Output:
/// - `Ok(())` when `wl-copy` or `xclip` accepted stdin.
///
/// # Errors
/// - Returns `Err` with install guidance when neither clipboard tool could be spawned.
///
/// Details:
/// - Prefers `wl-copy` when `WAYLAND_DISPLAY` is set, otherwise tries `xclip`.
/// - Does not invoke a shell; uses `Command` argument lists only.
pub fn copy_plain_text_to_clipboard(text: &str) -> Result<(), String> {
    let payload = text.as_bytes();
    if std::env::var("WAYLAND_DISPLAY").is_ok()
        && let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    {
        if let Some(mut sin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut sin, payload);
        }
        let _ = child.wait();
        return Ok(());
    }

    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut sin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut sin, payload);
        }
        let _ = child.wait();
        return Ok(());
    }

    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        Err("Clipboard tool not found. Install 'wl-clipboard' (wl-copy) or 'xclip'.".to_string())
    } else {
        Err("Clipboard tool not found. Install 'xclip' or 'wl-clipboard' (wl-copy).".to_string())
    }
}
