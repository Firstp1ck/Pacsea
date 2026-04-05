//! Rendering for foreign↔sync overlap wizard and AUR duplicate-results warning.

use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::AppState;
use crate::state::modal::ForeignRepoOverlapPhase;
use crate::theme::{Theme, theme};

/// Visible rows for scrollable overlap lists inside the wizard.
const LIST_VIEWPORT_ROWS: usize = 8;

/// What: Append non-empty translated paragraphs for wizard detail keys.
///
/// Inputs:
/// - `lines`: Target line buffer.
/// - `app`: Application state for translations.
/// - `keys`: i18n key paths.
/// - `fg`: Text color for each paragraph.
///
/// Output:
/// - Pushes blank lines between consecutive non-empty paragraphs.
///
/// Details:
/// - Skips keys whose translation is empty so locales can omit optional copy.
fn push_i18n_detail_paragraphs(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    keys: &[&str],
    fg: Style,
) {
    let mut first = true;
    for key in keys {
        let text = i18n::t(app, key);
        if text.is_empty() {
            continue;
        }
        if !first {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(text, fg)));
        first = false;
    }
}

/// What: Append the overlap package list heading and a scrolled viewport of rows.
///
/// Inputs:
/// - `lines`: Target line buffer.
/// - `app`: Application state for the list heading translation.
/// - `entries`: Overlap rows `(pkgname, version)`.
/// - `list_scroll`: First visible row index.
/// - `text_fg`, `sub_fg`: Colors for rows and heading.
///
/// Output:
/// - Pushes a blank separator, heading, then up to [`LIST_VIEWPORT_ROWS`] lines.
///
/// Details:
/// - Shared by both `WarnAck` steps so the package names stay visible on each warning screen.
fn push_overlap_warn_list_viewport(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    entries: &[(String, String)],
    list_scroll: u16,
    text_fg: Style,
    sub_fg: Style,
) {
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.foreign_overlap.list_heading"),
        sub_fg,
    )));
    let start = usize::from(list_scroll);
    for row in entries.iter().skip(start).take(LIST_VIEWPORT_ROWS) {
        lines.push(Line::from(Span::styled(
            format!("  {} {}", row.0, row.1),
            text_fg,
        )));
    }
}

/// What: Append lines for the two `WarnAck` steps of the foreign overlap wizard.
///
/// Inputs:
/// - `lines`: Target line buffer.
/// - `app`, `th`: State and theme for styling and i18n.
/// - `repo_name`: Repository label for step 0 (`warn_step0_line1` and `warn_step0_line2` use `{}`).
/// - `entries`: Overlap rows.
/// - `step`: `0` first acknowledgment, `1` migration explanation.
/// - `list_scroll`: Vertical scroll offset for the package list viewport on both steps.
///
/// Output:
/// - Extends `lines` with the current acknowledgment content and footer hint.
///
/// Details:
/// - Both steps render the same scrollable list viewport after their step-specific copy.
fn append_foreign_overlap_warn_ack(
    lines: &mut Vec<Line<'static>>,
    app: &AppState,
    th: &Theme,
    repo_name: &str,
    entries: &[(String, String)],
    step: u8,
    list_scroll: u16,
) {
    let red = Style::default().fg(th.red).add_modifier(Modifier::BOLD);
    let text_fg = Style::default().fg(th.text);
    let sub_fg = Style::default().fg(th.subtext1);
    if step == 0 {
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.foreign_overlap.warn_step0_line1",
                repo_name,
            ),
            red,
        )));
        lines.push(Line::from(Span::styled(
            i18n::t_fmt1(
                app,
                "app.modals.foreign_overlap.warn_step0_line2",
                repo_name,
            ),
            text_fg,
        )));
        lines.push(Line::from(""));
        push_i18n_detail_paragraphs(
            lines,
            app,
            &[
                "app.modals.foreign_overlap.warn_step0_detail1",
                "app.modals.foreign_overlap.warn_step0_detail2",
                "app.modals.foreign_overlap.warn_step0_detail3",
                "app.modals.foreign_overlap.warn_step0_detail4",
            ],
            text_fg,
        );
    } else {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.foreign_overlap.warn_step1_line"),
            red,
        )));
        lines.push(Line::from(""));
        push_i18n_detail_paragraphs(
            lines,
            app,
            &[
                "app.modals.foreign_overlap.warn_step1_detail1",
                "app.modals.foreign_overlap.warn_step1_detail2",
                "app.modals.foreign_overlap.warn_step1_detail3",
                "app.modals.foreign_overlap.warn_step1_detail4",
            ],
            text_fg,
        );
    }
    push_overlap_warn_list_viewport(lines, app, entries, list_scroll, text_fg, sub_fg);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.foreign_overlap.warn_ack_hint"),
        sub_fg,
    )));
}

/// What: Draw the one-step AUR vs repository duplicate warning.
///
/// Inputs:
/// - `f`: Frame.
/// - `app`: Application state for translations.
/// - `area`: Full screen bounds.
/// - `dup_names`: Conflicting `pkgname` values.
///
/// Output:
/// - Renders a red-accented confirmation panel.
///
/// Details:
/// - Lists up to a screenful of names; users continue or cancel from the modal handler.
#[allow(clippy::many_single_char_names, clippy::vec_init_then_push)]
pub fn render_warn_aur_repo_duplicate(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    dup_names: &[String],
) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(88);
    let h = area.height.saturating_sub(6).min(22);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let red = Style::default().fg(th.red).add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.warn_aur_repo_duplicate.title"),
        red,
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.warn_aur_repo_duplicate.line1"),
        red,
    )));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.warn_aur_repo_duplicate.line2"),
        Style::default().fg(th.text),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.warn_aur_repo_duplicate.names_heading"),
        Style::default().fg(th.subtext1),
    )));
    let max_rows = (h as usize).saturating_sub(10);
    for n in dup_names.iter().take(max_rows) {
        lines.push(Line::from(Span::styled(
            format!("  • {n}"),
            Style::default().fg(th.text),
        )));
    }
    if dup_names.len() > max_rows {
        lines.push(Line::from(Span::styled(
            format!("  … (+{} more)", dup_names.len() - max_rows),
            Style::default().fg(th.subtext1),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.warn_aur_repo_duplicate.hint"),
        Style::default().fg(th.subtext1),
    )));
    let p = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.red)),
        );
    f.render_widget(p, rect);
}

/// What: Draw the multi-phase foreign overlap wizard.
///
/// Inputs:
/// - `f`: Frame.
/// - `app`: Application state.
/// - `area`: Full screen bounds.
/// - `repo_name`: Repository that was applied.
/// - `entries`: Overlap rows `(pkgname, version)`.
/// - `phase`: Active wizard phase.
///
/// Output:
/// - Renders the current step with red emphasis on warnings.
///
/// Details:
/// - Delegates `WarnAck` body to `append_foreign_overlap_warn_ack` (step-specific copy plus shared list viewport).
#[allow(
    clippy::many_single_char_names,
    clippy::vec_init_then_push,
    clippy::too_many_arguments
)]
pub fn render_foreign_repo_overlap(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    repo_name: &str,
    entries: &[(String, String)],
    phase: &ForeignRepoOverlapPhase,
) {
    let th = theme();
    let w = area.width.saturating_sub(4).min(92);
    let h = area.height.saturating_sub(4).min(32);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let red = Style::default().fg(th.red).add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.foreign_overlap.title"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    match phase {
        ForeignRepoOverlapPhase::WarnAck { step, list_scroll } => {
            append_foreign_overlap_warn_ack(
                &mut lines,
                app,
                &th,
                repo_name,
                entries,
                *step,
                *list_scroll,
            );
        }
        ForeignRepoOverlapPhase::Select {
            cursor,
            list_scroll,
            selected,
        } => {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.foreign_overlap.select_heading"),
                Style::default().fg(th.text),
            )));
            lines.push(Line::from(""));
            let start = usize::from(*list_scroll);
            for row_idx in start..start.saturating_add(LIST_VIEWPORT_ROWS) {
                let Some(row) = entries.get(row_idx) else {
                    break;
                };
                let mark = if selected.contains(&row.0) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let style = if row_idx == *cursor {
                    Style::default().fg(th.yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(th.text)
                };
                lines.push(Line::from(Span::styled(
                    format!("{mark}  {} {}", row.0, row.1),
                    style,
                )));
            }
        }
        ForeignRepoOverlapPhase::FinalConfirm { selected, .. } => {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.foreign_overlap.final_heading"),
                red,
            )));
            lines.push(Line::from(""));
            let mut names: Vec<String> = selected.iter().cloned().collect();
            names.sort();
            let joined = names.join(" ");
            lines.push(Line::from(Span::styled(
                joined,
                Style::default().fg(th.text),
            )));
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.foreign_overlap.final_hint"),
                Style::default().fg(th.subtext1),
            )));
        }
    }

    let p = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.overlay1)),
        );
    f.render_widget(p, rect);
}
