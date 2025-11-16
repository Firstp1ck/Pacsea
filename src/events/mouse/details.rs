//! Details pane mouse event handling (URL, PKGBUILD buttons, scroll).

use crossterm::event::{MouseEvent, MouseEventKind};
use crossterm::execute;
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// Handle mouse events for the details pane.
///
/// What: Process mouse interactions within the package details pane, including URL clicks,
/// PKGBUILD viewer controls, and scroll handling.
///
/// Inputs:
/// - `m`: Mouse event including position, button, and modifiers
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `is_left_down`: Whether the left mouse button is pressed
/// - `ctrl`: Whether the Control modifier is active
/// - `shift`: Whether the Shift modifier is active
/// - `app`: Mutable application state containing details pane state and UI rectangles
/// - `pkgb_tx`: Channel to request PKGBUILD content when opening the viewer
///
/// Output:
/// - `Some(bool)` if the event was handled (consumed by details pane), `None` if not handled.
///   The boolean value indicates whether the application should exit (always `false` here).
///
/// Details:
/// - URL clicks: Ctrl+Shift+LeftClick on URL button opens the URL via `xdg-open`.
/// - PKGBUILD toggle: Left click on toggle button opens/closes the PKGBUILD viewer and requests content.
/// - Copy PKGBUILD: Left click on copy button copies PKGBUILD to clipboard (wl-copy/xclip).
/// - Reload PKGBUILD: Left click on reload button schedules a debounced reload.
/// - Scroll: Mouse wheel scrolls the details content when within the details rectangle.
/// - Text selection: When `mouse_disabled_in_details` is true, clicks are ignored to allow text selection.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_details_mouse(
    m: MouseEvent,
    mx: u16,
    my: u16,
    is_left_down: bool,
    ctrl: bool,
    shift: bool,
    app: &mut AppState,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> Option<bool> {
    // Handle modifier-clicks in details first, even when selection is enabled
    if is_left_down && ctrl && shift {
        // URL click
        if let Some((x, y, w, h)) = app.url_button_rect
            && mx >= x
            && mx < x + w
            && my >= y
            && my < y + h
            && !app.details.url.is_empty()
        {
            app.mouse_disabled_in_details = false; // temporarily allow action
            crate::util::open_url(&app.details.url);
            return Some(false);
        }
        // Show PKGBUILD click (legacy Ctrl+Shift) — no longer active
    }

    // New behavior: plain left click on Show/Hide PKGBUILD
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false; // allow this action
        if app.pkgb_visible {
            // Close if already open
            app.pkgb_visible = false;
            app.pkgb_text = None;
            app.pkgb_package_name = None;
            app.pkgb_scroll = 0;
            app.pkgb_rect = None;
        } else {
            // Open and (re)load
            app.pkgb_visible = true;
            app.pkgb_text = None;
            app.pkgb_package_name = None;
            if let Some(item) = app.results.get(app.selected).cloned() {
                let _ = pkgb_tx.send(item);
            }
        }
        return Some(false);
    }

    // Click on "Copy PKGBUILD" title button
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_check_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false;
        if let Some(text) = app.pkgb_text.clone() {
            // Best-effort: Wayland -> wl-copy; X11 -> xclip; otherwise show guidance modal
            let (tx_msg, rx_msg) = std::sync::mpsc::channel::<Option<String>>();
            std::thread::spawn(move || {
                let suffix = {
                    let s = crate::theme::settings().clipboard_suffix;
                    if s.trim().is_empty() {
                        String::new()
                    } else {
                        format!("\n\n{s}\n")
                    }
                };
                let payload = if suffix.is_empty() {
                    text.clone()
                } else {
                    format!("{text}{suffix}")
                };
                // Try wl-copy on Wayland
                let tried_wl = if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    if let Ok(mut child) = std::process::Command::new("wl-copy")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        if let Some(mut sin) = child.stdin.take() {
                            let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
                        }
                        let _ = child.wait();
                        let _ = tx_msg.send(Some("PKGBUILD is added to the Clipboard".to_string()));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                if tried_wl {
                    return;
                }

                // Try xclip as a generic fallback on X11
                if let Ok(mut child) = std::process::Command::new("xclip")
                    .args(["-selection", "clipboard"]) // send to clipboard selection
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    if let Some(mut sin) = child.stdin.take() {
                        let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
                    }
                    let _ = child.wait();
                    let _ = tx_msg.send(Some("PKGBUILD is added to the Clipboard".to_string()));
                    return;
                }

                // Neither wl-copy nor xclip worked — report guidance to UI thread
                let hint = if std::env::var("WAYLAND_DISPLAY").is_ok() {
                    "Clipboard tool not found. Please install 'wl-clipboard' (provides wl-copy) or 'xclip'.".to_string()
                } else {
                    "Clipboard tool not found. Please install 'xclip' or 'wl-clipboard' (wl-copy)."
                        .to_string()
                };
                let _ = tx_msg.send(Some(hint));
            });
            // Default optimistic toast; overwritten by worker if needed
            app.toast_message = Some(crate::i18n::t(app, "app.toasts.copying_pkgbuild"));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
            // Try to receive the result quickly without blocking UI long
            if let Ok(Some(msg)) = rx_msg.recv_timeout(std::time::Duration::from_millis(50)) {
                app.toast_message = Some(msg);
                app.toast_expires_at =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
            }
        } else {
            app.toast_message = Some(crate::i18n::t(app, "app.toasts.pkgbuild_not_loaded"));
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        }
        return Some(false);
    }

    // Click on "Reload PKGBUILD" title button
    if is_left_down
        && let Some((x, y, w, h)) = app.pkgb_reload_button_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        app.mouse_disabled_in_details = false;
        if let Some(item) = app.results.get(app.selected).cloned() {
            // Schedule debounced reload (same as auto-reload)
            app.pkgb_reload_requested_at = Some(std::time::Instant::now());
            app.pkgb_reload_requested_for = Some(item.name.clone());
            app.pkgb_text = None; // Clear old PKGBUILD while loading
        }
        return Some(false);
    }

    // Scroll support inside Package Info details pane using mouse wheel (before click blocking)
    // Allow scrolling even when mouse clicks are disabled for text selection
    if let Some((x, y, w, h)) = app.details_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        match m.kind {
            MouseEventKind::ScrollUp => {
                app.details_scroll = app.details_scroll.saturating_sub(1);
                return Some(false);
            }
            MouseEventKind::ScrollDown => {
                app.details_scroll = app.details_scroll.saturating_add(1);
                return Some(false);
            }
            _ => {}
        }
    }

    // If details should be markable, ignore other clicks within it
    if app.mouse_disabled_in_details
        && let Some((x, y, w, h)) = app.details_rect
        && mx >= x
        && mx < x + w
        && my >= y
        && my < y + h
    {
        // Ensure terminal mouse capture stays enabled globally, while app ignores clicks here
        if !app.mouse_capture_enabled {
            let _ = execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
            app.mouse_capture_enabled = true;
        }
        return Some(false);
    }

    None
}
