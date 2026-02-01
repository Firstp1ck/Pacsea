//! Terminal color query and theme derivation.
//!
//! This module provides functionality to query terminal foreground/background
//! colors via OSC 10/11 escape sequences, and derive a full theme palette from
//! those two colors.

use ratatui::style::Color;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use super::types::Theme;

/// Default timeout for OSC query response (milliseconds).
/// Increased to 250ms to allow slower terminals to respond.
const OSC_QUERY_TIMEOUT_MS: u64 = 250;

/// Number of drain iterations to perform after OSC query.
/// Multiple passes with small delays help catch delayed response bytes.
const DRAIN_ITERATIONS: usize = 3;

/// Delay between drain iterations (milliseconds).
/// Short enough to not noticeably slow down reload, but enough to catch stragglers.
const DRAIN_DELAY_MS: u64 = 10;

/// Drain any stray events from crossterm's event queue.
///
/// OSC responses may arrive in chunks or with slight delays. Some terminals
/// also echo the response to stdin in addition to /dev/tty. This function
/// drains events multiple times with small delays to ensure all stray bytes
/// are consumed before returning control to the main event loop.
///
/// Without this, OSC response bytes like `10;rgb:e0e0/dede/f4f4` can leak
/// into the search input field when reloading theme with Ctrl+R.
fn drain_stray_events() {
    use crossterm::event::{poll, read as crossterm_read};

    for _ in 0..DRAIN_ITERATIONS {
        // Drain all immediately available events
        while poll(Duration::from_millis(0)).unwrap_or(false) {
            let _ = crossterm_read();
        }
        // Small delay to allow any in-flight bytes to arrive
        std::thread::sleep(Duration::from_millis(DRAIN_DELAY_MS));
    }

    // Final drain after the last delay
    while poll(Duration::from_millis(0)).unwrap_or(false) {
        let _ = crossterm_read();
    }
}

/// What: Query the terminal for foreground and background colors.
///
/// Inputs:
/// - None. On Unix uses stdout and `/dev/tty`; on Windows/non-Unix uses stdout and stdin.
///
/// Output:
/// - `Some((foreground, background))` if query succeeds.
/// - `None` if query times out or parsing fails.
///
/// Details:
/// - Sends OSC 10 and OSC 11 queries to stdout.
/// - On Unix, reads responses from `/dev/tty` with a short timeout (avoids touching stdin).
/// - On Windows/non-Unix, reads responses from stdin with a short timeout.
/// - Parses `rgb:rrrr/gggg/bbbb` format responses.
/// - Requires terminal to be in raw mode.
/// - Retries once on failure to handle slow terminal initialization.
#[must_use]
pub fn query_terminal_colors() -> Option<(Color, Color)> {
    // Check if we're in a terminal
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() || !std::io::stdin().is_terminal() {
        tracing::debug!("Not a terminal, skipping OSC query");
        return None;
    }

    // Try query, with one retry on failure
    // The retry helps with slow terminal initialization at startup
    if let Some(colors) = query_with_raw_mode() {
        return Some(colors);
    }

    // Small delay before retry
    std::thread::sleep(Duration::from_millis(50));
    tracing::debug!("Retrying OSC query after initial failure");
    query_with_raw_mode()
}

/// Perform the actual query with terminal in raw mode.
fn query_with_raw_mode() -> Option<(Color, Color)> {
    use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
    use crossterm::execute;
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};

    // Check if already in raw mode (app is running)
    let was_raw_mode = is_raw_mode_enabled().unwrap_or(false);
    let is_headless = std::env::var("PACSEA_TEST_HEADLESS").ok().as_deref() == Some("1");
    tracing::debug!(was_raw_mode, is_headless, "Starting terminal color query");

    // Disable mouse capture only when not headless (prevents events mixing with OSC response).
    // In headless/test mode we skip all mouse-capture changes to keep output clean.
    if !is_headless {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }

    // Enable raw mode if not already enabled
    if !was_raw_mode && enable_raw_mode().is_err() {
        tracing::debug!("Failed to enable raw mode for OSC query");
        return None;
    }

    // Drain any pending events before sending query.
    // This clears any events that accumulated before we started.
    drain_stray_events();

    let result = (|| {
        let mut stdout = std::io::stdout();

        // Query foreground (OSC 10) and background (OSC 11)
        // OSC 10 ; ? ST - query foreground color
        // OSC 11 ; ? ST - query background color
        // We'll send both queries and parse both responses

        // Send queries
        write!(stdout, "\x1b]10;?\x07\x1b]11;?\x07").ok()?;
        stdout.flush().ok()?;

        // Read response with timeout using our thread-based approach
        // (crossterm's event reader doesn't handle OSC responses)
        let response = read_with_timeout(Duration::from_millis(OSC_QUERY_TIMEOUT_MS))?;
        tracing::debug!(response_len = response.len(), "Received OSC response");

        // Parse both colors from response
        // Response format: ESC ] 10 ; rgb:rrrr/gggg/bbbb ST ESC ] 11 ; rgb:rrrr/gggg/bbbb ST
        // ST can be BEL (\x07) or ESC \ (\x1b\x5c)
        let fg = parse_osc_color_response(&response, 10);
        let bg = parse_osc_color_response(&response, 11);

        if fg.is_none() || bg.is_none() {
            tracing::debug!(
                fg_parsed = fg.is_some(),
                bg_parsed = bg.is_some(),
                "Failed to parse OSC color response"
            );
            return None;
        }

        Some((fg?, bg?))
    })();

    // Drain any events that arrived during the query.
    // The OSC response may arrive in chunks or be delayed, so we drain multiple times
    // with small delays to ensure all stray bytes are consumed before returning control
    // to the main event loop. This prevents OSC responses from leaking into input fields.
    drain_stray_events();

    // Restore terminal mode if we enabled it
    if !was_raw_mode {
        let _ = disable_raw_mode();
    }

    // Re-enable mouse capture only if it was enabled before (was_raw_mode) and not headless.
    // Avoids enabling mouse reporting before setup_terminal() or in test mode.
    if !is_headless && was_raw_mode {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
    }

    result
}

/// Maximum buffer size when reading OSC responses (avoids runaway growth).
const OSC_READ_MAX_BYTES: usize = 4096;

/// Read from the terminal with a timeout.
///
/// On Unix: reads from `/dev/tty` using poll + non-blocking read so we never
/// touch stdin and cannot race with crossterm's event thread. No background
/// thread is used.
///
/// On Windows: uses a cancellable reader thread (`WaitForSingleObject` with short
/// timeout + cancel flag) so `join()` returns quickly and does not hang if the
/// OSC query never yields bytes.
/// On other non-Unix: uses a reader thread with a bounded join so the caller
/// does not block indefinitely.
fn read_with_timeout(timeout: Duration) -> Option<String> {
    #[cfg(unix)]
    return read_with_timeout_unix(timeout);

    #[cfg(not(unix))]
    return read_with_timeout_thread_joined(timeout);
}

#[cfg(unix)]
/// What: Read OSC response from /dev/tty with a deadline using poll + non-blocking read.
///
/// Inputs:
/// - `timeout`: Maximum time to wait for data.
///
/// Output:
/// - `Some(s)` if data was read before timeout (with both OSC 10 and 11), `None` otherwise.
///
/// Details:
/// - Uses a separate fd (/dev/tty) so stdin is never touched; avoids racing
///   with crossterm's stdin event thread. No background thread.
fn read_with_timeout_unix(timeout: Duration) -> Option<String> {
    use nix::fcntl::{FcntlArg, OFlag, fcntl};
    use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
    use std::os::fd::BorrowedFd;
    use std::os::unix::io::AsRawFd;

    let mut tty = std::fs::File::open("/dev/tty").ok()?;
    let raw_fd = tty.as_raw_fd();

    let fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
    let flags = fcntl(fd, FcntlArg::F_GETFL).ok()?;
    let mut oflags = OFlag::from_bits_truncate(flags);
    oflags.insert(OFlag::O_NONBLOCK);
    fcntl(fd, FcntlArg::F_SETFL(oflags)).ok()?;

    let deadline = Instant::now() + timeout;
    let mut result = Vec::new();
    let mut buffer = [0u8; 512];

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        // Cap to u16::MAX ms for PollTimeout; truncation is intentional.
        #[allow(clippy::cast_possible_truncation)]
        let ms = remaining.as_millis().min(u128::from(u16::MAX)) as u16;
        let poll_timeout = PollTimeout::from(ms);
        let poll_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
        let mut poll_fds = [PollFd::new(poll_fd, PollFlags::POLLIN)];
        match poll(&mut poll_fds, poll_timeout) {
            Ok(0) => {}
            Ok(_) => {
                loop {
                    match tty.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(n) => result.extend_from_slice(&buffer[..n]),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                        Err(_) => break,
                    }
                }
                if result.len() > OSC_READ_MAX_BYTES {
                    break;
                }
                let s = String::from_utf8_lossy(&result).to_string();
                if parse_osc_color_response(&s, 10).is_some()
                    && parse_osc_color_response(&s, 11).is_some()
                {
                    return Some(s);
                }
            }
            Err(_) => break,
        }
    }
    None
}

/// Interval (ms) at which the Windows reader thread waits on stdin handle before
/// checking cancel; keeps join() bounded when main times out.
#[cfg(windows)]
const READER_POLL_MS: u32 = 50;

#[cfg(all(not(unix), windows))]
/// What: Read from stdin with timeout using a cancellable reader thread (Windows).
///
/// Inputs:
/// - `timeout`: Maximum time to wait for the first data.
///
/// Output:
/// - `Some(s)` if data was received before timeout (with both OSC 10 and 11), `None` otherwise.
///
/// Details:
/// - Reader uses WaitForSingleObject on stdin handle with short timeout so it
///   wakes periodically; main sets a cancel flag on recv timeout then join() returns quickly.
/// - Avoids join() hanging when OSC query never yields bytes (e.g. startup/theme reload).
fn read_with_timeout_thread_joined(timeout: Duration) -> Option<String> {
    use std::os::windows::io::AsRawHandle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc;
    use std::thread;

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_reader = Arc::clone(&cancel);
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut result = Vec::new();
        let mut buffer = [0u8; 512];
        let stdin = std::io::stdin();
        let raw_handle = stdin.as_raw_handle();

        loop {
            if cancel_reader.load(Ordering::Relaxed) {
                break;
            }
            let wait_ms = READER_POLL_MS;
            let ret = unsafe {
                windows_sys::Win32::System::Threading::WaitForSingleObject(
                    raw_handle as *mut _,
                    wait_ms,
                )
            };
            if ret == windows_sys::Win32::System::Threading::WAIT_TIMEOUT {
                continue;
            }
            if ret != windows_sys::Win32::System::Threading::WAIT_OBJECT_0 {
                break;
            }
            let n = match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            result.extend_from_slice(&buffer[..n]);
            if result.len() > OSC_READ_MAX_BYTES {
                break;
            }
            let s = String::from_utf8_lossy(&result).to_string();
            if parse_osc_color_response(&s, 10).is_some()
                && parse_osc_color_response(&s, 11).is_some()
            {
                break;
            }
        }

        let _ = tx.send(String::from_utf8_lossy(&result).to_string());
    });

    let out = rx.recv_timeout(timeout).ok();
    cancel.store(true, Ordering::Relaxed);
    let _ = handle.join();
    out
}

#[cfg(all(not(unix), not(windows)))]
/// What: Read from stdin with timeout using a thread; joins with bounded wait.
///
/// Inputs:
/// - `timeout`: Maximum time to wait for the first data.
///
/// Output:
/// - `Some(s)` if data was received before timeout (with both OSC 10 and 11), `None` otherwise.
///
/// Details:
/// - Spawns a reader thread. On timeout, waits up to JOIN_TIMEOUT for the thread
///   to finish; if it does not (reader blocked on stdin), the handle is dropped
///   (thread detaches) so the caller does not block indefinitely.
fn read_with_timeout_thread_joined(timeout: Duration) -> Option<String> {
    use std::sync::mpsc;
    use std::thread;

    const JOIN_TIMEOUT: Duration = Duration::from_secs(2);

    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let mut result = Vec::new();
        let mut buffer = [0u8; 512];
        let mut stdin = std::io::stdin();

        loop {
            let n = match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            result.extend_from_slice(&buffer[..n]);
            if result.len() > OSC_READ_MAX_BYTES {
                break;
            }
            let s = String::from_utf8_lossy(&result).to_string();
            if parse_osc_color_response(&s, 10).is_some()
                && parse_osc_color_response(&s, 11).is_some()
            {
                break;
            }
        }

        let _ = tx.send(String::from_utf8_lossy(&result).to_string());
    });

    let out = rx.recv_timeout(timeout).ok();
    // Bounded join: avoid blocking forever if reader is stuck on stdin.read()
    let (join_tx, join_rx) = mpsc::channel();
    let join_handle = handle;
    thread::spawn(move || {
        let _ = join_handle.join();
        let _ = join_tx.send(());
    });
    let _ = join_rx.recv_timeout(JOIN_TIMEOUT);
    out
}

/// Parse an OSC color response for a specific code (10 for fg, 11 for bg).
fn parse_osc_color_response(response: &str, code: u8) -> Option<Color> {
    // Look for pattern: ESC ] <code> ; rgb:RRRR/GGGG/BBBB (BEL or ESC \)
    // The response might have ESC as \x1b or other forms

    let code_str = format!("]{code};");

    // Find the start of our response
    let start = response.find(&code_str)?;
    let after_code = &response[start + code_str.len()..];

    // Find "rgb:" or "rgba:"
    let rgb_start = if after_code.starts_with("rgb:") {
        4
    } else if after_code.starts_with("rgba:") {
        5 // rgba: format (we'll ignore alpha)
    } else {
        return None;
    };

    let color_part = &after_code[rgb_start..];

    // Parse RRRR/GGGG/BBBB or RR/GG/BB format
    // Find the terminator (BEL \x07 or ESC \ or end of string)
    let end = color_part
        .find('\x07')
        .or_else(|| color_part.find('\x1b'))
        .unwrap_or(color_part.len());

    let color_str = &color_part[..end];
    parse_rgb_color(color_str)
}

/// Parse an RGB color string in format RRRR/GGGG/BBBB or RR/GG/BB.
fn parse_rgb_color(s: &str) -> Option<Color> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() < 3 {
        return None;
    }

    // Handle both 4-digit (16-bit) and 2-digit (8-bit) formats
    let r = parse_color_component(parts[0])?;
    let g = parse_color_component(parts[1])?;
    let b = parse_color_component(parts[2])?;

    Some(Color::Rgb(r, g, b))
}

/// Parse a single color component (handles 2-digit or 4-digit hex).
fn parse_color_component(s: &str) -> Option<u8> {
    // If 4 digits, take first 2 (high byte of 16-bit value)
    // If 2 digits, use directly
    let hex = if s.len() == 4 { &s[0..2] } else { s };

    u8::from_str_radix(hex, 16).ok()
}

/// What: Build a complete theme from foreground and background colors.
///
/// Inputs:
/// - `fg`: Foreground (text) color from terminal.
/// - `bg`: Background color from terminal.
///
/// Output:
/// - A `Theme` with all 16 semantic colors derived from fg/bg.
///
/// Details:
/// - Maps `text` to fg, `base` to bg.
/// - Derives intermediate colors (mantle, crust, surfaces, overlays, subtexts).
/// - Uses fixed semantic accent colors that work well on both light and dark backgrounds.
/// - Detects light vs dark theme based on background luminance.
#[must_use]
pub fn theme_from_fg_bg(fg: Color, bg: Color) -> Theme {
    let (fg_r, fg_g, fg_b) = color_to_rgb(fg);
    let (bg_r, bg_g, bg_b) = color_to_rgb(bg);

    // Determine if this is a light or dark theme based on background luminance
    let bg_luminance = luminance(bg_r, bg_g, bg_b);
    let is_dark = bg_luminance < 0.5;

    // Derive background layers
    // For dark themes: crust < mantle < base
    // For light themes: crust > mantle > base
    let (crust, mantle) = if is_dark {
        let crust = darken(bg_r, bg_g, bg_b, 0.15);
        let mantle = darken(bg_r, bg_g, bg_b, 0.08);
        (crust, mantle)
    } else {
        let crust = lighten(bg_r, bg_g, bg_b, 0.15);
        let mantle = lighten(bg_r, bg_g, bg_b, 0.08);
        (crust, mantle)
    };

    // Derive surfaces (slightly lighter/darker than base)
    let (surface1, surface2) = if is_dark {
        (
            lighten(bg_r, bg_g, bg_b, 0.10),
            lighten(bg_r, bg_g, bg_b, 0.15),
        )
    } else {
        (
            darken(bg_r, bg_g, bg_b, 0.08),
            darken(bg_r, bg_g, bg_b, 0.12),
        )
    };

    // Derive overlays (for borders, muted UI elements)
    let (overlay1, overlay2) = if is_dark {
        (
            lighten(bg_r, bg_g, bg_b, 0.25),
            lighten(bg_r, bg_g, bg_b, 0.35),
        )
    } else {
        (
            darken(bg_r, bg_g, bg_b, 0.25),
            darken(bg_r, bg_g, bg_b, 0.35),
        )
    };

    // Derive subtexts (muted versions of fg)
    let subtext0 = blend(fg_r, fg_g, fg_b, bg_r, bg_g, bg_b, 0.75);
    let subtext1 = blend(fg_r, fg_g, fg_b, bg_r, bg_g, bg_b, 0.85);

    // Semantic accent colors - use a palette that works on both light and dark
    // These are chosen to have good contrast on most backgrounds
    let (sapphire, mauve, green, yellow, red, lavender) = if is_dark {
        // Dark theme accents (Catppuccin Mocha-inspired)
        (
            Color::Rgb(116, 199, 236), // sapphire - interactive
            Color::Rgb(203, 166, 247), // mauve - headings
            Color::Rgb(166, 227, 161), // green - success
            Color::Rgb(249, 226, 175), // yellow - warning
            Color::Rgb(243, 139, 168), // red - error
            Color::Rgb(180, 190, 254), // lavender - emphasis
        )
    } else {
        // Light theme accents (Catppuccin Latte-inspired)
        (
            Color::Rgb(30, 102, 245),  // sapphire - interactive
            Color::Rgb(136, 57, 239),  // mauve - headings
            Color::Rgb(64, 160, 43),   // green - success
            Color::Rgb(223, 142, 29),  // yellow - warning
            Color::Rgb(210, 15, 57),   // red - error
            Color::Rgb(114, 135, 253), // lavender - emphasis
        )
    };

    Theme {
        base: bg,
        mantle: rgb_to_color(mantle),
        crust: rgb_to_color(crust),
        surface1: rgb_to_color(surface1),
        surface2: rgb_to_color(surface2),
        overlay1: rgb_to_color(overlay1),
        overlay2: rgb_to_color(overlay2),
        text: fg,
        subtext0: rgb_to_color(subtext0),
        subtext1: rgb_to_color(subtext1),
        sapphire,
        mauve,
        green,
        yellow,
        red,
        lavender,
    }
}

/// Extract RGB components from a Color.
const fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        // For non-RGB colors, use reasonable defaults
        Color::Black => (0, 0, 0),
        Color::White => (255, 255, 255),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Yellow => (255, 255, 0),
        Color::Cyan => (0, 255, 255),
        Color::Magenta => (255, 0, 255),
        Color::DarkGray => (64, 64, 64),
        Color::LightRed => (255, 128, 128),
        Color::LightGreen => (128, 255, 128),
        Color::LightBlue => (128, 128, 255),
        Color::LightYellow => (255, 255, 128),
        Color::LightCyan => (128, 255, 255),
        Color::LightMagenta => (255, 128, 255),
        Color::Gray | Color::Indexed(_) | Color::Reset => (128, 128, 128),
    }
}

/// Convert RGB tuple to Color.
const fn rgb_to_color((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

/// Calculate relative luminance (0.0 = black, 1.0 = white).
fn luminance(r: u8, g: u8, b: u8) -> f32 {
    // sRGB to linear conversion then weighted sum
    let r_lin = srgb_to_linear(r);
    let g_lin = srgb_to_linear(g);
    let b_lin = srgb_to_linear(b);
    0.2126f32.mul_add(r_lin, 0.7152f32.mul_add(g_lin, 0.0722 * b_lin))
}

/// Clamp and round f32 to u8 for color components (0.0..=255.0).
/// Avoids truncation/sign-loss by rounding then clamping to 0..=255.
fn clamp_f32_to_u8(v: f32) -> u8 {
    // Value is 0.0..=255.0; round then clamp keeps n in 0..=255; i32 and u8 both hold that range
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (v.round() as i32).clamp(0, 255) as u8;
    n
}

/// Convert sRGB component to linear.
fn srgb_to_linear(c: u8) -> f32 {
    let c = f32::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Darken a color by a factor (0.0 = no change, 1.0 = black).
fn darken(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    let factor = 1.0 - factor;
    (
        clamp_f32_to_u8(f32::from(r) * factor),
        clamp_f32_to_u8(f32::from(g) * factor),
        clamp_f32_to_u8(f32::from(b) * factor),
    )
}

/// Lighten a color by a factor (0.0 = no change, 1.0 = white).
fn lighten(r: u8, g: u8, b: u8, factor: f32) -> (u8, u8, u8) {
    (
        clamp_f32_to_u8((255.0 - f32::from(r)).mul_add(factor, f32::from(r))),
        clamp_f32_to_u8((255.0 - f32::from(g)).mul_add(factor, f32::from(g))),
        clamp_f32_to_u8((255.0 - f32::from(b)).mul_add(factor, f32::from(b))),
    )
}

/// Blend two colors with a factor (0.0 = second color, 1.0 = first color).
fn blend(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8, factor: f32) -> (u8, u8, u8) {
    let inv = 1.0 - factor;
    (
        clamp_f32_to_u8(f32::from(r1).mul_add(factor, f32::from(r2) * inv)),
        clamp_f32_to_u8(f32::from(g1).mul_add(factor, f32::from(g2) * inv)),
        clamp_f32_to_u8(f32::from(b1).mul_add(factor, f32::from(b2) * inv)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rgb_color_4digit() {
        // 16-bit format: rrrr/gggg/bbbb
        let color = parse_rgb_color("ffff/0000/8080");
        let color = color.expect("valid 4-digit RGB");
        let Color::Rgb(r, g, b) = color else {
            panic!("Expected RGB color");
        };
        assert_eq!(r, 0xff);
        assert_eq!(g, 0x00);
        assert_eq!(b, 0x80);
    }

    #[test]
    fn test_parse_rgb_color_2digit() {
        // 8-bit format: rr/gg/bb
        let color = parse_rgb_color("cd/d6/f4");
        let color = color.expect("valid 2-digit RGB");
        let Color::Rgb(r, g, b) = color else {
            panic!("Expected RGB color");
        };
        assert_eq!(r, 0xcd);
        assert_eq!(g, 0xd6);
        assert_eq!(b, 0xf4);
    }

    #[test]
    fn test_theme_from_fg_bg_dark() {
        let fg = Color::Rgb(205, 214, 244); // Light text
        let bg = Color::Rgb(30, 30, 46); // Dark background

        let theme = theme_from_fg_bg(fg, bg);

        // Verify base colors match
        assert!(matches!(theme.base, Color::Rgb(30, 30, 46)));
        assert!(matches!(theme.text, Color::Rgb(205, 214, 244)));

        // Verify we detected dark theme (mantle should be darker than base)
        let Color::Rgb(m_r, m_g, m_b) = theme.mantle else {
            panic!("Expected RGB");
        };
        let Color::Rgb(b_r, b_g, b_b) = theme.base else {
            panic!("Expected RGB");
        };
        assert!(
            m_r <= b_r && m_g <= b_g && m_b <= b_b,
            "mantle should be darker than base for dark theme"
        );
    }

    #[test]
    fn test_theme_from_fg_bg_light() {
        let fg = Color::Rgb(28, 28, 34); // Dark text
        let bg = Color::Rgb(245, 245, 247); // Light background

        let theme = theme_from_fg_bg(fg, bg);

        // Verify base colors match
        assert!(matches!(theme.base, Color::Rgb(245, 245, 247)));
        assert!(matches!(theme.text, Color::Rgb(28, 28, 34)));

        // Verify we detected light theme (mantle should be lighter than base)
        let Color::Rgb(m_r, m_g, m_b) = theme.mantle else {
            panic!("Expected RGB");
        };
        let Color::Rgb(b_r, b_g, b_b) = theme.base else {
            panic!("Expected RGB");
        };
        assert!(
            m_r >= b_r && m_g >= b_g && m_b >= b_b,
            "mantle should be lighter than base for light theme"
        );
    }

    #[test]
    fn test_luminance() {
        // Black should have 0 luminance
        assert!((luminance(0, 0, 0) - 0.0).abs() < 0.01);
        // White should have 1 luminance
        assert!((luminance(255, 255, 255) - 1.0).abs() < 0.01);
        // Gray (128) has ~0.21 luminance due to sRGB gamma curve (not 0.5)
        // The sRGB transfer function makes mid-gray darker than linear 0.5
        let gray_lum = luminance(128, 128, 128);
        assert!(
            gray_lum > 0.1 && gray_lum < 0.4,
            "Gray luminance {gray_lum} should be between 0.1 and 0.4 (sRGB gamma)"
        );
    }
}
