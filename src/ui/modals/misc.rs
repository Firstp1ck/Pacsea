use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::common::render_simple_list_modal;
use crate::state::{
    AppState,
    types::{OptionalDepRow, RepositoryKeyTrust, RepositoryModalRow, RepositoryPacmanStatus},
};
use crate::theme::theme;

/// What: Render the optional dependencies modal with install status indicators.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `rows`: Optional dependency entries (label, package, status)
/// - `selected`: Index of the currently highlighted row
///
/// Output:
/// - Draws the modal content and highlights the selected row; no state mutations besides rendering.
///
/// Details:
/// - Marks installed rows, shows optional notes, and reuses the common simple modal renderer for
///   consistent styling.
pub fn render_optional_deps(
    f: &mut Frame,
    area: Rect,
    rows: &[OptionalDepRow],
    selected: usize,
    app: &crate::state::AppState,
) {
    let th = theme();
    // Build content lines with selection and install status markers
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.optional_deps.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    for (i, row) in rows.iter().enumerate() {
        let is_sel = selected == i;
        let (mark, color) = if row.installed {
            (
                crate::i18n::t(app, "app.modals.optional_deps.markers.installed"),
                th.green,
            )
        } else {
            (
                crate::i18n::t(app, "app.modals.optional_deps.markers.not_installed"),
                th.overlay1,
            )
        };
        let style = if is_sel {
            Style::default()
                .fg(th.crust)
                .bg(th.lavender)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.text)
        };
        let mut segs: Vec<Span> = Vec::new();
        segs.push(Span::styled(format!("{}  ", row.label), style));
        segs.push(Span::styled(
            format!("[{}]", row.package),
            Style::default().fg(th.overlay1),
        ));
        segs.push(Span::raw("  "));
        segs.push(Span::styled(mark.clone(), Style::default().fg(color)));
        if let Some(note) = &row.note {
            segs.push(Span::raw("  "));
            segs.push(Span::styled(
                format!("({note})"),
                Style::default().fg(th.overlay2),
            ));
        }
        lines.push(Line::from(segs));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.optional_deps.footer_hint"),
        Style::default().fg(th.subtext1),
    )));

    render_simple_list_modal(
        f,
        area,
        &crate::i18n::t(app, "app.modals.optional_deps.title"),
        lines,
    );
}

/// What: Draw the read-only Repositories modal listing `repos.conf` vs pacman sections.
///
/// Inputs:
/// - `frame`: Frame to render into.
/// - `area`: Full-screen area for centering.
/// - `rows`: Merged repository rows.
/// - `selected` / `scroll`: List focus and window start.
/// - `repos_conf_error` / `pacman_warnings`: Optional diagnostics.
/// - `app`: For i18n lookup.
///
/// Output:
/// - Renders a scrollable table-style list; does not mutate app state.
///
/// Details:
/// - Uses a wider box than `render_simple_list_modal` to fit column hints.
#[allow(clippy::too_many_arguments)]
pub fn render_repositories(
    frame: &mut Frame,
    area: Rect,
    rows: &[RepositoryModalRow],
    selected: usize,
    scroll: u16,
    repos_conf_error: Option<&str>,
    pacman_warnings: &[String],
    app: &AppState,
) {
    const VIEWPORT: usize = 12;
    let th = theme();
    let box_w = area.width.saturating_sub(6).min(102);
    let box_h = area.height.saturating_sub(6).min(28);
    let box_x = area.x + (area.width.saturating_sub(box_w)) / 2;
    let box_y = area.y + (area.height.saturating_sub(box_h)) / 2;
    let rect = Rect {
        x: box_x,
        y: box_y,
        width: box_w,
        height: box_h,
    };
    frame.render_widget(Clear, rect);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.repositories.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    if let Some(err) = repos_conf_error {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {err}",
                crate::i18n::t(app, "app.modals.repositories.parse_error")
            ),
            Style::default().fg(th.red),
        )));
        lines.push(Line::from(""));
    }

    if rows.is_empty() && repos_conf_error.is_none() {
        lines.push(Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.repositories.empty"),
            Style::default().fg(th.subtext1),
        )));
        lines.push(Line::from(""));
    }

    let header_style = Style::default()
        .fg(th.subtext0)
        .add_modifier(Modifier::BOLD);
    lines.push(Line::from(vec![
        Span::styled(
            pad_right_display(&crate::i18n::t(app, "app.modals.repositories.col.repo"), 22),
            header_style,
        ),
        Span::styled(
            pad_right_display(
                &crate::i18n::t(app, "app.modals.repositories.col.filter"),
                16,
            ),
            header_style,
        ),
        Span::styled(
            pad_right_display(
                &crate::i18n::t(app, "app.modals.repositories.col.pacman"),
                12,
            ),
            header_style,
        ),
        Span::styled(
            crate::i18n::t(app, "app.modals.repositories.col.key"),
            header_style,
        ),
    ]));
    lines.push(Line::from(""));

    let scroll_u = usize::from(scroll);
    let start = scroll_u.min(rows.len());
    let end = (start + VIEWPORT).min(rows.len());
    for (rel_i, row) in rows[start..end].iter().enumerate() {
        let i_global = start + rel_i;
        let is_sel = selected == i_global;
        let style = if is_sel {
            Style::default()
                .fg(th.crust)
                .bg(th.lavender)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.text)
        };
        let pname = pad_right_display(&truncate_display(&row.pacman_section_name, 20), 22);
        let pfilter = pad_right_display(&truncate_display(&row.results_filter_display, 14), 16);
        let pst = pad_right_display(&pacman_status_label(app, row.pacman_status), 12);
        let pk = key_trust_label(app, row.key_trust);
        let hint = row
            .source_hint
            .as_deref()
            .map(|s| format!(" [{}]", truncate_display(s, 20)))
            .unwrap_or_default();
        lines.push(Line::from(vec![
            Span::styled(pname, style),
            Span::styled(pfilter, style),
            Span::styled(pst, style),
            Span::styled(format!("{pk}{hint}"), style),
        ]));
    }

    lines.push(Line::from(""));
    if !pacman_warnings.is_empty() {
        let wtext = pacman_warnings
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(Line::from(Span::styled(
            format!(
                "{} {wtext}",
                crate::i18n::t(app, "app.modals.repositories.warnings_prefix")
            ),
            Style::default().fg(th.yellow),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        crate::i18n::t(app, "app.modals.repositories.footer_hint"),
        Style::default().fg(th.subtext1),
    )));

    let repo_paragraph = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(ratatui::text::Span::styled(
                    format!(" {} ", crate::i18n::t(app, "app.modals.repositories.title")),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    frame.render_widget(repo_paragraph, rect);
}

/// What: Truncate `s` to a maximum terminal display width, appending an ellipsis when shortened.
///
/// Inputs:
/// - `s`: Source text.
/// - `max_width`: Maximum display columns ([`UnicodeWidthStr::width`]); the result fits within this
///   width when rendered in a typical monospace terminal.
///
/// Output:
/// - Owned string at most `max_width` display columns, or empty when `max_width` is zero.
///
/// Details:
/// - Uses [`UnicodeWidthChar`] per scalar value (same approach as `results/status.rs`); combining
///   sequences are not grapheme-cluster aware.
fn truncate_display(s: &str, max_width: usize) -> String {
    const ELLIPSIS: char = '…';
    let ellipsis_w = ELLIPSIS.width().unwrap_or(0);
    let w = s.width();
    if w <= max_width {
        return s.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    let budget = max_width.saturating_sub(ellipsis_w);
    let mut out = String::new();
    let mut width_so_far = 0usize;
    for ch in s.chars() {
        let ch_w = ch.width().unwrap_or(0);
        if width_so_far.saturating_add(ch_w) > budget {
            break;
        }
        out.push(ch);
        width_so_far = width_so_far.saturating_add(ch_w);
    }
    out.push(ELLIPSIS);
    out
}

/// What: Append ASCII spaces so `s` spans at least `target_width` terminal display columns.
///
/// Inputs:
/// - `s`: Text to pad (callers should truncate first if it may exceed `target_width`).
/// - `target_width`: Minimum display width for the returned string.
///
/// Output:
/// - `s` unchanged when already wide enough; otherwise `s` plus trailing spaces.
///
/// Details:
/// - Matches [`UnicodeWidthStr::width`] so padding aligns with `truncate_display` and Ratatui spans.
fn pad_right_display(s: &str, target_width: usize) -> String {
    let w = s.width();
    if w >= target_width {
        return s.to_string();
    }
    let pad = target_width - w;
    format!("{s}{}", " ".repeat(pad))
}

/// What: Localized label for pacman section presence.
///
/// Inputs:
/// - `app`: For i18n.
/// - `st`: Enumeration from the row model.
///
/// Output:
/// - Short uppercase-ish token for the column.
fn pacman_status_label(app: &AppState, st: RepositoryPacmanStatus) -> String {
    match st {
        RepositoryPacmanStatus::Absent => {
            crate::i18n::t(app, "app.modals.repositories.pacman.absent")
        }
        RepositoryPacmanStatus::Active => {
            crate::i18n::t(app, "app.modals.repositories.pacman.active")
        }
        RepositoryPacmanStatus::Commented => {
            crate::i18n::t(app, "app.modals.repositories.pacman.commented")
        }
    }
}

/// What: Localized label for keyring trust column.
///
/// Inputs:
/// - `app`: For i18n.
/// - `kt`: Trust enum.
///
/// Output:
/// - Compact column text.
fn key_trust_label(app: &AppState, kt: RepositoryKeyTrust) -> String {
    match kt {
        RepositoryKeyTrust::NotApplicable => crate::i18n::t(app, "app.modals.repositories.key.na"),
        RepositoryKeyTrust::Trusted => crate::i18n::t(app, "app.modals.repositories.key.trusted"),
        RepositoryKeyTrust::NotTrusted => {
            crate::i18n::t(app, "app.modals.repositories.key.not_trusted")
        }
        RepositoryKeyTrust::Unknown => crate::i18n::t(app, "app.modals.repositories.key.unknown"),
    }
}

/// What: Render the guided AUR SSH setup modal.
///
/// Inputs:
/// - `f`: Frame to render into.
/// - `area`: Full screen area used to center the modal.
/// - `step`: Current SSH setup step.
/// - `status_lines`: Current status/instruction lines.
/// - `existing_host_block`: Existing host block shown during overwrite confirmation.
///
/// Output:
/// - Draws the SSH setup modal content.
#[allow(clippy::many_single_char_names)]
pub fn render_ssh_aur_setup(
    f: &mut Frame,
    area: Rect,
    step: crate::state::SshSetupStep,
    status_lines: &[String],
    existing_host_block: Option<&str>,
) {
    let th = theme();
    let w = area.width.saturating_sub(8).min(100);
    let h = area.height.saturating_sub(6).min(24);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let (title, accent, footer) = match step {
        crate::state::SshSetupStep::Intro => (
            "AUR SSH Setup",
            th.mauve,
            "Enter: run setup  |  O: open AUR account page  |  Esc: cancel",
        ),
        crate::state::SshSetupStep::ConfirmOverwrite => (
            "AUR SSH Setup: Confirm Overwrite",
            th.yellow,
            "Y/Enter: overwrite block  |  N/Esc: keep existing config  |  O: open account page",
        ),
        crate::state::SshSetupStep::Result => (
            "AUR SSH Setup: Result",
            th.green,
            "Enter/Esc: close  |  O: open account page",
        ),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    for line in status_lines {
        lines.push(Line::from(Span::styled(
            line.clone(),
            Style::default().fg(th.text),
        )));
    }
    if let Some(block) = existing_host_block {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Existing Host aur.archlinux.org block:",
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        )));
        for line in block.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!(
            "AUR account page: {}",
            crate::logic::ssh_setup::AUR_ACCOUNT_URL
        ),
        Style::default().fg(th.sapphire),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        footer,
        Style::default().fg(th.overlay1),
    )));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true }).block(
        Block::default()
            .title(Span::styled(
                title,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(accent))
            .style(Style::default().bg(th.mantle)),
    );
    f.render_widget(paragraph, rect);
}

#[allow(clippy::too_many_arguments)]
/// What: Render the scan configuration modal listing security tools to toggle.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `do_clamav`…`do_sleuth`: Flags indicating which scanners are enabled
/// - `cursor`: Index of the row currently focused
///
/// Output:
/// - Draws the configuration list, highlighting the focused entry and indicating current toggles.
///
/// Details:
/// - Presents each scanner with checkboxes, respecting theme emphasis for the cursor and summarizing
///   available shortcuts at the bottom.
#[allow(clippy::fn_params_excessive_bools)]
pub fn render_scan_config(
    f: &mut Frame,
    area: Rect,
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    do_sleuth: bool,
    cursor: usize,
) {
    let th = theme();
    let mut lines: Vec<Line<'static>> = Vec::new();

    let items: [(&str, bool); 7] = [
        ("ClamAV (antivirus)", do_clamav),
        ("Trivy (filesystem)", do_trivy),
        ("Semgrep (static analysis)", do_semgrep),
        ("ShellCheck (PKGBUILD/.install)", do_shellcheck),
        ("VirusTotal (hash lookups)", do_virustotal),
        ("Custom scan for Suspicious patterns", do_custom),
        ("aur-sleuth (LLM audit)", do_sleuth),
    ];

    for (i, (label, checked)) in items.iter().enumerate() {
        let mark = if *checked { "[x]" } else { "[ ]" };
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            format!("{mark} "),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        ));
        let style = if i == cursor {
            Style::default()
                .fg(th.text)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(th.subtext1)
        };
        spans.push(Span::styled((*label).to_string(), style));
        lines.push(Line::from(spans));
    }

    lines.push(Line::from(Span::raw("")));
    lines.push(Line::from(Span::styled(
        "Up/Down: select  •  Space: toggle  •  Enter: run  •  Esc: cancel",
        Style::default().fg(th.overlay1),
    )));

    render_simple_list_modal(f, area, "Scan Configuration", lines);
}

/// What: Render the news setup modal for configuring startup news popup.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `app`: Application state for i18n
/// - `show_arch_news`…`show_pkg_updates`: Flags indicating which news sources are enabled
/// - `max_age_days`: Selected maximum age (7, 30, or 90)
/// - `cursor`: Index of the row currently focused (0-4 for toggles, 5-7 for date buttons)
///
/// Output:
/// - Draws the configuration list, highlighting the focused entry and indicating current toggles.
///
/// Details:
/// - Presents 5 news source toggles with checkboxes, then date selection buttons (7/30/90 days).
/// - Respects theme emphasis for the cursor and summarizes available shortcuts at the bottom.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
pub fn render_news_setup(
    f: &mut Frame,
    area: Rect,
    app: &AppState,
    show_arch_news: bool,
    show_advisories: bool,
    show_aur_updates: bool,
    show_aur_comments: bool,
    show_pkg_updates: bool,
    max_age_days: Option<u32>,
    cursor: usize,
) {
    let th = theme();
    let mut lines: Vec<Line<'static>> = Vec::new();

    // News source toggles (cursor 0-4)
    let items: [(&str, bool); 5] = [
        (
            &crate::i18n::t(app, "app.modals.news_setup.arch_news"),
            show_arch_news,
        ),
        (
            &crate::i18n::t(app, "app.modals.news_setup.advisories"),
            show_advisories,
        ),
        (
            &crate::i18n::t(app, "app.modals.news_setup.aur_updates"),
            show_aur_updates,
        ),
        (
            &crate::i18n::t(app, "app.modals.news_setup.aur_comments"),
            show_aur_comments,
        ),
        (
            &crate::i18n::t(app, "app.modals.news_setup.pkg_updates"),
            show_pkg_updates,
        ),
    ];

    for (i, (label, checked)) in items.iter().enumerate() {
        let mark = if *checked { "[x]" } else { "[ ]" };
        let mut spans: Vec<Span> = Vec::new();
        spans.push(Span::styled(
            format!("{mark} "),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        ));
        let style = if i == cursor {
            Style::default()
                .fg(th.text)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(th.subtext1)
        };
        spans.push(Span::styled((*label).to_string(), style));
        lines.push(Line::from(spans));
    }

    // Date selection row (cursor 5-7)
    lines.push(Line::from(""));
    let date_label = crate::i18n::t(app, "app.modals.news_setup.date_selection");
    lines.push(Line::from(Span::styled(
        format!("{date_label}:"),
        Style::default().fg(th.subtext1),
    )));

    let date_options = [7, 30, 90];
    let mut date_spans: Vec<Span> = Vec::new();
    for (i, &days) in date_options.iter().enumerate() {
        let date_cursor = 5 + i; // cursor 5, 6, 7
        let is_selected = max_age_days == Some(days);
        let is_cursor = cursor == date_cursor;
        let button_text = if is_selected {
            format!("[{days} days]")
        } else {
            format!(" {days} days ")
        };
        let style = if is_cursor {
            Style::default()
                .fg(th.text)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else if is_selected {
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(th.subtext1)
        };
        date_spans.push(Span::styled(button_text.clone(), style));
        if i < date_options.len() - 1 {
            date_spans.push(Span::raw("  "));
        }
    }
    lines.push(Line::from(date_spans));

    lines.push(Line::from(Span::raw("")));
    let footer_hint = crate::i18n::t(app, "app.modals.news_setup.footer_hint");
    lines.push(Line::from(Span::styled(
        footer_hint,
        Style::default().fg(th.overlay1),
    )));

    render_simple_list_modal(
        f,
        area,
        &crate::i18n::t(app, "app.modals.news_setup.title"),
        lines,
    );
}

/// What: Render the prompt encouraging installation of GNOME Terminal in GNOME environments.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
///
/// Output:
/// - Draws a concise confirmation dialog describing recommended action and key hints.
///
/// Details:
/// - Highlights the heading, explains why the terminal is recommended, and warns about cancelling.
#[allow(clippy::many_single_char_names)]
pub fn render_gnome_terminal_prompt(f: &mut Frame, area: Rect) {
    let th = theme();
    // Centered confirmation dialog for installing GNOME Terminal
    let w = area.width.saturating_sub(10).min(90);
    let h = 9;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            "GNOME Terminal or Console recommended",
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "GNOME was detected, but no GNOME terminal (gnome-terminal or gnome-console/kgx) is installed.",
            Style::default().fg(th.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Press Enter to install gnome-terminal  •  Esc to cancel",
            Style::default().fg(th.subtext1),
        )),
        Line::from(Span::styled(
            "Cancel may lead to unexpected behavior.",
            Style::default().fg(th.yellow),
        )),
    ];

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " Install a GNOME Terminal ",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

/// What: Render the `VirusTotal` API setup modal with clickable URL and current input preview.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (records URL rect for mouse clicks)
/// - `area`: Full screen area used to center the modal
/// - `input`: Current API key buffer contents
///
/// Output:
/// - Draws the setup dialog, updates `app.vt_url_rect`, and shows current text entry.
///
/// Details:
/// - Provides direct link to the API portal, surfaces instructions, and mirrors the buffer so users
///   can verify pasted values.
#[allow(clippy::many_single_char_names)]
pub fn render_virustotal_setup(f: &mut Frame, app: &mut AppState, area: Rect, input: &str) {
    let th = theme();
    // Centered dialog for VirusTotal API key setup with clickable URL and input field
    let w = area.width.saturating_sub(10).min(90);
    let h = 11;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    // Build content
    let vt_url = "https://www.virustotal.com/gui/my-apikey";
    // Show input buffer (not masked)
    let shown = if input.is_empty() {
        "<empty>".to_string()
    } else {
        input.to_string()
    };
    let lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            "VirusTotal API Setup",
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Open the link to view your API key:",
            Style::default().fg(th.text),
        )),
        Line::from(vec![
            // Surround with spaces to avoid visual concatenation with underlying content
            Span::styled(" ", Style::default().fg(th.text)),
            Span::styled(
                vt_url.to_string(),
                Style::default()
                    .fg(th.lavender)
                    .add_modifier(Modifier::UNDERLINED | Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Enter/paste your API key below and press Enter to save (Esc to cancel):",
            Style::default().fg(th.subtext1),
        )),
        Line::from(Span::styled(
            format!("API key: {shown}"),
            Style::default().fg(th.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Tip: After saving, scans will auto-query VirusTotal by file hash.",
            Style::default().fg(th.overlay1),
        )),
    ];

    let inner_x = rect.x + 1;
    let inner_y = rect.y + 1;
    let url_line_y = inner_y + 3;
    let url_x = inner_x + 1;
    let url_w = u16::try_from(vt_url.len()).unwrap_or(u16::MAX);
    app.vt_url_rect = Some((url_x, url_line_y, url_w, 1));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    " VirusTotal ",
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

/// What: Render the import help modal describing expected file format and keybindings.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `app`: Application state for i18n translation
///
/// Output:
/// - Draws instructions for import file syntax and highlights confirm/cancel keys.
///
/// Details:
/// - Enumerates formatting rules, provides an example snippet, and keeps styling aligned with other
///   informational modals.
#[allow(clippy::many_single_char_names)]
pub fn render_import_help(f: &mut Frame, area: Rect, app: &crate::state::AppState) {
    let th = theme();
    let w = area.width.saturating_sub(10).min(85);
    let h = 19;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);

    let lines: Vec<Line<'static>> = vec![
        Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.import_help.heading"),
            Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.import_help.description"),
            Style::default().fg(th.text),
        )),
        Line::from(""),
        Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.import_help.format_label"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw(crate::i18n::t(
            app,
            "app.modals.import_help.format_one_per_line",
        ))),
        Line::from(Span::raw(crate::i18n::t(
            app,
            "app.modals.import_help.format_blank_ignored",
        ))),
        Line::from(Span::raw(crate::i18n::t(
            app,
            "app.modals.import_help.format_comments",
        ))),
        Line::from(""),
        Line::from(Span::styled(
            crate::i18n::t(app, "app.modals.import_help.example_label"),
            Style::default()
                .fg(th.overlay1)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::raw("  firefox")),
        Line::from(Span::raw(crate::i18n::t(
            app,
            "app.modals.import_help.example_comment",
        ))),
        Line::from(Span::raw("  vim")),
        Line::from(Span::raw("  paru")),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[Enter]",
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                crate::i18n::t(app, "app.modals.import_help.hint_confirm"),
                Style::default().fg(th.overlay1),
            ),
            Span::raw("  •  "),
            Span::styled(
                "[Esc]",
                Style::default().fg(th.text).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                crate::i18n::t(app, "app.modals.import_help.hint_cancel"),
                Style::default().fg(th.overlay1),
            ),
        ]),
    ];

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    crate::i18n::t(app, "app.modals.import_help.title"),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

/// What: Render a simple loading indicator modal.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full screen area used to center the modal
/// - `message`: Loading message to display
///
/// Output:
/// - Draws a centered loading modal with the given message.
///
/// Details:
/// - Shows a simple centered box with a loading message and spinner indicator.
pub fn render_loading(f: &mut Frame, area: Rect, message: &str) {
    let th = theme();

    // Small centered modal
    let width = 40_u16.min(area.width.saturating_sub(4));
    let height = 5_u16.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let rect = Rect::new(x, y, width, height);

    f.render_widget(Clear, rect);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("⏳ {message}"),
            Style::default().fg(th.text),
        )),
    ];

    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .alignment(ratatui::layout::Alignment::Center)
        .block(
            Block::default()
                .title(Span::styled(
                    " Loading ",
                    Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.yellow))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

#[cfg(test)]
mod truncate_and_pad_tests {
    use unicode_width::UnicodeWidthStr;

    use super::{pad_right_display, truncate_display};

    #[test]
    fn truncate_display_ascii_short_unchanged() {
        assert_eq!(truncate_display("core", 20), "core");
    }

    #[test]
    fn truncate_display_ascii_long_uses_ellipsis_within_width() {
        let s = truncate_display("very-long-repo-section-name-here", 20);
        assert!(s.ends_with('…'));
        assert!(s.width() <= 20);
    }

    #[test]
    fn truncate_display_cjk_respects_display_columns() {
        // Each CJK character is typically width 2; 9 chars => 18 columns, fits in 20.
        let narrow = "一二三四五六七八九";
        assert_eq!(truncate_display(narrow, 20), narrow);
        // 11 chars => 22 columns; truncate to <= 20 with ellipsis (width 1) => 9 display cols body.
        let wide = "一二三四五六七八九十甲";
        let t = truncate_display(wide, 20);
        assert!(t.ends_with('…'));
        assert!(t.width() <= 20);
    }

    #[test]
    fn pad_right_display_adds_spaces_by_display_width() {
        let s = pad_right_display("ab", 6);
        assert_eq!(s.width(), 6);
        assert_eq!(s, "ab    ");
    }

    #[test]
    fn pad_right_display_wide_prefix() {
        let s = pad_right_display("国", 6);
        assert_eq!(s.width(), 6);
        assert_eq!(s, "国    ");
    }
}
