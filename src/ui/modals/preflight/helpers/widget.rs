use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::theme::theme;

/// What: Builder for creating styled Paragraph widgets for preflight modal.
///
/// Inputs: None (struct definition).
///
/// Output: None (struct definition).
///
/// Details: Provides a fluent interface for building Paragraph widgets with consistent styling.
pub struct ParagraphBuilder {
    /// Lines of text to display.
    lines: Vec<Line<'static>>,
    /// Optional title for the paragraph.
    title: Option<String>,
    /// Scroll offset (x, y).
    scroll_offset: (u16, u16),
    /// Text color.
    text_color: ratatui::style::Color,
    /// Background color.
    bg_color: ratatui::style::Color,
    /// Border color.
    border_color: ratatui::style::Color,
}

impl ParagraphBuilder {
    /// What: Create a new `ParagraphBuilder` with default values.
    ///
    /// Inputs: None.
    ///
    /// Output: Returns a new `ParagraphBuilder` instance.
    ///
    /// Details: Initializes with empty lines and default colors from theme.
    pub fn new() -> Self {
        let th = theme();
        Self {
            lines: Vec::new(),
            title: None,
            scroll_offset: (0, 0),
            text_color: th.text,
            bg_color: th.crust,
            border_color: th.lavender,
        }
    }

    /// What: Set the content lines for the paragraph.
    ///
    /// Inputs:
    /// - `lines`: Vector of lines to display.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: Replaces any existing lines with the provided lines.
    pub fn with_lines(mut self, lines: Vec<Line<'static>>) -> Self {
        self.lines = lines;
        self
    }

    /// What: Set the title for the paragraph block.
    ///
    /// Inputs:
    /// - `title`: Title string to display in the block border.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: The title will be styled with bold modifier and border color.
    pub fn with_title(mut self, title: String) -> Self {
        self.title = Some(title);
        self
    }

    /// What: Set the scroll offset for the paragraph.
    ///
    /// Inputs:
    /// - `offset`: Tuple of (vertical, horizontal) scroll offset.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: Used for scrolling content within the paragraph widget.
    pub const fn with_scroll(mut self, offset: (u16, u16)) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// What: Set the text color.
    ///
    /// Inputs:
    /// - `color`: Text color to use.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: Overrides default text color from theme.
    pub const fn with_text_color(mut self, color: ratatui::style::Color) -> Self {
        self.text_color = color;
        self
    }

    /// What: Set the background color.
    ///
    /// Inputs:
    /// - `color`: Background color to use.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: Overrides default background color from theme.
    pub const fn with_bg_color(mut self, color: ratatui::style::Color) -> Self {
        self.bg_color = color;
        self
    }

    /// What: Set the border color.
    ///
    /// Inputs:
    /// - `color`: Border color to use.
    ///
    /// Output: Returns self for method chaining.
    ///
    /// Details: Overrides default border color from theme.
    pub const fn with_border_color(mut self, color: ratatui::style::Color) -> Self {
        self.border_color = color;
        self
    }

    /// What: Build the final Paragraph widget.
    ///
    /// Inputs: None.
    ///
    /// Output: Returns a fully configured Paragraph widget.
    ///
    /// Details:
    /// - Applies all styling, borders, title, and scroll settings.
    /// - Uses double border type and left/top/right borders only.
    pub fn build(self) -> Paragraph<'static> {
        let block = if let Some(title) = self.title {
            Block::default()
                .title(Span::styled(
                    title,
                    Style::default()
                        .fg(self.border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(self.border_color))
                .style(Style::default().bg(self.bg_color))
        } else {
            Block::default()
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(self.border_color))
                .style(Style::default().bg(self.bg_color))
        };

        Paragraph::new(self.lines)
            .style(Style::default().fg(self.text_color).bg(self.bg_color))
            .wrap(Wrap { trim: true })
            .scroll(self.scroll_offset)
            .block(block)
    }
}

impl Default for ParagraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
