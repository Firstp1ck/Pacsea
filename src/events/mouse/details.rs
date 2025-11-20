//! Details pane mouse event handling (URL, PKGBUILD buttons, scroll).

use crossterm::event::{MouseEvent, MouseEventKind};
use crossterm::execute;
use tokio::sync::mpsc;

use crate::state::{AppState, PackageItem};

/// Check if a point is within a rectangle.
///
/// What: Determines if coordinates (mx, my) fall within the bounds of a rectangle.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `rect`: Optional rectangle as (x, y, width, height)
///
/// Output:
/// - `true` if the point is within the rectangle, `false` otherwise.
fn is_point_in_rect(mx: u16, my: u16, rect: Option<(u16, u16, u16, u16)>) -> bool {
    if let Some((x, y, w, h)) = rect {
        mx >= x && mx < x + w && my >= y && my < y + h
    } else {
        false
    }
}

/// Handle URL button click with Ctrl+Shift modifier.
///
/// What: Opens the package URL when Ctrl+Shift+LeftClick is performed on the URL button.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
fn handle_url_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if is_point_in_rect(mx, my, app.url_button_rect) && !app.details.url.is_empty() {
        app.mouse_disabled_in_details = false;
        crate::util::open_url(&app.details.url);
        true
    } else {
        false
    }
}

/// Handle PKGBUILD toggle button click.
///
/// What: Opens or closes the PKGBUILD viewer and requests content when opening.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
/// - `pkgb_tx`: Channel to request PKGBUILD content
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
fn handle_pkgb_toggle_click(
    mx: u16,
    my: u16,
    app: &mut AppState,
    pkgb_tx: &mpsc::UnboundedSender<PackageItem>,
) -> bool {
    if !is_point_in_rect(mx, my, app.pkgb_button_rect) {
        return false;
    }

    app.mouse_disabled_in_details = false;
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
    true
}

/// Copy PKGBUILD text to clipboard using wl-copy or xclip.
///
/// What: Attempts to copy text to clipboard using Wayland (wl-copy) or X11 (xclip) tools.
///
/// Inputs:
/// - `text`: The text to copy
///
/// Output:
/// - `Some(String)` with success/error message, or `None` if clipboard tool is not available.
fn copy_to_clipboard(text: String) -> Option<String> {
    let suffix = {
        let s = crate::theme::settings().clipboard_suffix;
        if s.trim().is_empty() {
            String::new()
        } else {
            format!("\n\n{s}\n")
        }
    };
    let payload = if suffix.is_empty() {
        text
    } else {
        format!("{text}{suffix}")
    };

    // Try wl-copy on Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok()
        && let Ok(mut child) = std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
    {
        if let Some(mut sin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
        }
        let _ = child.wait();
        return Some("PKGBUILD is added to the Clipboard".to_string());
    }

    // Try xclip as a generic fallback on X11
    if let Ok(mut child) = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        if let Some(mut sin) = child.stdin.take() {
            let _ = std::io::Write::write_all(&mut sin, payload.as_bytes());
        }
        let _ = child.wait();
        return Some("PKGBUILD is added to the Clipboard".to_string());
    }

    // Neither wl-copy nor xclip worked — report guidance
    let hint = if std::env::var("WAYLAND_DISPLAY").is_ok() {
        "Clipboard tool not found. Please install 'wl-clipboard' (provides wl-copy) or 'xclip'."
            .to_string()
    } else {
        "Clipboard tool not found. Please install 'xclip' or 'wl-clipboard' (wl-copy).".to_string()
    };
    Some(hint)
}

/// Handle copy PKGBUILD button click.
///
/// What: Copies PKGBUILD text to clipboard in a background thread and shows toast notification.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
fn handle_copy_pkgb_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_point_in_rect(mx, my, app.pkgb_check_button_rect) {
        return false;
    }

    app.mouse_disabled_in_details = false;
    if let Some(text) = app.pkgb_text.clone() {
        let (tx_msg, rx_msg) = std::sync::mpsc::channel::<Option<String>>();
        std::thread::spawn(move || {
            let result = copy_to_clipboard(text);
            let _ = tx_msg.send(result);
        });
        // Default optimistic toast; overwritten by worker if needed
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.copying_pkgbuild"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
        // Try to receive the result quickly without blocking UI long
        if let Ok(Some(msg)) = rx_msg.recv_timeout(std::time::Duration::from_millis(50)) {
            app.toast_message = Some(msg);
            app.toast_expires_at =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(4));
        }
    } else {
        app.toast_message = Some(crate::i18n::t(app, "app.toasts.pkgbuild_not_loaded"));
        app.toast_expires_at = Some(std::time::Instant::now() + std::time::Duration::from_secs(3));
    }
    true
}

/// Handle reload PKGBUILD button click.
///
/// What: Schedules a debounced reload of the PKGBUILD content.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click was handled, `false` otherwise.
fn handle_reload_pkgb_click(mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_point_in_rect(mx, my, app.pkgb_reload_button_rect) {
        return false;
    }

    app.mouse_disabled_in_details = false;
    if let Some(item) = app.results.get(app.selected).cloned() {
        app.pkgb_reload_requested_at = Some(std::time::Instant::now());
        app.pkgb_reload_requested_for = Some(item.name.clone());
        app.pkgb_text = None; // Clear old PKGBUILD while loading
    }
    true
}

/// Handle mouse scroll events in the details pane.
///
/// What: Updates details scroll position when mouse wheel is used within the details rectangle.
///
/// Inputs:
/// - `m`: Mouse event including scroll kind
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the scroll was handled, `false` otherwise.
fn handle_details_scroll(m: &MouseEvent, mx: u16, my: u16, app: &mut AppState) -> bool {
    if !is_point_in_rect(mx, my, app.details_rect) {
        return false;
    }

    match m.kind {
        MouseEventKind::ScrollUp => {
            app.details_scroll = app.details_scroll.saturating_sub(1);
            true
        }
        MouseEventKind::ScrollDown => {
            app.details_scroll = app.details_scroll.saturating_add(1);
            true
        }
        _ => false,
    }
}

/// Handle text selection blocking in details pane.
///
/// What: Ignores clicks within details pane when text selection is enabled, ensuring mouse capture stays enabled.
///
/// Inputs:
/// - `mx`: Mouse X coordinate (column)
/// - `my`: Mouse Y coordinate (row)
/// - `app`: Mutable application state
///
/// Output:
/// - `true` if the click should be blocked, `false` otherwise.
fn handle_text_selection_block(mx: u16, my: u16, app: &mut AppState) -> bool {
    if !app.mouse_disabled_in_details {
        return false;
    }

    if !is_point_in_rect(mx, my, app.details_rect) {
        return false;
    }

    // Ensure terminal mouse capture stays enabled globally, while app ignores clicks here
    if !app.mouse_capture_enabled {
        // Skip mouse capture in headless/test mode to prevent escape sequences in test output
        if std::env::var("PACSEA_TEST_HEADLESS").ok().as_deref() != Some("1") {
            let _ = execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
        }
        app.mouse_capture_enabled = true;
    }
    true
}

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
    if is_left_down && ctrl && shift && handle_url_click(mx, my, app) {
        return Some(false);
    }

    // Handle button clicks
    if is_left_down {
        if handle_pkgb_toggle_click(mx, my, app, pkgb_tx) {
            return Some(false);
        }
        if handle_copy_pkgb_click(mx, my, app) {
            return Some(false);
        }
        if handle_reload_pkgb_click(mx, my, app) {
            return Some(false);
        }
    }

    // Handle scroll events (before click blocking)
    if handle_details_scroll(&m, mx, my, app) {
        return Some(false);
    }

    // Handle text selection blocking
    if handle_text_selection_block(mx, my, app) {
        return Some(false);
    }

    None
}
