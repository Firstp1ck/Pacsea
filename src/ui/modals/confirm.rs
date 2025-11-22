use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::i18n;
use crate::state::{AppState, PackageItem};
use crate::theme::theme;

/// What: Render the confirmation modal listing packages slated for installation.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: `AppState` for translations
/// - `area`: Full screen area used to center the modal
/// - `items`: Packages selected for installation
///
/// Output:
/// - Draws the install confirmation dialog and informs users about scan shortcuts.
///
/// Details:
/// - Highlights the heading, truncates the list to fit the modal, and shows instructions for
///   confirming, cancelling, or initiating security scans.
#[allow(clippy::many_single_char_names)]
pub fn render_confirm_install(f: &mut Frame, app: &AppState, area: Rect, items: &[PackageItem]) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(90);
    let h = area.height.saturating_sub(6).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_install.heading"),
        Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.confirm_install.none"),
            Style::default().fg(th.subtext1),
        )));
    } else {
        for p in items.iter().take((h as usize).saturating_sub(6)) {
            lines.push(Line::from(Span::styled(
                format!("- {}", p.name),
                Style::default().fg(th.text),
            )));
        }
        if items.len() + 6 > h as usize {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.confirm_install.list_ellipsis"),
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_install.confirm_hint"),
        Style::default().fg(th.subtext1),
    )));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_install.scan_hint"),
        Style::default().fg(th.overlay1),
    )));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    i18n::t(app, "app.modals.confirm_install.title"),
                    Style::default().fg(th.mauve).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.mauve))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}

/// What: Render the confirmation modal enumerating packages selected for removal.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: `AppState` for translations
/// - `area`: Full screen area used to center the modal
/// - `items`: Packages scheduled for removal
///
/// Output:
/// - Draws the removal confirmation dialog, including warnings for core packages.
///
/// Details:
/// - Emphasizes critical warnings when core packages are present, truncates long lists, and
///   instructs on confirm/cancel actions while matching the theme.
#[allow(clippy::many_single_char_names)]
pub fn render_confirm_remove(f: &mut Frame, app: &AppState, area: Rect, items: &[PackageItem]) {
    let th = theme();
    let w = area.width.saturating_sub(6).min(90);
    let h = area.height.saturating_sub(6).min(20);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let rect = ratatui::prelude::Rect {
        x,
        y,
        width: w,
        height: h,
    };
    f.render_widget(Clear, rect);
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_remove.heading"),
        Style::default().fg(th.red).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    // Warn explicitly if any core packages are present
    let has_core = items.iter().any(|p| match &p.source {
        crate::state::Source::Official { repo, .. } => repo.eq_ignore_ascii_case("core"),
        _ => false,
    });
    if has_core {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.confirm_remove.warning_core"),
            Style::default().fg(th.red).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    if items.is_empty() {
        lines.push(Line::from(Span::styled(
            i18n::t(app, "app.modals.confirm_remove.none"),
            Style::default().fg(th.subtext1),
        )));
    } else {
        for p in items.iter().take((h as usize).saturating_sub(6)) {
            lines.push(Line::from(Span::styled(
                format!("- {}", p.name),
                Style::default().fg(th.text),
            )));
        }
        if items.len() + 6 > h as usize {
            lines.push(Line::from(Span::styled(
                i18n::t(app, "app.modals.confirm_install.list_ellipsis"),
                Style::default().fg(th.subtext1),
            )));
        }
    }
    lines.push(Line::from(""));
    // Add warning about removal and no backup
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_remove.warning_removal"),
        Style::default().fg(th.red).add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        i18n::t(app, "app.modals.confirm_remove.confirm_hint"),
        Style::default().fg(th.subtext1),
    )));
    let boxw = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.mantle))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(Span::styled(
                    i18n::t(app, "app.modals.confirm_remove.title"),
                    Style::default().fg(th.red).add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(th.red))
                .style(Style::default().bg(th.mantle)),
        );
    f.render_widget(boxw, rect);
}
