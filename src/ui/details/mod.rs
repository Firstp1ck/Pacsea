use ratatui::{Frame, prelude::Rect, widgets::Wrap};

use crate::state::AppState;

/// Comments viewer rendering.
mod comments;
/// Footer rendering for details pane.
mod footer;
/// Layout calculation for details pane.
mod layout;
/// Package information rendering.
mod package_info;
/// PKGBUILD viewer rendering.
mod pkgbuild;
/// PKGBUILD syntax highlighting.
mod pkgbuild_highlight;

/// What: Render the bottom details pane, footer, optional PKGBUILD viewer, and optional comments viewer.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (details, PKGBUILD, comments, footer flags)
/// - `area`: Target rectangle for the details section
///
/// Output:
/// - Draws package information, optional PKGBUILD, optional comments, and footer while updating mouse hit-test rects.
///
/// Details:
/// - Computes layout splits for details, PKGBUILD, comments, and footer; records rects on [`AppState`] for
///   URL/PKGBUILD/comments interaction and toggles footer visibility based on available height.
pub fn render_details(f: &mut Frame, app: &mut AppState, area: Rect) {
    // Calculate footer height and layout areas
    let footer_height = layout::calculate_footer_height(app, area);
    let (_content_container, details_area, pkgb_area_opt, comments_area_opt, show_keybinds) =
        layout::calculate_layout_areas(app, area, footer_height);

    // Render Package Info pane
    package_info::render_package_info(f, app, details_area);

    // Render PKGBUILD pane if visible
    if let Some(pkgb_area) = pkgb_area_opt {
        pkgbuild::render_pkgbuild(f, app, pkgb_area);
    }

    // Render comments pane if visible
    if let Some(comments_area) = comments_area_opt {
        comments::render_comments(f, app, comments_area);
    }

    // Render footer/keybinds if enabled and there's space
    if show_keybinds {
        footer::render_footer(f, app, area, footer_height);
    }
}

/// What: Render news feed details pane when in news mode.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (news results/selection)
/// - `area`: Target rectangle for details
///
/// Output:
/// - Draws selected news item metadata and body.
// Track last-logged news selection to avoid log spam.
static LAST_LOGGED_NEWS_SEL: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(usize::MAX);

/// What: Render news details (title, metadata, article content) with code highlighting.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state (news results, selection, content cache)
/// - `area`: Target rectangle for the details pane
///
/// Output:
/// - Draws the news details pane, records clickable URL rect, and supports scroll/wrap.
pub fn render_news_details(f: &mut Frame, app: &mut AppState, area: Rect) {
    use std::sync::atomic::Ordering;
    let th = crate::theme::theme();
    let selected = app.news_results.get(app.news_selected).cloned();
    // Only log when selection changes (tracked via static)
    if LAST_LOGGED_NEWS_SEL.swap(app.news_selected, Ordering::Relaxed) != app.news_selected {
        tracing::debug!(
            news_selected = app.news_selected,
            news_results_len = app.news_results.len(),
            selected_title = selected.as_ref().map(|s| s.title.as_str()),
            "render_news_details: selection changed"
        );
    }
    let lines = selected.map_or_else(
        || vec![ratatui::text::Line::from("No news selected")],
        |item| build_news_body(app, &item, area, &th),
    );

    let footer_h: u16 = if app.show_keybinds_footer {
        footer::news_footer_height(app).min(area.height)
    } else {
        0
    };
    let content_height = area.height.saturating_sub(footer_h);
    let content_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: content_height,
    };

    let paragraph = ratatui::widgets::Paragraph::new(lines)
        .style(ratatui::style::Style::default().fg(th.text).bg(th.base))
        .block(
            ratatui::widgets::Block::default()
                .title(ratatui::text::Span::styled(
                    crate::i18n::t(app, "app.results.options_menu.news"),
                    ratatui::style::Style::default().fg(th.mauve),
                ))
                .borders(ratatui::widgets::Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .border_style(ratatui::style::Style::default().fg(th.surface2)),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.news_content_scroll, 0));
    f.render_widget(paragraph, content_area);
    app.details_rect = Some((
        content_area.x,
        content_area.y,
        content_area.width,
        content_area.height,
    ));

    if footer_h > 0 && area.height >= footer_h {
        footer::render_news_footer(f, app, area, footer_h);
    }
}

/// What: Build the lines for news metadata and content (without rendering).
///
/// Inputs:
/// - `app`: Application state (used for URL rects and content state)
/// - `item`: Selected news item
/// - `area`: Target rectangle (for URL hit-test geometry)
/// - `th`: Theme for styling
///
/// Output:
/// - Vector of lines ready to render in the details pane.
fn build_news_body(
    app: &mut AppState,
    item: &crate::state::types::NewsFeedItem,
    area: Rect,
    th: &crate::theme::Theme,
) -> Vec<ratatui::text::Line<'static>> {
    let mut body: Vec<ratatui::text::Line<'static>> = Vec::new();
    body.push(ratatui::text::Line::from(ratatui::text::Span::styled(
        item.title.clone(),
        ratatui::style::Style::default()
            .fg(th.mauve)
            .add_modifier(ratatui::style::Modifier::BOLD),
    )));
    body.push(ratatui::text::Line::from(""));
    body.push(ratatui::text::Line::from(format!("Date: {}", item.date)));
    body.push(ratatui::text::Line::from(format!(
        "Source: {:?}",
        item.source
    )));
    if let Some(sev) = item.severity {
        body.push(ratatui::text::Line::from(format!("Severity: {sev:?}")));
    }
    if !item.packages.is_empty() {
        body.push(ratatui::text::Line::from(format!(
            "Packages: {}",
            item.packages.join(", ")
        )));
    }
    if let Some(summary) = item.summary.clone() {
        body.push(ratatui::text::Line::from(""));
        body.push(ratatui::text::Line::from(summary));
    }
    if let Some(url) = item.url.clone() {
        let link_label = crate::i18n::t(app, "app.details.open_url_label");
        body.push(ratatui::text::Line::from(""));
        body.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                link_label.clone(),
                ratatui::style::Style::default()
                    .fg(th.mauve)
                    .add_modifier(ratatui::style::Modifier::UNDERLINED)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            ),
        ]));

        app.details.url.clone_from(&url);
        let line_idx = body.len().saturating_sub(1);
        let y = area
            .y
            .saturating_add(1 + u16::try_from(line_idx).unwrap_or(0));
        let x = area.x.saturating_add(1);
        let w = u16::try_from(link_label.len()).unwrap_or(20);
        app.url_button_rect = Some((x, y, w, 1));
    } else {
        app.details.url.clear();
        app.url_button_rect = None;
    }

    body.push(ratatui::text::Line::from(""));
    body.push(ratatui::text::Line::from(ratatui::text::Span::styled(
        "─── Article Content ───",
        ratatui::style::Style::default().fg(th.surface2),
    )));
    body.push(ratatui::text::Line::from(""));

    if app.news_content_loading {
        body.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            "Loading content...",
            ratatui::style::Style::default().fg(th.overlay1),
        )));
        return body;
    }

    let Some(content) = &app.news_content else {
        body.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            "Content not available",
            ratatui::style::Style::default().fg(th.overlay1),
        )));
        return body;
    };

    if content.is_empty() {
        body.push(ratatui::text::Line::from(ratatui::text::Span::styled(
            "Content not available",
            ratatui::style::Style::default().fg(th.overlay1),
        )));
        return body;
    }

    body.extend(render_news_content_lines(content, th));
    body
}

/// What: Render article content into styled lines, supporting fenced and inline code.
///
/// Inputs:
/// - `content`: Plaintext article content
/// - `th`: Theme to style normal and code text
///
/// Output:
/// - Vector of lines with code highlighting applied.
fn render_news_content_lines(
    content: &str,
    th: &crate::theme::Theme,
) -> Vec<ratatui::text::Line<'static>> {
    let code_block_style = ratatui::style::Style::default()
        .fg(th.lavender)
        .bg(th.surface1)
        .add_modifier(ratatui::style::Modifier::BOLD);
    let inline_code_style = ratatui::style::Style::default()
        .fg(th.lavender)
        .add_modifier(ratatui::style::Modifier::ITALIC);
    let link_style = ratatui::style::Style::default()
        .fg(th.sapphire)
        .add_modifier(ratatui::style::Modifier::UNDERLINED | ratatui::style::Modifier::BOLD);
    let normal_style = ratatui::style::Style::default().fg(th.text);

    let mut rendered: Vec<ratatui::text::Line<'static>> = Vec::new();
    let mut in_code_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            rendered.push(ratatui::text::Line::from(""));
            continue;
        }

        if in_code_block {
            rendered.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                line.to_string(),
                code_block_style,
            )));
            continue;
        }

        let mut spans: Vec<ratatui::text::Span> = Vec::new();
        let mut is_code = false;
        for (i, part) in line.split('`').enumerate() {
            if i > 0 {
                is_code = !is_code;
            }
            if part.is_empty() {
                continue;
            }
            if is_code {
                spans.push(ratatui::text::Span::styled(
                    part.to_string(),
                    inline_code_style,
                ));
            } else {
                spans.extend(style_links(part, normal_style, link_style));
            }
        }
        if spans.is_empty() {
            rendered.push(ratatui::text::Line::from(ratatui::text::Span::styled(
                line.to_string(),
                normal_style,
            )));
        } else {
            rendered.push(ratatui::text::Line::from(spans));
        }
    }
    rendered
}

/// What: Style inline links within a text segment by underlining URLs.
///
/// Inputs:
/// - `segment`: Raw text segment (outside of code spans) to scan.
/// - `normal_style`: Style applied to non-link text.
/// - `link_style`: Style applied to detected URLs.
///
/// Output:
/// - Spans with URLs underlined/bold for better visibility; whitespace preserved.
fn style_links(
    segment: &str,
    normal_style: ratatui::style::Style,
    link_style: ratatui::style::Style,
) -> Vec<ratatui::text::Span<'static>> {
    let mut spans: Vec<ratatui::text::Span> = Vec::new();
    let mut current = String::new();
    let flush_current = |spans: &mut Vec<ratatui::text::Span>, cur: &mut String| {
        if !cur.is_empty() {
            spans.push(ratatui::text::Span::styled(cur.clone(), normal_style));
            cur.clear();
        }
    };

    let mut word = String::new();
    for ch in segment.chars() {
        if ch.is_whitespace() {
            if !word.is_empty() {
                let span_style = if word.starts_with("http://") || word.starts_with("https://") {
                    link_style
                } else {
                    normal_style
                };
                flush_current(&mut spans, &mut current);
                spans.push(ratatui::text::Span::styled(word.clone(), span_style));
                word.clear();
            }
            current.push(ch);
            continue;
        }
        if !current.is_empty() {
            flush_current(&mut spans, &mut current);
        }
        word.push(ch);
    }

    if !word.is_empty() {
        let span_style = if word.starts_with("http://") || word.starts_with("https://") {
            link_style
        } else {
            normal_style
        };
        flush_current(&mut spans, &mut current);
        spans.push(ratatui::text::Span::styled(word, span_style));
    }
    flush_current(&mut spans, &mut current);

    spans
}

#[cfg(test)]
mod tests {
    /// What: Initialize minimal English translations for tests.
    ///
    /// Inputs:
    /// - `app`: `AppState` to populate with translations
    ///
    /// Output:
    /// - Populates `app.translations` and `app.translations_fallback` with minimal English translations
    ///
    /// Details:
    /// - Sets up only the translations needed for tests to pass
    fn init_test_translations(app: &mut crate::state::AppState) {
        use std::collections::HashMap;
        let mut translations = HashMap::new();
        translations.insert("app.details.fields.url".to_string(), "URL".to_string());
        translations.insert("app.details.url_label".to_string(), "URL:".to_string());
        translations.insert(
            "app.details.open_url_label".to_string(),
            "[Open in Browser]".to_string(),
        );
        translations.insert(
            "app.results.options_menu.news".to_string(),
            "News".to_string(),
        );
        app.translations = translations.clone();
        app.translations_fallback = translations;
    }

    /// What: Confirm rendering the details pane records hit-test rectangles and disables mouse interactions when appropriate.
    ///
    /// Inputs:
    /// - `AppState` containing package details with a URL and an expanded PKGBUILD view.
    ///
    /// Output:
    /// - Details, URL button, PKGBUILD toggle, and PKGBUILD area rectangles become `Some`, and the mouse flag toggles off.
    ///
    /// Details:
    /// - Uses a `TestBackend` terminal to drive layout without user interaction, ensuring the renderer updates state.
    #[test]
    fn details_sets_url_and_pkgb_rects() {
        use ratatui::{Terminal, backend::TestBackend};
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).expect("failed to create test terminal");

        let mut app = crate::state::AppState::default();
        init_test_translations(&mut app);
        app.details = crate::state::PackageDetails {
            repository: "extra".into(),
            name: "ripgrep".into(),
            version: "14".into(),
            description: String::new(),
            architecture: "x86_64".into(),
            url: "https://example.com".into(),
            licenses: vec![],
            groups: vec![],
            provides: vec![],
            depends: vec![],
            opt_depends: vec![],
            required_by: vec![],
            optional_for: vec![],
            conflicts: vec![],
            replaces: vec![],
            download_size: None,
            install_size: None,
            owner: String::new(),
            build_date: String::new(),
            popularity: None,
            out_of_date: None,
            orphaned: false,
        };
        // Show PKGBUILD area
        app.pkgb_visible = true;
        app.pkgb_text = Some("line1\nline2\nline3".into());

        term.draw(|f| {
            let area = f.area();
            super::render_details(f, &mut app, area);
        })
        .expect("failed to draw test terminal");

        assert!(app.details_rect.is_some());
        assert!(app.url_button_rect.is_some());
        assert!(app.pkgb_button_rect.is_some());
        assert!(app.pkgb_check_button_rect.is_some());
        assert!(app.pkgb_rect.is_some());
        assert!(app.mouse_disabled_in_details);
    }
}
