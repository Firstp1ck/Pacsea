//! Syntax highlighting for PKGBUILD files using syntect with incremental reuse.
//!
//! This module provides functions to highlight PKGBUILD content using syntect
//! and convert the highlighted spans to ratatui-compatible Spans, reusing cached
//! results for unchanged prefixes to reduce re-render latency.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
};
use std::sync::{Mutex, OnceLock};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};

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
    let avg_rgb = (u16::from(r) + u16::from(g) + u16::from(b)) / 3;

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

#[derive(Clone)]
struct PkgbHighlightCache {
    text: String,
    lines: Vec<Line<'static>>,
}

/// Global cache for PKGBUILD highlighting to enable dirty-region reuse across renders.
static PKGB_CACHE: OnceLock<Mutex<Option<PkgbHighlightCache>>> = OnceLock::new();

fn cache_lock() -> &'static Mutex<Option<PkgbHighlightCache>> {
    PKGB_CACHE.get_or_init(|| Mutex::new(None))
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
/// - Falls back to plain text if highlighting fails.
/// - Reuses cached highlighted prefixes and recomputes from the first differing line to preserve
///   syntect state; falls back to plain text per-line on parse errors.
pub fn highlight_pkgbuild(text: &str, th: &Theme) -> Vec<Line<'static>> {
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

    // Fast path: identical text to cache
    if let Ok(cache_guard) = cache_lock().lock()
        && let Some(cache) = cache_guard.as_ref()
        && cache.text == text
    {
        return cache.lines.clone();
    }

    // Snapshot old cache (if any) and release lock for work
    let old_cache = cache_lock().lock().ok().and_then(|c| c.clone());

    let new_lines_raw: Vec<String> = LinesWithEndings::from(text).map(str::to_string).collect();
    let old_lines_raw: Vec<String> = old_cache
        .as_ref()
        .map(|c| {
            LinesWithEndings::from(c.text.as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let prefix_len = new_lines_raw
        .iter()
        .zip(&old_lines_raw)
        .take_while(|(a, b)| a == b)
        .count();

    // Syntect highlighter (stateful; must process lines in order)
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut highlighted_lines: Vec<Line<'static>> = Vec::with_capacity(new_lines_raw.len());

    // Replay unchanged prefix to advance syntect state; reuse cached spans when present
    for line in new_lines_raw.iter().take(prefix_len) {
        match highlighter.highlight_line(line, syntax_set) {
            Ok(highlighted_line) => {
                if let Some(cache) = old_cache.as_ref()
                    && cache.lines.len() > highlighted_lines.len()
                {
                    highlighted_lines.push(cache.lines[highlighted_lines.len()].clone());
                } else {
                    highlighted_lines.push(to_ratatui_line(&highlighted_line, th, line));
                }
            }
            Err(_) => highlighted_lines.push(Line::from(line.clone())),
        }
    }

    // Highlight remaining (changed) region onward
    for line in new_lines_raw.iter().skip(prefix_len) {
        match highlighter.highlight_line(line, syntax_set) {
            Ok(highlighted_line) => {
                highlighted_lines.push(to_ratatui_line(&highlighted_line, th, line));
            }
            Err(_) => highlighted_lines.push(Line::from(line.clone())),
        }
    }

    // Update cache
    if let Ok(mut cache_guard) = cache_lock().lock() {
        *cache_guard = Some(PkgbHighlightCache {
            text: text.to_string(),
            lines: highlighted_lines.clone(),
        });
    }

    highlighted_lines
}

fn to_ratatui_line(
    highlighted_line: &[(syntect::highlighting::Style, &str)],
    th: &Theme,
    fallback: &str,
) -> Line<'static> {
    let mut spans = Vec::new();
    for (style, text) in highlighted_line {
        let color = map_syntect_color(style.foreground, th);
        let mut ratatui_style = Style::default().fg(color);

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

        spans.push(ratatui::text::Span::styled(
            (*text).to_string(),
            ratatui_style,
        ));
    }

    if spans.is_empty() {
        spans.push(ratatui::text::Span::raw(fallback.to_string()));
    }

    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::theme;

    #[test]
    /// What: Ensure identical text hits the cache and returns lines without recomputing content.
    ///
    /// Inputs:
    /// - Two invocations with the same PKGBUILD text.
    ///
    /// Output:
    /// - Highlighted lines are equal across calls, demonstrating cache reuse.
    fn highlight_pkgbuild_cache_hit() {
        reset_cache();
        let th = theme();
        let pkgbuild = "pkgname=test\npkgver=1\n";
        let first = highlight_pkgbuild(pkgbuild, &th);
        let second = highlight_pkgbuild(pkgbuild, &th);
        assert_eq!(first.len(), second.len());
        assert_eq!(first[0].to_string(), second[0].to_string());
    }

    #[test]
    /// What: Ensure changes late in the file avoid re-highlighting unchanged prefixes.
    ///
    /// Inputs:
    /// - Initial text, then an appended line.
    ///
    /// Output:
    /// - Highlighting succeeds and yields expected total line count.
    fn highlight_pkgbuild_incremental_appends() {
        reset_cache();
        let th = theme();
        let base = "pkgname=test\npkgver=1\n";
        let appended = "pkgname=test\npkgver=1\n# comment\n";
        let first = highlight_pkgbuild(base, &th);
        let second = highlight_pkgbuild(appended, &th);
        assert_eq!(second.len(), 3);
        assert_eq!(first.len(), 2);
    }

    #[test]
    /// What: Ensure empty text returns an empty or single-line result.
    fn test_highlight_pkgbuild_empty() {
        reset_cache();
        let th = theme();
        let lines = highlight_pkgbuild("", &th);
        assert!(lines.is_empty() || lines.len() == 1);
    }

    #[test]
    fn test_highlight_pkgbuild_basic() {
        reset_cache();
        let th = theme();
        let pkgbuild = r"pkgname=test
pkgver=1.0.0
# This is a comment
depends=('bash')
";

        let lines = highlight_pkgbuild(pkgbuild, &th);
        assert!(!lines.is_empty());
    }

    fn reset_cache() {
        if let Ok(mut guard) = cache_lock().lock() {
            *guard = None;
        }
    }
}
