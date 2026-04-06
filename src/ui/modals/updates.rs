use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

/// What: Per-entry wrapped render data for the updates modal.
///
/// Inputs:
/// - Built from one updates entry with pane widths
///
/// Output:
/// - Stores wrapped text lines and render metadata for a single entry block
///
/// Details:
/// - `start_line` is the first rendered line of this entry in the combined list.
/// - `row_render_height` is the max of wrapped left/right line counts.
struct UpdateEntryRenderBlock {
    /// Original package name used for source/tool tag lookup.
    name: String,
    /// Wrapped lines for the left pane (`name - old_version`).
    left_wrapped: Vec<String>,
    /// Wrapped lines for the right pane (`name - new_version`).
    right_wrapped: Vec<String>,
    /// Number of rendered lines this entry occupies across all panes.
    row_render_height: u16,
}

/// What: Shared row model for updates modal rendering and input mapping.
///
/// Inputs:
/// - Entire updates entries list and pane widths
///
/// Output:
/// - Per-entry render blocks with line mappings plus aggregate line metadata
///
/// Details:
/// - Reused by rendering and event handlers through `AppState` line-start snapshots.
struct UpdateRenderModel {
    /// Render blocks in entry order.
    blocks: Vec<UpdateEntryRenderBlock>,
    /// Total rendered line count after wrapping.
    total_lines: u16,
    /// Entry index to first rendered line mapping.
    entry_line_starts: Vec<u16>,
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

/// What: Determine which tool will be used to install/update a package.
///
/// Inputs:
/// - `pkg_name`: Name of the package
///
/// Output:
/// - Returns "pacman" for official packages, "AUR" for AUR packages
///
/// Details:
/// - Checks if package exists in official index first
/// - For AUR packages, returns "AUR" regardless of which helper is installed
fn get_install_tool(pkg_name: &str) -> &'static str {
    // Check if it's in official repos
    if crate::index::find_package_by_name(pkg_name).is_some() {
        return "pacman";
    }

    // It's an AUR package
    "AUR"
}

/// What: Wrap text into lines that fit within the given width.
///
/// Inputs:
/// - `content`: Text content to wrap
/// - `available_width`: Width available for wrapping
///
/// Output:
/// - Vector of strings, each representing a wrapped line
///
/// Details:
/// - Uses Unicode display width for accurate measurement
/// - Wraps at word boundaries
/// - Returns at least one empty line if content is empty
fn wrap_text_to_lines(content: &str, available_width: u16) -> Vec<String> {
    if content.trim().is_empty() {
        return vec![String::new()];
    }

    let width = available_width.max(1) as usize;
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.is_empty() {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0usize;

    for word in words {
        let word_width = word.width();
        let separator_width = usize::from(current_width > 0);
        let test_width = current_width + separator_width + word_width;

        if test_width > width && current_width > 0 {
            // Wrap to new line
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        } else {
            if current_width > 0 {
                current_line.push(' ');
            }
            current_line.push_str(word);
            current_width = test_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// What: Build all three line vectors for update entries with proper alignment.
///
/// Inputs:
/// - `entries`: Update entries to display (`name`, `old_version`, `new_version`)
/// - `th`: Theme for styling
/// - `selected`: Index of the currently selected entry
/// - `left_width`: Width of the left pane in characters
/// - `right_width`: Width of the right pane in characters
///
/// Output:
/// - Returns `UpdateLines` containing left, center, and right pane lines
///
/// Details:
/// - Pre-calculates wrapping for each entry to ensure all panes have matching line counts
/// - Left pane: old versions with right padding (right-aligned)
/// - Center pane: arrows with spacing (centered)
/// - Right pane: new versions with tool label (left-aligned)
/// - Highlights the selected entry with cursor indicator
/// - All three panes have the same number of lines per entry for proper alignment
fn build_update_render_model(
    entries: &[(String, String, String)],
    left_width: u16,
    right_width: u16,
) -> UpdateRenderModel {
    let mut blocks = Vec::new();
    let mut entry_line_starts = Vec::new();
    let mut running_line: u16 = 0;

    for (name, old_version, new_version) in entries {
        let left_text = format!("{name} - {old_version}     ");
        let right_text = format!("     {name} - {new_version}");
        let left_wrapped = wrap_text_to_lines(&left_text, left_width);
        let right_wrapped = wrap_text_to_lines(&right_text, right_width);

        let left_count = u16::try_from(left_wrapped.len()).unwrap_or(u16::MAX);
        let right_count = u16::try_from(right_wrapped.len()).unwrap_or(u16::MAX);
        let row_render_height = left_count.max(right_count).max(1);

        entry_line_starts.push(running_line);
        blocks.push(UpdateEntryRenderBlock {
            name: name.clone(),
            left_wrapped,
            right_wrapped,
            row_render_height,
        });
        running_line = running_line.saturating_add(row_render_height);
    }

    UpdateRenderModel {
        blocks,
        total_lines: running_line,
        entry_line_starts,
    }
}

/// What: Build pane lines from the shared updates row model.
///
/// Inputs:
/// - `model`: Precomputed wrapped row model
/// - `th`: Theme for styling
/// - `selected`: Selected entry index
///
/// Output:
/// - Three pane line buffers with aligned per-entry row heights
///
/// Details:
/// - Uses per-entry lockstep appending to avoid global vector-length padding drift.
fn build_update_lines_from_model(
    model: &UpdateRenderModel,
    th: &Theme,
    focused: usize,
    selected_pkg_names: &std::collections::HashSet<String>,
) -> UpdateLines {
    let mut left_lines = Vec::new();
    let mut center_lines = Vec::new();
    let mut right_lines = Vec::new();

    let text_style = Style::default().fg(th.text);
    let focused_style = Style::default().fg(th.mauve).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().fg(th.green).add_modifier(Modifier::BOLD);
    let focused_selected_style = Style::default().fg(th.yellow).add_modifier(Modifier::BOLD);
    let center_style = Style::default().fg(th.mauve).add_modifier(Modifier::BOLD);

    for (idx, block) in model.blocks.iter().enumerate() {
        let is_focused = idx == focused;
        let is_selected = selected_pkg_names.contains(&block.name);
        let tool = get_install_tool(&block.name);
        let tool_color = match tool {
            "pacman" => th.green,
            "AUR" => th.yellow,
            _ => th.overlay1,
        };
        let last_right_line_idx = block.right_wrapped.len().saturating_sub(1);

        for line_idx in 0..usize::from(block.row_render_height) {
            let left_line = block
                .left_wrapped
                .get(line_idx)
                .cloned()
                .unwrap_or_default();
            if line_idx == 0 {
                let (marker, marker_style) = match (is_focused, is_selected) {
                    (true, true) => ("◉ ", focused_selected_style),
                    (true, false) => ("▶ ", focused_style),
                    (false, true) => ("● ", selected_style),
                    (false, false) => ("  ", text_style),
                };
                left_lines.push(Line::from(vec![
                    Span::styled(marker, marker_style),
                    Span::styled(left_line, text_style),
                ]));
            } else {
                left_lines.push(Line::from(Span::styled(left_line, text_style)));
            }

            if line_idx == 0 {
                center_lines.push(Line::from(Span::styled("     →     ", center_style)));
            } else {
                center_lines.push(Line::from(Span::styled("", center_style)));
            }

            let right_line = block
                .right_wrapped
                .get(line_idx)
                .cloned()
                .unwrap_or_default();
            if line_idx == last_right_line_idx {
                right_lines.push(Line::from(vec![
                    Span::styled(right_line, text_style),
                    Span::styled(" ", text_style),
                    Span::styled(
                        format!("[{tool}]"),
                        Style::default().fg(tool_color).add_modifier(Modifier::BOLD),
                    ),
                ]));
            } else {
                right_lines.push(Line::from(Span::styled(right_line, text_style)));
            }
        }
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
/// - Creates a paragraph with common styling (text color, background, scroll)
/// - Applies the specified alignment
/// - Wrapping is pre-calculated in `build_update_lines()`, so no wrap needed here
fn render_pane(
    f: &mut Frame,
    lines: Vec<Line<'static>>,
    chunk: Rect,
    alignment: Alignment,
    scroll: u16,
    th: &Theme,
) {
    // Render the paragraph with base background
    let para = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .alignment(alignment)
        .scroll((scroll, 0));
    f.render_widget(para, chunk);
}

/// What: Truncate a footer help line to fit a fixed terminal width.
///
/// Inputs:
/// - `content`: Footer text to render.
/// - `max_width`: Maximum display width in terminal cells.
///
/// Output:
/// - Returns the original text if it fits, otherwise a deterministic ellipsized variant.
///
/// Details:
/// - Uses Unicode display width for cell-accurate truncation.
/// - For extremely narrow widths (<= 3), returns only dots (`.`) up to available width.
/// - Keeps truncation stable across frames to avoid resize flicker.
fn truncate_footer_help_line(content: &str, max_width: u16) -> String {
    if max_width == 0 {
        return String::new();
    }

    let max_width_usize = usize::from(max_width);
    if content.width() <= max_width_usize {
        return content.to_string();
    }

    let ellipsis = "...";
    if max_width_usize <= ellipsis.len() {
        return ".".repeat(max_width_usize);
    }

    let target_width = max_width_usize.saturating_sub(ellipsis.len());
    let mut truncated = String::new();
    let mut used_width = 0usize;
    for ch in content.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used_width + ch_width > target_width {
            break;
        }
        truncated.push(ch);
        used_width += ch_width;
    }
    truncated.push_str(ellipsis);
    truncated
}

/// What: Render the available updates modal with scrollable list.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (records rects)
/// - `area`: Full screen area used to center the modal
/// - `entries`: Update entries to display (`name`, `old_version`, `new_version`)
/// - `scroll`: Scroll offset (lines) for the updates list
/// - `selected`: Index of the currently selected entry
///
/// Output:
/// - Draws the updates list with scroll support and selection highlighting
///
/// Details:
/// - Shows update entries with old version on left, arrow in center, new version on right
/// - Highlights the selected entry with background color
/// - Records rects for mouse interaction and scrolling
#[allow(clippy::too_many_arguments)] // Rendering needs full updates/filter context to keep layout and interaction mappings in sync.
pub fn render_updates(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    entries: &[(String, String, String)],
    filtered_indices: &[usize],
    scroll: u16,
    selected_original: usize,
    filter_active: bool,
    filter_query: &str,
    _filter_caret: usize,
    selected_pkg_names: &std::collections::HashSet<String>,
) {
    let th = theme();
    let rect = calculate_modal_rect(area);
    f.render_widget(Clear, rect);

    // Record outer rect for mouse hit-testing
    app.updates_modal_rect = Some((rect.x, rect.y, rect.width, rect.height));

    // Split into header/content/footer while always reserving one footer row.
    let inner_rect = Rect {
        x: rect.x + 1,
        y: rect.y + 1,
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    };

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Header + content area
            Constraint::Length(1), // Footer help line
        ])
        .split(inner_rect);
    let main_chunk = vertical_chunks[0];
    let footer_chunk = vertical_chunks[1];
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Heading + blank line
            Constraint::Min(0),    // Content area
        ])
        .split(main_chunk);
    let header_chunk = main_chunks[0];
    let content_chunk = main_chunks[1];

    // Render heading
    let heading_line = Line::from(Span::styled(
        i18n::t(app, "app.modals.updates_window.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    ));
    let heading_para =
        Paragraph::new(heading_line).style(Style::default().fg(th.text).bg(th.mantle));
    f.render_widget(heading_para, header_chunk);

    let display_indices: Vec<usize> = if filtered_indices.is_empty() {
        (0..entries.len()).collect()
    } else {
        filtered_indices.to_vec()
    };
    let display_entries: Vec<(String, String, String)> = display_indices
        .iter()
        .filter_map(|&idx| entries.get(idx).cloned())
        .collect();
    let selected_visible = display_indices
        .iter()
        .position(|&idx| idx == selected_original)
        .unwrap_or(0);

    if display_entries.is_empty() {
        let empty_message = if filter_active && !filter_query.trim().is_empty() {
            "No updates match filter".to_string()
        } else {
            i18n::t(app, "app.modals.updates_window.none")
        };
        let none_line = Line::from(Span::styled(
            empty_message,
            Style::default().fg(th.subtext1),
        ));
        let none_para = Paragraph::new(none_line).style(Style::default().fg(th.text).bg(th.mantle));
        f.render_widget(none_para, content_chunk);
    } else {
        // Split content area into three sections: left pane, center arrow, right pane
        let pane_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(48), // Left pane (old versions)
                Constraint::Length(11), // Center arrow with spacing (5 spaces + arrow + 5 spaces = 11 chars)
                Constraint::Percentage(48), // Right pane (new versions)
            ])
            .split(content_chunk);

        // Calculate pane widths for wrapping calculations
        let left_width = pane_chunks[0].width;
        let right_width = pane_chunks[2].width;

        let update_model = build_update_render_model(&display_entries, left_width, right_width);
        let update_lines =
            build_update_lines_from_model(&update_model, &th, selected_visible, selected_pkg_names);
        app.updates_modal_entry_line_starts = update_model.entry_line_starts;
        app.updates_modal_total_lines = update_model.total_lines;

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

    let mut footer_help =
        "↑/k ↓/j Move  PgUp/PgDn Page  / Filter  Space Toggle  a All  Enter Update  Esc Close";
    let filter_hint = if filter_active {
        if filter_query.is_empty() {
            "  |  /"
        } else {
            // Truncate only after full footer string is assembled.
            ""
        }
    } else {
        ""
    };
    let footer_owned;
    if filter_active && !filter_query.is_empty() {
        footer_owned = format!("{footer_help}  |  /{filter_query}");
        footer_help = footer_owned.as_str();
    } else if !filter_hint.is_empty() {
        footer_owned = format!("{footer_help}{filter_hint}");
        footer_help = footer_owned.as_str();
    }
    let footer_help = truncate_footer_help_line(footer_help, footer_chunk.width);
    let footer_para = Paragraph::new(Line::from(Span::styled(
        footer_help,
        Style::default().fg(th.subtext1).bg(th.mantle),
    )))
    .alignment(Alignment::Left);
    f.render_widget(footer_para, footer_chunk);

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

    // Record content rect for scroll handling (list area only).
    let list_rect = content_chunk;
    app.updates_modal_content_rect =
        Some((list_rect.x, list_rect.y, list_rect.width, list_rect.height));

    if display_entries.is_empty() {
        app.updates_modal_entry_line_starts.clear();
        app.updates_modal_total_lines = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    /// What: Ensure update row model tracks wrapped heights and entry line starts.
    ///
    /// Inputs:
    /// - Entries containing one short and one long/wrapped row.
    ///
    /// Output:
    /// - Produces monotonic entry start offsets with non-zero total line count.
    ///
    /// Details:
    /// - Guards the shared row mapping used by keyboard and mouse handlers.
    fn updates_row_model_builds_entry_line_starts() {
        let entries = vec![
            (
                "pkg-a".to_string(),
                "1.0.0".to_string(),
                "1.1.0".to_string(),
            ),
            (
                "very-long-package-name-that-wraps".to_string(),
                "1234567890.1234567890".to_string(),
                "1234567890.1234567899".to_string(),
            ),
        ];
        let model = build_update_render_model(&entries, 16, 16);
        assert_eq!(model.entry_line_starts.len(), 2);
        assert_eq!(model.entry_line_starts[0], 0);
        assert!(model.entry_line_starts[1] > model.entry_line_starts[0]);
        assert!(model.total_lines >= 2);
    }

    #[test]
    /// What: Ensure all pane outputs stay line-aligned for wrapped entries.
    ///
    /// Inputs:
    /// - Multi-entry row model with tight pane widths to force wrapping.
    ///
    /// Output:
    /// - Left/center/right vectors have exactly the same number of lines.
    ///
    /// Details:
    /// - Prevents global-padding regressions in `build_update_lines_from_model`.
    fn updates_panes_remain_aligned_after_wrapping() {
        let entries = vec![
            (
                "first-entry-name".to_string(),
                "old-version-very-long".to_string(),
                "new-version-very-long".to_string(),
            ),
            (
                "pkg-b".to_string(),
                "2.0.0".to_string(),
                "2.1.0".to_string(),
            ),
        ];
        let model = build_update_render_model(&entries, 14, 14);
        let lines =
            build_update_lines_from_model(&model, &theme(), 0, &std::collections::HashSet::new());
        assert_eq!(lines.left.len(), lines.center.len());
        assert_eq!(lines.center.len(), lines.right.len());
    }

    #[test]
    /// What: Ensure footer hint line remains unchanged when width is sufficient.
    ///
    /// Inputs:
    /// - Footer text and a wide terminal width.
    ///
    /// Output:
    /// - Returns the exact original footer string.
    ///
    /// Details:
    /// - Guards against accidental truncation in normal terminal sizes.
    fn footer_help_line_keeps_full_text_when_wide() {
        let help =
            "↑/k ↓/j Move  PgUp/PgDn Page  / Filter  Space Toggle  a All  Enter Update  Esc Close";
        let rendered = truncate_footer_help_line(help, 120);
        assert_eq!(rendered, help);
    }

    #[test]
    /// What: Ensure footer hint truncation is deterministic and ellipsized when narrow.
    ///
    /// Inputs:
    /// - Footer text and a narrow terminal width.
    ///
    /// Output:
    /// - Returns a shortened string ending with `...` and fitting within width.
    ///
    /// Details:
    /// - Protects small-terminal rendering from overflow and unstable truncation.
    fn footer_help_line_truncates_with_ellipsis_when_narrow() {
        let help =
            "↑/k ↓/j Move  PgUp/PgDn Page  / Filter  Space Toggle  a All  Enter Update  Esc Close";
        let rendered = truncate_footer_help_line(help, 20);
        assert!(rendered.ends_with("..."));
        assert!(rendered.width() <= 20);
    }

    #[test]
    /// What: Ensure tiny footer widths still produce valid output.
    ///
    /// Inputs:
    /// - Footer text and tiny widths in the 0..=3 range.
    ///
    /// Output:
    /// - Returns empty output for width 0 and dot-only placeholders for 1..=3.
    ///
    /// Details:
    /// - Avoids panics and preserves deterministic rendering in very small terminals.
    fn footer_help_line_handles_tiny_widths() {
        let help = "↑/k ↓/j Move";
        assert_eq!(truncate_footer_help_line(help, 0), "");
        assert_eq!(truncate_footer_help_line(help, 1), ".");
        assert_eq!(truncate_footer_help_line(help, 2), "..");
        assert_eq!(truncate_footer_help_line(help, 3), "...");
    }

    #[test]
    /// What: Ensure updates modal records content rect height from actual viewport size.
    ///
    /// Inputs:
    /// - Two renders with different terminal heights and the same updates data.
    ///
    /// Output:
    /// - `updates_modal_content_rect` height is larger for the larger viewport.
    ///
    /// Details:
    /// - Guards against regressions back to fixed visible-line assumptions.
    fn render_updates_uses_viewport_height_for_content_rect() {
        let entries = vec![
            (
                "pkg-a".to_string(),
                "1.0.0".to_string(),
                "1.1.0".to_string(),
            ),
            (
                "pkg-b".to_string(),
                "2.0.0".to_string(),
                "2.1.0".to_string(),
            ),
        ];

        let mut app_small = AppState::default();
        let backend_small = TestBackend::new(100, 14);
        let mut terminal_small =
            Terminal::new(backend_small).expect("failed to create small test terminal");
        terminal_small
            .draw(|f| {
                let area = f.area();
                render_updates(
                    f,
                    &mut app_small,
                    area,
                    &entries,
                    &[0, 1],
                    0,
                    0,
                    false,
                    "",
                    0,
                    &std::collections::HashSet::new(),
                );
            })
            .expect("failed to draw small updates modal");
        let small_height = app_small
            .updates_modal_content_rect
            .map_or(0, |(_, _, _, h)| h);

        let mut app_large = AppState::default();
        let backend_large = TestBackend::new(100, 32);
        let mut terminal_large =
            Terminal::new(backend_large).expect("failed to create large test terminal");
        terminal_large
            .draw(|f| {
                let area = f.area();
                render_updates(
                    f,
                    &mut app_large,
                    area,
                    &entries,
                    &[0, 1],
                    0,
                    0,
                    false,
                    "",
                    0,
                    &std::collections::HashSet::new(),
                );
            })
            .expect("failed to draw large updates modal");
        let large_height = app_large
            .updates_modal_content_rect
            .map_or(0, |(_, _, _, h)| h);

        assert!(small_height > 0);
        assert!(large_height > small_height);
    }

    #[test]
    /// What: Ensure filtered-empty state message renders when no rows match.
    ///
    /// Inputs:
    /// - Active filter query with no matching update rows.
    ///
    /// Output:
    /// - Buffer contains the `No updates match filter` message.
    ///
    /// Details:
    /// - Confirms UX feedback in filter mode when result set is empty.
    fn render_updates_shows_filtered_empty_state_message() {
        let entries = vec![(
            "pkg-a".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
        )];
        let mut app = AppState::default();
        let backend = TestBackend::new(100, 20);
        let mut terminal = Terminal::new(backend).expect("failed to create test terminal");

        terminal
            .draw(|f| {
                let area = f.area();
                render_updates(
                    f,
                    &mut app,
                    area,
                    &entries,
                    &[99],
                    0,
                    0,
                    true,
                    "no-match",
                    8,
                    &std::collections::HashSet::new(),
                );
            })
            .expect("failed to draw filtered empty-state modal");

        let buffer = terminal.backend().buffer();
        let mut all_text = String::new();
        for y in 0..buffer.area.height {
            for x in 0..buffer.area.width {
                all_text.push_str(buffer[(x, y)].symbol());
            }
            all_text.push('\n');
        }

        assert!(
            all_text.contains("No updates match filter")
                || all_text.contains("No updates available")
        );
    }

    #[test]
    /// What: Ensure focused-only and selected-only rows have distinct markers.
    fn updates_markers_distinguish_focus_and_selection() {
        let entries = vec![
            ("alpha".to_string(), "1.0".to_string(), "2.0".to_string()),
            ("beta".to_string(), "1.0".to_string(), "2.0".to_string()),
        ];
        let model = build_update_render_model(&entries, 24, 24);
        let selected_pkg_names = std::collections::HashSet::from(["beta".to_string()]);
        let lines = build_update_lines_from_model(&model, &theme(), 0, &selected_pkg_names);

        let first_marker = lines.left[0].spans[0].content.as_ref();
        assert_eq!(first_marker, "▶ ");

        let second_row_start = usize::from(model.entry_line_starts[1]);
        let second_marker = lines.left[second_row_start].spans[0].content.as_ref();
        assert_eq!(second_marker, "● ");
    }

    #[test]
    /// What: Ensure focused+selected rows use a dedicated marker.
    fn updates_markers_show_focused_selected_marker() {
        let entries = vec![("alpha".to_string(), "1.0".to_string(), "2.0".to_string())];
        let model = build_update_render_model(&entries, 24, 24);
        let selected_pkg_names = std::collections::HashSet::from(["alpha".to_string()]);
        let lines = build_update_lines_from_model(&model, &theme(), 0, &selected_pkg_names);
        let marker = lines.left[0].spans[0].content.as_ref();
        assert_eq!(marker, "◉ ");
    }
}
