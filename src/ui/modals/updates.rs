use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::{Theme, theme};

/// What: Collection of line vectors for the three panes in the updates modal.
///
/// Inputs:
/// - None (constructed by `build_update_lines`)
///
/// Output:
/// - Holds left, center, and right pane lines
///
/// Details:
/// - Used to group related line collections and reduce data flow complexity.
struct UpdateLines {
    /// Left pane lines showing old versions (right-aligned).
    left: Vec<Line<'static>>,
    /// Center pane lines showing arrows (centered).
    center: Vec<Line<'static>>,
    /// Right pane lines showing new versions (left-aligned).
    right: Vec<Line<'static>>,
}

/// What: Calculate the modal rectangle centered within the available area.
///
/// Inputs:
/// - `area`: Full screen area used to center the modal
///
/// Output:
/// - Returns a `Rect` representing the modal's position and size
///
/// Details:
/// - Calculates desired dimensions (half width, constrained height)
/// - Clamps dimensions to fit within available area with margins
/// - Centers the modal and ensures it fits within bounds
fn calculate_modal_rect(area: Rect) -> Rect {
    // Calculate desired dimensions
    let desired_w = area.width / 2;
    let desired_h = (area.height.saturating_sub(8).min(20)) * 2;

    // Clamp dimensions to fit within available area (with 2px margins on each side)
    let w = desired_w.min(area.width.saturating_sub(4)).max(20);
    let h = desired_h.min(area.height.saturating_sub(4)).max(10);

    // Center the modal within the area
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;

    // Final clamp: ensure the entire rect fits within the area
    let x = x.max(area.x);
    let y = y.max(area.y);
    let max_w = (area.x + area.width).saturating_sub(x);
    let max_h = (area.y + area.height).saturating_sub(y);
    let w = w.min(max_w);
    let h = h.min(max_h);

    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

/// What: Build all three line vectors for update entries in a single pass.
///
    /// Inputs:
    /// - `entries`: Update entries to display (`name`, `old_version`, `new_version`)
    /// - `th`: Theme for styling
///
/// Output:
/// - Returns `UpdateLines` containing left, center, and right pane lines
///
/// Details:
/// - Iterates over entries once to build all three line vectors simultaneously
/// - Left pane: old versions with right padding (right-aligned)
/// - Center pane: arrows with spacing (centered)
/// - Right pane: new versions with left padding (left-aligned)
fn build_update_lines(entries: &[(String, String, String)], th: &Theme) -> UpdateLines {
    let mut left_lines = Vec::new();
    let mut center_lines = Vec::new();
    let mut right_lines = Vec::new();

    for (name, old_version, new_version) in entries.iter() {
        // Build left pane line (old versions) - right-aligned with padding
        let left_text = format!("{name} - {old_version}     ");
        left_lines.push(Line::from(Span::styled(
            left_text,
            Style::default().fg(th.text),
        )));

        // Build center arrow line with spacing (5 spaces on each side)
        center_lines.push(Line::from(Span::styled(
            "     →     ",
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )));

        // Build right pane line (new versions) with padding
        let right_text = format!("     {name} - {new_version}");
        right_lines.push(Line::from(Span::styled(
            right_text,
            Style::default().fg(th.text),
        )));
    }

    UpdateLines {
        left: left_lines,
        center: center_lines,
        right: right_lines,
    }
}

/// What: Render a scrollable pane with common styling.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `lines`: Lines to render in the pane
/// - `chunk`: Rect area for the pane
/// - `alignment`: Text alignment (Left, Right, or Center)
/// - `scroll`: Scroll offset (lines) for the pane
/// - `th`: Theme for styling
///
/// Output:
/// - Renders the paragraph widget to the frame
///
/// Details:
/// - Creates a paragraph with common styling (text color, background, wrap, scroll)
/// - Applies the specified alignment
fn render_pane(
    f: &mut Frame,
    lines: Vec<Line<'static>>,
    chunk: Rect,
    alignment: Alignment,
    scroll: u16,
    th: &Theme,
) {
    let para = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .alignment(alignment)
        .wrap(Wrap { trim: true })
        .scroll((scroll, 0));
    f.render_widget(para, chunk);
}

/// What: Render the available updates modal with scrollable list.
///
/// Inputs:
/// - `f`: Frame to render into
    /// - `app`: Mutable application state (records rects)
    /// - `area`: Full screen area used to center the modal
    /// - `entries`: Update entries to display (`name`, `old_version`, `new_version`)
    /// - `scroll`: Scroll offset (lines) for the updates list
///
/// Output:
/// - Draws the updates list with scroll support
///
/// Details:
/// - Shows update entries with old version on left, arrow in center, new version on right
/// - Records rects for mouse interaction and scrolling
pub fn render_updates(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    entries: &[(String, String, String)],
    scroll: u16,
) {
    let th = theme();
    let rect = calculate_modal_rect(area);
    f.render_widget(Clear, rect);

    // Record outer rect for mouse hit-testing
    app.updates_modal_rect = Some((rect.x, rect.y, rect.width, rect.height));

    // Split into header and content areas
    let inner_rect = Rect {
        x: rect.x + 1,
        y: rect.y + 1,
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Heading + blank line
            Constraint::Min(1),    // Content area
        ])
        .split(inner_rect);

    // Render heading
    let heading_line = Line::from(Span::styled(
        i18n::t(app, "app.modals.updates_window.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    ));
    let heading_para =
        Paragraph::new(heading_line).style(Style::default().fg(th.text).bg(th.mantle));
    f.render_widget(heading_para, chunks[0]);

    if entries.is_empty() {
        let none_line = Line::from(Span::styled(
            i18n::t(app, "app.modals.updates_window.none"),
            Style::default().fg(th.subtext1),
        ));
        let none_para = Paragraph::new(none_line).style(Style::default().fg(th.text).bg(th.mantle));
        f.render_widget(none_para, chunks[1]);
    } else {
        // Split content area into three sections: left pane, center arrow, right pane
        let pane_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(48), // Left pane (old versions)
                Constraint::Length(11), // Center arrow with spacing (5 spaces + arrow + 5 spaces = 11 chars)
                Constraint::Percentage(48), // Right pane (new versions)
            ])
            .split(chunks[1]);

        let update_lines = build_update_lines(entries, &th);

        // Render panes using helper function
        render_pane(
            f,
            update_lines.left,
            pane_chunks[0],
            Alignment::Right,
            scroll,
            &th,
        );
        render_pane(
            f,
            update_lines.center,
            pane_chunks[1],
            Alignment::Center,
            scroll,
            &th,
        );
        render_pane(
            f,
            update_lines.right,
            pane_chunks[2],
            Alignment::Left,
            scroll,
            &th,
        );
    }

    // Render modal border
    let border_block = Block::default()
        .title(Span::styled(
            i18n::t(app, "app.modals.updates_window.title"),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(th.mauve))
        .style(Style::default().bg(th.mantle));
    f.render_widget(border_block, rect);

    // Record inner content rect for scroll handling (reuse inner_rect)
    app.updates_modal_content_rect = Some((
        inner_rect.x,
        inner_rect.y,
        inner_rect.width,
        inner_rect.height,
    ));
}
