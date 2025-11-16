use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::i18n;
use crate::state::modal::{FileChangeType, PackageFileInfo};
use crate::state::{AppState, PackageItem};
use crate::theme::theme;

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
    let th = theme();
    let mut lines = Vec::new();

    let is_resolving = app.preflight_files_resolving || app.files_resolving;

    if is_resolving {
        // Show package headers first, then loading message
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
            i18n::t(app, "app.modals.preflight.files.updating"),
            Style::default().fg(th.yellow),
        )));
    } else if let Some(err_msg) = files_error {
        // Display error with retry hint
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(app, "app.modals.preflight.files.error", err_msg),
            Style::default().fg(th.red),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.preflight.files.retry_hint"),
            Style::default().fg(th.subtext1),
        )));
    } else if file_info.is_empty() {
        // Show package headers first, then resolving message
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
            i18n::t(app, "app.modals.preflight.files.resolving"),
            Style::default().fg(th.subtext1),
        )));
    } else {
        // Build flat list of display items: package headers + files (only if expanded)
        // Store owned data to avoid lifetime issues
        // Performance: This builds the full display list, but only visible items are rendered
        // below. For very large file lists (thousands of files), consider lazy building or caching.
        type FileDisplayItem = (
            bool,
            String,
            Option<(FileChangeType, String, bool, bool, bool)>,
        );
        let mut display_items: Vec<FileDisplayItem> = Vec::new();
        for pkg_info in file_info.iter() {
            if !pkg_info.files.is_empty() {
                let is_expanded = file_tree_expanded.contains(&pkg_info.name);
                display_items.push((true, pkg_info.name.clone(), None)); // Package header
                // Add files only if package is expanded
                if is_expanded {
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
                        )); // File entry
                    }
                }
            }
        }

        let sync_info = crate::logic::files::get_file_db_sync_info();
        // Check if file database is stale (older than 7 days)
        const STALE_THRESHOLD_DAYS: u64 = 7;
        let is_stale = crate::logic::files::is_file_db_stale(STALE_THRESHOLD_DAYS);

        if display_items.is_empty() {
            // Check if we have package entries but they're all empty
            let has_aur_packages = items
                .iter()
                .any(|p| matches!(p.source, crate::state::Source::Aur));
            let has_official_packages = items
                .iter()
                .any(|p| matches!(p.source, crate::state::Source::Official { .. }));

            // Count packages with empty file lists
            let mut unresolved_packages = Vec::new();
            for pkg_info in file_info.iter() {
                if pkg_info.files.is_empty() {
                    unresolved_packages.push(pkg_info.name.clone());
                }
            }

            if !file_info.is_empty() {
                // File resolution completed but no files found
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

                    // Show appropriate notes based on package types
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
                // File resolution hasn't completed or failed
                lines.push(Line::from(Span::styled(
                    i18n::t(app, "app.modals.preflight.files.file_resolution_progress"),
                    Style::default().fg(th.subtext1),
                )));
            }

            // Show stale file database warning if applicable
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

            // Show file database sync timestamp
            if let Some((_age_days, date_str, color_category)) = sync_info.clone() {
                lines.push(Line::from(""));
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
                lines.push(Line::from(Span::styled(
                    sync_text,
                    Style::default().fg(sync_color),
                )));
            }
        } else {
            // Display summary first (needed to calculate available_height accurately)
            let total_files: usize = file_info.iter().map(|p| p.total_count).sum();
            let total_new: usize = file_info.iter().map(|p| p.new_count).sum();
            let total_changed: usize = file_info.iter().map(|p| p.changed_count).sum();
            let total_removed: usize = file_info.iter().map(|p| p.removed_count).sum();
            let total_config: usize = file_info.iter().map(|p| p.config_count).sum();
            let total_pacnew: usize = file_info.iter().map(|p| p.pacnew_candidates).sum();
            let total_pacsave: usize = file_info.iter().map(|p| p.pacsave_candidates).sum();

            let mut summary_parts = vec![i18n::t_fmt1(
                app,
                "app.modals.preflight.files.total",
                total_files,
            )];
            if total_new > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.files.new",
                    total_new,
                ));
            }
            if total_changed > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.files.changed",
                    total_changed,
                ));
            }
            if total_removed > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.files.removed",
                    total_removed,
                ));
            }
            if total_config > 0 {
                summary_parts.push(i18n::t_fmt1(
                    app,
                    "app.modals.preflight.files.config",
                    total_config,
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

            // Show file database sync timestamp
            let sync_timestamp_lines = if let Some((_age_days, date_str, color_category)) =
                sync_info.clone()
            {
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
                lines.push(Line::from(Span::styled(
                    sync_text,
                    Style::default().fg(sync_color),
                )));
                lines.push(Line::from(""));
                2 // timestamp line + empty line
            } else {
                0
            };

            // Calculate available height for file list AFTER adding summary and sync timestamp
            // Lines used before file list: tab header (1) + empty (1) + summary (1) + empty (1) + sync timestamp (0-2)
            // Total: 4-6 lines
            let header_lines = 4 + sync_timestamp_lines;
            let available_height = (content_rect.height.saturating_sub(1) as usize)
                .saturating_sub(header_lines)
                .max(1);

            // Calculate scroll position
            // Performance optimization: Only render visible items (viewport-based rendering)
            // This prevents performance issues with large file lists
            let total_items = display_items.len();
            // Clamp file_selected to valid range
            let file_selected_clamped = (*file_selected).min(total_items.saturating_sub(1));
            if *file_selected != file_selected_clamped {
                *file_selected = file_selected_clamped;
            }
            // Only scroll if there are more items than can fit on screen
            let (start_idx, end_idx) = if total_items <= available_height {
                // All items fit - show everything starting from 0
                (0, total_items)
            } else {
                // More items than fit - center selected item or scroll to show it
                let start = file_selected_clamped
                    .saturating_sub(available_height / 2)
                    .min(total_items.saturating_sub(available_height));
                let end = (start + available_height).min(total_items);
                (start, end)
            };

            // Display files with scrolling (only render visible items)
            for (display_idx, (is_header, pkg_name, file_opt)) in display_items
                .iter()
                .enumerate()
                .skip(start_idx)
                .take(end_idx - start_idx)
            {
                let is_selected = display_idx == *file_selected;
                if *is_header {
                    // Find package info for this header
                    let pkg_info = file_info.iter().find(|p| p.name == *pkg_name).unwrap();
                    let is_expanded = file_tree_expanded.contains(pkg_name);

                    // Package header with expand/collapse indicator
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
                                i18n::t_fmt1(
                                    app,
                                    "app.modals.preflight.files.new",
                                    pkg_info.new_count
                                )
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

                    lines.push(Line::from(spans));
                } else if let Some((
                    change_type,
                    path,
                    is_config,
                    predicted_pacnew,
                    predicted_pacsave,
                )) = file_opt
                {
                    // File entry
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

                    if *is_config {
                        let cfg_style = if let Some(bg) = highlight_bg {
                            Style::default().fg(th.mauve).bg(bg)
                        } else {
                            Style::default().fg(th.mauve)
                        };
                        spans.push(Span::styled("⚙ ", cfg_style));
                    }

                    // Add pacnew/pacsave indicators
                    if *predicted_pacnew {
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
                    if *predicted_pacsave {
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

                    spans.push(Span::styled(path.clone(), path_style));

                    lines.push(Line::from(spans));
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
        }
    }

    lines
}
