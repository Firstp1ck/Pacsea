//! Color palette definitions for Pacsea's TUI.
//!
//! This module exposes a small, opinionated theme used throughout the user
//! interface. Colors are grouped into neutrals (base/mantle/crust/surfaces),
//! overlays/subtexts, and accents for highlighting and semantic states.
use ratatui::style::Color;

/// Application theme palette used by rendering code.
///
/// All colors are provided as [`ratatui::style::Color`] and are suitable for
/// direct use with widgets and styles.
pub struct Theme {
    /// Primary background color for the canvas.
    pub base: Color,
    /// Slightly lighter background layer used behind panels.
    pub mantle: Color,
    /// Darkest background shade for deep contrast areas.
    pub crust: Color,
    /// Subtle surface color for component backgrounds (level 1).
    pub surface1: Color,
    /// Subtle surface color for component backgrounds (level 2).
    pub surface2: Color,
    /// Muted overlay line/border color (primary).
    pub overlay1: Color,
    /// Muted overlay line/border color (secondary).
    pub overlay2: Color,
    /// Primary foreground text color.
    pub text: Color,
    /// Secondary text for less prominent content.
    pub subtext0: Color,
    /// Tertiary text for captions and low-emphasis content.
    pub subtext1: Color,
    /// Accent color commonly used for selection and interactive highlights.
    pub sapphire: Color,
    /// Accent color for emphasized headings or selections.
    pub mauve: Color,
    /// Success/positive state color.
    pub green: Color,
    /// Warning/attention state color.
    pub yellow: Color,
    /// Error/danger state color.
    pub red: Color,
    /// Accent color for subtle emphasis and borders.
    pub lavender: Color,
}

/// Construct a [`Color::Rgb`] from an 8-bit RGB triplet.
///
/// This is a small helper to keep the palette definition concise.
fn hex(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

/// Return the application's default theme palette.
///
/// Example
///
/// ```rust
/// use pacsea::theme::theme;
/// let t = theme();
/// let primary_text = t.text;
/// ```
pub fn theme() -> Theme {
    Theme {
        base: hex((0x1e, 0x1e, 0x2e)),
        mantle: hex((0x18, 0x18, 0x25)),
        crust: hex((0x11, 0x11, 0x1b)),
        surface1: hex((0x45, 0x47, 0x5a)),
        surface2: hex((0x58, 0x5b, 0x70)),
        overlay1: hex((0x7f, 0x84, 0x9c)),
        overlay2: hex((0x93, 0x99, 0xb2)),
        text: hex((0xcd, 0xd6, 0xf4)),
        subtext0: hex((0xa6, 0xad, 0xc8)),
        subtext1: hex((0xba, 0xc2, 0xde)),
        sapphire: hex((0x74, 0xc7, 0xec)),
        mauve: hex((0xcb, 0xa6, 0xf7)),
        green: hex((0xa6, 0xe3, 0xa1)),
        yellow: hex((0xf9, 0xe2, 0xaf)),
        red: hex((0xf3, 0x8b, 0xa8)),
        lavender: hex((0xb4, 0xbe, 0xfe)),
    }
}
