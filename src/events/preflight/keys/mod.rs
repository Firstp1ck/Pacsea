//! Key event handling for Preflight modal.

mod action_keys;
mod command_keys;
mod context;
mod navigation;
mod tab_handlers;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::state::AppState;

use action_keys::{
    handle_a_key, handle_d_key, handle_enter_key, handle_esc_key, handle_r_key, handle_shift_r_key,
    handle_space_key,
};
use command_keys::{
    handle_c_key, handle_dry_run_key, handle_f_key, handle_help_key, handle_m_key, handle_p_key,
    handle_q_key, handle_s_key,
};
use context::PreflightKeyContext;
use navigation::{handle_down_key, handle_tab_switch, handle_up_key};

/// What: Handle keys that need access to app fields outside of modal.
///
/// Inputs:
/// - `ke`: Key event
/// - `app`: Mutable application state
///
/// Output:
/// - Always returns `false`.
fn handle_keys_needing_app(ke: KeyEvent, app: &mut AppState) -> bool {
    match ke.code {
        KeyCode::Esc => {
            handle_esc_key(app);
            false
        }
        KeyCode::Enter => {
            handle_enter_key(app);
            false
        }
        KeyCode::Left => handle_tab_switch(app, false),
        KeyCode::Right | KeyCode::Tab => handle_tab_switch(app, true),
        KeyCode::Char('r' | 'R') => {
            if ke.modifiers.contains(KeyModifiers::SHIFT) {
                handle_shift_r_key(app)
            } else {
                false // Handled in first block
            }
        }
        KeyCode::Char('f' | 'F') => handle_f_key(app),
        KeyCode::Char('s' | 'S') => handle_s_key(app),
        KeyCode::Char('d') => handle_dry_run_key(app),
        KeyCode::Char('m') => handle_m_key(app),
        KeyCode::Char('p') => handle_p_key(app),
        KeyCode::Char('c') => handle_c_key(app),
        KeyCode::Char('q') => handle_q_key(app),
        KeyCode::Char('?') => handle_help_key(app),
        _ => false,
    }
}

/// What: Handle key events while the Preflight modal is active (install/remove workflows).
///
/// Inputs:
/// - `ke`: Key event received from crossterm while Preflight is focused
/// - `app`: Mutable application state containing the Preflight modal data
///
/// Output:
/// - Always returns `false` so the outer event loop continues processing.
///
/// Details:
/// - Supports tab switching, tree expansion, dependency/file navigation, scans, dry-run toggles, and
///   command execution across install/remove flows.
/// - Mutates `app.modal` (and related cached fields) to close the modal, open nested dialogs, or
///   keep it updated with resolved dependency/file data.
/// - Returns `false` so callers continue processing, matching existing event-loop expectations.
pub fn handle_preflight_key(ke: KeyEvent, app: &mut AppState) -> bool {
    // First, handle keys that only need ctx (no app access required)
    // This avoids borrow checker conflicts
    {
        if let crate::state::Modal::Preflight {
            tab,
            items,
            action,
            dependency_info,
            dep_selected,
            dep_tree_expanded,
            deps_error,
            file_info,
            file_selected,
            file_tree_expanded,
            files_error,
            service_info,
            service_selected,
            services_loaded,
            services_error,
            sandbox_info,
            sandbox_selected,
            sandbox_tree_expanded,
            selected_optdepends,
            ..
        } = &mut app.modal
        {
            let mut ctx = PreflightKeyContext {
                tab,
                items,
                action,
                dependency_info,
                dep_selected,
                dep_tree_expanded,
                deps_error,
                file_info,
                file_selected,
                file_tree_expanded,
                files_error,
                service_info,
                service_selected,
                services_loaded,
                services_error,
                sandbox_info,
                sandbox_selected,
                sandbox_tree_expanded,
                selected_optdepends,
            };

            match ke.code {
                KeyCode::Up => {
                    handle_up_key(&mut ctx);
                    return false;
                }
                KeyCode::Down => {
                    handle_down_key(&mut ctx);
                    return false;
                }
                KeyCode::Char(' ') => {
                    handle_space_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('D') => {
                    handle_d_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('a' | 'A') => {
                    handle_a_key(&mut ctx);
                    return false;
                }
                KeyCode::Char('r' | 'R') => {
                    if !ke.modifiers.contains(KeyModifiers::SHIFT) {
                        handle_r_key(&mut ctx);
                        return false;
                    }
                    // Shift+R needs app, fall through
                }
                _ => {
                    // Keys that need app access, fall through
                }
            }
        }
        false
    };

    // Now handle keys that need app access
    // The borrow of app.modal has been released, so we can mutably borrow app again
    handle_keys_needing_app(ke, app)
}

#[cfg(test)]
mod tests {
    use crate::state::PackageItem;
    use crate::state::modal::DependencyInfo;
    use std::collections::{HashMap, HashSet};

    use super::context::EnterOrSpaceContext;
    use super::tab_handlers::handle_deps_tab;

    // Helper to create dummy items
    fn make_item(name: &str) -> PackageItem {
        PackageItem {
            name: name.to_string(),
            version: "1.0".to_string(),
            description: "desc".to_string(),
            source: crate::state::Source::Official {
                repo: "core".into(),
                arch: "x86_64".into(),
            },
            popularity: None,
        }
    }

    #[test]
    fn test_handle_deps_tab_toggle() {
        // Setup
        let items = vec![make_item("pkg1")];
        let deps = vec![DependencyInfo {
            name: "dep1".to_string(),
            version: "1.0".to_string(),
            status: crate::state::modal::DependencyStatus::ToInstall,
            source: crate::state::modal::DependencySource::Official {
                repo: "core".into(),
            },
            required_by: vec!["pkg1".to_string()],
            depends_on: vec![],
            is_core: false,
            is_system: false,
        }];
        let mut expanded = HashSet::new();
        let mut selected_optdepends = HashMap::new();
        let mut service_info = Vec::new();

        let mut ctx = EnterOrSpaceContext {
            tab: &crate::state::PreflightTab::Deps,
            items: &items,
            dependency_info: &deps,
            dep_selected: 0, // "pkg1" header
            dep_tree_expanded: &mut expanded,
            file_info: &[],
            file_selected: 0,
            file_tree_expanded: &mut HashSet::new(),
            sandbox_info: &[],
            sandbox_selected: 0,
            sandbox_tree_expanded: &mut HashSet::new(),
            selected_optdepends: &mut selected_optdepends,
            service_info: &mut service_info,
            service_selected: 0,
        };

        // Act: Expand pkg1
        handle_deps_tab(&mut ctx);
        assert!(ctx.dep_tree_expanded.contains("pkg1"));

        // Act: Collapse pkg1
        handle_deps_tab(&mut ctx);
        assert!(!ctx.dep_tree_expanded.contains("pkg1"));
    }
}
