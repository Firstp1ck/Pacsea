use ratatui::prelude::Rect;

use crate::state::{AppState, Focus};

/// Calculate the required height for the footer/keybinds section.
///
/// Returns the number of rows needed for the footer, including:
/// - Baseline lines (GLOBALS + currently focused pane)
/// - Optional Normal Mode lines when Search is focused and in normal mode
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
            .map(|c| c.label())
            .unwrap_or_else(|| "Esc".to_string());
        let insert_label = km
            .search_normal_insert
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "i".to_string());
        let left_label = km
            .search_normal_select_left
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "h".to_string());
        let right_label = km
            .search_normal_select_right
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "l".to_string());
        let delete_label = km
            .search_normal_delete
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "d".to_string());
        let clear_label = km
            .search_normal_clear
            .first()
            .map(|c| c.label())
            .unwrap_or_else(|| "Shift+Del".to_string());

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
                    line2.push_str(&format!("[{}] Config", k.label()));
                }
                if let Some(k) = km.options_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    line2.push_str(&format!("[{}] Options", k.label()));
                }
                if let Some(k) = km.panels_menu_toggle.first() {
                    if !line2.ends_with("menus: ") {
                        line2.push_str(", ");
                    }
                    line2.push_str(&format!("[{}] Panels", k.label()));
                }
            }
            // Import / Export
            if !app.installed_only_mode
                && (!km.search_normal_import.is_empty() || !km.search_normal_export.is_empty())
            {
                line2.push_str("  •  ");
                if let Some(k) = km.search_normal_import.first() {
                    line2.push_str(&format!("[{}] Import", k.label()));
                    if let Some(k2) = km.search_normal_export.first() {
                        line2.push_str(&format!(", [{}] Export", k2.label()));
                    }
                } else if let Some(k) = km.search_normal_export.first() {
                    line2.push_str(&format!("[{}] Export", k.label()));
                }
            }
        }
        let w = if footer_w == 0 { 1 } else { footer_w };
        let rows1 = ((line1.len() as u16).div_ceil(w)).max(1);
        let rows2 = if line2.is_empty() {
            0
        } else {
            ((line2.len() as u16).div_ceil(w)).max(1)
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

/// Calculate layout areas for details and PKGBUILD panes.
///
/// Returns:
/// - `content_container`: The area available for content (after reserving footer space)
/// - `details_area`: The area for Package Info pane
/// - `pkgb_area_opt`: Optional area for PKGBUILD pane (if visible)
/// - `show_keybinds`: Whether to show the footer/keybinds
pub fn calculate_layout_areas(
    app: &AppState,
    bottom_container: Rect,
    footer_height: u16,
) -> (Rect, Rect, Option<Rect>, bool) {
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
    let (details_area, pkgb_area_opt) = if app.pkgb_visible {
        use ratatui::layout::{Constraint, Direction, Layout};
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(content_container);
        (split[0], Some(split[1]))
    } else {
        (content_container, None)
    };

    (
        content_container,
        details_area,
        pkgb_area_opt,
        show_keybinds,
    )
}
