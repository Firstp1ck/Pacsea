use ratatui::Frame;
use ratatui::prelude::Rect;

use crate::state::{AppState, Modal, modal::PreflightHeaderChips, types::OptionalDepRow};
use crate::ui::modals::{
    alert, confirm, help, misc, news, post_summary, preflight, preflight_exec, system_update,
};

/// What: Context struct grouping PreflightExec modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct PreflightExecContext {
    items: Vec<crate::state::PackageItem>,
    action: crate::state::PreflightAction,
    tab: crate::state::PreflightTab,
    verbose: bool,
    log_lines: Vec<String>,
    abortable: bool,
    header_chips: PreflightHeaderChips,
}

/// What: Context struct grouping SystemUpdate modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct SystemUpdateContext {
    do_mirrors: bool,
    do_pacman: bool,
    do_aur: bool,
    do_cache: bool,
    country_idx: usize,
    countries: Vec<String>,
    mirror_count: u16,
    cursor: usize,
}

/// What: Context struct grouping PostSummary modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct PostSummaryContext {
    success: bool,
    changed_files: usize,
    pacnew_count: usize,
    pacsave_count: usize,
    services_pending: Vec<String>,
    snapshot_label: Option<String>,
}

/// What: Context struct grouping ScanConfig modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ScanConfigContext {
    do_clamav: bool,
    do_trivy: bool,
    do_semgrep: bool,
    do_shellcheck: bool,
    do_virustotal: bool,
    do_custom: bool,
    do_sleuth: bool,
    cursor: usize,
}

/// What: Trait for rendering modal variants and managing their state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `modal`: The modal variant to render (taken by value)
///
/// Output:
/// - Returns the modal back (for state reconstruction)
///
/// Details:
/// - Each implementation handles field extraction, rendering, and state reconstruction.
/// - This trait pattern eliminates repetitive match arms in the main render function.
trait ModalRenderer {
    fn render(self, f: &mut Frame, app: &mut AppState, area: Rect) -> Modal;
}

impl ModalRenderer for Modal {
    fn render(self, f: &mut Frame, app: &mut AppState, area: Rect) -> Modal {
        match self {
            Modal::Alert { message } => render_alert_modal(f, app, area, message),
            Modal::ConfirmInstall { items } => render_confirm_install_modal(f, app, area, items),
            Modal::Preflight { .. } => {
                unreachable!("Preflight should be handled separately before trait dispatch")
            }
            Modal::PreflightExec {
                items,
                action,
                tab,
                verbose,
                log_lines,
                abortable,
                header_chips,
            } => {
                let ctx = PreflightExecContext {
                    items,
                    action,
                    tab,
                    verbose,
                    log_lines,
                    abortable,
                    header_chips,
                };
                render_preflight_exec_modal(f, area, ctx)
            }
            Modal::PostSummary {
                success,
                changed_files,
                pacnew_count,
                pacsave_count,
                services_pending,
                snapshot_label,
            } => {
                let ctx = PostSummaryContext {
                    success,
                    changed_files,
                    pacnew_count,
                    pacsave_count,
                    services_pending,
                    snapshot_label,
                };
                render_post_summary_modal(f, app, area, ctx)
            }
            Modal::ConfirmRemove { items } => render_confirm_remove_modal(f, app, area, items),
            Modal::SystemUpdate {
                do_mirrors,
                do_pacman,
                do_aur,
                do_cache,
                country_idx,
                countries,
                mirror_count,
                cursor,
            } => {
                let ctx = SystemUpdateContext {
                    do_mirrors,
                    do_pacman,
                    do_aur,
                    do_cache,
                    country_idx,
                    countries,
                    mirror_count,
                    cursor,
                };
                render_system_update_modal(f, app, area, ctx)
            }
            Modal::Help => render_help_modal(f, app, area),
            Modal::News { items, selected } => render_news_modal(f, app, area, items, selected),
            Modal::OptionalDeps { rows, selected } => {
                render_optional_deps_modal(f, area, rows, selected, app)
            }
            Modal::ScanConfig {
                do_clamav,
                do_trivy,
                do_semgrep,
                do_shellcheck,
                do_virustotal,
                do_custom,
                do_sleuth,
                cursor,
            } => {
                let ctx = ScanConfigContext {
                    do_clamav,
                    do_trivy,
                    do_semgrep,
                    do_shellcheck,
                    do_virustotal,
                    do_custom,
                    do_sleuth,
                    cursor,
                };
                render_scan_config_modal(f, area, ctx)
            }
            Modal::GnomeTerminalPrompt => render_gnome_terminal_prompt_modal(f, area),
            Modal::VirusTotalSetup { input, cursor } => {
                render_virustotal_setup_modal(f, app, area, input, cursor)
            }
            Modal::ImportHelp => render_import_help_modal(f, area),
            Modal::None => Modal::None,
        }
    }
}

/// What: Render Alert modal and return reconstructed state.
fn render_alert_modal(f: &mut Frame, app: &mut AppState, area: Rect, message: String) -> Modal {
    alert::render_alert(f, app, area, &message);
    Modal::Alert { message }
}

/// What: Render ConfirmInstall modal and return reconstructed state.
fn render_confirm_install_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    items: Vec<crate::state::PackageItem>,
) -> Modal {
    confirm::render_confirm_install(f, app, area, &items);
    Modal::ConfirmInstall { items }
}

/// What: Render PreflightExec modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `ctx`: Context struct containing all PreflightExec fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_preflight_exec_modal(f: &mut Frame, area: Rect, ctx: PreflightExecContext) -> Modal {
    preflight_exec::render_preflight_exec(
        f,
        area,
        &ctx.items,
        ctx.action,
        ctx.tab,
        ctx.verbose,
        &ctx.log_lines,
        ctx.abortable,
        &ctx.header_chips,
    );
    Modal::PreflightExec {
        items: ctx.items,
        action: ctx.action,
        tab: ctx.tab,
        verbose: ctx.verbose,
        log_lines: ctx.log_lines,
        abortable: ctx.abortable,
        header_chips: ctx.header_chips,
    }
}

/// What: Render PostSummary modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all PostSummary fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_post_summary_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    ctx: PostSummaryContext,
) -> Modal {
    post_summary::render_post_summary(
        f,
        app,
        area,
        ctx.success,
        ctx.changed_files,
        ctx.pacnew_count,
        ctx.pacsave_count,
        &ctx.services_pending,
        ctx.snapshot_label.as_ref(),
    );
    Modal::PostSummary {
        success: ctx.success,
        changed_files: ctx.changed_files,
        pacnew_count: ctx.pacnew_count,
        pacsave_count: ctx.pacsave_count,
        services_pending: ctx.services_pending,
        snapshot_label: ctx.snapshot_label,
    }
}

/// What: Render ConfirmRemove modal and return reconstructed state.
fn render_confirm_remove_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    items: Vec<crate::state::PackageItem>,
) -> Modal {
    confirm::render_confirm_remove(f, app, area, &items);
    Modal::ConfirmRemove { items }
}

/// What: Render SystemUpdate modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all SystemUpdate fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_system_update_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    ctx: SystemUpdateContext,
) -> Modal {
    system_update::render_system_update(
        f,
        app,
        area,
        ctx.do_mirrors,
        ctx.do_pacman,
        ctx.do_aur,
        ctx.do_cache,
        ctx.country_idx,
        &ctx.countries,
        ctx.mirror_count,
        ctx.cursor,
    );
    Modal::SystemUpdate {
        do_mirrors: ctx.do_mirrors,
        do_pacman: ctx.do_pacman,
        do_aur: ctx.do_aur,
        do_cache: ctx.do_cache,
        country_idx: ctx.country_idx,
        countries: ctx.countries,
        mirror_count: ctx.mirror_count,
        cursor: ctx.cursor,
    }
}

/// What: Render Help modal and return reconstructed state.
fn render_help_modal(f: &mut Frame, app: &mut AppState, area: Rect) -> Modal {
    help::render_help(f, app, area);
    Modal::Help
}

/// What: Render News modal and return reconstructed state.
fn render_news_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    items: Vec<crate::state::NewsItem>,
    selected: usize,
) -> Modal {
    news::render_news(f, app, area, &items, selected);
    Modal::News { items, selected }
}

/// What: Render OptionalDeps modal and return reconstructed state.
fn render_optional_deps_modal(
    f: &mut Frame,
    area: Rect,
    rows: Vec<OptionalDepRow>,
    selected: usize,
    app: &mut AppState,
) -> Modal {
    misc::render_optional_deps(f, area, &rows, selected, app);
    Modal::OptionalDeps { rows, selected }
}

/// What: Render ScanConfig modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `ctx`: Context struct containing all ScanConfig fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_scan_config_modal(f: &mut Frame, area: Rect, ctx: ScanConfigContext) -> Modal {
    misc::render_scan_config(
        f,
        area,
        ctx.do_clamav,
        ctx.do_trivy,
        ctx.do_semgrep,
        ctx.do_shellcheck,
        ctx.do_virustotal,
        ctx.do_custom,
        ctx.do_sleuth,
        ctx.cursor,
    );
    Modal::ScanConfig {
        do_clamav: ctx.do_clamav,
        do_trivy: ctx.do_trivy,
        do_semgrep: ctx.do_semgrep,
        do_shellcheck: ctx.do_shellcheck,
        do_virustotal: ctx.do_virustotal,
        do_custom: ctx.do_custom,
        do_sleuth: ctx.do_sleuth,
        cursor: ctx.cursor,
    }
}

/// What: Render GnomeTerminalPrompt modal and return reconstructed state.
fn render_gnome_terminal_prompt_modal(f: &mut Frame, area: Rect) -> Modal {
    misc::render_gnome_terminal_prompt(f, area);
    Modal::GnomeTerminalPrompt
}

/// What: Render VirusTotalSetup modal and return reconstructed state.
fn render_virustotal_setup_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    input: String,
    cursor: usize,
) -> Modal {
    misc::render_virustotal_setup(f, app, area, &input);
    Modal::VirusTotalSetup { input, cursor }
}

/// What: Render ImportHelp modal and return reconstructed state.
fn render_import_help_modal(f: &mut Frame, area: Rect) -> Modal {
    misc::render_import_help(f, area);
    Modal::ImportHelp
}

/// What: Dispatch modal rendering using the trait-based approach.
///
/// Inputs:
/// - `modal`: The modal variant to render
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
///
/// Output:
/// - Returns the modal back (for state reconstruction)
///
/// Details:
/// - Handles Preflight separately since it needs mutable access to the whole modal.
/// - All other modals use the trait-based renderer pattern.
pub fn render_modal(modal: Modal, f: &mut Frame, app: &mut AppState, area: Rect) -> Modal {
    // Handle Preflight separately since it needs mutable access to the whole modal
    if let Modal::Preflight { .. } = modal {
        let mut preflight_modal = modal;
        preflight::render_preflight(f, area, app, &mut preflight_modal);
        return preflight_modal;
    }

    // Use trait-based rendering for all other modals
    modal.render(f, app, area)
}
