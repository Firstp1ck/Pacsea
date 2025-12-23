use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Detect the type of alert message based on content.
///
/// Inputs:
/// - `message`: The alert message text.
///
/// Output:
/// - Tuple of (`is_help`, `is_config`, `is_clipboard`, `is_account_locked`, `is_config_dirs`).
///
/// Details:
/// - Checks message content for various patterns to determine alert type.
#[must_use]
fn detect_message_type(message: &str) -> (bool, bool, bool, bool, bool) {
    let is_help = message.contains("Help") || message.contains("Tab Help");
    let is_config = message.contains("Unknown key")
        || message.contains("Missing required keys")
        || message.contains("Missing '='")
        || message.contains("Missing key before '='")
        || message.contains("Duplicate key")
        || message.contains("Invalid color")
        || message.to_lowercase().contains("theme configuration");
    let is_clipboard = {
        let ml = message.to_lowercase();
        ml.contains("clipboard")
            || ml.contains("wl-copy")
            || ml.contains("xclip")
            || ml.contains("wl-clipboard")
    };
    let is_account_locked = message.to_lowercase().contains("account")
        && (message.to_lowercase().contains("locked")
            || message.to_lowercase().contains("lockout"));
    // Detect config directory messages by checking for path patterns
    // Format: "package: /path/to/dir" - language agnostic detection
    // The message contains lines with "package: /path" pattern followed by paths
    let is_config_dirs = {
        let lines: Vec<&str> = message.lines().collect();
        // Check if message has multiple lines with "package: /path" pattern
        // This pattern is language-agnostic as paths are always in the same format
        lines.iter().any(|line| {
            let trimmed = line.trim();
            // Pattern: "package_name: /absolute/path" or "package_name: ~/.config/package"
            // Must have colon followed by whitespace and a path
            trimmed.find(':').is_some_and(|colon_pos| {
                let after_colon = &trimmed[colon_pos + 1..].trim();
                // Check if after colon there's a path (starts with /, ~, or contains .config/)
                after_colon.starts_with('/')
                    || after_colon.starts_with("~/")
                    || after_colon.contains("/.config/")
                    || after_colon.contains("\\.config\\") // Windows paths
            })
        })
    };
    (
        is_help,
        is_config,
        is_clipboard,
        is_account_locked,
        is_config_dirs,
    )
}

/// What: Get header text and box title for alert based on message type.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `is_help`, `is_config`, `is_clipboard`, `is_account_locked`, `is_config_dirs`: Message type flags.
///
/// Output:
/// - Tuple of (`header_text`, `box_title`).
///
/// Details:
/// - Returns appropriate i18n strings based on message type.
#[must_use]
// Multiple bools are used here because message types are mutually exclusive flags
// that are easier to work with as separate parameters than as an enum with many variants.
#[allow(clippy::fn_params_excessive_bools)]
fn get_alert_labels(
    app: &AppState,
    is_help: bool,
    is_config: bool,
    is_clipboard: bool,
    is_account_locked: bool,
    is_config_dirs: bool,
) -> (String, String) {
    let header_text = if is_help {
        i18n::t(app, "app.modals.help.heading")
    } else if is_config {
        i18n::t(app, "app.modals.alert.header_configuration_error")
    } else if is_clipboard {
        i18n::t(app, "app.modals.alert.header_clipboard_copy")
    } else if is_account_locked {
        i18n::t(app, "app.modals.alert.header_account_locked")
    } else if is_config_dirs {
        i18n::t(app, "app.modals.alert.header_config_directories")
    } else {
        i18n::t(app, "app.modals.alert.header_connection_issue")
    };
    let box_title = if is_help {
        format!(" {} ", i18n::t(app, "app.modals.help.title"))
    } else if is_config {
        i18n::t(app, "app.modals.alert.title_configuration_error")
    } else if is_clipboard {
        i18n::t(app, "app.modals.alert.title_clipboard_copy")
    } else if is_account_locked {
        i18n::t(app, "app.modals.alert.title_account_locked")
    } else if is_config_dirs {
        i18n::t(app, "app.modals.alert.title_config_directories")
    } else {
        i18n::t(app, "app.modals.alert.title_connection_issue")
    };
    (header_text, box_title)
}

/// What: Format account locked message with command highlighting.
///
/// Inputs:
/// - `message`: The message text.
/// - `th`: Theme for styling.
///
/// Output:
/// - Vector of formatted lines.
///
/// Details:
/// - Highlights commands in backticks with mauve color and bold.
fn format_account_locked_message(message: &str, th: &crate::theme::Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let message_lines: Vec<&str> = message.lines().collect();
    for (i, line) in message_lines.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        // Highlight commands in backticks
        let parts: Vec<&str> = line.split('`').collect();
        let mut spans = Vec::new();
        for (idx, part) in parts.iter().enumerate() {
            if idx % 2 == 0 {
                // Regular text
                spans.push(Span::styled(
                    (*part).to_string(),
                    Style::default().fg(th.text),
                ));
            } else {
                // Command in backticks - highlight it
                spans.push(Span::styled(
                    format!("`{part}`"),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// What: Render the alert modal with contextual styling for help/config/network messages.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (help scroll used for large help dialogs)
/// - `area`: Full screen area used to center the modal
/// - `message`: Alert message text to display
///
/// Output:
/// - Draws a centered alert box and adjusts styling/size based on the message content.
///
/// Details:
/// - Detects help/configuration/clipboard keywords to pick header titles, resizes large help
///   dialogs, and instructs users on dismissal while respecting the current theme.
#[allow(clippy::many_single_char_names)]
#[allow(clippy::missing_const_for_fn)]
pub fn render_alert(f: &mut Frame, app: &AppState, area: Rect, message: &str) {
    let th = theme();
    let (is_help, is_config, is_clipboard, is_account_locked, is_config_dirs) =
        detect_message_type(message);
    let w = area
        .width
        .saturating_sub(10)
        .min(if is_help { 90 } else { 80 });
    let h = if is_help {
        area.height.saturating_sub(6).min(28)
    } else {
        7
    };
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let (header_text, box_title) = get_alert_labels(
        app,
        is_help,
        is_config,
        is_clipboard,
        is_account_locked,
        is_config_dirs,
    );
    let header_color = if is_help || is_config || is_config_dirs {
        th.mauve
    } else {
        th.red
    };

    // Parse message into lines for help messages
    let mut lines: Vec<Line<'static>> = Vec::new();
    if is_help {
        for line in message.lines() {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(th.text),
            )));
        }
    } else {
        // Don't show header text again if it's the same as the title
        if !is_account_locked {
            lines.push(Line::from(Span::styled(
                header_text,
                Style::default()
                    .fg(header_color)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));
        }
        // Format account locked messages more nicely
        if is_account_locked {
            lines.extend(format_account_locked_message(message, &th));
        } else if is_config_dirs {
            // Format config directory messages line by line for better readability
            for line in message.lines() {
                if line.trim().is_empty() {
                    lines.push(Line::from(""));
                } else {
                    lines.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(th.text),
                    )));
                }
            }
        } else {
            lines.push(Line::from(Span::styled(
                message.to_string(),
                Style::default().fg(th.text),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.common.close_hint"),
        Style::default().fg(th.subtext1),
    )));

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .scroll((if is_help { app.help_scroll } else { 0 }, 0))
        .block(
            Block::default()
                .title(Span::styled(
                    box_title,
                    Style::default()
                        .fg(header_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(header_color))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
