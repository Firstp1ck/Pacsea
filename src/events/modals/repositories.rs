//! Repositories modal: navigation and privileged apply (Phase 3).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::logic::repos::{
    DEFAULT_MAIN_PACMAN_PATH, ReposConfFile, build_repo_apply_bundle,
    build_repo_key_refresh_bundle, load_resolve_repos_from_str, read_main_pacman_conf_text,
    repositories_linux_actions_supported,
};
use crate::state::AppState;
use crate::state::types::RepositoryModalRow;
use crate::theme::{config_dir, resolve_repos_config_path};

/// What: Shipped `repos.conf` example text embedded at compile time.
///
/// Inputs:
/// - None.
///
/// Output:
/// - Static string matching `config/examples/repos.conf.example` in the source tree.
///
/// Details:
/// - Used so the Repositories modal can open a real file path on machines that do not have the
///   repository checkout (for example AUR or distro packages).
const REPOS_CONF_EXAMPLE_SHIPPED: &str =
    include_str!("../../../config/examples/repos.conf.example");

/// What: Height of the scroll viewport (data rows) for the Repositories modal.
const REPOS_VIEWPORT_ROWS: usize = 12;

/// What: Handle keys for the read-only Repositories modal.
///
/// Inputs:
/// - `ke`: Terminal key event.
/// - `app`: Application state (closes modal on Esc).
/// - `row_count`: Number of data rows.
/// - `selected`: Selected index.
/// - `scroll`: First visible row index.
///
/// Output:
/// - `Some(...)` when the key was handled; `None` otherwise.
///
/// Details:
/// - Enter is handled in [`crate::events::modals::handlers::handle_repositories_modal`] via [`enter_repo_apply`].
/// - `o` opens `repos.conf` like the Config menu; PageUp/PageDown scroll the list.
pub(super) fn handle_repositories_modal_keys(
    ke: KeyEvent,
    app: &mut AppState,
    row_count: usize,
    selected: &mut usize,
    scroll: &mut u16,
) -> Option<bool> {
    match ke.code {
        KeyCode::Char('o' | 'O') => {
            open_user_repos_conf_in_editor(app);
            Some(false)
        }
        KeyCode::Esc | KeyCode::Char('q') => {
            app.modal = crate::state::Modal::None;
            Some(false)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if *selected > 0 {
                *selected -= 1;
            }
            clamp_scroll_for_selection(*selected, scroll, row_count);
            Some(false)
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if *selected + 1 < row_count {
                *selected += 1;
            }
            clamp_scroll_for_selection(*selected, scroll, row_count);
            Some(false)
        }
        KeyCode::PageUp => {
            *scroll = scroll.saturating_sub(u16::try_from(REPOS_VIEWPORT_ROWS).unwrap_or(u16::MAX));
            clamp_scroll_bounds(scroll, row_count);
            Some(false)
        }
        KeyCode::PageDown => {
            *scroll = scroll.saturating_add(u16::try_from(REPOS_VIEWPORT_ROWS).unwrap_or(12));
            clamp_scroll_bounds(scroll, row_count);
            Some(false)
        }
        KeyCode::Home => {
            *scroll = 0;
            Some(false)
        }
        KeyCode::End => {
            end_scroll(scroll, row_count);
            Some(false)
        }
        _ => {
            if ke.modifiers.contains(KeyModifiers::CONTROL) && matches!(ke.code, KeyCode::Char('d'))
            {
                *scroll =
                    scroll.saturating_add(u16::try_from(REPOS_VIEWPORT_ROWS / 2).unwrap_or(6));
                clamp_scroll_bounds(scroll, row_count);
                return Some(false);
            }
            if ke.modifiers.contains(KeyModifiers::CONTROL) && matches!(ke.code, KeyCode::Char('u'))
            {
                *scroll =
                    scroll.saturating_sub(u16::try_from(REPOS_VIEWPORT_ROWS / 2).unwrap_or(6));
                clamp_scroll_bounds(scroll, row_count);
                return Some(false);
            }
            None
        }
    }
}

/// What: Resolve a `repos.conf` path for reading (existing file), matching modal apply behavior.
///
/// Inputs:
/// - None.
///
/// Output:
/// - `Some(path)` when a candidate file exists.
///
/// Details:
/// - Uses [`resolve_repos_config_path`] first, then `config_dir()/repos.conf` if that file exists.
fn repos_conf_path_for_read() -> Option<PathBuf> {
    resolve_repos_config_path().or_else(|| {
        let p = config_dir().join("repos.conf");
        p.is_file().then_some(p)
    })
}

/// What: Open the user `repos.conf` in the system editor (same strategy as the Config menu).
///
/// Inputs:
/// - `app`: Application state for toasts / i18n.
///
/// Output:
/// - None.
///
/// Details:
/// - Ensures the config directory exists and seeds an empty commented file when missing so editors
///   open a real path. Shows a short toast on success or a persistent-style error toast on failure.
pub(super) fn open_user_repos_conf_in_editor(app: &mut AppState) {
    let path = config_dir().join("repos.conf");
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        app.toast_message = Some(crate::i18n::t(
            app,
            "app.modals.repositories.open_config.mkdir_failed",
        ));
        app.toast_expires_at = Some(Instant::now() + Duration::from_secs(4));
        return;
    }
    if !path.exists() {
        let seed =
            "# Pacsea repos.conf — add [[repo]] blocks (see config/examples/repos.conf.example).\n";
        if std::fs::write(&path, seed).is_err() {
            app.toast_message = Some(crate::i18n::t(
                app,
                "app.modals.repositories.open_config.create_failed",
            ));
            app.toast_expires_at = Some(Instant::now() + Duration::from_secs(4));
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        crate::util::open_file(&path);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let editor_cmd = crate::install::editor_open_config_command(&path);
        let cmds = vec![editor_cmd];
        std::thread::spawn(move || {
            crate::install::spawn_shell_commands_in_terminal(&cmds);
        });
    }
    app.toast_message = Some(crate::i18n::t_fmt1(
        app,
        "app.modals.repositories.open_config.started",
        path.display().to_string(),
    ));
    app.toast_expires_at = Some(Instant::now() + Duration::from_secs(3));
}

/// What: Open the shipped `repos.conf` example in an editor for setup guidance.
///
/// Inputs:
/// - `app`: Application state for toasts / i18n.
///
/// Output:
/// - None.
///
/// Details:
/// - Writes [`REPOS_CONF_EXAMPLE_SHIPPED`] to `config_dir()/repos.conf.example` so external editors
///   receive a stable path (embedded content works for packaged installs, unlike
///   `CARGO_MANIFEST_DIR` source paths). Overwrites that file on each open so the buffer matches
///   the version shipped in the binary.
pub(super) fn open_repos_conf_example_in_editor(app: &mut AppState) {
    let path = config_dir().join("repos.conf.example");
    if let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        app.toast_message = Some(crate::i18n::t(
            app,
            "app.modals.repositories.open_config.mkdir_failed",
        ));
        app.toast_expires_at = Some(Instant::now() + Duration::from_secs(4));
        return;
    }
    if std::fs::write(&path, REPOS_CONF_EXAMPLE_SHIPPED).is_err() {
        app.toast_message = Some(crate::i18n::t(
            app,
            "app.modals.repositories.setup_example_write_failed",
        ));
        app.toast_expires_at = Some(Instant::now() + Duration::from_secs(4));
        return;
    }
    #[cfg(target_os = "windows")]
    {
        crate::util::open_file(&path);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let editor_cmd = crate::install::editor_open_config_command(&path);
        let cmds = vec![editor_cmd];
        std::thread::spawn(move || {
            crate::install::spawn_shell_commands_in_terminal(&cmds);
        });
    }
    app.toast_message = Some(crate::i18n::t_fmt1(
        app,
        "app.modals.repositories.setup_example_started",
        path.display().to_string(),
    ));
    app.toast_expires_at = Some(Instant::now() + Duration::from_secs(3));
}

/// What: Queue `pacman-key` receive + local sign for the focused row’s `key_id` only.
///
/// Inputs:
/// - `app`: Application state.
/// - `rows` / `selected`: Modal list focus.
/// - `repos_conf_error`: When set, apply-style flows are blocked.
///
/// Output:
/// - `Ok(())` when the executor hand-off started.
/// - `Err` with a user-visible message when planning fails or the platform is unsupported.
///
/// Details:
/// - Linux only; reuses [`queue_repo_apply_execution`] and `PasswordPurpose::RepoApply`.
pub(super) fn enter_repo_key_refresh(
    app: &mut AppState,
    rows: &[RepositoryModalRow],
    selected: usize,
    repos_conf_error: Option<&str>,
) -> Result<(), String> {
    if !repositories_linux_actions_supported() {
        return Err(crate::i18n::t(
            app,
            "app.modals.repositories.refresh_key.unsupported_platform",
        ));
    }
    if repos_conf_error.is_some() {
        return Err(crate::i18n::t(
            app,
            "app.modals.repositories.apply.fix_repos_conf",
        ));
    }
    let row = rows
        .get(selected)
        .ok_or_else(|| crate::i18n::t(app, "app.modals.repositories.apply.no_selection"))?;
    let path = repos_conf_path_for_read()
        .ok_or_else(|| crate::i18n::t(app, "app.modals.repositories.apply.no_config_path"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "{} {}: {e}",
            crate::i18n::t(app, "app.modals.repositories.apply.read_failed"),
            path.display()
        )
    })?;
    let (repo_rows, _) = load_resolve_repos_from_str(&text).map_err(|e| {
        format!(
            "{} {e}",
            crate::i18n::t(app, "app.modals.repositories.apply.parse_failed"),
        )
    })?;
    let repos = ReposConfFile { repo: repo_rows };
    let section = row.pacman_section_name.trim();
    let bundle = build_repo_key_refresh_bundle(&repos, section)?;
    app.pending_repo_apply_summary = Some(bundle.summary_lines);
    queue_repo_apply_execution(app, bundle.commands);
    Ok(())
}

/// What: Keep the scroll offset so the selected row stays visible.
///
/// Inputs:
/// - `selected`: Cursor row.
/// - `scroll`: Mutable first visible index.
/// - `row_count`: Total rows.
///
/// Output:
/// - None (updates `scroll`).
///
/// Details:
/// - No-op when `row_count` is zero.
fn clamp_scroll_for_selection(selected: usize, scroll: &mut u16, row_count: usize) {
    if row_count == 0 {
        *scroll = 0;
        return;
    }
    let sc = *scroll as usize;
    if selected < sc {
        *scroll = u16::try_from(selected).unwrap_or(0);
        return;
    }
    let bottom = sc.saturating_add(REPOS_VIEWPORT_ROWS);
    if selected >= bottom && REPOS_VIEWPORT_ROWS > 0 {
        let new_sc = selected + 1 - REPOS_VIEWPORT_ROWS;
        *scroll = u16::try_from(new_sc).unwrap_or(0);
    }
    clamp_scroll_bounds(scroll, row_count);
}

/// What: Clamp scroll so the window stays within row range.
///
/// Inputs:
/// - `scroll`: First visible index.
/// - `row_count`: Length of row list.
///
/// Output:
/// - None.
fn clamp_scroll_bounds(scroll: &mut u16, row_count: usize) {
    if row_count == 0 {
        *scroll = 0;
        return;
    }
    let max_start = row_count.saturating_sub(1);
    let cap = max_start
        .saturating_sub(REPOS_VIEWPORT_ROWS.saturating_sub(1))
        .min(max_start);
    let cap_u16 = u16::try_from(cap).unwrap_or(u16::MAX);
    if *scroll > cap_u16 {
        *scroll = cap_u16;
    }
}

/// What: Scroll to the last possible window start.
///
/// Inputs:
/// - `scroll`: Updated to show the tail of the list.
/// - `row_count`: Total rows.
///
/// Output:
/// - None.
fn end_scroll(scroll: &mut u16, row_count: usize) {
    if row_count == 0 {
        *scroll = 0;
        return;
    }
    let max_start = row_count.saturating_sub(1);
    let cap = max_start.saturating_sub(REPOS_VIEWPORT_ROWS.saturating_sub(1));
    *scroll = u16::try_from(cap).unwrap_or(0);
}

/// What: Start repo apply from the Repositories modal after the user presses Enter.
///
/// Inputs:
/// - `app`: Application state (may set password prompt or preflight).
/// - `rows` / `selected`: Current modal list and cursor.
/// - `repos_conf_error`: Set when `repos.conf` failed to load in the modal.
///
/// Output:
/// - `Ok(())` when the auth/executor flow was started.
/// - `Err` with a user-visible message when planning failed.
///
/// Details:
/// - Builds a full managed drop-in for all eligible rows; see [`build_repo_apply_bundle`].
pub(super) fn enter_repo_apply(
    app: &mut AppState,
    rows: &[RepositoryModalRow],
    selected: usize,
    repos_conf_error: Option<&str>,
) -> Result<(), String> {
    if repos_conf_error.is_some() {
        return Err(crate::i18n::t(
            app,
            "app.modals.repositories.apply.fix_repos_conf",
        ));
    }
    if rows.is_empty() {
        return Err(crate::i18n::t(app, "app.modals.repositories.apply.no_rows"));
    }
    rows.get(selected)
        .ok_or_else(|| crate::i18n::t(app, "app.modals.repositories.apply.no_selection"))?;

    let path = resolve_repos_config_path()
        .ok_or_else(|| crate::i18n::t(app, "app.modals.repositories.apply.no_config_path"))?;
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "{} {}: {e}",
            crate::i18n::t(app, "app.modals.repositories.apply.read_failed"),
            path.display()
        )
    })?;
    let (repo_rows, _) = load_resolve_repos_from_str(&text).map_err(|e| {
        format!(
            "{} {e}",
            crate::i18n::t(app, "app.modals.repositories.apply.parse_failed"),
        )
    })?;
    let repos = ReposConfFile { repo: repo_rows };
    let main = read_main_pacman_conf_text(Path::new(DEFAULT_MAIN_PACMAN_PATH))?;
    let section = rows[selected].pacman_section_name.trim();
    let bundle = build_repo_apply_bundle(&repos, &main, section)?;
    app.pending_repo_apply_summary = Some(bundle.summary_lines);
    queue_repo_apply_execution(app, bundle.commands);
    Ok(())
}

/// What: Route repo apply commands through the same privilege/auth paths as system update.
///
/// Inputs:
/// - `app`: Application state.
/// - `cmds`: Privilege-wrapped shell commands from [`build_repo_apply_bundle`].
///
/// Output:
/// - None (sets modals / pending fields).
///
/// Details:
/// - Uses `PasswordPurpose::RepoApply` and `pending_repo_apply_commands`.
fn queue_repo_apply_execution(app: &mut AppState, cmds: Vec<String>) {
    if cmds.is_empty() {
        app.pending_repo_apply_summary = None;
        app.modal = crate::state::Modal::Alert {
            message: crate::i18n::t(app, "app.modals.repositories.apply.empty_plan"),
        };
        return;
    }

    if std::env::var("PACSEA_TEST_OUT").is_ok() {
        crate::install::spawn_shell_commands_in_terminal(&cmds);
        app.modal = crate::state::Modal::None;
        app.pending_repo_apply_summary = None;
        app.pending_repo_apply_commands = None;
        return;
    }

    let settings = crate::theme::settings();

    if crate::logic::password::should_use_interactive_auth_handoff(&settings) {
        match crate::events::try_interactive_auth_handoff() {
            Ok(true) => {
                let log_lines = app.pending_repo_apply_summary.take().unwrap_or_default();
                app.modal = crate::state::Modal::PreflightExec {
                    items: Vec::new(),
                    action: crate::state::PreflightAction::Install,
                    tab: crate::state::PreflightTab::Summary,
                    verbose: false,
                    log_lines,
                    abortable: false,
                    header_chips: crate::state::modal::PreflightHeaderChips::default(),
                    success: None,
                };
                app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
                    commands: cmds,
                    password: None,
                    dry_run: app.dry_run,
                });
            }
            Ok(false) => {
                app.pending_repo_apply_summary = None;
                app.pending_repo_apply_commands = None;
                app.modal = crate::state::Modal::Alert {
                    message: crate::i18n::t(app, "app.errors.authentication_failed"),
                };
            }
            Err(e) => {
                app.pending_repo_apply_summary = None;
                app.pending_repo_apply_commands = None;
                app.modal = crate::state::Modal::Alert { message: e };
            }
        }
        return;
    }

    if crate::logic::password::resolve_auth_mode(&settings)
        == crate::logic::privilege::AuthMode::PasswordlessOnly
        && crate::logic::password::should_use_passwordless_sudo(&settings)
    {
        let log_lines = app.pending_repo_apply_summary.take().unwrap_or_default();
        app.modal = crate::state::Modal::PreflightExec {
            items: Vec::new(),
            action: crate::state::PreflightAction::Install,
            tab: crate::state::PreflightTab::Summary,
            verbose: false,
            log_lines,
            abortable: false,
            header_chips: crate::state::modal::PreflightHeaderChips::default(),
            success: None,
        };
        app.pending_executor_request = Some(crate::install::ExecutorRequest::Update {
            commands: cmds,
            password: None,
            dry_run: app.dry_run,
        });
        return;
    }

    app.pending_repo_apply_commands = Some(cmds);
    app.modal = crate::state::Modal::PasswordPrompt {
        purpose: crate::state::modal::PasswordPurpose::RepoApply,
        items: Vec::new(),
        input: String::new(),
        cursor: 0,
        error: None,
    };
}

#[cfg(test)]
mod repos_conf_example_embed_tests {
    use super::REPOS_CONF_EXAMPLE_SHIPPED;

    /// What: Assert the embedded repos example is non-empty and recognizable.
    ///
    /// Inputs:
    /// - None.
    ///
    /// Output:
    /// - None.
    ///
    /// Details:
    /// - Guards against a broken `include_str!` path or an accidentally emptied example file.
    #[test]
    fn embedded_repos_example_is_non_empty() {
        assert!(
            REPOS_CONF_EXAMPLE_SHIPPED.len() > 20,
            "embedded example should contain substantial content"
        );
        assert!(
            REPOS_CONF_EXAMPLE_SHIPPED.contains("Pacsea"),
            "embedded example should identify itself as Pacsea documentation"
        );
    }
}
