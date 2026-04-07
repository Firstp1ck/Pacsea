use ratatui::Frame;
use ratatui::prelude::Rect;

use crate::state::{
    AppState, Modal,
    modal::PreflightHeaderChips,
    types::{OptionalDepRow, RepositoryModalRow},
};
use crate::ui::modals::{
    alert, announcement, confirm, foreign_overlap, help, misc, news, password, post_summary,
    preflight, preflight_exec, system_update, updates,
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

/// What: Render the confirm batch update modal.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Rendering area
/// - `ctx`: Context with items and dry-run flag
///
/// Output:
/// - Returns the modal state after rendering
///
/// Details:
/// - Displays confirmation dialog for batch package updates
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

/// What: Render `ConfirmAurUpdate` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Full screen area
/// - `ctx`: Context struct containing all `ConfirmAurUpdate` fields (taken by value)
///
/// Output:
/// - Returns reconstructed `Modal::ConfirmAurUpdate` variant
///
/// Details:
/// - Delegates to `confirm::render_confirm_aur_update` and reconstructs the modal variant
fn render_confirm_aur_update_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmAurUpdateContext,
) -> Modal {
    confirm::render_confirm_aur_update(f, app, area, &ctx.message);
    Modal::ConfirmAurUpdate {
        message: ctx.message,
    }
}

/// What: Render `ConfirmAurVote` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Application state
/// - `area`: Full screen area
/// - `ctx`: Context struct containing all `ConfirmAurVote` fields (taken by value)
///
/// Output:
/// - Returns reconstructed `Modal::ConfirmAurVote` variant
///
/// Details:
/// - Delegates to `confirm::render_confirm_aur_vote` and reconstructs the modal variant.
fn render_confirm_aur_vote_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ConfirmAurVoteContext,
) -> Modal {
    confirm::render_confirm_aur_vote(f, app, area, ctx.action, &ctx.message);
    Modal::ConfirmAurVote {
        pkgbase: ctx.pkgbase,
        action: ctx.action,
        message: ctx.message,
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
    /// Package items being processed.
    items: Vec<crate::state::PackageItem>,
    /// Preflight action (install/remove/downgrade).
    action: crate::state::PreflightAction,
    /// Currently active preflight tab.
    tab: crate::state::PreflightTab,
    /// Whether verbose logging is enabled.
    verbose: bool,
    /// Log lines to display.
    log_lines: Vec<String>,
    /// Whether the operation can be aborted.
    abortable: bool,
    /// Header chip metrics.
    header_chips: PreflightHeaderChips,
    /// Operation success status (None if still running).
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
    /// Whether to update mirrors.
    do_mirrors: bool,
    /// Whether to update official packages.
    do_pacman: bool,
    /// Whether to force database sync.
    force_sync: bool,
    /// Whether to update AUR packages.
    do_aur: bool,
    /// Whether to clean package cache.
    do_cache: bool,
    /// Currently selected country index.
    country_idx: usize,
    /// List of available countries for mirror selection.
    countries: Vec<String>,
    /// Number of mirrors to use.
    mirror_count: u16,
    /// Cursor position in the UI.
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
    /// Whether the operation succeeded.
    success: bool,
    /// Number of files changed.
    changed_files: usize,
    /// Number of .pacnew files created.
    pacnew_count: usize,
    /// Number of .pacsave files created.
    pacsave_count: usize,
    /// List of services pending restart.
    services_pending: Vec<String>,
    /// Snapshot label if a snapshot was created.
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
    /// Whether to run `ClamAV` scan.
    do_clamav: bool,
    /// Whether to run `Trivy` scan.
    do_trivy: bool,
    /// Whether to run `Semgrep` scan.
    do_semgrep: bool,
    /// Whether to run `ShellCheck` scan.
    do_shellcheck: bool,
    /// Whether to run `VirusTotal` scan.
    do_virustotal: bool,
    /// Whether to run custom scan.
    do_custom: bool,
    /// Whether to run Sleuth scan.
    do_sleuth: bool,
    /// Cursor position in the UI.
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
    /// Alert message to display.
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
    /// Package items to install.
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
    /// Package items to remove.
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
    /// Packages that are already installed (shown in confirmation).
    items: Vec<crate::state::PackageItem>,
    /// All packages to install (including both installed and not installed).
    all_items: Vec<crate::state::PackageItem>,
    /// Header chip metrics.
    header_chips: PreflightHeaderChips,
}

/// What: Context struct grouping `ConfirmBatchUpdate` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmBatchUpdateContext {
    /// Package items to update.
    items: Vec<crate::state::PackageItem>,
    /// Whether this is a dry-run operation.
    dry_run: bool,
}

/// What: Context struct grouping `ConfirmAurUpdate` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmAurUpdateContext {
    /// Confirmation message text to display to the user.
    message: String,
}

/// What: Context struct grouping `ConfirmAurVote` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct ConfirmAurVoteContext {
    /// AUR package base targeted by the pending action.
    pkgbase: String,
    /// Pending vote action awaiting confirmation.
    action: crate::sources::VoteAction,
    /// Confirmation message text to display to the user.
    message: String,
}

/// What: Context struct grouping News modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct NewsContext {
    /// News feed items to display.
    items: Vec<crate::state::types::NewsFeedItem>,
    /// Currently selected news item index.
    selected: usize,
    /// Scroll offset (lines) for the news list.
    scroll: u16,
}

/// What: Context struct grouping Announcement modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct AnnouncementContext {
    /// Announcement title.
    title: String,
    /// Announcement content text.
    content: String,
    /// Announcement identifier.
    id: String,
    /// Scroll offset in lines.
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
    /// Update entries (package name, current version, new version).
    entries: Vec<(String, String, String)>,
    /// Scroll offset in lines.
    scroll: u16,
    /// Currently selected entry index.
    selected: usize,
    /// Whether slash-filter text mode is active.
    filter_active: bool,
    /// Current slash-filter query text.
    filter_query: String,
    /// Caret position for slash-filter query.
    filter_caret: usize,
    /// Last selected package identity for restore logic.
    last_selected_pkg_name: Option<String>,
    /// Visible entries as original-entry indices after filtering.
    filtered_indices: Vec<usize>,
    /// Selected package names used for batch preflight actions.
    selected_pkg_names: std::collections::HashSet<String>,
}

/// What: Context struct grouping `OptionalDeps` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct OptionalDepsContext {
    /// Optional dependency rows to display.
    rows: Vec<OptionalDepRow>,
    /// Currently selected row index.
    selected: usize,
}

/// What: Context for the read-only Repositories modal.
struct RepositoriesContext {
    /// Merged rows from `repos.conf` and pacman scan.
    rows: Vec<RepositoryModalRow>,
    /// Selected row index.
    selected: usize,
    /// Scroll offset for the row viewport.
    scroll: u16,
    /// `repos.conf` diagnostic when parsing failed.
    repos_conf_error: Option<String>,
    /// Warnings collected while reading pacman configuration.
    pacman_warnings: Vec<String>,
}

/// What: Context struct grouping `SshAurSetup` modal fields.
struct SshAurSetupContext {
    /// Current setup step.
    step: crate::state::SshSetupStep,
    /// Status/instruction lines.
    status_lines: Vec<String>,
    /// Existing host block shown for overwrite confirmation.
    existing_host_block: Option<String>,
}

/// What: Context struct grouping `VirusTotalSetup` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct VirusTotalSetupContext {
    /// API key input buffer.
    input: String,
    /// Cursor position within the input buffer.
    cursor: usize,
}

/// What: Context struct grouping `NewsSetup` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
#[allow(clippy::struct_excessive_bools)]
struct NewsSetupContext {
    /// Whether to show Arch news.
    show_arch_news: bool,
    /// Whether to show security advisories.
    show_advisories: bool,
    /// Whether to show AUR updates.
    show_aur_updates: bool,
    /// Whether to show AUR comments.
    show_aur_comments: bool,
    /// Whether to show official package updates.
    show_pkg_updates: bool,
    /// Maximum age of news items in days (7, 30, or 90).
    max_age_days: Option<u32>,
    /// Current cursor position (0-4 for toggles, 5-7 for date buttons).
    cursor: usize,
}

/// What: Context struct grouping `StartupSetupSelector` modal fields.
struct StartupSetupSelectorContext {
    /// Current selector cursor row.
    cursor: usize,
    /// Selected setup tasks in the selector checklist.
    selected: std::collections::HashSet<crate::state::modal::StartupSetupTask>,
}

/// What: Context struct grouping `WarnAurRepoDuplicate` modal fields.
///
/// Inputs: None (constructed from `Modal` variant).
///
/// Output: Fields for the AUR vs repository warning renderer.
///
/// Details: Preserves the install set and preflight chips across the warning step.
struct WarnAurRepoDuplicateContext {
    /// Conflicting package names.
    dup_names: Vec<String>,
    /// Packages to install after continue.
    packages: Vec<crate::state::PackageItem>,
    /// Preflight chips carried through the warning.
    header_chips: PreflightHeaderChips,
}

/// What: Context struct grouping `ForeignRepoOverlap` modal fields.
///
/// Inputs: None (constructed from `Modal` variant).
///
/// Output: Groups related fields for the overlap wizard renderer.
///
/// Details: Keeps repository name, overlap rows, and phase together.
struct ForeignRepoOverlapContext {
    /// Repository that was applied.
    repo_name: String,
    /// Overlap rows.
    entries: Vec<(String, String)>,
    /// Wizard phase.
    phase: crate::state::modal::ForeignRepoOverlapPhase,
}

/// What: Context struct grouping `PasswordPrompt` modal fields to reduce data flow complexity.
///
/// Inputs: None (constructed from Modal variant).
///
/// Output: Groups related fields together for passing to render functions.
///
/// Details: Reduces individual field extractions and uses, lowering data flow complexity.
struct PasswordPromptContext {
    /// Purpose for requesting the password (install/remove/update/etc.).
    purpose: crate::state::modal::PasswordPurpose,
    /// Items involved in the operation requiring authentication.
    items: Vec<crate::state::PackageItem>,
    /// Current password input buffer.
    input: String,
    /// Cursor position within the input buffer.
    cursor: usize,
    /// Optional error message to display.
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
    /// What: Render a modal variant to the frame.
    ///
    /// Inputs:
    /// - `f`: Frame to render into.
    /// - `app`: Application state.
    /// - `area`: Area to render within.
    ///
    /// Output: Returns the next modal state (may be the same or different).
    ///
    /// Details: Each modal variant implements this trait to render itself.
    fn render(self, f: &mut Frame, app: &mut AppState, area: Rect) -> Modal;
}

impl ModalRenderer for Modal {
    #[allow(clippy::too_many_lines)] // Modal match with many variants (function has 215 lines)
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
            Self::ConfirmAurUpdate { message } => {
                let ctx = ConfirmAurUpdateContext { message };
                render_confirm_aur_update_modal(f, app, area, ctx)
            }
            Self::ConfirmAurVote {
                pkgbase,
                action,
                message,
            } => {
                let ctx = ConfirmAurVoteContext {
                    pkgbase,
                    action,
                    message,
                };
                render_confirm_aur_vote_modal(f, app, area, ctx)
            }
            Self::SystemUpdate {
                do_mirrors,
                do_pacman,
                force_sync,
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
                    force_sync,
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
            Self::News {
                items,
                selected,
                scroll,
            } => {
                let ctx = NewsContext {
                    items,
                    selected,
                    scroll,
                };
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
                filter_active,
                filter_query,
                filter_caret,
                last_selected_pkg_name,
                filtered_indices,
                selected_pkg_names,
            } => {
                let ctx = UpdatesContext {
                    entries,
                    scroll,
                    selected,
                    filter_active,
                    filter_query,
                    filter_caret,
                    last_selected_pkg_name,
                    filtered_indices,
                    selected_pkg_names,
                };
                render_updates_modal(f, app, area, ctx)
            }
            Self::OptionalDeps { rows, selected } => {
                let ctx = OptionalDepsContext { rows, selected };
                render_optional_deps_modal(f, area, ctx, app)
            }
            Self::Repositories {
                rows,
                selected,
                scroll,
                repos_conf_error,
                pacman_warnings,
            } => {
                let ctx = RepositoriesContext {
                    rows,
                    selected,
                    scroll,
                    repos_conf_error,
                    pacman_warnings,
                };
                render_repositories_modal(f, area, ctx, app)
            }
            Self::SshAurSetup {
                step,
                status_lines,
                existing_host_block,
            } => {
                let ctx = SshAurSetupContext {
                    step,
                    status_lines,
                    existing_host_block,
                };
                render_ssh_setup_modal(f, area, ctx)
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
            Self::NewsSetup {
                show_arch_news,
                show_advisories,
                show_aur_updates,
                show_aur_comments,
                show_pkg_updates,
                max_age_days,
                cursor,
            } => {
                let ctx = NewsSetupContext {
                    show_arch_news,
                    show_advisories,
                    show_aur_updates,
                    show_aur_comments,
                    show_pkg_updates,
                    max_age_days,
                    cursor,
                };
                render_news_setup_modal(f, app, area, ctx)
            }
            Self::StartupSetupSelector { cursor, selected } => {
                let ctx = StartupSetupSelectorContext { cursor, selected };
                render_startup_setup_selector_modal(f, app, area, ctx)
            }
            Self::WarnAurRepoDuplicate {
                dup_names,
                packages,
                header_chips,
            } => {
                let ctx = WarnAurRepoDuplicateContext {
                    dup_names,
                    packages,
                    header_chips,
                };
                render_warn_aur_repo_duplicate_modal(f, app, area, ctx)
            }
            Self::ForeignRepoOverlap {
                repo_name,
                entries,
                phase,
            } => {
                let ctx = ForeignRepoOverlapContext {
                    repo_name,
                    entries,
                    phase,
                };
                render_foreign_repo_overlap_modal(f, app, area, ctx)
            }
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
        ctx.force_sync,
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
        force_sync: ctx.force_sync,
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
    news::render_news(f, app, area, &ctx.items, ctx.selected, ctx.scroll);
    Modal::News {
        items: ctx.items,
        selected: ctx.selected,
        scroll: ctx.scroll,
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
    updates::render_updates(
        f,
        app,
        area,
        &ctx.entries,
        &ctx.filtered_indices,
        ctx.scroll,
        ctx.selected,
        ctx.filter_active,
        &ctx.filter_query,
        ctx.filter_caret,
        &ctx.selected_pkg_names,
    );
    Modal::Updates {
        entries: ctx.entries,
        scroll: ctx.scroll,
        selected: ctx.selected,
        filter_active: ctx.filter_active,
        filter_query: ctx.filter_query,
        filter_caret: ctx.filter_caret,
        last_selected_pkg_name: ctx.last_selected_pkg_name,
        filtered_indices: ctx.filtered_indices,
        selected_pkg_names: ctx.selected_pkg_names,
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
    app: &mut AppState,
) -> Modal {
    misc::render_optional_deps(f, area, &ctx.rows, ctx.selected, app);
    Modal::OptionalDeps {
        rows: ctx.rows,
        selected: ctx.selected,
    }
}

/// What: Render `Repositories` modal (read-only) and return reconstructed state.
fn render_repositories_modal(
    f: &mut Frame,
    area: Rect,
    ctx: RepositoriesContext,
    app: &AppState,
) -> Modal {
    misc::render_repositories(
        f,
        area,
        &ctx.rows,
        ctx.selected,
        ctx.scroll,
        ctx.repos_conf_error.as_deref(),
        &ctx.pacman_warnings,
        app,
    );
    Modal::Repositories {
        rows: ctx.rows,
        selected: ctx.selected,
        scroll: ctx.scroll,
        repos_conf_error: ctx.repos_conf_error,
        pacman_warnings: ctx.pacman_warnings,
    }
}

/// What: Render `SshAurSetup` modal and return reconstructed state.
fn render_ssh_setup_modal(f: &mut Frame, area: Rect, ctx: SshAurSetupContext) -> Modal {
    misc::render_ssh_aur_setup(
        f,
        area,
        ctx.step,
        &ctx.status_lines,
        ctx.existing_host_block.as_deref(),
    );
    Modal::SshAurSetup {
        step: ctx.step,
        status_lines: ctx.status_lines,
        existing_host_block: ctx.existing_host_block,
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

/// What: Render `NewsSetup` modal and return reconstructed state.
///
/// Inputs:
/// - `f`: Frame to render into
/// - `app`: Mutable application state
/// - `area`: Full available area
/// - `ctx`: Context struct containing all `NewsSetup` fields (taken by value)
///
/// Output:
/// - Returns the reconstructed Modal
///
/// Details:
/// - Uses context struct to reduce data flow complexity by grouping related fields.
/// - Takes context by value to avoid cloning when reconstructing the Modal.
#[allow(clippy::needless_pass_by_value)]
fn render_news_setup_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: NewsSetupContext,
) -> Modal {
    let NewsSetupContext {
        show_arch_news,
        show_advisories,
        show_aur_updates,
        show_aur_comments,
        show_pkg_updates,
        max_age_days,
        cursor,
    } = ctx;
    misc::render_news_setup(
        f,
        area,
        app,
        show_arch_news,
        show_advisories,
        show_aur_updates,
        show_aur_comments,
        show_pkg_updates,
        max_age_days,
        cursor,
    );
    Modal::NewsSetup {
        show_arch_news,
        show_advisories,
        show_aur_updates,
        show_aur_comments,
        show_pkg_updates,
        max_age_days,
        cursor,
    }
}

/// What: Render `StartupSetupSelector` modal and return reconstructed state.
#[allow(clippy::needless_pass_by_value)]
fn render_startup_setup_selector_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: StartupSetupSelectorContext,
) -> Modal {
    misc::render_startup_setup_selector(f, area, app, ctx.cursor, &ctx.selected);
    Modal::StartupSetupSelector {
        cursor: ctx.cursor,
        selected: ctx.selected,
    }
}

/// What: Render `WarnAurRepoDuplicate` and return reconstructed state.
fn render_warn_aur_repo_duplicate_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: WarnAurRepoDuplicateContext,
) -> Modal {
    foreign_overlap::render_warn_aur_repo_duplicate(f, app, area, &ctx.dup_names);
    Modal::WarnAurRepoDuplicate {
        dup_names: ctx.dup_names,
        packages: ctx.packages,
        header_chips: ctx.header_chips,
    }
}

/// What: Render `ForeignRepoOverlap` and return reconstructed state.
fn render_foreign_repo_overlap_modal(
    f: &mut Frame,
    app: &AppState,
    area: Rect,
    ctx: ForeignRepoOverlapContext,
) -> Modal {
    foreign_overlap::render_foreign_repo_overlap(
        f,
        app,
        area,
        &ctx.repo_name,
        &ctx.entries,
        &ctx.phase,
    );
    Modal::ForeignRepoOverlap {
        repo_name: ctx.repo_name,
        entries: ctx.entries,
        phase: ctx.phase,
    }
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
