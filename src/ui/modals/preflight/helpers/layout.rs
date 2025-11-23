use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::Rect,
};

/// What: Calculate modal layout dimensions and split into content and keybinds areas.
///
/// Inputs:
/// - `area`: Full screen area used to center the modal
///
/// Output:
/// - Returns a tuple of (`modal_rect`, `content_rect`, `keybinds_rect`)
///
/// Details:
/// - Calculates centered modal size (max 96x32, with 6/8 pixel margins)
/// - Splits modal into content area and keybinds pane (4 lines for keybinds)
/// - Returns the full modal rect, content rect, and keybinds rect
pub fn calculate_modal_layout(area: Rect) -> (Rect, Rect, Rect) {
    // Calculate modal size and position
    let w = area.width.saturating_sub(6).min(96);
    let h = area.height.saturating_sub(8).min(32);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };

    // Split rect into content area and keybinds pane (reserve 4 lines for keybinds to account for borders)
    // With double borders, we need: 1 top border + 2 content lines + 1 bottom border = 4 lines minimum
    let keybinds_height = 4;
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(keybinds_height)])
        .split(rect);
    let content_rect = layout[0];
    let keybinds_rect = layout[1];

    (rect, content_rect, keybinds_rect)
}
