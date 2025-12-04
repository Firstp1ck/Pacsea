use ratatui::Frame;
use ratatui::prelude::Rect;

use crate::state::{AppState, Modal, modal::PreflightHeaderChips, types::OptionalDepRow};
use crate::ui::modals::{
    alert, announcement, confirm, help, misc, news, password, post_summary, preflight,
    preflight_exec, system_update, updates,
};

/// What: Render `ConfirmBatchUpdate` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Full screen area
/// - `ctx`: Context struct containing all `ConfirmBatchUpdate` fields (taken by value)
///
/// Output:
/// - Returns reconstructed `Modal::ConfirmBatchUpdate` variant
///
/// Details:
/// - Delegates to `confirm::render_confirm_batch_update` and reconstructs the modal variant
fn render_confirm_reinstall_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmReinstallContext,
) -> Modal {
    confirm::render_confirm_reinstall(f, app, area, &ctx.items);
    Modal::ConfirmReinstall {
        items: ctx.items,
        all_items: ctx.all_items,
        header_chips: ctx.header_chips,
    }
}

fn render_confirm_batch_update_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmBatchUpdateContext,
) -> Modal {
    confirm::render_confirm_batch_update(f, app, area, &ctx.items);
    Modal::ConfirmBatchUpdate {
        items: ctx.items,
        dry_run: ctx.dry_run,
    }
}

/// What: Context struct grouping `PreflightExec` modal fields to reduce data flow complexity.
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
    success: Option<bool>,
}

/// What: Context struct grouping `SystemUpdate` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
#[allow(clippy::struct_excessive_bools)]
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

/// What: Context struct grouping `PostSummary` modal fields to reduce data flow complexity.
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

/// What: Context struct grouping `ScanConfig` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
#[allow(clippy::struct_excessive_bools)]
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

/// What: Context struct grouping Alert modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct AlertContext {
    message: String,
}

/// What: Context struct grouping `ConfirmInstall` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmInstallContext {
    items: Vec<crate::state::PackageItem>,
}

/// What: Context struct grouping `ConfirmRemove` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmRemoveContext {
    items: Vec<crate::state::PackageItem>,
}

/// What: Context struct grouping `ConfirmBatchUpdate` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmReinstallContext {
    items: Vec<crate::state::PackageItem>,
    all_items: Vec<crate::state::PackageItem>,
    header_chips: PreflightHeaderChips,
}

struct ConfirmBatchUpdateContext {
    items: Vec<crate::state::PackageItem>,
    dry_run: bool,
}

/// What: Context struct grouping News modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct NewsContext {
    items: Vec<crate::state::NewsItem>,
    selected: usize,
}

/// What: Context struct grouping Announcement modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct AnnouncementContext {
    title: String,
    content: String,
    id: String,
    scroll: u16,
}

/// What: Context struct grouping Updates modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct UpdatesContext {
    entries: Vec<(String, String, String)>,
    scroll: u16,
    selected: usize,
}

/// What: Context struct grouping `OptionalDeps` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct OptionalDepsContext {
    rows: Vec<OptionalDepRow>,
    selected: usize,
}

/// What: Context struct grouping `VirusTotalSetup` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct VirusTotalSetupContext {
    input: String,
    cursor: usize,
}

/// What: Context struct grouping `PasswordPrompt` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct PasswordPromptContext {
    purpose: crate::state::modal::PasswordPurpose,
    items: Vec<crate::state::PackageItem>,
    input: String,
    cursor: usize,
    error: Option<String>,
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
    #[allow(clippy::too_many_lines)] // Modal match with many variants
    fn render(self, f: &mut Frame, app: &mut AppState, area: Rect) -> Modal {
        match self {
            Self::Alert { message } => {
                let ctx = AlertContext { message };
                render_alert_modal(f, app, area, ctx)
            }
            Self::Loading { message } => {
                render_loading_modal(f, area, &message);
                Self::Loading { message }
            }
            Self::ConfirmInstall { items } => {
                let ctx = ConfirmInstallContext { items };
                render_confirm_install_modal(f, app, area, ctx)
            }
            Self::Preflight { .. } => {
                unreachable!("Preflight should be handled separately before trait dispatch")
            }
            Self::PreflightExec {
                items,
                action,
                tab,
                verbose,
                log_lines,
                abortable,
                header_chips,
                success,
            } => {
                let ctx = PreflightExecContext {
                    items,
                    action,
                    tab,
                    verbose,
                    log_lines,
                    abortable,
                    header_chips,
                    success,
                };
                render_preflight_exec_modal(f, app, area, ctx)
            }
            Self::PostSummary {
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
            Self::ConfirmRemove { items } => {
                let ctx = ConfirmRemoveContext { items };
                render_confirm_remove_modal(f, app, area, ctx)
            }
            Self::ConfirmReinstall {
                items,
                all_items,
                header_chips,
            } => {
                let ctx = ConfirmReinstallContext {
                    items,
                    all_items,
                    header_chips,
                };
                render_confirm_reinstall_modal(f, app, area, ctx)
            }
            Self::ConfirmBatchUpdate { items, dry_run } => {
                let ctx = ConfirmBatchUpdateContext { items, dry_run };
                render_confirm_batch_update_modal(f, app, area, ctx)
            }
            Self::SystemUpdate {
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
            Self::Help => render_help_modal(f, app, area),
            Self::News { items, selected } => {
                let ctx = NewsContext { items, selected };
                render_news_modal(f, app, area, ctx)
            }
            Self::Announcement {
                title,
                content,
                id,
                scroll,
            } => {
                let ctx = AnnouncementContext {
                    title,
                    content,
                    id,
                    scroll,
                };
                render_announcement_modal(f, app, area, ctx)
            }
            Self::Updates {
                entries,
                scroll,
                selected,
            } => {
                let ctx = UpdatesContext {
                    entries,
                    scroll,
                    selected,
                };
                render_updates_modal(f, app, area, ctx)
            }
            Self::OptionalDeps { rows, selected } => {
                let ctx = OptionalDepsContext { rows, selected };
                render_optional_deps_modal(f, area, ctx, app)
            }
            Self::ScanConfig {
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
                render_scan_config_modal(f, area, &ctx)
            }
            Self::GnomeTerminalPrompt => render_gnome_terminal_prompt_modal(f, area),
            Self::VirusTotalSetup { input, cursor } => {
                let ctx = VirusTotalSetupContext { input, cursor };
                render_virustotal_setup_modal(f, app, area, ctx)
            }
            Self::ImportHelp => render_import_help_modal(f, app, area),
            Self::PasswordPrompt {
                purpose,
                items,
                input,
                cursor,
                error,
            } => {
                let ctx = PasswordPromptContext {
                    purpose,
                    items,
                    input,
                    cursor,
                    error,
                };
                render_password_prompt_modal(f, app, area, ctx)
            }
            Self::None => Self::None,
        }
    }
}

/// What: Render Alert modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all Alert fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_alert_modal(f: &mut Frame, app: &AppState, area: Rect, ctx: AlertContext) -> Modal {
    alert::render_alert(f, app, area, &ctx.message);
    Modal::Alert {
        message: ctx.message,
    }
}

/// What: Render Loading modal.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `message`: Loading message to display
///
/// Output: None (rendering only)
///
/// Details:
/// - Shows a simple centered loading indicator.
fn render_loading_modal(f: &mut Frame, area: Rect, message: &str) {
    misc::render_loading(f, area, message);
}

/// What: Render `ConfirmInstall` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `ConfirmInstall` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_confirm_install_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmInstallContext,
) -> Modal {
    confirm::render_confirm_install(f, app, area, &ctx.items);
    Modal::ConfirmInstall { items: ctx.items }
}

/// What: Render `PreflightExec` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `PreflightExec` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_preflight_exec_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: PreflightExecContext,
) -> Modal {
    preflight_exec::render_preflight_exec(
        f,
        app,
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
        success: ctx.success,
    }
}

/// What: Render `PostSummary` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `PostSummary` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_post_summary_modal(
    f: &mut Frame,
    app: &AppState,
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

/// What: Render `ConfirmRemove` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `ConfirmRemove` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_confirm_remove_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmRemoveContext,
) -> Modal {
    confirm::render_confirm_remove(f, app, area, &ctx.items);
    Modal::ConfirmRemove { items: ctx.items }
}

/// What: Render `SystemUpdate` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `SystemUpdate` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_system_update_modal(
    f: &mut Frame,
    app: &AppState,
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
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all News fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_news_modal(f: &mut Frame, app: &mut AppState, area: Rect, ctx: NewsContext) -> Modal {
    news::render_news(f, app, area, &ctx.items, ctx.selected);
    Modal::News {
        items: ctx.items,
        selected: ctx.selected,
    }
}

/// What: Render Announcement modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all Announcement fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_announcement_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    ctx: AnnouncementContext,
) -> Modal {
    announcement::render_announcement(f, app, area, &ctx.title, &ctx.content, ctx.scroll);
    Modal::Announcement {
        title: ctx.title,
        content: ctx.content,
        id: ctx.id,
        scroll: ctx.scroll,
    }
}

/// What: Render Updates modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all Updates fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_updates_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    ctx: UpdatesContext,
) -> Modal {
    updates::render_updates(f, app, area, &ctx.entries, ctx.scroll, ctx.selected);
    Modal::Updates {
        entries: ctx.entries,
        scroll: ctx.scroll,
        selected: ctx.selected,
    }
}

/// What: Render `OptionalDeps` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `OptionalDeps` fields (taken by value)
/// - `app`: Mutable application state
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_optional_deps_modal(
    f: &mut Frame,
    area: Rect,
    ctx: OptionalDepsContext,
    app: &AppState,
) -> Modal {
    misc::render_optional_deps(f, area, &ctx.rows, ctx.selected, app);
    Modal::OptionalDeps {
        rows: ctx.rows,
        selected: ctx.selected,
    }
}

/// What: Render `ScanConfig` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `ScanConfig` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_scan_config_modal(f: &mut Frame, area: Rect, ctx: &ScanConfigContext) -> Modal {
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

/// What: Render `GnomeTerminalPrompt` modal and return reconstructed state.
fn render_gnome_terminal_prompt_modal(f: &mut Frame, area: Rect) -> Modal {
    misc::render_gnome_terminal_prompt(f, area);
    Modal::GnomeTerminalPrompt
}

/// What: Render `VirusTotalSetup` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `VirusTotalSetup` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_virustotal_setup_modal(
    f: &mut Frame,
    app: &mut AppState,
    area: Rect,
    ctx: VirusTotalSetupContext,
) -> Modal {
    misc::render_virustotal_setup(f, app, area, &ctx.input);
    Modal::VirusTotalSetup {
        input: ctx.input,
        cursor: ctx.cursor,
    }
}

/// What: Render `ImportHelp` modal and return reconstructed state.
fn render_import_help_modal(f: &mut Frame, app: &AppState, area: Rect) -> Modal {
    misc::render_import_help(f, area, app);
    Modal::ImportHelp
}

/// What: Render `PasswordPrompt` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `PasswordPrompt` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
fn render_password_prompt_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: PasswordPromptContext,
) -> Modal {
    password::render_password_prompt(
        f,
        app,
        area,
        ctx.purpose,
        &ctx.items,
        &ctx.input,
        ctx.error.as_deref(),
    );
    Modal::PasswordPrompt {
        purpose: ctx.purpose,
        items: ctx.items,
        input: ctx.input,
        cursor: ctx.cursor,
        error: ctx.error,
    }
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
