use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

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
        format!(
            "Status: {} [{}]",
            app.arch_status_text,
            key_label_opt.unwrap()
        )
    } else {
        format!("Status: {}", app.arch_status_text)
    };
    let sx = area.x.saturating_add(2); // a bit of left padding after corner
    let sy = area.y.saturating_add(area.height.saturating_sub(1));
    let maxw = area.width.saturating_sub(4); // avoid right corner
    let mut content = status_text.clone();
    if content.len() as u16 > maxw {
        content.truncate(maxw as usize);
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
    let cw = ((content.len() + dot.len() + 1) as u16).min(maxw); // +1 for the space
    let pad_left = maxw.saturating_sub(cw) / 2;
    let start_x = sx.saturating_add(pad_left);
    // Clickable rect only over the text portion, not the dot or space
    let click_start_x = start_x.saturating_add((dot.len() + 1) as u16);
    app.arch_status_rect = Some((
        click_start_x,
        sy,
        (content.len() as u16).min(maxw.saturating_sub((dot.len() + 1) as u16)),
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
