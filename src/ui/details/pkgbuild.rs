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
use crate::state::app_state::PkgbuildCheckTool;
use crate::theme::Theme;
use crate::theme::theme;

use super::pkgbuild_highlight;

/// What: Tell whether a checker was skipped because its binary was not found on `PATH`.
///
/// Inputs:
/// - `missing_tools`: Worker hints copied into [`AppState::pkgb_check_missing_tools`].
/// - `tool`: Subsection being rendered (`Shellcheck` or `Namcap`).
///
/// Output:
/// - `true` when `missing_tools` includes the hint for that tool.
///
/// Details:
/// - Matches the prefix of strings produced when a checker binary is absent (`shellcheck missing:` /
///   `namcap missing:` in the PKGBUILD check worker), compared ASCII-case-insensitively.
fn pkgbuild_check_tool_missing_on_path(missing_tools: &[String], tool: PkgbuildCheckTool) -> bool {
    let lower_prefix = match tool {
        PkgbuildCheckTool::Shellcheck => "shellcheck missing",
        PkgbuildCheckTool::Namcap => "namcap missing",
    };
    missing_tools.iter().any(|hint| {
        hint.trim_start()
            .to_ascii_lowercase()
            .starts_with(lower_prefix)
    })
}

/// What: Line indices for jumping between PKGBUILD body and per-tool static-check subsections.
///
/// Inputs: Built by [`build_pkgbuild_all_lines`].
///
/// Output:
/// - `checks_start_line` is the first line after the PKGBUILD body (blank separator when checks exist).
/// - `shellcheck_line` / `namcap_line` point at subsection headers when `has_checks_block`.
///
/// Details:
/// - When `has_checks_block` is false, only the PKGBUILD body is present; cycle falls back to top.
#[derive(Clone, Copy, Debug)]
pub struct PkgbuildScrollAnchors {
    /// First line index of the static-checks block (after PKGBUILD body).
    pub checks_start_line: usize,
    /// Line index of the `ShellCheck` subsection header, if static checks are shown.
    pub shellcheck_line: Option<usize>,
    /// Line index of the `Namcap` subsection header, if static checks are shown.
    pub namcap_line: Option<usize>,
    /// Whether the static-checks appendix is rendered at all.
    pub has_checks_block: bool,
}

/// What: Append per-tool raw check output lines to the PKGBUILD details pane.
///
/// Inputs:
/// - `all_lines`: Output line buffer to append into.
/// - `raw_results`: Captured raw command results from static checks.
///
/// Output:
/// - None (mutates `all_lines`).
///
/// Details:
/// - Emits a friendly success line when `namcap` produced no stdout/stderr.
/// - Otherwise prints the command preview and any stdout/stderr lines.
fn append_pkgbuild_raw_output_lines(
    all_lines: &mut Vec<Line<'static>>,
    raw_results: &[crate::state::app_state::PkgbuildToolRawResult],
) {
    all_lines.push(Line::from(""));
    all_lines.push(Line::from("Raw output:"));
    for raw in raw_results {
        if matches!(raw.tool, PkgbuildCheckTool::Namcap)
            && raw.stdout.trim().is_empty()
            && raw.stderr.trim().is_empty()
        {
            all_lines.push(Line::from("  Namcap check executed - No issues found"));
            continue;
        }
        all_lines.push(Line::from(format!("  {:?} cmd: {}", raw.tool, raw.command)));
        if !raw.stdout.trim().is_empty() {
            for line in raw.stdout.lines() {
                all_lines.push(Line::from(format!("    {line}")));
            }
        }
        if !raw.stderr.trim().is_empty() {
            for line in raw.stderr.lines() {
                all_lines.push(Line::from(format!("    [stderr] {line}")));
            }
        }
    }
}

/// What: Build full PKGBUILD pane lines plus scroll anchors for section cycling.
///
/// Inputs:
/// - `app`: Application state (PKGBUILD text, check results).
/// - `th`: Theme for syntax highlighting.
///
/// Output:
/// - All lines to render and [`PkgbuildScrollAnchors`] for `Ctrl+D` jumps.
///
/// Details:
/// - Mirrors [`render_pkgbuild`] content: body, then `Static checks` + `ShellCheck` + `Namcap` subsections, optional raw block.
pub fn build_pkgbuild_all_lines(
    app: &AppState,
    th: &Theme,
) -> (Vec<Line<'static>>, PkgbuildScrollAnchors) {
    let loading_text = i18n::t(app, "app.details.loading_pkgb");
    let pkgb_text = app.pkgb_text.as_deref().unwrap_or(&loading_text);
    let mut anchors = PkgbuildScrollAnchors {
        checks_start_line: 0,
        shellcheck_line: None,
        namcap_line: None,
        has_checks_block: false,
    };
    let mut all_lines = if pkgb_text == loading_text {
        vec![Line::from(loading_text)]
    } else {
        pkgbuild_highlight::highlight_pkgbuild(pkgb_text, th)
    };
    anchors.checks_start_line = all_lines.len();
    let show_checks = !matches!(
        app.pkgb_check_status,
        crate::state::app_state::PkgbuildCheckStatus::Idle
    ) || !app.pkgb_check_findings.is_empty()
        || !app.pkgb_check_missing_tools.is_empty()
        || !app.pkgb_check_raw_results.is_empty();
    if show_checks {
        anchors.has_checks_block = true;
        all_lines.push(Line::from(""));
        all_lines.push(Line::from("─── Static checks ───"));
        all_lines.push(Line::from(format!("Status: {:?}", app.pkgb_check_status)));
        for missing in &app.pkgb_check_missing_tools {
            all_lines.push(Line::from(format!("[missing] {missing}")));
        }
        anchors.shellcheck_line = Some(all_lines.len());
        all_lines.push(Line::from("─── ShellCheck ───"));
        let mut any_shellcheck = false;
        for finding in &app.pkgb_check_findings {
            if !matches!(finding.tool, PkgbuildCheckTool::Shellcheck) {
                continue;
            }
            any_shellcheck = true;
            let line_ref = finding
                .line
                .map_or_else(String::new, |line| format!("L{line} "));
            all_lines.push(Line::from(format!(
                "[{:?}] {}{:?}: {}",
                finding.tool, line_ref, finding.severity, finding.message
            )));
        }
        if !any_shellcheck {
            let line = if pkgbuild_check_tool_missing_on_path(
                &app.pkgb_check_missing_tools,
                PkgbuildCheckTool::Shellcheck,
            ) {
                "  shellcheck not installed"
            } else {
                "  (no findings)"
            };
            all_lines.push(Line::from(line));
        }
        anchors.namcap_line = Some(all_lines.len());
        all_lines.push(Line::from("─── Namcap ───"));
        let mut any_namcap = false;
        for finding in &app.pkgb_check_findings {
            if !matches!(finding.tool, PkgbuildCheckTool::Namcap) {
                continue;
            }
            any_namcap = true;
            let line_ref = finding
                .line
                .map_or_else(String::new, |line| format!("L{line} "));
            all_lines.push(Line::from(format!(
                "[{:?}] {}{:?}: {}",
                finding.tool, line_ref, finding.severity, finding.message
            )));
        }
        if !any_namcap {
            let line = if pkgbuild_check_tool_missing_on_path(
                &app.pkgb_check_missing_tools,
                PkgbuildCheckTool::Namcap,
            ) {
                "  namcap not installed"
            } else {
                "  (no findings)"
            };
            all_lines.push(Line::from(line));
        }
        if app.pkgb_check_show_raw_output
            && crate::theme::settings().pkgbuild_checks_show_raw_output
        {
            append_pkgbuild_raw_output_lines(&mut all_lines, &app.pkgb_check_raw_results);
        }
    }
    (all_lines, anchors)
}

/// What: Advance PKGBUILD pane scroll to the next section in rotation: body → `ShellCheck` → `Namcap` → body.
///
/// Inputs:
/// - `app`: Application state; uses [`AppState::pkgb_section_cycle`] and updates [`AppState::pkgb_scroll`].
///
/// Output:
/// - None.
///
/// Details:
/// - No-op when the PKGBUILD appendix is absent beyond the body (scroll stays at top).
/// - Uses [`build_pkgbuild_all_lines`] so anchors stay aligned with rendering.
pub fn cycle_pkgbuild_view_section(app: &mut AppState) {
    let th = theme();
    let (lines, anchors) = build_pkgbuild_all_lines(app, &th);
    let max_scroll = lines.len().saturating_sub(1);
    app.pkgb_section_cycle = (app.pkgb_section_cycle + 1) % 3;
    if !anchors.has_checks_block {
        app.pkgb_scroll = 0;
        return;
    }
    let shell_line = anchors.shellcheck_line.unwrap_or(anchors.checks_start_line);
    let namcap_line = anchors.namcap_line.unwrap_or(shell_line);
    let target = match app.pkgb_section_cycle {
        1 => shell_line,
        2 => namcap_line,
        _ => 0usize,
    };
    app.pkgb_scroll = u16::try_from(target.min(max_scroll)).unwrap_or(0);
}

/// What: Combine the `Run checks` jump sentinel with clamping to the document end.
///
/// Inputs:
/// - `raw_scroll`: Current [`AppState::pkgb_scroll`] before this frame.
/// - `max_scroll`: Last valid zero-based line index (`line_count.saturating_sub(1)`).
/// - `all_lines_len`: `line_count` for the PKGBUILD pane (`max_scroll.saturating_add(1)` when non-empty).
/// - `checks_start_line`: First line index of the static-checks appendix (from [`build_pkgbuild_all_lines`]).
/// - `check_status`: PKGBUILD check lifecycle state.
///
/// Output:
/// - Scroll index in `0..=max_scroll` (or `checks_start_line` when jumping to the appendix).
///
/// Details:
/// - Mouse wheel updates use unbounded `saturating_add`; without storing the clamped index, the offset
///   sticks at a huge value so every frame displays only the tail lines (often hiding `ShellCheck` /
///   `Namcap` findings above the viewport).
fn resolve_pkgbuild_clamped_scroll(
    raw_scroll: u16,
    max_scroll: usize,
    all_lines_len: usize,
    checks_start_line: usize,
    check_status: crate::state::app_state::PkgbuildCheckStatus,
) -> usize {
    if raw_scroll == u16::MAX
        && checks_start_line < all_lines_len
        && !matches!(
            check_status,
            crate::state::app_state::PkgbuildCheckStatus::Idle
        )
    {
        checks_start_line
    } else {
        usize::min(usize::from(raw_scroll), max_scroll)
    }
}

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
/// - Writes the clamped scroll index back to [`AppState::pkgb_scroll`] so wheel deltas cannot strand
///   the view on the tail of the buffer.
pub fn render_pkgbuild(f: &mut Frame, app: &mut AppState, pkgb_area: Rect) {
    let th = theme();
    let (all_lines, anchors) = build_pkgbuild_all_lines(app, &th);
    let checks_start_line = anchors.checks_start_line;

    // Remember PKGBUILD rect for mouse interactions (scrolling)
    app.pkgb_rect = Some((
        pkgb_area.x + 1,
        pkgb_area.y + 1,
        pkgb_area.width.saturating_sub(2),
        pkgb_area.height.saturating_sub(2),
    ));

    let all_lines_len = all_lines.len();
    let max_scroll = all_lines_len.saturating_sub(1);
    let clamped_scroll = resolve_pkgbuild_clamped_scroll(
        app.pkgb_scroll,
        max_scroll,
        all_lines_len,
        checks_start_line,
        app.pkgb_check_status,
    );
    app.pkgb_scroll = u16::try_from(clamped_scroll).unwrap_or(u16::MAX);
    let visible_lines: Vec<Line> = all_lines.into_iter().skip(clamped_scroll).collect();
    // Title with clickable "Copy PKGBUILD" button and optional "Reload PKGBUILD" button
    let check_button_label = i18n::t(app, "app.details.copy_pkgbuild");
    let pkgb_title_text = i18n::t(app, "app.titles.pkgb");
    let mut pkgb_title_spans: Vec<Span> = vec![Span::styled(
        pkgb_title_text.clone(),
        Style::default().fg(th.overlay1),
    )];
    if !matches!(
        app.pkgb_check_status,
        crate::state::app_state::PkgbuildCheckStatus::Idle
    ) {
        let status_text = match app.pkgb_check_status {
            crate::state::app_state::PkgbuildCheckStatus::Idle => "idle",
            crate::state::app_state::PkgbuildCheckStatus::Running => "running",
            crate::state::app_state::PkgbuildCheckStatus::Complete => "complete",
        };
        pkgb_title_spans.push(Span::raw(" "));
        pkgb_title_spans.push(Span::styled(
            format!("[checks: {status_text}]"),
            Style::default().fg(th.yellow).add_modifier(Modifier::BOLD),
        ));
    }
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
    pkgb_title_spans.push(Span::raw("  "));
    let run_checks_button_label = "Run checks";
    pkgb_title_spans.push(Span::styled(run_checks_button_label, check_btn_style));
    let run_checks_x = if let Some((reload_x, _, reload_w, _)) = app.pkgb_reload_button_rect {
        reload_x.saturating_add(reload_w).saturating_add(2)
    } else {
        btn_x.saturating_add(btn_w).saturating_add(2)
    };
    app.pkgb_run_checks_button_rect = Some((
        run_checks_x,
        btn_y,
        u16::try_from(run_checks_button_label.width()).unwrap_or(u16::MAX),
        1,
    ));

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

#[cfg(test)]
mod tests {
    use super::{
        build_pkgbuild_all_lines, cycle_pkgbuild_view_section, resolve_pkgbuild_clamped_scroll,
    };
    use crate::state::AppState;
    use crate::state::app_state::{PkgbuildCheckSeverity, PkgbuildCheckStatus, PkgbuildCheckTool};
    use crate::theme::theme;

    #[test]
    /// What: Oversized scroll values clamp to the last line so earlier appendix lines stay reachable.
    fn pkgbuild_scroll_clamps_wheel_overflow() {
        let scrolled =
            resolve_pkgbuild_clamped_scroll(50_000, 12, 13, 10, PkgbuildCheckStatus::Complete);
        assert_eq!(scrolled, 12);
    }

    #[test]
    /// What: `u16::MAX` scroll jumps to the checks appendix while a run is active or results are shown.
    fn pkgbuild_scroll_max_sentinel_jumps_to_checks() {
        let j =
            resolve_pkgbuild_clamped_scroll(u16::MAX, 99, 100, 80, PkgbuildCheckStatus::Running);
        assert_eq!(j, 80);
    }

    #[test]
    /// What: `u16::MAX` scroll does not jump when checks are idle; clamp to last line instead.
    fn pkgbuild_scroll_max_idle_falls_back_to_clamp() {
        let j = resolve_pkgbuild_clamped_scroll(u16::MAX, 19, 20, 10, PkgbuildCheckStatus::Idle);
        assert_eq!(j, 19);
    }

    #[test]
    /// What: A missing checker shows `not installed` under that subsection instead of `(no findings)`.
    ///
    /// Inputs:
    /// - `AppState` with completed checks, no parsed findings, and `pkgb_check_missing_tools` listing
    ///   only `shellcheck`.
    ///
    /// Output:
    /// - Rendered lines include `shellcheck not installed` and do not claim no `ShellCheck` findings.
    ///
    /// Details:
    /// - Ensures missing binaries are not confused with a clean `ShellCheck` run.
    fn pkgbuild_subsection_shows_not_installed_when_tool_missing() {
        let app = AppState {
            pkgb_text: Some("pkgname=demo\npkgver=1\n".into()),
            pkgb_check_status: PkgbuildCheckStatus::Complete,
            pkgb_check_missing_tools: vec![
                "shellcheck missing: install package `shellcheck`".into(),
            ],
            ..Default::default()
        };
        let th = theme();
        let (lines, _) = build_pkgbuild_all_lines(&app, &th);
        let flat: Vec<String> = lines.iter().map(ToString::to_string).collect();
        let sc_start = flat
            .iter()
            .position(|s| s.contains("─── ShellCheck ───"))
            .expect("ShellCheck header");
        let nc_start = flat
            .iter()
            .position(|s| s.contains("─── Namcap ───"))
            .expect("Namcap header");
        let shellcheck_body = &flat[sc_start + 1..nc_start];
        assert!(
            shellcheck_body
                .iter()
                .any(|s| s.contains("shellcheck not installed")),
            "expected shellcheck missing line, got {shellcheck_body:?}"
        );
        assert!(
            !shellcheck_body.iter().any(|s| s.contains("(no findings)")),
            "shellcheck subsection must not claim no findings when the tool did not run: {shellcheck_body:?}"
        );
    }

    #[test]
    /// What: Section anchors point at `ShellCheck` / `Namcap` headers when checks are present.
    fn pkgbuild_anchors_track_subsections() {
        let app = AppState {
            pkgb_text: Some("pkgname=x\npkgver=1\n".into()),
            pkgb_check_status: crate::state::app_state::PkgbuildCheckStatus::Complete,
            pkgb_check_findings: vec![
                crate::state::app_state::PkgbuildCheckFinding {
                    tool: PkgbuildCheckTool::Shellcheck,
                    severity: PkgbuildCheckSeverity::Warning,
                    line: Some(1),
                    message: "sc".into(),
                },
                crate::state::app_state::PkgbuildCheckFinding {
                    tool: PkgbuildCheckTool::Namcap,
                    severity: PkgbuildCheckSeverity::Info,
                    line: None,
                    message: "na".into(),
                },
            ],
            ..Default::default()
        };
        let th = theme();
        let (_lines, a) = build_pkgbuild_all_lines(&app, &th);
        assert!(a.has_checks_block);
        let sc = a.shellcheck_line.expect("shellcheck header");
        let na = a.namcap_line.expect("namcap header");
        assert!(na > sc);
    }

    #[test]
    /// What: `cycle_pkgbuild_view_section` rotates scroll between body and tool headers.
    fn pkgbuild_cycle_updates_scroll() {
        let mut app = AppState {
            pkgb_text: Some("a\nb\n".into()),
            pkgb_check_status: crate::state::app_state::PkgbuildCheckStatus::Complete,
            pkgb_section_cycle: 2,
            ..Default::default()
        };
        cycle_pkgbuild_view_section(&mut app);
        assert_eq!(app.pkgb_section_cycle, 0);
        assert_eq!(app.pkgb_scroll, 0);
        cycle_pkgbuild_view_section(&mut app);
        assert_eq!(app.pkgb_section_cycle, 1);
        assert!(app.pkgb_scroll > 0);
    }
}
