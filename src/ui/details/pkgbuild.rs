use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

use super::pkgbuild_highlight;

/// What: Render the PKGBUILD viewer pane with scroll support and action buttons.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (PKGBUILD text, scroll, cached rects)
/// - `pkgb_area`: Rect assigned to the PKGBUILD pane
///
/// Output:
/// - Draws PKGBUILD text and updates button rectangles for copy/reload interactions.
///
/// Details:
/// - Applies scroll offset, records the scrollable inner region, and toggles presence of the reload
///   button when the cached PKGBUILD belongs to a different package.
pub fn render_pkgbuild(f: &mut Frame, app: &mut AppState, pkgb_area: Rect) {
    let th = theme();

    let loading_text = i18n::t(app, "app.details.loading_pkgb");
    let pkgb_text = app.pkgb_text.as_deref().unwrap_or(&loading_text);
    // Remember PKGBUILD rect for mouse interactions (scrolling)
    app.pkgb_rect = Some((
        pkgb_area.x + 1,
        pkgb_area.y + 1,
        pkgb_area.width.saturating_sub(2),
        pkgb_area.height.saturating_sub(2),
    ));

    // Apply vertical scroll offset by trimming top lines
    // First, get all lines (highlighted or plain)
    let all_lines = if pkgb_text == loading_text {
        // For loading text, use plain text
        vec![Line::from(loading_text)]
    } else {
        // Apply syntax highlighting
        pkgbuild_highlight::highlight_pkgbuild(pkgb_text, &th)
    };

    // Apply scroll offset
    let visible_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(app.pkgb_scroll as usize)
        .collect();
    // Title with clickable "Copy PKGBUILD" button and optional "Reload PKGBUILD" button
    let check_button_label = i18n::t(app, "app.details.copy_pkgbuild");
    let pkgb_title_text = i18n::t(app, "app.titles.pkgb");
    let mut pkgb_title_spans: Vec<Span> = vec![Span::styled(
        pkgb_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    pkgb_title_spans.push(Span::raw("  "));
    let check_btn_style = Style::default()
        .fg(th.mauve)
        .bg(th.surface2)
        .add_modifier(Modifier::BOLD);
    pkgb_title_spans.push(Span::styled(check_button_label.clone(), check_btn_style));

    // Check if PKGBUILD is for a different package than currently selected
    let current_package = app.results.get(app.selected).map(|i| i.name.as_str());
    let needs_reload =
        app.pkgb_package_name.as_deref() != current_package && app.pkgb_package_name.is_some();

    // Record clickable rect for the "Copy PKGBUILD" button on the top border row
    // Use Unicode display width, not byte length, to handle wide characters
    let btn_y = pkgb_area.y;
    let btn_x = pkgb_area
        .x
        .saturating_add(1)
        .saturating_add(u16::try_from(pkgb_title_text.width()).unwrap_or(u16::MAX))
        .saturating_add(2);
    let btn_w = u16::try_from(check_button_label.width()).unwrap_or(u16::MAX);
    app.pkgb_check_button_rect = Some((btn_x, btn_y, btn_w, 1));

    // Add "Reload PKGBUILD" button if needed
    app.pkgb_reload_button_rect = None;
    if needs_reload {
        pkgb_title_spans.push(Span::raw("  "));
        let reload_button_label = i18n::t(app, "app.details.reload_pkgbuild");
        let reload_btn_style = Style::default()
            .fg(th.mauve)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD);
        pkgb_title_spans.push(Span::styled(reload_button_label.clone(), reload_btn_style));

        // Record clickable rect for the reload button
        let reload_btn_x = btn_x.saturating_add(btn_w).saturating_add(2);
        let reload_btn_w = u16::try_from(reload_button_label.width()).unwrap_or(u16::MAX);
        app.pkgb_reload_button_rect = Some((reload_btn_x, btn_y, reload_btn_w, 1));
    }

    let pkgb = Paragraph::new(visible_lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .title(Line::from(pkgb_title_spans))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.surface2)),
        );
    f.render_widget(pkgb, pkgb_area);
}
