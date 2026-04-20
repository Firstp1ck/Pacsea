use ratatui::style::Color;

use crate::state::PreflightAction;
use crate::theme::Theme;

/// Dark orange for remove preflight borders and titles (distinct from install/downgrade lavender).
const PREFLIGHT_REMOVE_BORDER_RGB: Color = Color::Rgb(178, 88, 22);

/// What: Border (and block title) color for preflight / preflight-exec modals by action.
///
/// Inputs:
/// - `action`: Install, remove, or downgrade
/// - `th`: Active theme (used for non-remove actions)
///
/// Output:
/// - `Color` for double borders and title text on the main preflight window.
///
/// Details:
/// - Remove uses a fixed dark orange so removal is visually obvious across Catppuccin themes.
#[allow(clippy::missing_const_for_fn)] // reads dynamic theme fields for non-remove actions
pub fn preflight_modal_border_color(action: PreflightAction, th: &Theme) -> Color {
    match action {
        PreflightAction::Remove => PREFLIGHT_REMOVE_BORDER_RGB,
        PreflightAction::Install | PreflightAction::Downgrade => th.lavender,
    }
}
