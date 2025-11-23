use crate::state::{AppState, Modal, PreflightTab};

/// What: Calculate scroll offset for the preflight modal content.
///
/// Inputs:
/// - `app`: Application state containing modal scroll information.
/// - `tab`: Current active tab.
///
/// Output:
/// - Returns tuple of (`vertical_offset`, `horizontal_offset`) for scrolling.
///
/// Details:
/// - Only applies scroll offset for Summary tab (mouse scrolling only).
/// - Returns (0, 0) for all other tabs.
pub const fn calculate_scroll_offset(app: &AppState, tab: PreflightTab) -> (u16, u16) {
    if !matches!(tab, PreflightTab::Summary) {
        return (0, 0);
    }

    if let Modal::Preflight { summary_scroll, .. } = &app.modal {
        (*summary_scroll, 0)
    } else {
        (0, 0)
    }
}
