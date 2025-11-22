use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Draw the status label on the bottom border line of the Results block.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (reads status info, updates rect)
/// - `area`: Target rectangle for the results block
///
/// Output:
/// - Renders the status badge/text and records the clickable rect for opening the status page.
///
/// Details:
/// - Shows optional shortcut when Search normal mode is active, centers text within the border, and
///   colors the status dot based on [`AppState::arch_status_color`].
pub fn render_status(f: &mut Frame, app: &mut AppState, area: Rect) {
    let th = theme();

    // Bottom border y coordinate is area.y + area.height - 1
    // Append the Normal-mode keybind used to open the status page only when Search Normal mode is active
    let key_label_opt = app
        .keymap
        .search_normal_open_status
        .first()
        .map(|c| c.label());
    let show_key = matches!(app.focus, crate::state::Focus::Search)
        && app.search_normal_mode
        && key_label_opt.is_some();
    let status_text = if show_key {
        i18n::t_fmt(
            app,
            "app.results.status_with_key",
            &[&app.arch_status_text, &key_label_opt.expect("key_label_opt should be Some when show_key is true")],
        )
    } else {
        format!(
            "{} {}",
            i18n::t(app, "app.results.status_label"),
            app.arch_status_text
        )
    };
    let sx = area.x.saturating_add(2); // a bit of left padding after corner
    let sy = area.y.saturating_add(area.height.saturating_sub(1));
    let maxw = area.width.saturating_sub(4); // avoid right corner
    let mut content = status_text.clone();
    // Truncate by display width, not byte length, to handle wide characters
    if u16::try_from(content.width()).unwrap_or(u16::MAX) > maxw {
        let mut truncated = String::new();
        let mut width_so_far = 0u16;
        for ch in content.chars() {
            let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
            if width_so_far + ch_width > maxw {
                break;
            }
            truncated.push(ch);
            width_so_far += ch_width;
        }
        content = truncated;
    }
    // Compute style to blend with border line
    // Compose a dot + text with color depending on status
    let mut dot = "";
    let mut dot_color = th.overlay1;
    match app.arch_status_color {
        crate::state::ArchStatusColor::Operational => {
            dot = "●";
            dot_color = th.green;
        }
        crate::state::ArchStatusColor::IncidentToday => {
            dot = "●";
            dot_color = th.yellow;
        }
        crate::state::ArchStatusColor::IncidentSevereToday => {
            dot = "●";
            dot_color = th.red;
        }
        crate::state::ArchStatusColor::None => {
            // If we have a nominal message, still show a green dot
            if app
                .arch_status_text
                .to_lowercase()
                .contains("arch systems nominal")
            {
                dot = "●";
                dot_color = th.green;
            }
        }
    }
    let style_text = Style::default()
        .fg(th.mauve)
        .bg(th.base)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let line = Paragraph::new(Line::from(vec![
        Span::styled(
            dot.to_string(),
            Style::default()
                .fg(dot_color)
                .bg(th.base)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(content.clone(), style_text),
    ]));
    // Record clickable rect centered within the available width
    // Use Unicode display width, not byte length, to handle wide characters
    let dot_width = u16::try_from(dot.width()).unwrap_or(u16::MAX);
    let content_width = u16::try_from(content.width()).unwrap_or(u16::MAX);
    let cw = (content_width + dot_width + 1).min(maxw); // +1 for the space
    let pad_left = maxw.saturating_sub(cw) / 2;
    let start_x = sx.saturating_add(pad_left);
    // Clickable rect only over the text portion, not the dot or space
    let click_start_x = start_x.saturating_add(dot_width + 1);
    app.arch_status_rect = Some((
        click_start_x,
        sy,
        content_width.min(maxw.saturating_sub(dot_width + 1)),
        1,
    ));
    let rect = ratatui::prelude::Rect {
        x: start_x,
        y: sy,
        width: cw,
        height: 1,
    };
    f.render_widget(line, rect);
}
