//! Navigation key handlers for Preflight modal.

use crate::state::AppState;

use super::context::PreflightKeyContext;
use crate::events::preflight::display::{
    compute_display_items_len, compute_file_display_items_len, compute_sandbox_display_items_len,
};
use crate::events::preflight::modal::switch_preflight_tab;

/// What: Handle Left/Right/Tab keys - switch tabs.
///
/// Inputs:
/// - `app`: Mutable application state
/// - `direction`: Direction to switch (true for right/tab, false for left)
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_tab_switch(app: &mut AppState, direction: bool) -> bool {
    let (new_tab, items, action) =
        if let crate::state::Modal::Preflight {
            tab, items, action, ..
        } = &app.modal
        {
            let current_tab = *tab;
            let next_tab = if direction {
                match current_tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Services,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Summary,
                }
            } else {
                match current_tab {
                    crate::state::PreflightTab::Summary => crate::state::PreflightTab::Sandbox,
                    crate::state::PreflightTab::Deps => crate::state::PreflightTab::Summary,
                    crate::state::PreflightTab::Files => crate::state::PreflightTab::Deps,
                    crate::state::PreflightTab::Services => crate::state::PreflightTab::Files,
                    crate::state::PreflightTab::Sandbox => crate::state::PreflightTab::Services,
                }
            };
            (next_tab, items.clone(), *action)
        } else {
            return false;
        };

    if let crate::state::Modal::Preflight { tab, .. } = &mut app.modal {
        let old_tab = *tab;
        *tab = new_tab;
        tracing::info!(
            "[Preflight] Keyboard tab switch: Updated tab field from {:?} to {:?}",
            old_tab,
            new_tab
        );
    }

    switch_preflight_tab(new_tab, app, &items, &action);
    false
}

/// What: Handle Up key - move selection up in current tab.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_up_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.items.is_empty() {
        if *ctx.dep_selected > 0 {
            *ctx.dep_selected -= 1;
            tracing::debug!(
                "[Preflight] Deps Up: dep_selected={}, items={}",
                *ctx.dep_selected,
                ctx.items.len()
            );
        } else {
            tracing::debug!(
                "[Preflight] Deps Up: already at top (dep_selected=0), items={}",
                ctx.items.len()
            );
        }
    } else if *ctx.tab == crate::state::PreflightTab::Files
        && !ctx.file_info.is_empty()
        && *ctx.file_selected > 0
    {
        *ctx.file_selected -= 1;
    } else if *ctx.tab == crate::state::PreflightTab::Services
        && !ctx.service_info.is_empty()
        && *ctx.service_selected > 0
    {
        *ctx.service_selected -= 1;
    } else if *ctx.tab == crate::state::PreflightTab::Sandbox
        && !ctx.items.is_empty()
        && *ctx.sandbox_selected > 0
    {
        *ctx.sandbox_selected -= 1;
    }
    false
}

/// What: Handle Down key - move selection down in current tab.
///
/// Inputs:
/// - `ctx`: Preflight key context
///
/// Output:
/// - Always returns `false`.
pub(super) fn handle_down_key(ctx: &mut PreflightKeyContext<'_>) -> bool {
    if *ctx.tab == crate::state::PreflightTab::Deps && !ctx.items.is_empty() {
        let display_len =
            compute_display_items_len(ctx.items, ctx.dependency_info, ctx.dep_tree_expanded);
        tracing::debug!(
            "[Preflight] Deps Down: dep_selected={}, display_len={}, items={}, deps={}, expanded_count={}",
            *ctx.dep_selected,
            display_len,
            ctx.items.len(),
            ctx.dependency_info.len(),
            ctx.dep_tree_expanded.len()
        );
        if *ctx.dep_selected < display_len.saturating_sub(1) {
            *ctx.dep_selected += 1;
            tracing::debug!(
                "[Preflight] Deps Down: moved to dep_selected={}",
                *ctx.dep_selected
            );
        } else {
            tracing::debug!(
                "[Preflight] Deps Down: already at bottom (dep_selected={}, display_len={})",
                *ctx.dep_selected,
                display_len
            );
        }
    } else if *ctx.tab == crate::state::PreflightTab::Files {
        let display_len =
            compute_file_display_items_len(ctx.items, ctx.file_info, ctx.file_tree_expanded);
        if *ctx.file_selected < display_len.saturating_sub(1) {
            *ctx.file_selected += 1;
        }
    } else if *ctx.tab == crate::state::PreflightTab::Services && !ctx.service_info.is_empty() {
        let max_index = ctx.service_info.len().saturating_sub(1);
        if *ctx.service_selected < max_index {
            *ctx.service_selected += 1;
        }
    } else if *ctx.tab == crate::state::PreflightTab::Sandbox && !ctx.items.is_empty() {
        let display_len = compute_sandbox_display_items_len(
            ctx.items,
            ctx.sandbox_info,
            ctx.sandbox_tree_expanded,
        );
        if *ctx.sandbox_selected < display_len.saturating_sub(1) {
            *ctx.sandbox_selected += 1;
        }
    }
    false
}
