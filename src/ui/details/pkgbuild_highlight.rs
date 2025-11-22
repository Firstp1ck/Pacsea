//! Syntax highlighting for PKGBUILD files using syntect.
//!
//! This module provides functions to highlight PKGBUILD content using syntect
//! and convert the highlighted spans to ratatui-compatible Spans.

use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::theme::Theme;

/// Lazy-loaded syntax set for bash/shell syntax highlighting.
static SYNTAX_SET: std::sync::OnceLock<SyntaxSet> = std::sync::OnceLock::new();

/// Lazy-loaded theme set for syntax highlighting.
static THEME_SET: std::sync::OnceLock<ThemeSet> = std::sync::OnceLock::new();

/// What: Initialize the syntax set and theme set for syntax highlighting.
///
/// Output:
/// - Initializes static syntax and theme sets if not already initialized.
///
/// Details:
/// - Uses `OnceLock` to ensure initialization happens only once.
/// - Loads bash syntax definition and a dark theme suitable for TUI.
fn init_syntax() {
    SYNTAX_SET.get_or_init(|| {
        // Load syntax definitions (includes bash)
        SyntaxSet::load_defaults_newlines()
    });
    THEME_SET.get_or_init(|| {
        // Load theme set (includes various themes)
        ThemeSet::load_defaults()
    });
}

/// What: Map syntect color to theme colors based on color characteristics.
///
/// Inputs:
/// - `sc`: Syntect color (RGBA) - syntect already assigns colors based on scope matching
/// - `th`: Application theme for color mapping
///
/// Output:
/// - `ratatui::style::Color` mapped from theme colors
///
/// Details:
/// - Syntect already does scope-based highlighting internally and assigns appropriate colors.
/// - We map syntect's colors to our theme colors based on color characteristics.
/// - Only handles edge cases (too dark/light colors).
fn map_syntect_color(sc: syntect::highlighting::Color, th: &Theme) -> Color {
    let r = sc.r;
    let g = sc.g;
    let b = sc.b;

    let max_rgb = r.max(g).max(b);
    let min_rgb = r.min(g).min(b);
    let avg_rgb = (r as u16 + g as u16 + b as u16) / 3;

    // If color is very dark (close to black), use theme text color
    if max_rgb < 30 {
        return th.text;
    }

    // If color is too light (close to white), use theme text color
    if r > 200 && g > 200 && b > 200 {
        return th.text;
    }

    // Map syntect's scope-based colors to theme colors based on color characteristics
    // Check color characteristics first (more specific)

    // Green-ish colors -> green (strings, comments with green tint)
    if g > r + 20 && g > b + 20 && g > 60 {
        return th.green;
    }

    // Purple/magenta-ish colors -> mauve (keywords, control flow)
    if ((r > 120 && b > 120 && g < r.min(b) + 40) || (r > 150 && b > 150)) && max_rgb > 60 {
        return th.mauve;
    }

    // Blue-ish colors -> sapphire (variables, functions, commands)
    if b > r + 20 && b > g + 20 && b > 60 {
        return th.sapphire;
    }

    // Yellow/orange-ish colors -> yellow (numbers, constants)
    if r > 150 && g > 120 && b < 120 && max_rgb > 60 {
        return th.yellow;
    }

    // Red-ish colors -> red (errors, warnings)
    if r > g + 30 && r > b + 30 && r > 60 {
        return th.red;
    }

    // Grey-ish colors (medium grey) -> subtext0 (comments, muted text)
    if max_rgb > 40 && max_rgb < 200 && (max_rgb - min_rgb) < 50 {
        return th.subtext0;
    }

    // Dark colors that don't match specific characteristics
    // Map to appropriate theme colors based on slight color hints
    if max_rgb < 100 {
        // Very dark colors - try to detect slight color hints
        if b > r + 10 && b > g + 10 {
            return th.sapphire; // Slight blue tint -> sapphire
        }
        if g > r + 10 && g > b + 10 {
            return th.green; // Slight green tint -> green
        }
        if r > g + 10 && r > b + 10 && b < 50 {
            return th.yellow; // Slight yellow/orange tint -> yellow
        }
        if r > 80 && b > 80 && g < 60 {
            return th.mauve; // Slight purple tint -> mauve
        }
        // Default dark colors -> subtext0 (muted)
        return th.subtext0;
    }

    // Medium brightness colors that don't match specific characteristics
    // Default to sapphire for commands/functions
    if avg_rgb > 100 && avg_rgb < 180 {
        return th.sapphire;
    }

    // Use syntect color directly for bright, well-defined colors
    Color::Rgb(r, g, b)
}

/// What: Highlight PKGBUILD text using syntect and convert to ratatui Spans.
///
/// Inputs:
/// - `text`: PKGBUILD content to highlight
/// - `th`: Application theme for color mapping
///
/// Output:
/// - `Vec<ratatui::text::Line>` containing highlighted lines ready for rendering
///
/// Details:
/// - Uses bash syntax definition for highlighting.
/// - Falls back to plain text if syntax highlighting fails.
/// - Maps syntect colors to theme colors for consistency.
pub fn highlight_pkgbuild(text: &str, th: &Theme) -> Vec<ratatui::text::Line<'static>> {
    init_syntax();

    let syntax_set = SYNTAX_SET.get().expect("syntax set should be initialized");
    let theme_set = THEME_SET.get().expect("theme set should be initialized");

    // Try to find bash syntax, fallback to plain text if not found
    let syntax = syntax_set
        .find_syntax_by_extension("sh")
        .or_else(|| syntax_set.find_syntax_by_extension("bash"))
        .or_else(|| syntax_set.find_syntax_by_name("Bash"))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());

    // Use a dark theme suitable for TUI (InspiredGitHub or similar)
    let theme = theme_set
        .themes
        .get("InspiredGitHub")
        .or_else(|| theme_set.themes.values().next())
        .expect("at least one theme should be available");

    // Use HighlightLines - syntect already does scope-based highlighting internally
    // The colors it assigns are based on scope matching, we just map them to our theme
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines = Vec::new();

    for line in LinesWithEndings::from(text) {
        let mut spans = Vec::new();

        match highlighter.highlight_line(line, syntax_set) {
            Ok(highlighted_line) => {
                for (style, text) in highlighted_line {
                    // Syntect's colors are already scope-based - map them to our theme colors
                    let color = map_syntect_color(style.foreground, th);
                    let mut ratatui_style = Style::default().fg(color);

                    // Apply modifiers based on syntect style
                    if style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::BOLD)
                    {
                        ratatui_style = ratatui_style.add_modifier(Modifier::BOLD);
                    }
                    if style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::ITALIC)
                    {
                        ratatui_style = ratatui_style.add_modifier(Modifier::ITALIC);
                    }
                    if style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::UNDERLINE)
                    {
                        ratatui_style = ratatui_style.add_modifier(Modifier::UNDERLINED);
                    }

                    spans.push(ratatui::text::Span::styled(text.to_string(), ratatui_style));
                }
            }
            Err(_) => {
                // Fallback to plain text if highlighting fails
                spans.push(ratatui::text::Span::styled(
                    line.to_string(),
                    Style::default().fg(th.text),
                ));
            }
        }

        lines.push(ratatui::text::Line::from(spans));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme;

    #[test]
    fn test_highlight_pkgbuild_basic() {
        let th = theme();
        let pkgbuild = r"pkgname=test
pkgver=1.0.0
# This is a comment
depends=('bash')
";

        let lines = highlight_pkgbuild(pkgbuild, &th);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_highlight_pkgbuild_empty() {
        let th = theme();
        let lines = highlight_pkgbuild("", &th);
        assert!(lines.is_empty() || lines.len() == 1);
    }
}
