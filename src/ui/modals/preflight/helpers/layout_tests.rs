//! Unit tests for `calculate_modal_layout` function.

use super::layout;

/// What: Test `calculate_modal_layout` with standard area.
///
/// Inputs:
/// - `area`: Standard terminal area (120x40)
///
/// Output:
/// - Returns properly sized and centered modal rects
///
/// Details:
/// - Verifies that layout calculation works for standard sizes.
#[test]
fn test_calculate_modal_layout_standard() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 40,
    };

    let (modal_rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);

    // Modal should be centered and within max size
    assert!(modal_rect.width <= 96);
    assert!(modal_rect.height <= 32);
    assert!(modal_rect.x > 0); // Centered
    assert!(modal_rect.y > 0); // Centered

    // Content and keybinds should fit within modal
    assert!(content_rect.width <= modal_rect.width);
    assert!(content_rect.height + keybinds_rect.height <= modal_rect.height);
    assert_eq!(keybinds_rect.height, 4);
}

/// What: Test `calculate_modal_layout` with small area.
///
/// Inputs:
/// - `area`: Small terminal area (50x20)
///
/// Output:
/// - Returns modal that fits within area
///
/// Details:
/// - Verifies that layout calculation handles small areas correctly.
#[test]
fn test_calculate_modal_layout_small() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 20,
    };

    let (modal_rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);

    // Modal should fit within area (with margins)
    assert!(modal_rect.width <= area.width);
    assert!(modal_rect.height <= area.height);
    assert!(content_rect.width <= modal_rect.width);
    assert!(content_rect.height + keybinds_rect.height <= modal_rect.height);
}

/// What: Test `calculate_modal_layout` with maximum constraints.
///
/// Inputs:
/// - `area`: Very large terminal area (200x100)
///
/// Output:
/// - Returns modal constrained to max 96x32
///
/// Details:
/// - Verifies that maximum size constraints are enforced.
#[test]
fn test_calculate_modal_layout_max_constraints() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 0,
        y: 0,
        width: 200,
        height: 100,
    };

    let (modal_rect, _, _) = layout::calculate_modal_layout(area);

    // Modal should be constrained to max size
    assert_eq!(modal_rect.width, 96);
    assert_eq!(modal_rect.height, 32);
}

/// What: Test `calculate_modal_layout` with offset area.
///
/// Inputs:
/// - `area`: Area with non-zero offset (x=10, y=5)
///
/// Output:
/// - Returns modal centered within offset area
///
/// Details:
/// - Verifies that layout calculation handles offset areas correctly.
#[test]
fn test_calculate_modal_layout_offset() {
    use ratatui::prelude::Rect;
    let area = Rect {
        x: 10,
        y: 5,
        width: 120,
        height: 40,
    };

    let (modal_rect, _, _) = layout::calculate_modal_layout(area);

    // Modal should be centered within offset area
    assert!(modal_rect.x >= area.x);
    assert!(modal_rect.y >= area.y);
    assert!(modal_rect.x + modal_rect.width <= area.x + area.width);
    assert!(modal_rect.y + modal_rect.height <= area.y + area.height);
}
