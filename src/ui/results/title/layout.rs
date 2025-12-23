use unicode_width::UnicodeWidthStr;

use super::super::OptionalRepos;
use super::types::{
    ArtixVisibilityContext, CoreFilterLabels, OptionalReposLabels, TitleI18nStrings,
    TitleLayoutInfo,
};
use super::width::{
    calculate_base_consumed_space, calculate_consumed_without_specific,
    calculate_optional_repos_width, create_repos_without_specific,
};

/// What: Determine if Artix-specific repos should be shown initially.
///
/// Inputs:
/// - `base_consumed`: Base consumed space
/// - `consumed_left`: Consumed space with all filters
/// - `inner_width`: Available width
/// - `right_w`: Width needed for right-aligned buttons
/// - `optional_repos`: Optional repository flags
/// - `optional_labels`: Labels for optional repos
///
/// Output: Tuple of (`show_artix_specific_repos`, `final_consumed_left`, `pad`).
///
/// Details: Determines initial visibility of Artix-specific repos based on available space.
fn determine_initial_artix_visibility(
    base_consumed: u16,
    consumed_left: u16,
    inner_width: u16,
    right_w: u16,
    optional_repos: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
) -> (bool, u16, u16) {
    let pad = inner_width.saturating_sub(consumed_left.saturating_add(right_w));

    if pad >= 1 {
        return (true, consumed_left, pad);
    }

    // Not enough space, try without Artix-specific repos
    let repos_without_specific = create_repos_without_specific(optional_repos);
    let consumed_without_specific = calculate_consumed_without_specific(
        base_consumed,
        &repos_without_specific,
        optional_labels,
        optional_repos.has_artix,
    );
    let new_pad = inner_width.saturating_sub(consumed_without_specific.saturating_add(right_w));

    if new_pad >= 1 {
        (false, consumed_without_specific, new_pad)
    } else {
        (true, consumed_left, pad)
    }
}

/// What: Adjust Artix visibility for collapsed menu scenario.
///
/// Inputs:
/// - `show_artix_specific_repos`: Current visibility state
/// - `ctx`: Context containing calculation parameters
/// - `optional_repos`: Optional repository flags
/// - `optional_labels`: Labels for optional repos
///
/// Output: Tuple of (`show_artix_specific_repos`, `final_consumed_left`).
///
/// Details: Adjusts Artix visibility when collapsed menu might be used.
fn adjust_artix_visibility_for_collapsed_menu(
    show_artix_specific_repos: bool,
    ctx: &ArtixVisibilityContext,
    optional_repos: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
) -> (bool, u16) {
    if !show_artix_specific_repos {
        return (false, ctx.final_consumed_left);
    }

    let space_with_filters = ctx
        .consumed_left
        .saturating_add(ctx.menu_w)
        .saturating_add(1);
    let repos_without_specific = create_repos_without_specific(optional_repos);
    let consumed_without_specific = calculate_consumed_without_specific(
        ctx.base_consumed,
        &repos_without_specific,
        optional_labels,
        optional_repos.has_artix,
    );
    let space_without_filters = consumed_without_specific
        .saturating_add(ctx.menu_w)
        .saturating_add(1);

    if ctx.inner_width < space_with_filters && ctx.inner_width >= space_without_filters {
        (false, consumed_without_specific)
    } else {
        (show_artix_specific_repos, ctx.final_consumed_left)
    }
}

/// What: Finalize Artix visibility when menu can't fit.
///
/// Inputs:
/// - `show_artix_specific_repos`: Current visibility state
/// - `consumed_left`: Consumed space with all filters
/// - `inner_width`: Available width
/// - `menu_w`: Menu button width
/// - `base_consumed`: Base consumed space
/// - `optional_repos`: Optional repository flags
/// - `optional_labels`: Labels for optional repos
///
/// Output: Tuple of (`show_artix_specific_repos`, `final_consumed_left`).
///
/// Details: Hides Artix-specific repos if there's not enough space even without menu.
fn finalize_artix_visibility_when_menu_cant_fit(
    show_artix_specific_repos: bool,
    consumed_left: u16,
    inner_width: u16,
    menu_w: u16,
    base_consumed: u16,
    optional_repos: &OptionalRepos,
    optional_labels: &OptionalReposLabels,
) -> (bool, u16) {
    if !show_artix_specific_repos {
        return (false, consumed_left);
    }

    let space_needed_with_filters = consumed_left.saturating_add(menu_w).saturating_add(1);
    if inner_width >= space_needed_with_filters {
        return (true, consumed_left);
    }

    // Not enough space, hide Artix-specific repos
    let repos_without_specific = create_repos_without_specific(optional_repos);
    let consumed_without_specific = calculate_consumed_without_specific(
        base_consumed,
        &repos_without_specific,
        optional_labels,
        optional_repos.has_artix,
    );
    (false, consumed_without_specific)
}

/// What: Calculate shared layout information for title bar.
///
/// Inputs:
/// - `i18n`: Pre-computed i18n strings
/// - `results_len`: Number of results
/// - `inner_width`: Inner width of the area (excluding borders)
/// - `optional_repos`: Optional repository availability flags
///
/// Output: `TitleLayoutInfo` containing all calculated layout values.
///
/// Details: Performs all layout calculations shared between rendering and rect recording.
/// Uses helper functions to reduce data flow complexity.
pub(super) fn calculate_title_layout_info(
    i18n: &TitleI18nStrings,
    results_len: usize,
    inner_width: u16,
    optional_repos: &OptionalRepos,
) -> TitleLayoutInfo {
    let results_title_text = format!("{} ({})", i18n.results_title, results_len);
    let sort_button_label = format!("{} v", i18n.sort_button);
    let options_button_label = format!("{} v", i18n.options_button);
    let panels_button_label = format!("{} v", i18n.panels_button);
    let config_button_label = format!("{} v", i18n.config_button);
    let menu_button_label = format!("{} v", i18n.menu_button);

    let core_labels = CoreFilterLabels {
        aur: format!("[{}]", i18n.filter_aur),
        core: format!("[{}]", i18n.filter_core),
        extra: format!("[{}]", i18n.filter_extra),
        multilib: format!("[{}]", i18n.filter_multilib),
    };
    let optional_labels = OptionalReposLabels {
        eos: format!("[{}]", i18n.filter_eos),
        cachyos: format!("[{}]", i18n.filter_cachyos),
        artix: format!("[{}]", i18n.filter_artix),
        artix_omniverse: format!("[{}]", i18n.filter_artix_omniverse),
        artix_universe: format!("[{}]", i18n.filter_artix_universe),
        artix_lib32: format!("[{}]", i18n.filter_artix_lib32),
        artix_galaxy: format!("[{}]", i18n.filter_artix_galaxy),
        artix_world: format!("[{}]", i18n.filter_artix_world),
        artix_system: format!("[{}]", i18n.filter_artix_system),
        manjaro: format!("[{}]", i18n.filter_manjaro),
    };

    // Calculate consumed space with all filters first
    let base_consumed =
        calculate_base_consumed_space(&results_title_text, &sort_button_label, &core_labels);
    let optional_consumed = calculate_optional_repos_width(optional_repos, &optional_labels);
    let consumed_left = base_consumed.saturating_add(optional_consumed);

    // Use Unicode display width, not byte length, to handle wide characters
    let options_w = u16::try_from(options_button_label.width()).unwrap_or(u16::MAX);
    let panels_w = u16::try_from(panels_button_label.width()).unwrap_or(u16::MAX);
    let config_w = u16::try_from(config_button_label.width()).unwrap_or(u16::MAX);
    let menu_w = u16::try_from(menu_button_label.width()).unwrap_or(u16::MAX);
    let right_w = config_w
        .saturating_add(1)
        .saturating_add(panels_w)
        .saturating_add(1)
        .saturating_add(options_w);

    // Determine initial Artix visibility and consumed space
    let (mut show_artix_specific_repos, mut final_consumed_left, pad) =
        determine_initial_artix_visibility(
            base_consumed,
            consumed_left,
            inner_width,
            right_w,
            optional_repos,
            &optional_labels,
        );

    // Adjust Artix visibility for collapsed menu scenario
    if pad < 1 && show_artix_specific_repos {
        let ctx = ArtixVisibilityContext {
            consumed_left,
            final_consumed_left,
            inner_width,
            menu_w,
            base_consumed,
        };
        let (new_show, new_consumed) = adjust_artix_visibility_for_collapsed_menu(
            show_artix_specific_repos,
            &ctx,
            optional_repos,
            &optional_labels,
        );
        show_artix_specific_repos = new_show;
        final_consumed_left = new_consumed;
    }

    // Determine if we should use collapsed menu instead of individual buttons
    // Decision logic:
    // - pad is the remaining space after accounting for final_consumed_left + right_w
    // - If pad >= 1: we have space for all three buttons (use_collapsed_menu = false)
    // - If pad < 1: check if we have space for collapsed menu
    //   Calculate space needed for collapsed menu: final_consumed_left + menu_w
    //   If inner_width >= (final_consumed_left + menu_w + 1): use collapsed menu
    //   Otherwise: show nothing
    let use_collapsed_menu = if pad < 1 {
        let space_needed_for_menu = final_consumed_left.saturating_add(menu_w).saturating_add(1);
        inner_width >= space_needed_for_menu
    } else {
        false
    };

    // If collapsed menu can't fit, ensure Artix filters stay hidden when space is very tight
    // This prevents filters from expanding when the menu dropdown vanishes
    if !use_collapsed_menu && pad < 1 {
        let (new_show, new_consumed) = finalize_artix_visibility_when_menu_cant_fit(
            show_artix_specific_repos,
            consumed_left,
            inner_width,
            menu_w,
            base_consumed,
            optional_repos,
            &optional_labels,
        );
        show_artix_specific_repos = new_show;
        final_consumed_left = new_consumed;
    }

    // Calculate padding for collapsed menu (space after accounting for consumed_left + menu_w)
    let menu_pad = if use_collapsed_menu {
        inner_width.saturating_sub(final_consumed_left.saturating_add(menu_w))
    } else {
        pad
    };

    TitleLayoutInfo {
        results_title_text,
        sort_button_label,
        options_button_label,
        panels_button_label,
        config_button_label,
        menu_button_label,
        core_labels,
        optional_labels,
        inner_width,
        show_artix_specific_repos,
        pad,
        use_collapsed_menu,
        menu_pad,
    }
}
