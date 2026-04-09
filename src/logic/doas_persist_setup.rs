//! Helpers for the optional `doas` persist setup wizard.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use crate::state::AppState;
use crate::state::modal::DoasPersistChoice;
use crate::theme::theme;

/// Visible instruction lines in the doas persist wizard (scroll viewport height).
pub const DOAS_PERSIST_INSTRUCTION_VIEWPORT_LINES: usize = 10;

/// What: Best-effort current username resolution for display-only snippet guidance.
///
/// Inputs: None.
///
/// Output:
/// - Username string to embed in suggested user-scoped `doas.conf` snippets.
///
/// Details:
/// - Prefers `$USER`, then `$LOGNAME`.
/// - Falls back to `id -un` when env vars are unavailable.
/// - Final fallback is `"youruser"` to keep guidance deterministic.
fn current_username_for_snippet() -> String {
    let from_env = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(name) = from_env {
        return name;
    }
    let from_id = std::process::Command::new("id")
        .args(["-un"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    from_id.unwrap_or_else(|| "youruser".to_string())
}

/// What: Check whether a parsed `doas.conf` body has at least one active `permit persist` rule.
///
/// Inputs:
/// - `contents`: Raw text contents from `doas.conf`.
///
/// Output:
/// - `true` if a non-comment rule line includes both `permit` and `persist`.
///
/// Details:
/// - Ignores blank lines and comments (`# ...`).
/// - Uses token matching to avoid false positives from unrelated substrings.
#[must_use]
pub fn doas_conf_has_persist_rule(contents: &str) -> bool {
    contents.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return false;
        }
        let without_inline_comment = trimmed
            .split_once('#')
            .map_or(trimmed, |(rule, _)| rule.trim());
        let tokens: Vec<&str> = without_inline_comment.split_whitespace().collect();
        tokens.contains(&"permit") && tokens.contains(&"persist")
    })
}

/// What: Detect whether the system `doas.conf` appears configured with a persist rule.
///
/// Inputs: None.
///
/// Output:
/// - `true` when `/etc/doas.conf` exists and contains an active `permit persist` rule.
///
/// Details:
/// - Read-only check; never mutates system files.
/// - Returns `false` on missing/unreadable config or parse mismatch.
#[must_use]
pub fn pacsea_doas_persist_configured() -> bool {
    std::fs::read_to_string("/etc/doas.conf")
        .ok()
        .is_some_and(|contents| doas_conf_has_persist_rule(&contents))
}

/// What: Build a recommended `doas.conf` persist snippet for the selected profile.
///
/// Inputs:
/// - `choice`: Selected snippet profile.
///
/// Output:
/// - A single-line `permit persist` recommendation.
///
/// Details:
/// - Generated text is guidance only. The app never writes `/etc/doas.conf` automatically.
#[must_use]
pub fn doas_persist_snippet(choice: DoasPersistChoice, username: &str) -> String {
    match choice {
        DoasPersistChoice::WheelScoped => "permit persist :wheel as root".to_string(),
        DoasPersistChoice::UserScoped => format!("permit persist {username} as root"),
        DoasPersistChoice::Skip => "# Skip setup".to_string(),
    }
}

/// What: Build shell commands for validating doas policy configuration.
///
/// Inputs: None.
///
/// Output:
/// - Validation command list (`doas -C`, optional non-interactive probe).
///
/// Details:
/// - `doas -C /etc/doas.conf` validates syntax and rule matching.
/// - `doas -n true` is a non-interactive capability probe and may fail when `nopass` is not configured.
#[must_use]
pub fn validation_commands() -> Vec<&'static str> {
    vec!["doas -C /etc/doas.conf", "doas -n true"]
}

/// What: Build scrollable instruction lines for the doas persist setup wizard.
///
/// Inputs:
/// - `app`: Application state for localized strings.
/// - `choice`: Selected persist setup profile.
///
/// Output:
/// - Owned lines for the instructions pane.
///
/// Details:
/// - Used by the renderer and key handler for scroll clamping.
#[must_use]
#[allow(clippy::vec_init_then_push)]
pub fn doas_persist_instruction_lines(
    app: &AppState,
    choice: DoasPersistChoice,
) -> Vec<Line<'static>> {
    let th = theme();
    let user = current_username_for_snippet();
    let snippet = doas_persist_snippet(choice, &user);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.doas_persist_setup.instructions_heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.doas_persist_setup.instructions_note"),
        Style::default().fg(th.text),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.doas_persist_setup.label_snippet"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(Span::styled(
        snippet,
        Style::default().fg(th.lavender),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.doas_persist_setup.label_checks"),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    for cmd in validation_commands() {
        lines.push(Line::from(Span::styled(
            format!("  {cmd}"),
            Style::default().fg(th.subtext1),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.doas_persist_setup.instructions_footer"),
        Style::default().fg(th.overlay1),
    )));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_generation_uses_expected_forms() {
        assert_eq!(
            doas_persist_snippet(DoasPersistChoice::WheelScoped, "alice"),
            "permit persist :wheel as root"
        );
        assert_eq!(
            doas_persist_snippet(DoasPersistChoice::UserScoped, "alice"),
            "permit persist alice as root"
        );
    }

    #[test]
    fn validation_commands_include_required_checks() {
        let cmds = validation_commands();
        assert!(cmds.iter().any(|c| c.contains("doas -C /etc/doas.conf")));
        assert!(cmds.iter().any(|c| c.contains("doas -n true")));
    }

    #[test]
    fn doas_conf_parser_detects_active_persist_rule() {
        let conf = r"
            # comment
            permit persist :wheel as root
        ";
        assert!(doas_conf_has_persist_rule(conf));
    }

    #[test]
    fn doas_conf_parser_ignores_commented_persist_rule() {
        let conf = r"
            # permit persist :wheel as root
            permit :wheel as root
        ";
        assert!(!doas_conf_has_persist_rule(conf));
    }
}
