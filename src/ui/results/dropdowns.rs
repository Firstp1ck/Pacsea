use ratatui::{
    Frame,
    prelude::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Calculate menu dimensions based on options and available space.
///
/// Inputs:
/// - `opts`: Menu option strings
/// - `results_area`: Available area for positioning
/// - `extra_width`: Additional width needed (e.g., for checkboxes)
///
/// Output:
/// - Tuple of (`width`, `height`, `max_number_width`)
///
/// Details:
/// - Uses Unicode display width for accurate sizing with wide characters.
fn calculate_menu_dimensions(
    opts: &[String],
    results_area: Rect,
    extra_width: u16,
) -> (u16, u16, u16) {
    let widest = opts
        .iter()
        .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0);
    let max_num_width = u16::try_from(format!("{}", opts.len()).len()).unwrap_or(u16::MAX);
    let w = widest
        .saturating_add(max_num_width)
        .saturating_add(2) // spacing between text and number
        .saturating_add(extra_width)
        .min(results_area.width.saturating_sub(2));
    let h = u16::try_from(opts.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2); // borders
    (w, h, max_num_width)
}

/// What: Calculate menu rectangle position aligned to a button.
///
/// Inputs:
/// - `button_rect`: Optional button rectangle (x, y, width, height)
/// - `menu_width`: Calculated menu width
/// - `menu_height`: Calculated menu height
/// - `results_area`: Available area for positioning
///
/// Output:
/// - Menu rectangle and inner hit-test rectangle
///
/// Details:
/// - Aligns menu below button, clamps to viewport boundaries.
fn calculate_menu_rect(
    button_rect: Option<(u16, u16, u16, u16)>,
    menu_width: u16,
    menu_height: u16,
    results_area: Rect,
) -> (Rect, (u16, u16, u16, u16)) {
    let rect_w = menu_width.saturating_add(2);
    let max_x = results_area.x + results_area.width.saturating_sub(rect_w);
    let button_x = button_rect.map(|(x, _, _, _)| x).unwrap_or(max_x);
    let menu_x = button_x.min(max_x);
    let menu_y = results_area.y.saturating_add(1);
    let rect = Rect {
        x: menu_x,
        y: menu_y,
        width: rect_w,
        height: menu_height,
    };
    let inner_rect = (
        rect.x + 1,
        rect.y + 1,
        menu_width,
        menu_height.saturating_sub(2),
    );
    (rect, inner_rect)
}

/// What: Build menu lines with right-aligned row numbers.
///
/// Inputs:
/// - `opts`: Menu option strings
/// - `widest`: Width of widest option
/// - `max_num_width`: Maximum width needed for numbers
/// - `total_line_width`: Target display width for each line
/// - `spacing`: Spacing between text and numbers
/// - `th`: Theme colors
///
/// Output:
/// - Vector of styled lines ready for rendering
///
/// Details:
/// - Handles Unicode display width for accurate alignment with wide characters.
fn build_numbered_menu_lines(
    opts: &[String],
    widest: u16,
    max_num_width: u16,
    total_line_width: u16,
    spacing: u16,
    th: crate::theme::Theme,
) -> Vec<Line<'static>> {
    let num_start_col = widest + spacing;
    let mut lines: Vec<Line> = Vec::new();
    for (i, text) in opts.iter().enumerate() {
        let num_str = format!("{}", i + 1);
        let num_width = u16::try_from(num_str.len()).unwrap_or(u16::MAX);
        let num_padding = max_num_width.saturating_sub(num_width);
        let padded_num = format!("{}{}", " ".repeat(num_padding as usize), num_str);

        let text_display_width = u16::try_from(text.width()).unwrap_or(u16::MAX);
        let text_padding = widest.saturating_sub(text_display_width);

        let mut complete_line = format!(
            "{}{}{}{}",
            text,
            " ".repeat(text_padding as usize),
            " ".repeat(spacing as usize),
            padded_num
        );

        let current_width = u16::try_from(complete_line.width()).unwrap_or(u16::MAX);
        if current_width < total_line_width {
            complete_line.push_str(&" ".repeat((total_line_width - current_width) as usize));
        } else if current_width > total_line_width {
            let mut truncated = String::new();
            let mut width_so_far = 0u16;
            for ch in complete_line.chars() {
                let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
                if width_so_far + ch_width > total_line_width {
                    break;
                }
                truncated.push(ch);
                width_so_far += ch_width;
            }
            complete_line = truncated;
        }

        let mut text_part = String::new();
        let mut width_so_far = 0u16;
        for ch in complete_line.chars() {
            let ch_width = u16::try_from(ch.width().unwrap_or(0)).unwrap_or(u16::MAX);
            if width_so_far + ch_width > num_start_col {
                break;
            }
            text_part.push(ch);
            width_so_far += ch_width;
        }
        let num_part = complete_line
            .chars()
            .skip(text_part.chars().count())
            .collect::<String>();

        lines.push(Line::from(vec![
            Span::styled(text_part, Style::default().fg(th.text)),
            Span::styled(num_part, Style::default().fg(th.overlay1)),
        ]));
    }
    lines
}

/// What: Build menu lines with checkbox indicators.
///
/// Inputs:
/// - `opts`: Menu options with enabled state
/// - `menu_width`: Target width for each line
/// - `th`: Theme colors
///
/// Output:
/// - Vector of styled lines ready for rendering
fn build_checkbox_menu_lines(
    opts: &[(String, bool)],
    menu_width: u16,
    th: crate::theme::Theme,
) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    for (text, enabled) in opts {
        let indicator = if *enabled { "✓ " } else { "  " };
        let pad = menu_width
            .saturating_sub(u16::try_from(text.len()).unwrap_or(u16::MAX))
            .saturating_sub(u16::try_from(indicator.len()).unwrap_or(u16::MAX));
        let padding = " ".repeat(pad as usize);
        lines.push(Line::from(vec![
            Span::styled(
                indicator.to_string(),
                Style::default().fg(if *enabled { th.green } else { th.overlay1 }),
            ),
            Span::styled(text.clone(), Style::default().fg(th.text)),
            Span::raw(padding),
        ]));
    }
    lines
}

/// What: Create a styled menu block with title.
///
/// Inputs:
/// - `lines`: Menu lines to display
/// - `title_first_letter_key`: i18n key for first letter of title
/// - `title_suffix_key`: i18n key for suffix of title
/// - `app`: Application state for i18n
/// - `th`: Theme colors
///
/// Output:
/// - Styled Paragraph widget ready for rendering
fn create_menu_block(
    lines: Vec<Line<'static>>,
    title_first_letter_key: &str,
    title_suffix_key: &str,
    app: &AppState,
    th: crate::theme::Theme,
) -> Paragraph<'static> {
    let first_letter = i18n::t(app, title_first_letter_key);
    let suffix = i18n::t(app, title_suffix_key);
    Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: false })
        .block(
            Block::default()
                .style(Style::default().bg(th.base))
                .title(Line::from(vec![
                    Span::styled(" ", Style::default().fg(th.overlay1)),
                    Span::styled(
                        first_letter,
                        Style::default()
                            .fg(th.overlay1)
                            .add_modifier(Modifier::UNDERLINED),
                    ),
                    Span::styled(suffix, Style::default().fg(th.overlay1)),
                ]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.mauve)),
        )
}

/// What: Render Config/Lists dropdown menu.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `results_area`: Available area for positioning
/// - `th`: Theme colors
///
/// Output:
/// - Updates `app.config_menu_rect` if menu is rendered
fn render_config_menu(
    f: &mut Frame,
    app: &mut AppState,
    results_area: Rect,
    th: crate::theme::Theme,
) {
    app.config_menu_rect = None;
    if !app.config_menu_open {
        return;
    }

    let opts: Vec<String> = vec![
        i18n::t(app, "app.results.config_menu.options.settings"),
        i18n::t(app, "app.results.config_menu.options.theme"),
        i18n::t(app, "app.results.config_menu.options.keybindings"),
        i18n::t(app, "app.results.config_menu.options.install_list"),
        i18n::t(app, "app.results.config_menu.options.installed_packages"),
        i18n::t(app, "app.results.config_menu.options.recent_searches"),
    ];

    let widest = opts
        .iter()
        .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0);
    let (w, h, max_num_width) = calculate_menu_dimensions(&opts, results_area, 0);
    let (rect, inner_rect) = calculate_menu_rect(app.config_button_rect, w, h, results_area);
    app.config_menu_rect = Some(inner_rect);

    let spacing = 2u16;
    let lines = build_numbered_menu_lines(&opts, widest, max_num_width, w, spacing, th);
    let menu = create_menu_block(
        lines,
        "app.results.menus.config_lists.first_letter",
        "app.results.menus.config_lists.suffix",
        app,
        th,
    );
    f.render_widget(Clear, rect);
    f.render_widget(menu, rect);
}

/// What: Render Panels dropdown menu.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `results_area`: Available area for positioning
/// - `th`: Theme colors
///
/// Output:
/// - Updates `app.panels_menu_rect` if menu is rendered
fn render_panels_menu(
    f: &mut Frame,
    app: &mut AppState,
    results_area: Rect,
    th: crate::theme::Theme,
) {
    app.panels_menu_rect = None;
    if !app.panels_menu_open {
        return;
    }

    let label_recent = if app.show_recent_pane {
        i18n::t(app, "app.results.panels_menu.hide_recent")
    } else {
        i18n::t(app, "app.results.panels_menu.show_recent")
    };
    let label_install = if app.show_install_pane {
        i18n::t(app, "app.results.panels_menu.hide_install_list")
    } else {
        i18n::t(app, "app.results.panels_menu.show_install_list")
    };
    let label_keybinds = if app.show_keybinds_footer {
        i18n::t(app, "app.results.panels_menu.hide_keybinds")
    } else {
        i18n::t(app, "app.results.panels_menu.show_keybinds")
    };
    let opts: Vec<String> = vec![label_recent, label_install, label_keybinds];

    let widest = opts
        .iter()
        .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0);
    let (w, h, max_num_width) = calculate_menu_dimensions(&opts, results_area, 0);
    let (rect, inner_rect) = calculate_menu_rect(app.panels_button_rect, w, h, results_area);
    app.panels_menu_rect = Some(inner_rect);

    let spacing = 2u16;
    let lines = build_numbered_menu_lines(&opts, widest, max_num_width, w, spacing, th);
    let menu = create_menu_block(
        lines,
        "app.results.menus.panels.first_letter",
        "app.results.menus.panels.suffix",
        app,
        th,
    );
    f.render_widget(Clear, rect);
    f.render_widget(menu, rect);
}

/// What: Render Options dropdown menu.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `results_area`: Available area for positioning
/// - `th`: Theme colors
///
/// Output:
/// - Updates `app.options_menu_rect` if menu is rendered
fn render_options_menu(
    f: &mut Frame,
    app: &mut AppState,
    results_area: Rect,
    th: crate::theme::Theme,
) {
    app.options_menu_rect = None;
    if !app.options_menu_open {
        return;
    }

    let label_toggle = if app.installed_only_mode {
        i18n::t(app, "app.results.options_menu.list_all_packages")
    } else {
        i18n::t(app, "app.results.options_menu.list_installed_packages")
    };
    let opts = [
        label_toggle,
        i18n::t(app, "app.results.options_menu.update_system"),
        i18n::t(app, "app.results.options_menu.news"),
        i18n::t(app, "app.results.options_menu.tui_optional_deps"),
    ];
    let opts: Vec<String> = opts.iter().map(|s| s.clone()).collect();

    let widest = opts
        .iter()
        .map(|s| u16::try_from(s.width()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0);
    let (w, h, max_num_width) = calculate_menu_dimensions(&opts, results_area, 0);
    let (rect, inner_rect) = calculate_menu_rect(app.options_button_rect, w, h, results_area);
    app.options_menu_rect = Some(inner_rect);

    let spacing = 2u16;
    let lines = build_numbered_menu_lines(&opts, widest, max_num_width, w, spacing, th);
    let menu = create_menu_block(
        lines,
        "app.results.menus.options.first_letter",
        "app.results.menus.options.suffix",
        app,
        th,
    );
    f.render_widget(Clear, rect);
    f.render_widget(menu, rect);
}

/// What: Render Artix filter dropdown menu.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `results_area`: Available area for positioning
/// - `th`: Theme colors
///
/// Output:
/// - Updates `app.artix_filter_menu_rect` if menu is rendered
fn render_artix_filter_menu(
    f: &mut Frame,
    app: &mut AppState,
    results_area: Rect,
    th: crate::theme::Theme,
) {
    app.artix_filter_menu_rect = None;
    if !app.artix_filter_menu_open {
        return;
    }

    let has_hidden_filters = app.results_filter_artix_omniverse_rect.is_none()
        && app.results_filter_artix_universe_rect.is_none()
        && app.results_filter_artix_lib32_rect.is_none()
        && app.results_filter_artix_galaxy_rect.is_none()
        && app.results_filter_artix_world_rect.is_none()
        && app.results_filter_artix_system_rect.is_none();

    if !has_hidden_filters {
        return;
    }

    let all_on = app.results_filter_show_artix_omniverse
        && app.results_filter_show_artix_universe
        && app.results_filter_show_artix_lib32
        && app.results_filter_show_artix_galaxy
        && app.results_filter_show_artix_world
        && app.results_filter_show_artix_system;

    let opts: Vec<(String, bool)> = vec![
        (i18n::t(app, "app.results.filters.artix"), all_on),
        (
            i18n::t(app, "app.results.filters.artix_omniverse"),
            app.results_filter_show_artix_omniverse,
        ),
        (
            i18n::t(app, "app.results.filters.artix_universe"),
            app.results_filter_show_artix_universe,
        ),
        (
            i18n::t(app, "app.results.filters.artix_lib32"),
            app.results_filter_show_artix_lib32,
        ),
        (
            i18n::t(app, "app.results.filters.artix_galaxy"),
            app.results_filter_show_artix_galaxy,
        ),
        (
            i18n::t(app, "app.results.filters.artix_world"),
            app.results_filter_show_artix_world,
        ),
        (
            i18n::t(app, "app.results.filters.artix_system"),
            app.results_filter_show_artix_system,
        ),
    ];

    let widest = opts
        .iter()
        .map(|(s, _)| u16::try_from(s.len()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0);
    let w = widest
        .saturating_add(4) // space for checkbox indicator
        .saturating_add(2)
        .min(results_area.width.saturating_sub(2));
    let h = u16::try_from(opts.len())
        .unwrap_or(u16::MAX)
        .saturating_add(2);
    let (rect, inner_rect) = calculate_menu_rect(app.results_filter_artix_rect, w, h, results_area);
    app.artix_filter_menu_rect = Some(inner_rect);

    let lines = build_checkbox_menu_lines(&opts, w, th);
    let menu = Paragraph::new(lines)
        .style(Style::default().fg(th.text).bg(th.base))
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .style(Style::default().bg(th.base))
                .title(Line::from(vec![Span::styled(
                    "Artix Filters",
                    Style::default().fg(th.overlay1),
                )]))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(th.mauve)),
        );
    f.render_widget(Clear, rect);
    f.render_widget(menu, rect);
}

/// What: Render dropdown menus (Config/Lists, Panels, Options) on the overlay layer.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state (tracks menu open flags and rects)
/// - `results_area`: Rect of the results pane used for positioning
///
/// Output:
/// - Draws any open dropdowns and records their inner rectangles for hit-testing.
///
/// Details:
/// - Aligns menus with their buttons, clamps width to viewport, clears background, and numbers rows
///   for keyboard shortcuts while ensuring menus render above other content.
pub fn render_dropdowns(f: &mut Frame, app: &mut AppState, results_area: Rect) {
    let th = theme();
    render_config_menu(f, app, results_area, th);
    render_panels_menu(f, app, results_area, th);
    render_options_menu(f, app, results_area, th);
    render_artix_filter_menu(f, app, results_area, th);
}
