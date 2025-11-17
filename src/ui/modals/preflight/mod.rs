use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, PreflightAction, PreflightTab};
use crate::theme::theme;

mod footer;
mod header;
mod helpers;
mod tabs;

use footer::render_footer;
use header::render_tab_header;
use helpers::{layout, sync};
use tabs::{
    render_deps_tab, render_files_tab, render_sandbox_tab, render_services_tab, render_summary_tab,
};

/// What: Render the preflight modal summarizing dependency/file checks before install/remove.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `app`: Mutable application state (stores tab rects/content rects)
/// - `modal`: Mutable modal state containing all preflight data
///
/// Output:
/// - Draws the modal content for the chosen tab and updates cached data along with clickable rects.
///
/// Details:
/// - Lazily resolves dependencies/files when first accessed, lays out tab headers, records tab
///   rectangles for mouse navigation, and tailors summaries per tab with theming cues.
pub fn render_preflight(
    f: &mut Frame,
    area: Rect,
    app: &mut AppState,
    modal: &mut crate::state::Modal,
) {
    let render_start = std::time::Instant::now();

    // Extract Preflight variant, return early if not Preflight
    let crate::state::Modal::Preflight {
        items,
        action,
        tab,
        summary,
        summary_selected,
        header_chips,
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error,
        file_info,
        file_selected,
        file_tree_expanded,
        files_error,
        service_info,
        service_selected,
        services_loaded,
        services_error,
        sandbox_info,
        sandbox_selected,
        sandbox_tree_expanded,
        sandbox_loaded,
        sandbox_error,
        selected_optdepends,
        cascade_mode,
    } = modal
    else {
        return;
    };

    let th = theme();
    tracing::debug!(
        "[UI] render_preflight START: tab={:?}, items={}, deps={}, files={}, services={}, sandbox={}",
        tab,
        items.len(),
        dependency_info.len(),
        file_info.len(),
        service_info.len(),
        sandbox_info.len()
    );

    // Sync data from app cache to modal state
    sync::sync_dependencies(app, items, action, tab, dependency_info, dep_selected);
    sync::sync_files(app, items, tab, file_info, file_selected);
    sync::sync_services(
        app,
        items,
        action,
        service_info,
        service_selected,
        services_loaded,
    );
    sync::sync_sandbox(app, items, action, tab, sandbox_info, sandbox_loaded);

    // Calculate layout
    let (rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);
    f.render_widget(Clear, rect);

    // Prepare rendering data
    let title = match action {
        PreflightAction::Install => i18n::t(app, "app.modals.preflight.title_install"),
        PreflightAction::Remove => i18n::t(app, "app.modals.preflight.title_remove"),
    };
    let border_color = th.lavender;
    let bg_color = th.crust;

    // Render tab header
    let (header_chips_line, tab_header_line) = render_tab_header(
        app,
        content_rect,
        tab,
        header_chips,
        items,
        summary,
        dependency_info,
        file_info,
        service_info,
        *services_loaded,
        sandbox_info,
        *sandbox_loaded,
    );

    let mut lines: Vec<Line<'static>> = Vec::new();
    // Header chips line
    lines.push(header_chips_line);
    // Tab header line with progress indicators
    lines.push(tab_header_line);
    lines.push(Line::from(""));

    // Render tab content
    match tab {
        PreflightTab::Summary => {
            let tab_lines = render_summary_tab(
                app,
                items,
                action,
                summary,
                *summary_selected,
                header_chips,
                dependency_info,
                *cascade_mode,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Deps => {
            let tab_lines = render_deps_tab(
                app,
                items,
                action,
                dependency_info,
                dep_selected,
                dep_tree_expanded,
                deps_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Files => {
            let tab_lines = render_files_tab(
                app,
                items,
                file_info,
                file_selected,
                file_tree_expanded,
                files_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Services => {
            let tab_lines = render_services_tab(
                app,
                service_info,
                service_selected,
                *services_loaded,
                services_error,
                content_rect,
            );
            lines.extend(tab_lines);
        }
        PreflightTab::Sandbox => {
            let tab_lines = render_sandbox_tab(
                app,
                items,
                sandbox_info,
                sandbox_selected,
                sandbox_tree_expanded,
                *sandbox_loaded,
                sandbox_error,
                selected_optdepends,
                content_rect,
            );
            lines.extend(tab_lines);
        }
    }

    // Render content area (no bottom border - keybinds pane will have top border)
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(bg_color))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(ratatui::text::Span::styled(
                    title,
                    Style::default()
                        .fg(border_color)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::LEFT | Borders::TOP | Borders::RIGHT)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(border_color))
                .style(Style::default().bg(bg_color)),
        );
    f.render_widget(boxw, content_rect);

    // Render footer
    render_footer(
        f,
        app,
        items,
        action,
        tab,
        content_rect,
        keybinds_rect,
        bg_color,
        border_color,
    );

    let render_duration = render_start.elapsed();
    if render_duration.as_millis() > 50 {
        tracing::warn!("[UI] render_preflight took {:?} (slow!)", render_duration);
    } else {
        tracing::debug!("[UI] render_preflight completed in {:?}", render_duration);
    }
}
