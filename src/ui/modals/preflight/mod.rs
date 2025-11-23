use ratatui::{Frame, prelude::Rect, widgets::Clear};

use crate::i18n;
use crate::state::{AppState, PreflightAction};

mod footer;
mod header;
mod helpers;
mod tabs;

use footer::render_footer;
use header::render_tab_header;
use helpers::{extract, layout, scroll, sync, tabs as tab_helpers, widget};

use ratatui::text::Line;

use crate::theme::theme;

/// What: Sync all preflight data from app cache to modal state.
///
/// Inputs:
/// - `app`: Application state
/// - `fields`: Preflight fields extracted from modal
///
/// Output:
/// - Updates modal state with synced data.
fn sync_preflight_data(app: &AppState, fields: &mut extract::PreflightFields) {
    sync::sync_dependencies(
        app,
        fields.items,
        *fields.action,
        *fields.tab,
        fields.dependency_info,
        fields.dep_selected,
    );
    sync::sync_files(
        app,
        fields.items,
        *fields.tab,
        fields.file_info,
        fields.file_selected,
    );
    sync::sync_services(
        app,
        fields.items,
        *fields.action,
        fields.service_info,
        fields.service_selected,
        fields.services_loaded,
    );
    sync::sync_sandbox(
        app,
        fields.items,
        *fields.action,
        *fields.tab,
        fields.sandbox_info,
        fields.sandbox_loaded,
    );
}

/// What: Build content lines for preflight modal.
///
/// Inputs:
/// - `app`: Application state
/// - `fields`: Preflight fields
/// - `content_rect`: Content area rectangle
///
/// Output:
/// - Vector of lines to render.
fn build_preflight_content_lines(
    app: &mut AppState,
    fields: &mut extract::PreflightFields,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let (header_chips_line, tab_header_line) = render_tab_header(
        app,
        content_rect,
        *fields.tab,
        fields.header_chips,
        fields.items,
        fields.summary.as_ref().map(|b| b.as_ref()),
        fields.dependency_info,
        fields.file_info,
        *fields.services_loaded,
        fields.sandbox_info,
        *fields.sandbox_loaded,
    );
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(header_chips_line);
    lines.push(tab_header_line);
    lines.push(Line::from(""));
    let tab_lines = tab_helpers::render_tab_content(
        app,
        *fields.tab,
        fields.items,
        *fields.action,
        fields.summary.as_ref().map(|b| b.as_ref()),
        fields.header_chips,
        fields.dependency_info,
        fields.dep_selected,
        fields.dep_tree_expanded,
        fields.deps_error.as_ref(),
        fields.file_info,
        fields.file_selected,
        fields.file_tree_expanded,
        fields.files_error.as_ref(),
        fields.service_info,
        fields.service_selected,
        *fields.services_loaded,
        fields.services_error.as_ref(),
        fields.sandbox_info,
        fields.sandbox_selected,
        fields.sandbox_tree_expanded,
        *fields.sandbox_loaded,
        fields.sandbox_error.as_ref(),
        fields.selected_optdepends,
        *fields.cascade_mode,
        content_rect,
    );
    lines.extend(tab_lines);
    lines
}

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
    let Some(mut fields) = extract::extract_preflight_fields(modal) else {
        return;
    };

    let th = theme();
    tracing::debug!(
        "[UI] render_preflight START: tab={:?}, items={}, deps={}, files={}, services={}, sandbox={}, cache_deps={}, cache_files={}",
        fields.tab,
        fields.items.len(),
        fields.dependency_info.len(),
        fields.file_info.len(),
        fields.service_info.len(),
        fields.sandbox_info.len(),
        app.install_list_deps.len(),
        app.install_list_files.len()
    );

    sync_preflight_data(app, &mut fields);

    tracing::debug!(
        "[UI] render_preflight AFTER SYNC: tab={:?}, deps={}, files={}, cache_deps={}, cache_files={}, resolving_deps={}, resolving_files={}",
        fields.tab,
        fields.dependency_info.len(),
        fields.file_info.len(),
        app.install_list_deps.len(),
        app.install_list_files.len(),
        app.preflight_deps_resolving || app.deps_resolving,
        app.preflight_files_resolving || app.files_resolving
    );

    let (rect, content_rect, keybinds_rect) = layout::calculate_modal_layout(area);
    f.render_widget(Clear, rect);

    let title = match *fields.action {
        PreflightAction::Install => i18n::t(app, "app.modals.preflight.title_install"),
        PreflightAction::Remove => i18n::t(app, "app.modals.preflight.title_remove"),
    };
    let border_color = th.lavender;
    let bg_color = th.crust;

    let lines = build_preflight_content_lines(app, &mut fields, content_rect);
    let scroll_offset = scroll::calculate_scroll_offset(app, *fields.tab);

    let content_widget = widget::ParagraphBuilder::new()
        .with_lines(lines)
        .with_title(title)
        .with_scroll(scroll_offset)
        .with_text_color(th.text)
        .with_bg_color(bg_color)
        .with_border_color(border_color)
        .build();
    f.render_widget(content_widget, content_rect);

    render_footer(
        f,
        app,
        fields.items,
        *fields.action,
        *fields.tab,
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
