use ratatui::{
    prelude::Rect,
    style::{Modifier, Style},
    text::Span,
};

use crate::i18n;
use crate::state::AppState;
use crate::theme::theme;

/// What: Build title spans with Sort button, filter toggles, and right-aligned buttons.
///
/// This version takes individual values instead of &AppState to avoid borrow conflicts.
///
/// Inputs:
/// - `results_len`: Number of results
/// - `area`: Target rectangle for the results block
/// - `has_eos`, `has_cachyos`, `has_artix`, `has_manjaro`: Whether optional repos are available
/// - `sort_menu_open`, `config_menu_open`, `panels_menu_open`, `options_menu_open`: Menu states
/// - Filter flags for each repo type
///
/// Output:
/// - Vector of `Span` widgets forming the title line
///
/// Details:
/// - Applies theme styling for active buttons, ensures right-side buttons align within the title,
///   and toggles optional repo chips based on availability flags.
#[allow(clippy::too_many_arguments)]
pub fn build_title_spans_from_values(
    app: &AppState,
    results_len: usize,
    area: Rect,
    has_eos: bool,
    has_cachyos: bool,
    has_artix: bool,
    has_manjaro: bool,
    sort_menu_open: bool,
    config_menu_open: bool,
    panels_menu_open: bool,
    options_menu_open: bool,
    results_filter_show_aur: bool,
    results_filter_show_core: bool,
    results_filter_show_extra: bool,
    results_filter_show_multilib: bool,
    results_filter_show_eos: bool,
    results_filter_show_cachyos: bool,
    results_filter_show_artix: bool,
    results_filter_show_manjaro: bool,
) -> Vec<Span<'static>> {
    let th = theme();
    let results_title_text = format!("{} ({})", i18n::t(app, "app.results.title"), results_len);
    let sort_button_label = format!("{} v", i18n::t(app, "app.results.buttons.sort"));
    let options_button_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_button_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_button_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));
    let mut title_spans: Vec<Span> = vec![Span::styled(
        results_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    title_spans.push(Span::raw("  "));
    // Style the sort button differently when menu is open
    let btn_style = if sort_menu_open {
        Style::default()
            .fg(th.crust)
            .bg(th.mauve)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(th.mauve)
            .bg(th.surface2)
            .add_modifier(Modifier::BOLD)
    };
    title_spans.push(Span::styled(sort_button_label.clone(), btn_style));
    title_spans.push(Span::raw("  "));
    // Filter toggles: [AUR] [core] [extra] [multilib] and optional [EOS]/[CachyOS]
    let filt = |label: &str, on: bool| -> Span<'static> {
        let (fg, bg) = if on {
            (th.crust, th.green)
        } else {
            (th.mauve, th.surface2)
        };
        Span::styled(
            format!("[{label}]"),
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
        )
    };
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.aur"),
        results_filter_show_aur,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.core"),
        results_filter_show_core,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.extra"),
        results_filter_show_extra,
    ));
    title_spans.push(Span::raw(" "));
    title_spans.push(filt(
        &i18n::t(app, "app.results.filters.multilib"),
        results_filter_show_multilib,
    ));
    if has_eos {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt(
            &i18n::t(app, "app.results.filters.eos"),
            results_filter_show_eos,
        ));
    }
    if has_cachyos {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt(
            &i18n::t(app, "app.results.filters.cachyos"),
            results_filter_show_cachyos,
        ));
    }
    if has_artix {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt(
            &i18n::t(app, "app.results.filters.artix"),
            results_filter_show_artix,
        ));
    }
    if has_manjaro {
        title_spans.push(Span::raw(" "));
        title_spans.push(filt(
            &i18n::t(app, "app.results.filters.manjaro"),
            results_filter_show_manjaro,
        ));
    }

    // Right-aligned Config/Lists, Panels and Options buttons: compute remaining space and append to title spans
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let aur_label = format!("[{}]", i18n::t(app, "app.results.filters.aur"));
    let core_label = format!("[{}]", i18n::t(app, "app.results.filters.core"));
    let extra_label = format!("[{}]", i18n::t(app, "app.results.filters.extra"));
    let multilib_label = format!("[{}]", i18n::t(app, "app.results.filters.multilib"));
    let eos_label = format!("[{}]", i18n::t(app, "app.results.filters.eos"));
    let cachyos_label = format!("[{}]", i18n::t(app, "app.results.filters.cachyos"));
    let artix_label = format!("[{}]", i18n::t(app, "app.results.filters.artix"));
    let manjaro_label = format!("[{}]", i18n::t(app, "app.results.filters.manjaro"));
    let mut consumed_left = (results_title_text.len()
        + 2 // spaces before Sort
        + sort_button_label.len()
        + 2 // spaces after Sort
        + aur_label.len()
        + 1 // space
        + core_label.len()
        + 1 // space
        + extra_label.len()
        + 1 // space
        + multilib_label.len()) as u16;
    if has_eos {
        consumed_left = consumed_left.saturating_add(1 + eos_label.len() as u16);
    }
    if has_cachyos {
        consumed_left = consumed_left.saturating_add(1 + cachyos_label.len() as u16);
    }
    if has_artix {
        consumed_left = consumed_left.saturating_add(1 + artix_label.len() as u16);
    }
    if has_manjaro {
        consumed_left = consumed_left.saturating_add(1 + manjaro_label.len() as u16);
    }
    // Minimum single space before right-side buttons when possible
    let options_w = options_button_label.len() as u16;
    let panels_w = panels_button_label.len() as u16;
    let config_w = config_button_label.len() as u16;
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w); // "Config/Lists" + space + "Panels" + space + "Options"
    let pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));
    if pad >= 1 {
        title_spans.push(Span::raw(" ".repeat(pad as usize)));
        let cfg_btn_style = if config_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        // Render Config/Lists button with underlined first char (C)
        if let Some(first) = config_button_label.chars().next() {
            let rest = &config_button_label[first.len_utf8()..];
            title_spans.push(Span::styled(
                first.to_string(),
                cfg_btn_style.add_modifier(Modifier::UNDERLINED),
            ));
            title_spans.push(Span::styled(rest.to_string(), cfg_btn_style));
        } else {
            title_spans.push(Span::styled(config_button_label.clone(), cfg_btn_style));
        }
        title_spans.push(Span::raw(" "));
        let pan_btn_style = if panels_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        // Render Panels button with underlined first char (P)
        if let Some(first) = panels_button_label.chars().next() {
            let rest = &panels_button_label[first.len_utf8()..];
            title_spans.push(Span::styled(
                first.to_string(),
                pan_btn_style.add_modifier(Modifier::UNDERLINED),
            ));
            title_spans.push(Span::styled(rest.to_string(), pan_btn_style));
        } else {
            title_spans.push(Span::styled(panels_button_label.clone(), pan_btn_style));
        }
        title_spans.push(Span::raw(" "));
        let opt_btn_style = if options_menu_open {
            Style::default()
                .fg(th.crust)
                .bg(th.mauve)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(th.mauve)
                .bg(th.surface2)
                .add_modifier(Modifier::BOLD)
        };
        // Render Options button with underlined first char (O)
        if let Some(first) = options_button_label.chars().next() {
            let rest = &options_button_label[first.len_utf8()..];
            title_spans.push(Span::styled(
                first.to_string(),
                opt_btn_style.add_modifier(Modifier::UNDERLINED),
            ));
            title_spans.push(Span::styled(rest.to_string(), opt_btn_style));
        } else {
            title_spans.push(Span::styled(options_button_label.clone(), opt_btn_style));
        }
    }

    title_spans
}

/// What: Record clickable rectangles for title bar controls.
///
/// Inputs:
/// - `app`: Mutable application state (rects will be updated)
/// - `area`: Target rectangle for the results block
/// - `has_eos`, `has_cachyos`, `has_artix`, `has_manjaro`: Whether optional repos are available
///
/// Output:
/// - Updates `app` with rectangles for filters, buttons, and optional repo chips.
///
/// Details:
/// - Mirrors title layout calculations to align rects with rendered elements and clears entries when
///   controls cannot fit in the available width.
pub fn record_title_rects(
    app: &mut AppState,
    area: Rect,
    has_eos: bool,
    has_cachyos: bool,
    has_artix: bool,
    has_manjaro: bool,
) {
    let results_title_text = format!(
        "{} ({})",
        i18n::t(app, "app.results.title"),
        app.results.len()
    );
    let sort_button_label = format!("{} v", i18n::t(app, "app.results.buttons.sort"));
    let options_button_label = format!("{} v", i18n::t(app, "app.results.buttons.options"));
    let panels_button_label = format!("{} v", i18n::t(app, "app.results.buttons.panels"));
    let config_button_label = format!("{} v", i18n::t(app, "app.results.buttons.config_lists"));

    // Estimate and record clickable rects for controls on the title line (top border row)
    let mut x_cursor = area
        .x
        .saturating_add(1) // left border inset
        .saturating_add(results_title_text.len() as u16)
        .saturating_add(2); // two spaces before Sort
    let btn_w = sort_button_label.len() as u16;
    let btn_x = x_cursor;
    let btn_y = area.y; // top border row
    app.sort_button_rect = Some((btn_x, btn_y, btn_w, 1));
    x_cursor = x_cursor.saturating_add(btn_w).saturating_add(2); // space after sort

    // Filter rects in sequence, with single space between
    let rec_rect = |start_x: u16, label: &str| -> (u16, u16, u16, u16) {
        (start_x, btn_y, label.len() as u16, 1)
    };
    let aur_label = "[AUR]";
    app.results_filter_aur_rect = Some(rec_rect(x_cursor, aur_label));
    x_cursor = x_cursor
        .saturating_add(aur_label.len() as u16)
        .saturating_add(1);
    let core_label = "[core]";
    app.results_filter_core_rect = Some(rec_rect(x_cursor, core_label));
    x_cursor = x_cursor
        .saturating_add(core_label.len() as u16)
        .saturating_add(1);
    let extra_label = "[extra]";
    app.results_filter_extra_rect = Some(rec_rect(x_cursor, extra_label));
    x_cursor = x_cursor
        .saturating_add(extra_label.len() as u16)
        .saturating_add(1);
    let multilib_label = "[multilib]";
    app.results_filter_multilib_rect = Some(rec_rect(x_cursor, multilib_label));
    x_cursor = x_cursor
        .saturating_add(multilib_label.len() as u16)
        .saturating_add(1);
    let eos_label = "[EOS]";
    if has_eos {
        app.results_filter_eos_rect = Some(rec_rect(x_cursor, eos_label));
        x_cursor = x_cursor
            .saturating_add(eos_label.len() as u16)
            .saturating_add(1);
    } else {
        app.results_filter_eos_rect = None;
    }
    let cachyos_label = "[CachyOS]";
    if has_cachyos {
        app.results_filter_cachyos_rect = Some(rec_rect(x_cursor, cachyos_label));
        x_cursor = x_cursor
            .saturating_add(cachyos_label.len() as u16)
            .saturating_add(1);
    } else {
        app.results_filter_cachyos_rect = None;
    }
    let artix_label = format!("[{}]", i18n::t(app, "app.results.filters.artix"));
    if has_artix {
        app.results_filter_artix_rect = Some(rec_rect(x_cursor, &artix_label));
        x_cursor = x_cursor
            .saturating_add(artix_label.len() as u16)
            .saturating_add(1);
    } else {
        app.results_filter_artix_rect = None;
    }
    let manjaro_label = format!("[{}]", i18n::t(app, "app.results.filters.manjaro"));
    if has_manjaro {
        app.results_filter_manjaro_rect = Some(rec_rect(x_cursor, &manjaro_label));
    } else {
        app.results_filter_manjaro_rect = None;
    }

    // Right-aligned Config/Lists, Panels and Options buttons: compute remaining space
    let inner_width = area.width.saturating_sub(2); // exclude borders
    let mut consumed_left = (results_title_text.len()
        + 2 // spaces before Sort
        + sort_button_label.len()
        + 2 // spaces after Sort
        + aur_label.len()
        + 1 // space
        + core_label.len()
        + 1 // space
        + extra_label.len()
        + 1 // space
        + multilib_label.len()) as u16;
    if has_eos {
        consumed_left = consumed_left.saturating_add(1 + eos_label.len() as u16);
    }
    if has_cachyos {
        consumed_left = consumed_left.saturating_add(1 + cachyos_label.len() as u16);
    }
    if has_artix {
        consumed_left = consumed_left.saturating_add(1 + artix_label.len() as u16);
    }
    if has_manjaro {
        consumed_left = consumed_left.saturating_add(1 + manjaro_label.len() as u16);
    }
    let options_w = options_button_label.len() as u16;
    let panels_w = panels_button_label.len() as u16;
    let config_w = config_button_label.len() as u16;
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w);
    let pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));
    if pad >= 1 {
        // Record clickable rects at the computed right edge (Panels to the left of Options)
        let opt_x = area
            .x
            .saturating_add(1) // left border inset
            .saturating_add(inner_width.saturating_sub(options_w));
        let pan_x = opt_x.saturating_sub(1).saturating_sub(panels_w);
        let cfg_x = pan_x.saturating_sub(1).saturating_sub(config_w);
        app.config_button_rect = Some((cfg_x, btn_y, config_w, 1));
        app.options_button_rect = Some((opt_x, btn_y, options_w, 1));
        app.panels_button_rect = Some((pan_x, btn_y, panels_w, 1));
    } else {
        app.config_button_rect = None;
        app.options_button_rect = None;
        app.panels_button_rect = None;
    }
}
