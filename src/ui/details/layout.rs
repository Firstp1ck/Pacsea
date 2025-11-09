use ratatui::prelude::Rect;

use crate::state::{AppState, Focus};

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

/// What: Compute layout rectangles for package details, PKGBUILD, and optional footer.
///
/// Inputs:
/// - `app`: Application state controlling PKGBUILD visibility and footer toggle
/// - `bottom_container`: Rect covering the full details section (including footer space)
/// - `footer_height`: Height previously reserved for the footer
///
/// Output:
/// - Tuple of `(content_container, details_area, pkgb_area_opt, show_keybinds)` describing splits.
///
/// Details:
/// - Reserves footer space only when toggled on and space allows; splits remaining area evenly when
///   PKGBUILD view is active.
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
