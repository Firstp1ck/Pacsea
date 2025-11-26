use ratatui::prelude::Rect;

use crate::state::{AppState, Focus};
use crate::theme::KeyChord;
use std::fmt::Write;

/// What: Calculate the number of rows required for the footer/keybinds section.
///
/// Inputs:
/// - `app`: Application state with keymap, focus, and footer visibility flags
/// - `bottom_container`: Rect available for the details pane and footer
///
/// Output:
/// - Row count (including padding) reserved for footer content when rendered.
///
/// Details:
/// - Accounts for baseline GLOBALS + focused pane lines and optionally Search Normal Mode help
///   while respecting available width in `bottom_container`.
pub fn calculate_footer_height(app: &AppState, bottom_container: Rect) -> u16 {
    // Reserve footer height: baseline lines + optional Normal Mode line
    // Baseline: always 2 lines visible by default: GLOBALS + currently focused pane
    let baseline_lines: u16 = 2;

    // Compute adaptive extra rows for Search Normal Mode footer based on available width
    let km = &app.keymap;
    let footer_w: u16 = bottom_container.width.saturating_sub(2);
    let nm_rows: u16 = if matches!(app.focus, Focus::Search) && app.search_normal_mode {
        // Build the same labels used in the footer
        let toggle_label = km
            .search_normal_toggle
            .first()
            .map_or_else(|| "Esc".to_string(), KeyChord::label);
        let insert_label = km
            .search_normal_insert
            .first()
            .map_or_else(|| "i".to_string(), KeyChord::label);
        let left_label = km
            .search_normal_select_left
            .first()
            .map_or_else(|| "h".to_string(), KeyChord::label);
        let right_label = km
            .search_normal_select_right
            .first()
            .map_or_else(|| "l".to_string(), KeyChord::label);
        let delete_label = km
            .search_normal_delete
            .first()
            .map_or_else(|| "d".to_string(), KeyChord::label);
        let clear_label = km
            .search_normal_clear
            .first()
            .map_or_else(|| "Shift+Del".to_string(), KeyChord::label);

        let line1 = format!(
            "Normal Mode (Focused Search Window):  [{toggle_label}/{insert_label}] Insert Mode, [j / k] move, [Ctrl+d / Ctrl+u] page, [{left_label} / {right_label}] Select text, [{delete_label}] Delete text, [{clear_label}] Clear input"
        );
        // Menus and Import/Export on an additional line when present
        let mut line2 = String::new();
        if !km.config_menu_toggle.is_empty()
            || !km.options_menu_toggle.is_empty()
            || !km.panels_menu_toggle.is_empty()
            || (!app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty()))
        {
            // Menus
            if !km.config_menu_toggle.is_empty()
                || !km.options_menu_toggle.is_empty()
                || !km.panels_menu_toggle.is_empty()
            {
                line2.push_str("  •  Open Menus: ");
                if let Some(k) = km.config_menu_toggle.first() {
                    let _ = write!(line2, "[{}] Config", k.label());
                }
                if let Some(k) = km.options_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    let _ = write!(line2, "[{}] Options", k.label());
                }
                if let Some(k) = km.panels_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    let _ = write!(line2, "[{}] Panels", k.label());
                }
            }
            // Import / Export
            if !app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
            {
                line2.push_str("  •  ");
                if let Some(k) = km.search_normal_import.first() {
                    let _ = write!(line2, "[{}] Import", k.label());
                    if let Some(k2) = km.search_normal_export.first() {
                        let _ = write!(line2, ", [{}] Export", k2.label());
                    }
                } else if let Some(k) = km.search_normal_export.first() {
                    let _ = write!(line2, "[{}] Export", k.label());
                }
            }
        }
        let w = if footer_w == 0 { 1 } else { footer_w };
        let rows1 = (u16::try_from(line1.len()).unwrap_or(u16::MAX).div_ceil(w)).max(1);
        let rows2 = if line2.is_empty() {
            0
        } else {
            (u16::try_from(line2.len()).unwrap_or(u16::MAX).div_ceil(w)).max(1)
        };
        rows1 + rows2
    } else {
        0
    };

    // Calculate required keybinds height
    let base_help_h: u16 = if app.show_keybinds_footer {
        baseline_lines
    } else {
        0
    };
    base_help_h.saturating_add(nm_rows).saturating_add(2)
}

/// What: Compute layout rectangles for package details, PKGBUILD, comments, and optional footer.
///
/// Inputs:
/// - `app`: Application state controlling PKGBUILD and comments visibility and footer toggle
/// - `bottom_container`: Rect covering the full details section (including footer space)
/// - `footer_height`: Height previously reserved for the footer
///
/// Output:
/// - Tuple of `(content_container, details_area, pkgb_area_opt, comments_area_opt, show_keybinds)` describing splits.
///
/// Details:
/// - Reserves footer space only when toggled on and space allows
/// - When only PKGBUILD visible: Package info 50%, PKGBUILD 50%
/// - When only comments visible: Package info 50%, Comments 50%
/// - When both visible: Package info 50%, remaining 50% split vertically between PKGBUILD and Comments (25% each)
pub fn calculate_layout_areas(
    app: &AppState,
    bottom_container: Rect,
    footer_height: u16,
) -> (Rect, Rect, Option<Rect>, Option<Rect>, bool) {
    use ratatui::layout::{Constraint, Direction, Layout};

    // Minimum height for Package Info content (including borders: 2 lines)
    const MIN_PACKAGE_INFO_H: u16 = 3; // 1 visible line + 2 borders

    // Keybinds vanish first: only show if there's enough space for Package Info + Keybinds
    // Package Info needs at least MIN_PACKAGE_INFO_H, so keybinds only show if:
    // bottom_container.height >= MIN_PACKAGE_INFO_H + footer_height
    let show_keybinds =
        app.show_keybinds_footer && bottom_container.height >= MIN_PACKAGE_INFO_H + footer_height;

    let help_h: u16 = if show_keybinds { footer_height } else { 0 };
    let content_container = Rect {
        x: bottom_container.x,
        y: bottom_container.y,
        width: bottom_container.width,
        height: bottom_container.height.saturating_sub(help_h),
    };

    let (details_area, pkgb_area_opt, comments_area_opt) =
        match (app.pkgb_visible, app.comments_visible) {
            (true, true) => {
                // Both visible: Package info 50%, PKGBUILD 25%, Comments 25%
                let split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ])
                    .split(content_container);
                (split[0], Some(split[1]), Some(split[2]))
            }
            (true, false) => {
                // Only PKGBUILD visible: Package info 50%, PKGBUILD 50%
                let split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(content_container);
                (split[0], Some(split[1]), None)
            }
            (false, true) => {
                // Only comments visible: Package info 50%, Comments 50%
                let split = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(content_container);
                (split[0], None, Some(split[1]))
            }
            (false, false) => {
                // Neither visible: Package info takes full width
                (content_container, None, None)
            }
        };

    (
        content_container,
        details_area,
        pkgb_area_opt,
        comments_area_opt,
        show_keybinds,
    )
}
