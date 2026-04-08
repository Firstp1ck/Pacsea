//! Helpers for the optional `sudo` credential cache (`timestamp_timeout`) setup wizard.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::state::AppState;
use crate::state::modal::SudoTimestampChoice;
use crate::theme::theme;

/// Visible instruction lines in the sudo timestamp wizard (scroll viewport height).
pub const SUDO_TIMESTAMP_INSTRUCTION_VIEWPORT_LINES: usize = 9;

/// What: Build scrollable instruction lines for the sudo timestamp setup wizard.
///
/// Inputs:
/// - `app`: Application state for localized strings.
/// - `choice`: Selected `timestamp_timeout` mapping.
///
/// Output:
/// - Owned lines for the instructions pane (excluding the modal title block).
///
/// Details:
/// - Used by the renderer and by the key handler for scroll clamping.
#[must_use]
#[allow(clippy::vec_init_then_push)] // Imperative layout mirrors the modal text structure.
pub fn sudo_timestamp_instruction_lines(
    app: &AppState,
    choice: SudoTimestampChoice,
) -> Vec<Line<'static>> {
    let th = theme();
    let user = std::env::var("USER").unwrap_or_else(|_| "youruser".to_string());
    let heading_key = match choice {
        SudoTimestampChoice::TenMinutes => {
            "app.modals.sudo_timestamp_setup.instructions_heading_ten"
        }
        SudoTimestampChoice::ThirtyMinutes => {
            "app.modals.sudo_timestamp_setup.instructions_heading_thirty"
        }
        SudoTimestampChoice::Infinity => {
            "app.modals.sudo_timestamp_setup.instructions_heading_infinity"
        }
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, heading_key),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.instructions_note_tui"),
        Style::default().fg(th.text),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.label_dropin_contents"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    for l in sudoers_defaults_line(choice).lines() {
        lines.push(Line::from(Span::styled(
            l.to_string(),
            Style::default().fg(th.lavender),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.label_user_scoped"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    for l in sudoers_user_scoped_line(&user, choice).lines() {
        lines.push(Line::from(Span::styled(
            l.to_string(),
            Style::default().fg(th.lavender),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.label_manual"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.manual_step_1"),
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.manual_step_2"),
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.sudo_timestamp_setup.manual_step_3"),
        Style::default().fg(th.subtext1),
    )));
    lines
}

/// Drop-in file path suggested by the wizard (stable name for detection).
pub const SUDOERS_DROP_IN_PATH: &str = "/etc/sudoers.d/99-pacsea-timestamp";

/// Marker comment inside the drop-in so we can detect Pacsea-created files when readable.
const DROP_IN_MARKER: &str = "Pacsea optional:";

/// What: Map a wizard choice to the `sudoers` `timestamp_timeout` value.
///
/// Inputs:
/// - `choice`: User-selected duration.
///
/// Output:
/// - Minutes as a positive integer, or `-1` for no expiry in the session (per `sudoers(5)`).
///
/// Details:
/// - `-1` behavior depends on sudo version/policy; document in UI copy.
#[must_use]
pub const fn timestamp_timeout_value(choice: SudoTimestampChoice) -> i32 {
    match choice {
        SudoTimestampChoice::TenMinutes => 10,
        SudoTimestampChoice::ThirtyMinutes => 30,
        SudoTimestampChoice::Infinity => -1,
    }
}

/// What: Build a single-line `Defaults` entry for a global `timestamp_timeout`.
///
/// Inputs:
/// - `choice`: Selected cache duration.
///
/// Output:
/// - One or two lines: marker comment plus `Defaults timestamp_timeout=…`.
///
/// Details:
/// - Intended for `/etc/sudoers.d/*`; validate with `visudo -cf` before installing.
#[must_use]
pub fn sudoers_defaults_line(choice: SudoTimestampChoice) -> String {
    let v = timestamp_timeout_value(choice);
    format!(
        "# {DROP_IN_MARKER} extend sudo credential cache for long installs/updates\nDefaults timestamp_timeout={v}"
    )
}

/// What: Build a user-scoped `Defaults` line alternative for the instructions pane.
///
/// Inputs:
/// - `username`: Target login name (typically `$USER`).
/// - `choice`: Selected cache duration.
///
/// Output:
/// - Two lines: marker comment plus `Defaults:username timestamp_timeout=…`.
///
/// Details:
/// - Safer than a global `Defaults` on shared systems; requires correct username spelling.
#[must_use]
pub fn sudoers_user_scoped_line(username: &str, choice: SudoTimestampChoice) -> String {
    let v = timestamp_timeout_value(choice);
    format!(
        "# {DROP_IN_MARKER} extend sudo credential cache for this user only\nDefaults:{username} timestamp_timeout={v}"
    )
}

/// What: Detect whether the suggested Pacsea drop-in is already present and readable.
///
/// Inputs:
/// - None; reads [`SUDOERS_DROP_IN_PATH`] from disk.
///
/// Output:
/// - `true` when the file is readable and mentions our marker and `timestamp_timeout`.
///
/// Details:
/// - Best-effort only: unreadable paths (permission denied) yield `false`.
#[must_use]
pub fn pacsea_sudo_timestamp_drop_in_present() -> bool {
    std::fs::read_to_string(SUDOERS_DROP_IN_PATH)
        .ok()
        .is_some_and(|s| s.contains(DROP_IN_MARKER) && s.contains("timestamp_timeout"))
}

/// What: Build a POSIX shell script that validates and installs the drop-in with `sudo`.
///
/// Inputs:
/// - `choice`: Selected cache duration.
///
/// Output:
/// - A script suitable for `spawn_shell_commands_in_terminal`: writes temp file, `visudo -cf`, `install -m 0440`.
///
/// Details:
/// - Ends with `read` so the user can see success/failure in an external terminal.
/// - Uses [`SUDOERS_DROP_IN_PATH`] as the destination path.
#[must_use]
pub fn apply_drop_in_shell_script(choice: SudoTimestampChoice) -> String {
    let block = sudoers_defaults_line(choice);
    let path = SUDOERS_DROP_IN_PATH;
    format!(
        r#"set -euo pipefail
DROP_IN="{path}"
TMP="$(mktemp)"
trap 'rm -f "$TMP"' EXIT
cat <<'EOF' >"$TMP"
{block}
EOF
sudo visudo -cf "$TMP"
sudo install -m 0440 "$TMP" "$DROP_IN"
echo "Installed $DROP_IN"
read -r -p "Press Enter to close... " _
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamp_timeout_value_maps_choices() {
        assert_eq!(timestamp_timeout_value(SudoTimestampChoice::TenMinutes), 10);
        assert_eq!(
            timestamp_timeout_value(SudoTimestampChoice::ThirtyMinutes),
            30
        );
        assert_eq!(timestamp_timeout_value(SudoTimestampChoice::Infinity), -1);
    }

    #[test]
    fn sudoers_defaults_line_contains_marker_and_defaults() {
        let s = sudoers_defaults_line(SudoTimestampChoice::ThirtyMinutes);
        assert!(s.contains("Defaults timestamp_timeout=30"));
        assert!(s.contains(DROP_IN_MARKER));
    }

    #[test]
    fn sudoers_user_scoped_line_contains_username() {
        let s = sudoers_user_scoped_line("alice", SudoTimestampChoice::TenMinutes);
        assert!(s.contains("Defaults:alice timestamp_timeout=10"));
    }

    #[test]
    fn apply_script_runs_visudo_and_install() {
        let script = apply_drop_in_shell_script(SudoTimestampChoice::Infinity);
        assert!(script.contains("visudo -cf"));
        assert!(script.contains("install -m 0440"));
        assert!(script.contains(SUDOERS_DROP_IN_PATH));
    }
}
