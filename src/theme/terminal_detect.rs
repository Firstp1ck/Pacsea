//! Terminal detection for theme fallback support.
//!
//! This module provides functionality to detect whether the current terminal
//! is on the supported list for OSC 10/11 color queries.

use std::env;

/// Terminals known to support OSC 10/11 color queries.
/// These terminals can report their foreground/background colors reliably.
const SUPPORTED_TERMINALS: [&str; 11] = [
    "alacritty",
    "kitty",
    "konsole",
    "ghostty",
    "xterm",
    "gnome-terminal",
    "xfce4-terminal", // Binary name for xfce-terminal
    "tilix",
    "mate-terminal",
    "wezterm",     // TERM_PROGRAM=WezTerm or binary wezterm
    "wezterm-gui", // WezTerm GUI binary when launched from desktop
];

/// Alternative names for some terminals (e.g., process names vs binary names).
const TERMINAL_ALIASES: [(&str, &str); 3] = [
    ("xfce-terminal", "xfce4-terminal"),
    ("gnome-terminal-server", "gnome-terminal"),
    ("mate-terminal.wrapper", "mate-terminal"),
];

/// What: Check if the current terminal is on the supported list for OSC queries.
///
/// Inputs:
/// - None (reads from environment and/or process info).
///
/// Output:
/// - `true` if the terminal is supported for OSC 10/11 color queries.
/// - `false` if the terminal is unknown or not supported.
///
/// Details:
/// - Checks `TERM_PROGRAM` environment variable first (set by some terminals).
/// - Checks `TERM` environment variable (most reliable for terminal type).
/// - Falls back to reading the parent process name on Linux via `/proc`.
/// - Case-insensitive matching against the supported terminal list.
/// - Returns `false` if not running in a TTY or detection fails.
#[must_use]
pub fn is_supported_terminal_for_theme() -> bool {
    // Quick check: if not a TTY, terminal theme won't work
    if !is_tty() {
        return false;
    }

    // Try TERM_PROGRAM first (set by some terminals like Alacritty, Kitty)
    if let Ok(term_program) = env::var("TERM_PROGRAM")
        && is_supported_name(&term_program)
    {
        return true;
    }

    // Try TERM (most reliable - set by the terminal itself, e.g., "alacritty", "xterm-256color")
    if let Ok(term) = env::var("TERM")
        && is_supported_name(&term)
    {
        return true;
    }

    // Try COLORTERM (sometimes set to the terminal name, but often just "truecolor")
    if let Ok(colorterm) = env::var("COLORTERM")
        && colorterm != "truecolor"
        && colorterm != "24bit"
        && is_supported_name(&colorterm)
    {
        return true;
    }

    // On Linux, try to detect from parent process
    #[cfg(target_os = "linux")]
    {
        if let Some(parent_name) = get_parent_process_name()
            && is_supported_name(&parent_name)
        {
            return true;
        }
    }

    false
}

/// What: Get the detected terminal name, if available.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(String)` with the terminal name if detected.
/// - `None` if detection failed or terminal is unknown.
///
/// Details:
/// - Useful for logging/debugging terminal detection.
#[must_use]
#[allow(dead_code)]
pub fn get_terminal_name() -> Option<String> {
    // Try TERM_PROGRAM first
    if let Ok(term_program) = env::var("TERM_PROGRAM")
        && !term_program.is_empty()
    {
        return Some(term_program);
    }

    // Try TERM (most reliable for terminal type)
    if let Ok(term) = env::var("TERM")
        && !term.is_empty()
    {
        return Some(term);
    }

    // Try COLORTERM (if not just a capability indicator)
    if let Ok(colorterm) = env::var("COLORTERM")
        && !colorterm.is_empty()
        && colorterm != "truecolor"
        && colorterm != "24bit"
    {
        return Some(colorterm);
    }

    // On Linux, try parent process
    #[cfg(target_os = "linux")]
    {
        if let Some(parent_name) = get_parent_process_name() {
            return Some(parent_name);
        }
    }

    None
}

/// Check if stdout is a TTY.
fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Check if a terminal name (case-insensitive) is in the supported list.
fn is_supported_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();

    // Direct match
    if SUPPORTED_TERMINALS
        .iter()
        .any(|t| lower == *t || lower.contains(t))
    {
        return true;
    }

    // Check aliases
    for (alias, canonical) in &TERMINAL_ALIASES {
        if lower == *alias || lower.contains(alias) {
            return SUPPORTED_TERMINALS.contains(canonical);
        }
    }

    false
}

/// Get the parent process name on Linux by reading /proc.
#[cfg(target_os = "linux")]
fn get_parent_process_name() -> Option<String> {
    use std::fs;

    // Read PPID from /proc/self/stat
    let stat = fs::read_to_string("/proc/self/stat").ok()?;

    // Format: pid (comm) state ppid ...
    // Find the closing paren to get past the comm field
    let close_paren = stat.rfind(')')?;
    let after_comm = &stat[close_paren + 2..]; // Skip ") "
    let mut parts = after_comm.split_whitespace();
    parts.next()?; // state
    let ppid_str = parts.next()?;
    let ppid: u32 = ppid_str.parse().ok()?;

    // Read parent process comm
    let comm_path = format!("/proc/{ppid}/comm");
    let comm = fs::read_to_string(comm_path).ok()?;
    Some(comm.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_name_direct_match() {
        assert!(is_supported_name("alacritty"));
        assert!(is_supported_name("Alacritty"));
        assert!(is_supported_name("kitty"));
        assert!(is_supported_name("KITTY"));
        assert!(is_supported_name("konsole"));
        assert!(is_supported_name("ghostty"));
        assert!(is_supported_name("xterm"));
        assert!(is_supported_name("gnome-terminal"));
        assert!(is_supported_name("tilix"));
        assert!(is_supported_name("mate-terminal"));
    }

    #[test]
    fn test_is_supported_name_aliases() {
        assert!(is_supported_name("xfce-terminal"));
        assert!(is_supported_name("gnome-terminal-server"));
        assert!(is_supported_name("mate-terminal.wrapper"));
    }

    #[test]
    fn test_is_supported_name_contains() {
        // Some terminals report longer names
        assert!(is_supported_name("alacritty-0.13.0"));
        assert!(is_supported_name("/usr/bin/kitty"));
        // TERM variable often has suffixes like -256color
        assert!(is_supported_name("xterm-256color"));
        assert!(is_supported_name("xterm-direct"));
    }

    #[test]
    fn test_is_supported_name_wezterm() {
        assert!(is_supported_name("wezterm"));
        assert!(is_supported_name("WezTerm"));
        assert!(is_supported_name("wezterm-gui"));
        // TERM_PROGRAM is often set to "WezTerm" by WezTerm
        assert!(is_supported_name("WezTerm"));
    }

    #[test]
    fn test_is_supported_name_unsupported() {
        assert!(!is_supported_name("urxvt"));
        assert!(!is_supported_name("st"));
        assert!(!is_supported_name("screen"));
        assert!(!is_supported_name("tmux"));
        assert!(!is_supported_name("unknown"));
        assert!(!is_supported_name(""));
    }
}
