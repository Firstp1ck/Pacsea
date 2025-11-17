use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::{FileChangeType, PackageFileInfo};
use crate::state::{AppState, PackageItem};
use crate::theme::theme;
use crate::ui::modals::preflight::helpers::format_count_with_indicator;

type FileDisplayItem = (
    bool,
    String,
    Option<(FileChangeType, String, bool, bool, bool)>,
);

/// What: Render loading/resolving state with package headers.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages under review.
/// - `message_key`: i18n key for the message to display.
/// - `message_color`: Color for the message text.
///
/// Output:
/// - Returns a vector of lines to render.
fn render_loading_state(
    app: &AppState,
    items: &[PackageItem],
    message_key: &str,
    message_color: ratatui::style::Color,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    for item in items.iter() {
        let mut spans = Vec::new();
        spans.push(Span::styled(
            format!("▶ {} ", item.name),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("(0 files)", Style::default().fg(th.subtext1)));
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, message_key),
        Style::default().fg(message_color),
    )));

    lines
}

/// What: Render error state with retry hint.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `err_msg`: Error message to display.
///
/// Output:
/// - Returns a vector of lines to render.
fn render_error_state(app: &AppState, err_msg: &str) -> Vec<Line<'static>> {
    let th = theme();
    vec![
        Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.preflight.files.error", err_msg),
            Style::default().fg(th.red),
        )),
        Line::from(""),
        Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.files.retry_hint"),
            Style::default().fg(th.subtext1),
        )),
    ]
}

/// What: Build flat list of display items from file info.
///
/// Inputs:
/// - `items`: All packages under review.
/// - `file_info`: File information.
/// - `file_tree_expanded`: Set of expanded package names.
///
/// Output:
/// - Returns a vector of display items (headers and files).
///
/// Details:
/// - Always shows ALL packages from items, even if they have no files.
/// - This ensures packages that failed to resolve files (e.g., due to conflicts) are still visible.
fn build_display_items(
    items: &[PackageItem],
    file_info: &[PackageFileInfo],
    file_tree_expanded: &std::collections::HashSet<String>,
) -> Vec<FileDisplayItem> {
    use std::collections::HashMap;
    // Create a map for quick lookup of file info by package name
    let file_info_map: HashMap<String, &PackageFileInfo> = file_info
        .iter()
        .map(|info| (info.name.clone(), info))
        .collect();

    let mut display_items = Vec::new();
    // Always show ALL packages from items, even if they have no file info
    for item in items.iter() {
        let pkg_name = &item.name;
        let is_expanded = file_tree_expanded.contains(pkg_name);
        display_items.push((true, pkg_name.clone(), None)); // Package header

        if is_expanded {
            // Show files if available
            if let Some(pkg_info) = file_info_map.get(pkg_name) {
                for file in pkg_info.files.iter() {
                    display_items.push((
                        false,
                        String::new(),
                        Some((
                            file.change_type.clone(),
                            file.path.clone(),
                            file.is_config,
                            file.predicted_pacnew,
                            file.predicted_pacsave,
                        )),
                    ));
                }
            }
        }
    }
    display_items
}

/// What: Render sync timestamp line.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `sync_info`: Optional sync info tuple (age_days, date_str, color_category).
///
/// Output:
/// - Returns optional line and number of lines added (0 or 2).
fn render_sync_timestamp(
    app: &AppState,
    sync_info: &Option<(u64, String, u8)>,
) -> (Option<Line<'static>>, usize) {
    let th = theme();
    if let Some((_age_days, date_str, color_category)) = sync_info {
        let (sync_color, sync_text) = match color_category {
            0 => (
                th.green,
                i18n::t_fmt1(app, "app.modals.preflight.files.files_updated_on", date_str),
            ),
            1 => (
                th.yellow,
                i18n::t_fmt1(app, "app.modals.preflight.files.files_updated_on", date_str),
            ),
            _ => (
                th.red,
                i18n::t_fmt1(app, "app.modals.preflight.files.files_updated_on", date_str),
            ),
        };
        (
            Some(Line::from(Span::styled(
                sync_text,
                Style::default().fg(sync_color),
            ))),
            2,
        )
    } else {
        (None, 0)
    }
}

/// What: Render empty state when no files are found.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `items`: Packages under review.
/// - `file_info`: File information.
/// - `is_stale`: Whether file database is stale.
/// - `sync_info`: Optional sync info.
///
/// Output:
/// - Returns a vector of lines to render.
fn render_empty_state(
    app: &AppState,
    items: &[PackageItem],
    file_info: &[PackageFileInfo],
    is_stale: &Option<bool>,
    sync_info: &Option<(u64, String, u8)>,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let has_aur_packages = items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Aur));
    let has_official_packages = items
        .iter()
        .any(|p| matches!(p.source, crate::state::Source::Official { .. }));

    let mut unresolved_packages = Vec::new();
    for pkg_info in file_info.iter() {
        if pkg_info.files.is_empty() {
            unresolved_packages.push(pkg_info.name.clone());
        }
    }

    if !file_info.is_empty() {
        if !unresolved_packages.is_empty() {
            lines.push(Line::from(Span::styled(
                i18n::t_fmt1(
                    app,
                    "app.modals.preflight.files.no_file_changes",
                    unresolved_packages.len(),
                ),
                Style::default().fg(th.subtext1),
            )));
            lines.push(Line::from(""));

            if has_official_packages {
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.file_db_sync_note"),
                    Style::default().fg(th.subtext0),
                )));
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.sync_file_db_hint"),
                    Style::default().fg(th.subtext0),
                )));
            }
            if has_aur_packages {
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.aur_file_note"),
                    Style::default().fg(th.subtext0),
                )));
            }
        } else {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.preflight.files.no_file_changes_display"),
                Style::default().fg(th.subtext1),
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.files.file_resolution_progress"),
            Style::default().fg(th.subtext1),
        )));
    }

    if let Some(true) = is_stale {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.files.file_db_stale"),
            Style::default().fg(th.yellow),
        )));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.files.sync_file_db_root"),
            Style::default().fg(th.subtext0),
        )));
    }

    let (sync_line, _) = render_sync_timestamp(app, sync_info);
    if let Some(line) = sync_line {
        lines.push(Line::from(""));
        lines.push(line);
    }

    lines
}

/// What: Render package header line.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `pkg_info`: Package file information.
/// - `pkg_name`: Package name.
/// - `is_expanded`: Whether package is expanded.
/// - `is_selected`: Whether package is selected.
///
/// Output:
/// - Returns a line to render.
fn render_package_header(
    app: &AppState,
    pkg_info: &PackageFileInfo,
    pkg_name: &str,
    is_expanded: bool,
    is_selected: bool,
) -> Line<'static> {
    let th = theme();
    let arrow_symbol = if is_expanded { "▼" } else { "▶" };
    let header_style = if is_selected {
        Style::default()
            .fg(th.crust)
            .bg(th.lavender)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD)
    };

    let mut spans = vec![
        Span::styled(format!("{} {} ", arrow_symbol, pkg_name), header_style),
        Span::styled(
            format!("({} files", pkg_info.total_count),
            Style::default().fg(th.subtext1),
        ),
    ];

    if pkg_info.new_count > 0 {
        spans.push(Span::styled(
            format!(
                ", {}",
                i18n::t_fmt1(app, "app.modals.preflight.files.new", pkg_info.new_count)
            ),
            Style::default().fg(th.green),
        ));
    }
    if pkg_info.changed_count > 0 {
        spans.push(Span::styled(
            format!(", {} changed", pkg_info.changed_count),
            Style::default().fg(th.yellow),
        ));
    }
    if pkg_info.removed_count > 0 {
        spans.push(Span::styled(
            format!(", {} removed", pkg_info.removed_count),
            Style::default().fg(th.red),
        ));
    }
    if pkg_info.config_count > 0 {
        spans.push(Span::styled(
            format!(", {} config", pkg_info.config_count),
            Style::default().fg(th.mauve),
        ));
    }
    if pkg_info.pacnew_candidates > 0 {
        spans.push(Span::styled(
            format!(", {} pacnew", pkg_info.pacnew_candidates),
            Style::default().fg(th.yellow),
        ));
    }
    if pkg_info.pacsave_candidates > 0 {
        spans.push(Span::styled(
            format!(", {} pacsave", pkg_info.pacsave_candidates),
            Style::default().fg(th.red),
        ));
    }
    spans.push(Span::styled(")", Style::default().fg(th.subtext1)));

    Line::from(spans)
}

/// What: Render file entry line.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `change_type`: Type of file change.
/// - `path`: File path.
/// - `is_config`: Whether file is a config file.
/// - `predicted_pacnew`: Whether pacnew is predicted.
/// - `predicted_pacsave`: Whether pacsave is predicted.
/// - `is_selected`: Whether file is selected.
///
/// Output:
/// - Returns a line to render.
fn render_file_entry(
    app: &AppState,
    change_type: &FileChangeType,
    path: &str,
    is_config: bool,
    predicted_pacnew: bool,
    predicted_pacsave: bool,
    is_selected: bool,
) -> Line<'static> {
    let th = theme();
    let (icon, color) = match change_type {
        FileChangeType::New => ("+", th.green),
        FileChangeType::Changed => ("~", th.yellow),
        FileChangeType::Removed => ("-", th.red),
    };

    let highlight_bg = if is_selected { Some(th.lavender) } else { None };
    let icon_style = if let Some(bg) = highlight_bg {
        Style::default().fg(color).bg(bg)
    } else {
        Style::default().fg(color)
    };
    let mut spans = vec![Span::styled(format!("  {} ", icon), icon_style)];

    if is_config {
        let cfg_style = if let Some(bg) = highlight_bg {
            Style::default().fg(th.mauve).bg(bg)
        } else {
            Style::default().fg(th.mauve)
        };
        spans.push(Span::styled("⚙ ", cfg_style));
    }

    if predicted_pacnew {
        let pacnew_style = if let Some(bg) = highlight_bg {
            Style::default().fg(th.yellow).bg(bg)
        } else {
            Style::default().fg(th.yellow)
        };
        spans.push(Span::styled(
            i18n::t(app, "app.modals.preflight.files.pacnew_label"),
            pacnew_style,
        ));
    }
    if predicted_pacsave {
        let pacsave_style = if let Some(bg) = highlight_bg {
            Style::default().fg(th.red).bg(bg)
        } else {
            Style::default().fg(th.red)
        };
        spans.push(Span::styled(
            i18n::t(app, "app.modals.preflight.files.pacsave_label"),
            pacsave_style,
        ));
    }

    let path_style = if let Some(bg) = highlight_bg {
        Style::default()
            .fg(th.crust)
            .bg(bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(th.text)
    };

    spans.push(Span::styled(path.to_string(), path_style));

    Line::from(spans)
}

/// What: Render file list with summary and scrolling.
///
/// Inputs:
/// - `app`: Application state for i18n.
/// - `file_info`: File information.
/// - `items`: Packages under review.
/// - `display_items`: Flat list of display items.
/// - `file_selected`: Currently selected file index (mutable).
/// - `file_tree_expanded`: Set of expanded package names.
/// - `is_resolving`: Whether files are being resolved.
/// - `content_rect`: Content area rectangle.
/// - `sync_info`: Optional sync info.
///
/// Output:
/// - Returns a vector of lines to render.
#[allow(clippy::too_many_arguments)]
fn render_file_list(
    app: &AppState,
    file_info: &[PackageFileInfo],
    items: &[PackageItem],
    display_items: &[FileDisplayItem],
    file_selected: &mut usize,
    file_tree_expanded: &std::collections::HashSet<String>,
    is_resolving: bool,
    content_rect: Rect,
    sync_info: &Option<(u64, String, u8)>,
) -> Vec<Line<'static>> {
    let th = theme();
    let mut lines = Vec::new();

    let total_files: usize = file_info.iter().map(|p| p.total_count).sum();
    let total_new: usize = file_info.iter().map(|p| p.new_count).sum();
    let total_changed: usize = file_info.iter().map(|p| p.changed_count).sum();
    let total_removed: usize = file_info.iter().map(|p| p.removed_count).sum();
    let total_config: usize = file_info.iter().map(|p| p.config_count).sum();
    let total_pacnew: usize = file_info.iter().map(|p| p.pacnew_candidates).sum();
    let total_pacsave: usize = file_info.iter().map(|p| p.pacsave_candidates).sum();

    let has_incomplete_data = (is_resolving && file_info.len() < items.len())
        || (!is_resolving && file_info.len() < items.len());

    let total_files_text =
        format_count_with_indicator(total_files, items.len() * 10, has_incomplete_data);
    let mut summary_parts = vec![i18n::t_fmt1(
        app,
        "app.modals.preflight.files.total",
        total_files_text,
    )];
    if total_new > 0 {
        let count_text = format_count_with_indicator(total_new, total_files, has_incomplete_data);
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.new",
            count_text,
        ));
    }
    if total_changed > 0 {
        let count_text =
            format_count_with_indicator(total_changed, total_files, has_incomplete_data);
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.changed",
            count_text,
        ));
    }
    if total_removed > 0 {
        let count_text =
            format_count_with_indicator(total_removed, total_files, has_incomplete_data);
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.removed",
            count_text,
        ));
    }
    if total_config > 0 {
        let count_text =
            format_count_with_indicator(total_config, total_files, has_incomplete_data);
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.config",
            count_text,
        ));
    }
    if total_pacnew > 0 {
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.pacnew",
            total_pacnew,
        ));
    }
    if total_pacsave > 0 {
        summary_parts.push(i18n::t_fmt1(
            app,
            "app.modals.preflight.files.pacsave",
            total_pacsave,
        ));
    }

    lines.push(Line::from(Span::styled(
        i18n::t_fmt1(
            app,
            "app.modals.preflight.files.files_label",
            summary_parts.join(", "),
        ),
        Style::default()
            .fg(th.overlay1)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    let (sync_line, sync_timestamp_lines) = render_sync_timestamp(app, sync_info);
    if let Some(line) = sync_line {
        lines.push(line);
        lines.push(Line::from(""));
    }

    let header_lines = 4 + sync_timestamp_lines;
    let available_height = (content_rect.height.saturating_sub(1) as usize)
        .saturating_sub(header_lines)
        .max(1);

    let total_items = display_items.len();
    let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
    if *file_selected != file_selected_clamped {
        *file_selected = file_selected_clamped;
    }

    // Calculate viewport: center the selected item, but ensure it's always visible
    let (start_idx, end_idx) = if total_items <= available_height {
        // All items fit on screen
        (0, total_items)
    } else {
        // Try to center the selected item
        let start = file_selected_clamped
            .saturating_sub(available_height / 2)
            .min(total_items.saturating_sub(available_height));
        let end = (start + available_height).min(total_items);

        // Safety check: ensure selected item is always visible
        let (final_start, final_end) = if file_selected_clamped < start {
            // Selected is before start - adjust to include it
            (
                file_selected_clamped,
                (file_selected_clamped + available_height).min(total_items),
            )
        } else if file_selected_clamped >= end {
            // Selected is at or beyond end - position it at bottom
            let new_end = (file_selected_clamped + 1).min(total_items);
            (new_end.saturating_sub(available_height).max(0), new_end)
        } else {
            (start, end)
        };
        (final_start, final_end)
    };

    for (display_idx, (is_header, pkg_name, file_opt)) in display_items
        .iter()
        .enumerate()
        .skip(start_idx)
        .take(end_idx - start_idx)
    {
        let is_selected = display_idx == *file_selected;
        if *is_header {
            let is_expanded = file_tree_expanded.contains(pkg_name);
            // Handle packages that may not have file info yet
            if let Some(pkg_info) = file_info.iter().find(|p| p.name == *pkg_name) {
                lines.push(render_package_header(
                    app,
                    pkg_info,
                    pkg_name,
                    is_expanded,
                    is_selected,
                ));
            } else {
                // Package has no file info - render simple header
                let th = theme();
                let arrow_symbol = if is_expanded { "▼" } else { "▶" };
                let header_style = if is_selected {
                    Style::default()
                        .fg(th.crust)
                        .bg(th.lavender)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(th.overlay1)
                        .add_modifier(Modifier::BOLD)
                };
                let spans = vec![
                    Span::styled(format!("{} {} ", arrow_symbol, pkg_name), header_style),
                    Span::styled(
                        "(0 files)",
                        Style::default().fg(th.subtext1),
                    ),
                ];
                lines.push(Line::from(spans));
            }
        } else if let Some((change_type, path, is_config, predicted_pacnew, predicted_pacsave)) =
            file_opt
        {
            lines.push(render_file_entry(
                app,
                change_type,
                path,
                *is_config,
                *predicted_pacnew,
                *predicted_pacsave,
                is_selected,
            ));
        }
    }

    if total_items > available_height {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t_fmt(
                app,
                "app.modals.preflight.files.showing_range_items",
                &[
                    &(start_idx + 1).to_string(),
                    &end_idx.to_string(),
                    &total_items.to_string(),
                ],
            ),
            Style::default().fg(th.subtext1),
        )));
    }

    lines
}

/// What: Render the Files tab content for the preflight modal.
///
/// Inputs:
/// - `app`: Application state for i18n and data access.
/// - `items`: Packages under review.
/// - `file_info`: File information.
/// - `file_selected`: Currently selected file index (mutable).
/// - `file_tree_expanded`: Set of expanded package names.
/// - `files_error`: Optional error message.
/// - `content_rect`: Content area rectangle.
///
/// Output:
/// - Returns a vector of lines to render.
///
/// Details:
/// - Shows file changes grouped by package with expand/collapse.
/// - Supports viewport-based rendering for large file lists.
#[allow(clippy::too_many_arguments)]
pub fn render_files_tab(
    app: &AppState,
    items: &[PackageItem],
    file_info: &[PackageFileInfo],
    file_selected: &mut usize,
    file_tree_expanded: &std::collections::HashSet<String>,
    files_error: &Option<String>,
    content_rect: Rect,
) -> Vec<Line<'static>> {
    let is_resolving = app.preflight_files_resolving || app.files_resolving;

    // Log render state for debugging
    tracing::debug!(
        "[UI] render_files_tab: items={}, file_info={}, file_selected={}, expanded={}, resolving={}/{}, error={:?}",
        items.len(),
        file_info.len(),
        *file_selected,
        file_tree_expanded.len(),
        app.preflight_files_resolving,
        app.files_resolving,
        files_error.is_some()
    );
    if !file_info.is_empty() {
        tracing::info!(
            "[UI] render_files_tab: Rendering {} file info entries",
            file_info.len()
        );
        for pkg_info in file_info.iter() {
            tracing::info!(
                "[UI] render_files_tab: Package '{}' - total={}, new={}, changed={}, removed={}, config={}, files={}",
                pkg_info.name,
                pkg_info.total_count,
                pkg_info.new_count,
                pkg_info.changed_count,
                pkg_info.removed_count,
                pkg_info.config_count,
                pkg_info.files.len()
            );
        }
    }

    if is_resolving {
        let th = theme();
        return render_loading_state(app, items, "app.modals.preflight.files.updating", th.yellow);
    }

    if let Some(err_msg) = files_error {
        return render_error_state(app, err_msg);
    }

    if file_info.is_empty() {
        let th = theme();
        return render_loading_state(
            app,
            items,
            "app.modals.preflight.files.resolving",
            th.subtext1,
        );
    }

    let display_items = build_display_items(items, file_info, file_tree_expanded);
    let sync_info = crate::logic::files::get_file_db_sync_info();
    const STALE_THRESHOLD_DAYS: u64 = 7;
    let is_stale = crate::logic::files::is_file_db_stale(STALE_THRESHOLD_DAYS);

    if display_items.is_empty() {
        render_empty_state(app, items, file_info, &is_stale, &sync_info)
    } else {
        render_file_list(
            app,
            file_info,
            items,
            &display_items,
            file_selected,
            file_tree_expanded,
            is_resolving,
            content_rect,
            &sync_info,
        )
    }
}
