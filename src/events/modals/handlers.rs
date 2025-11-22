//! Individual modal handler functions that encapsulate field extraction and restoration.

use crossterm::event::{KeyCode, KeyEvent};
use tokio::sync::mpsc;

use super::restore;
use crate::state::{AppState, Modal, PackageItem};

/// What: Handle key events for Alert modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Alert modal variant with message
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and handles restoration
/// - Returns the result from common handler to prevent event propagation when Esc is pressed
pub(crate) fn handle_alert_modal(ke: KeyEvent, app: &mut AppState, modal: Modal) -> bool {
    if let Modal::Alert { ref message } = modal {
        super::common::handle_alert(ke, app, message)
    } else {
        false
    }
}

/// What: Handle key events for `PreflightExec` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `PreflightExec` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler, updates verbose flag, and restores modal if needed
pub(crate) fn handle_preflight_exec_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::PreflightExec {
        ref mut verbose,
        ref log_lines,
        ref abortable,
        ref items,
        ref action,
        ref tab,
        ref header_chips,
    } = modal
    {
        super::common::handle_preflight_exec(ke, app, verbose, *abortable, items);
        restore::restore_if_not_closed_with_excluded_keys(
            app,
            &ke,
            &[KeyCode::Esc, KeyCode::Char('q')],
            Modal::PreflightExec {
                verbose: *verbose,
                log_lines: log_lines.clone(),
                abortable: *abortable,
                items: items.clone(),
                action: *action,
                tab: *tab,
                header_chips: header_chips.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `PostSummary` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `PostSummary` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(crate) fn handle_post_summary_modal(ke: KeyEvent, app: &mut AppState, modal: Modal) -> bool {
    if let Modal::PostSummary {
        ref success,
        ref changed_files,
        ref pacnew_count,
        ref pacsave_count,
        ref services_pending,
        ref snapshot_label,
    } = modal
    {
        super::common::handle_post_summary(ke, app, services_pending);
        restore::restore_if_not_closed_with_excluded_keys(
            app,
            &ke,
            &[KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')],
            Modal::PostSummary {
                success: *success,
                changed_files: *changed_files,
                pacnew_count: *pacnew_count,
                pacsave_count: *pacsave_count,
                services_pending: services_pending.clone(),
                snapshot_label: snapshot_label.clone(),
            },
        );
    }
    false
}

/// What: Handle key events for `SystemUpdate` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `SystemUpdate` modal variant
///
/// Output:
/// - `true` if event propagation should stop, otherwise `false`
///
/// Details:
/// - Delegates to `system_update` handler and restores modal if needed
pub(crate) fn handle_system_update_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::SystemUpdate {
        ref mut do_mirrors,
        ref mut do_pacman,
        ref mut do_aur,
        ref mut do_cache,
        ref mut country_idx,
        ref countries,
        ref mut mirror_count,
        ref mut cursor,
    } = modal
    {
        let should_stop = super::system_update::handle_system_update(
            ke,
            app,
            do_mirrors,
            do_pacman,
            do_aur,
            do_cache,
            country_idx,
            countries,
            mirror_count,
            cursor,
        );
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::SystemUpdate {
                do_mirrors: *do_mirrors,
                do_pacman: *do_pacman,
                do_aur: *do_aur,
                do_cache: *do_cache,
                country_idx: *country_idx,
                countries: countries.clone(),
                mirror_count: *mirror_count,
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `ConfirmInstall` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmInstall` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to install handler
pub(crate) fn handle_confirm_install_modal(ke: KeyEvent, app: &mut AppState, modal: Modal) -> bool {
    if let Modal::ConfirmInstall { ref items } = modal {
        super::install::handle_confirm_install(ke, app, items);
    }
    false
}

/// What: Handle key events for `ConfirmRemove` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ConfirmRemove` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to install handler
pub(crate) fn handle_confirm_remove_modal(ke: KeyEvent, app: &mut AppState, modal: Modal) -> bool {
    if let Modal::ConfirmRemove { ref items } = modal {
        super::install::handle_confirm_remove(ke, app, items);
    }
    false
}

/// What: Handle key events for Help modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Help modal variant (unit type)
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler
pub(crate) fn handle_help_modal(ke: KeyEvent, app: &mut AppState, _modal: Modal) -> bool {
    super::common::handle_help(ke, app)
}

/// What: Handle key events for News modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: News modal variant
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(crate) fn handle_news_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::News {
        ref items,
        ref mut selected,
    } = modal
    {
        let result = super::common::handle_news(ke, app, items, selected);
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::News {
                items: items.clone(),
                selected: *selected,
            },
        );
    }
    false
}

/// What: Handle key events for Updates modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: Updates modal variant
///
/// Output:
/// - `true` if Esc was pressed (to stop propagation), otherwise `false`
///
/// Details:
/// - Delegates to common handler and restores modal if needed
pub(crate) fn handle_updates_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::Updates {
        ref entries,
        ref mut scroll,
    } = modal
    {
        let result = super::common::handle_updates(ke, app, entries, scroll);
        return restore::restore_if_not_closed_with_bool_result(
            app,
            result,
            Modal::Updates {
                entries: entries.clone(),
                scroll: *scroll,
            },
        );
    }
    false
}

/// What: Handle key events for `OptionalDeps` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `OptionalDeps` modal variant
///
/// Output:
/// - `true` if event propagation should stop, otherwise `false`
///
/// Details:
/// - Delegates to `optional_deps` handler and restores modal if needed
pub(crate) fn handle_optional_deps_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::OptionalDeps {
        ref rows,
        ref mut selected,
    } = modal
    {
        let should_stop = super::optional_deps::handle_optional_deps(ke, app, rows, selected);
        return restore::restore_if_not_closed_with_option_result(
            app,
            &ke,
            should_stop,
            Modal::OptionalDeps {
                rows: rows.clone(),
                selected: *selected,
            },
        );
    }
    false
}

/// What: Handle key events for `ScanConfig` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `ScanConfig` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to scan handler and restores modal if needed
pub(crate) fn handle_scan_config_modal(ke: KeyEvent, app: &mut AppState, mut modal: Modal) -> bool {
    if let Modal::ScanConfig {
        ref mut do_clamav,
        ref mut do_trivy,
        ref mut do_semgrep,
        ref mut do_shellcheck,
        ref mut do_virustotal,
        ref mut do_custom,
        ref mut do_sleuth,
        ref mut cursor,
    } = modal
    {
        super::scan::handle_scan_config(
            ke,
            app,
            do_clamav,
            do_trivy,
            do_semgrep,
            do_shellcheck,
            do_virustotal,
            do_custom,
            do_sleuth,
            cursor,
        );
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::ScanConfig {
                do_clamav: *do_clamav,
                do_trivy: *do_trivy,
                do_semgrep: *do_semgrep,
                do_shellcheck: *do_shellcheck,
                do_virustotal: *do_virustotal,
                do_custom: *do_custom,
                do_sleuth: *do_sleuth,
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `VirusTotalSetup` modal, including restoration logic.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `VirusTotalSetup` modal variant
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to scan handler and restores modal if needed
pub(crate) fn handle_virustotal_setup_modal(
    ke: KeyEvent,
    app: &mut AppState,
    mut modal: Modal,
) -> bool {
    if let Modal::VirusTotalSetup {
        ref mut input,
        ref mut cursor,
    } = modal
    {
        super::scan::handle_virustotal_setup(ke, app, input, cursor);
        restore::restore_if_not_closed_with_esc(
            app,
            &ke,
            Modal::VirusTotalSetup {
                input: input.clone(),
                cursor: *cursor,
            },
        );
    }
    false
}

/// What: Handle key events for `GnomeTerminalPrompt` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `modal`: `GnomeTerminalPrompt` modal variant (unit type)
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to common handler
pub(crate) fn handle_gnome_terminal_prompt_modal(
    ke: KeyEvent,
    app: &mut AppState,
    _modal: Modal,
) -> bool {
    super::common::handle_gnome_terminal_prompt(ke, app);
    false
}

/// What: Handle key events for `ImportHelp` modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
/// - `add_tx`: Channel for adding packages
/// - `modal`: `ImportHelp` modal variant (unit type)
///
/// Output:
/// - `false` (never stops propagation)
///
/// Details:
/// - Delegates to import handler
pub(crate) fn handle_import_help_modal(
    ke: KeyEvent,
    app: &mut AppState,
    add_tx: &mpsc::UnboundedSender<PackageItem>,
    _modal: Modal,
) -> bool {
    super::import::handle_import_help(ke, app, add_tx);
    false
}
