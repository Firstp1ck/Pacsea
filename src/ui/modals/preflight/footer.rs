use ratatui::{
    Frame,
    prelude::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, PackageItem, PreflightAction, PreflightTab};
use crate::theme::theme;

/// What: Render footer/keybinds pane at the bottom of the modal.
///
/// Inputs:
/// - `f`: Frame to render into.
/// - `app`: Application state for i18n.
/// - `items`: Packages being reviewed.
/// - `action`: Whether install or remove.
/// - `tab`: Current active tab.
/// - `content_rect`: Content area rectangle.
/// - `keybinds_rect`: Keybinds area rectangle.
/// - `bg_color`: Background color.
/// - `border_color`: Border color.
///
/// Output:
/// - Renders the footer widget with keybinds hints.
///
/// Details:
/// - Builds footer hint based on current tab and whether AUR packages are present.
/// - Adds cascade mode hint for remove actions.
#[allow(clippy::too_many_arguments)]
pub fn render_footer(
    f: &mut Frame,
    app: &AppState,
    items: &[PackageItem],
    action: &PreflightAction,
    tab: &PreflightTab,
    content_rect: Rect,
    keybinds_rect: Rect,
    bg_color: ratatui::style::Color,
    border_color: ratatui::style::Color,
) {
    let th = theme();

    // Render keybinds pane at the bottom
    // Check if any AUR packages are present for scanning
    let has_aur = items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Aur));

    // Build footer hint based on current tab
    let mut scan_hint = match tab {
        PreflightTab::Deps => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.deps_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.deps_without_aur")
            }
        }
        PreflightTab::Files => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.files_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.files_without_aur")
            }
        }
        PreflightTab::Services => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.services_with_aur")
            } else {
                i18n::t(
                    app,
                    "app.modals.preflight.footer_hints.services_without_aur",
                )
            }
        }
        _ => {
            if has_aur {
                i18n::t(app, "app.modals.preflight.footer_hints.default_with_aur")
            } else {
                i18n::t(app, "app.modals.preflight.footer_hints.default_without_aur")
            }
        }
    };

    if matches!(*action, PreflightAction::Remove) {
        scan_hint.push_str(&i18n::t(
            app,
            "app.modals.preflight.footer_hints.cascade_mode",
        ));
    }

    let keybinds_lines = vec![
        Line::from(""), // Empty line for spacing
        Line::from(Span::styled(scan_hint, Style::default().fg(th.subtext1))),
    ];

    // Adjust keybinds rect to start exactly where content rect ends (no gap)
    let keybinds_rect_adjusted = Rect {
        x: keybinds_rect.x,
        y: content_rect.y + content_rect.height,
        width: keybinds_rect.width,
        height: keybinds_rect.height,
    };

    let keybinds_widget = Paragraph::new(keybinds_lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::LEFT | Borders::BOTTOM | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(keybinds_widget, keybinds_rect_adjusted);
}
