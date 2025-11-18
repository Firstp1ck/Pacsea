//! Tests for Preflight modal event handling.

#[cfg(test)]
use super::display::{
    build_file_display_items, compute_display_items_len, compute_file_display_items_len,
};
#[cfg(test)]
use super::keys::handle_preflight_key;
#[cfg(test)]
use crate::state::modal::{
    CascadeMode, DependencyInfo, DependencySource, DependencyStatus, FileChange, FileChangeType,
    PackageFileInfo, ServiceImpact, ServiceRestartDecision,
};
#[cfg(test)]
use crate::state::{AppState, Modal, PackageItem, PreflightAction, PreflightTab, Source};
#[cfg(test)]
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
#[cfg(test)]
use std::collections::HashSet;

/// What: Construct a minimal `PackageItem` fixture used by preflight tests.
///
/// Inputs:
/// - `name`: Package identifier to embed in the resulting fixture.
///
/// Output:
/// - `PackageItem` populated with deterministic metadata for assertions.
///
/// Details:
/// - Provides consistent version/description/source values so each test can focus on modal behaviour.
fn pkg(name: &str) -> PackageItem {
    PackageItem {
        name: name.into(),
        version: "1.0.0".into(),
        description: "pkg".into(),
        source: Source::Official {
            repo: "extra".into(),
            arch: "x86_64".into(),
        },
        popularity: None,
    }
}

/// What: Build a `DependencyInfo` fixture describing a package edge for dependency tests.
///
/// Inputs:
/// - `name`: Dependency package name to populate the struct.
/// - `required_by`: Slice of package names that declare the dependency.
///
/// Output:
/// - `DependencyInfo` instance configured for deterministic assertions.
///
/// Details:
/// - Sets predictable version/status/source fields so tests can concentrate on tree expansion logic.
fn dep(name: &str, required_by: &[&str]) -> DependencyInfo {
    DependencyInfo {
        name: name.into(),
        version: ">=1".into(),
        status: DependencyStatus::ToInstall,
        source: DependencySource::Official {
            repo: "extra".into(),
        },
        required_by: required_by.iter().map(|s| (*s).into()).collect(),
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }
}

/// What: Build a `DependencyInfo` fixture with a specific status for testing status display.
///
/// Inputs:
/// - `name`: Dependency package name to populate the struct.
/// - `required_by`: Slice of package names that declare the dependency.
/// - `status`: Specific dependency status to test.
///
/// Output:
/// - `DependencyInfo` instance with the specified status for assertions.
///
/// Details:
/// - Allows testing different dependency statuses (Installed, ToInstall, etc.) to verify correct display.
fn dep_with_status(name: &str, required_by: &[&str], status: DependencyStatus) -> DependencyInfo {
    DependencyInfo {
        name: name.into(),
        version: ">=1".into(),
        status,
        source: DependencySource::Official {
            repo: "extra".into(),
        },
        required_by: required_by.iter().map(|s| (*s).into()).collect(),
        depends_on: Vec::new(),
        is_core: false,
        is_system: false,
    }
}

/// What: Create a `PackageFileInfo` fixture with a configurable number of synthetic files.
///
/// Inputs:
/// - `name`: Package identifier associated with the file changes.
/// - `file_count`: Number of file entries to generate.
///
/// Output:
/// - `PackageFileInfo` containing `file_count` new file records under `/tmp`.
///
/// Details:
/// - Each generated file is marked as a new change, allowing tests to validate expansion counts easily.
fn file_info(name: &str, file_count: usize) -> PackageFileInfo {
    let mut files = Vec::new();
    for idx in 0..file_count {
        files.push(FileChange {
            path: format!("/tmp/{name}_{idx}"),
            change_type: FileChangeType::New,
            package: name.into(),
            is_config: false,
            predicted_pacnew: false,
            predicted_pacsave: false,
        });
    }
    PackageFileInfo {
        name: name.into(),
        files,
        total_count: file_count,
        new_count: file_count,
        changed_count: 0,
        removed_count: 0,
        config_count: 0,
        pacnew_candidates: 0,
        pacsave_candidates: 0,
    }
}

/// What: Construct a `ServiceImpact` fixture representing a single unit for Services tab tests.
///
/// Inputs:
/// - `unit`: Fully-qualified systemd unit identifier to populate the struct.
/// - `decision`: Initial restart preference that the test expects to mutate.
///
/// Output:
/// - `ServiceImpact` configured with deterministic metadata for assertions.
///
/// Details:
/// - Marks the unit as active and needing restart so event handlers may flip the decision.
fn svc(unit: &str, decision: ServiceRestartDecision) -> ServiceImpact {
    ServiceImpact {
        unit_name: unit.into(),
        providers: vec!["target".into()],
        is_active: true,
        needs_restart: true,
        recommended_decision: ServiceRestartDecision::Restart,
        restart_decision: decision,
    }
}

#[test]
/// What: Ensure dependency display length counts unique entries when groups are expanded.
///
/// Inputs:
/// - Dependency list with duplicates and an expanded set containing the first package.
///
/// Output:
/// - Computed length includes headers plus unique dependencies, yielding four rows.
///
/// Details:
/// - Demonstrates deduplication of repeated dependency records across packages.
fn deps_display_len_counts_unique_expanded_dependencies() {
    let items = vec![pkg("app"), pkg("tool")];
    let deps = vec![
        dep("libfoo", &["app"]),
        dep("libbar", &["app", "tool"]),
        dep("libbar", &["app"]),
    ];
    let mut expanded = HashSet::new();
    expanded.insert("app".to_string());
    let len = compute_display_items_len(&items, &deps, &expanded);
    assert_eq!(len, 4);
}

#[test]
/// What: Verify the collapsed dependency view only counts package headers.
///
/// Inputs:
/// - Dependency list with multiple packages but an empty expanded set.
///
/// Output:
/// - Display length equals the number of packages (two).
///
/// Details:
/// - Confirms collapsed state omits dependency children entirely.
fn deps_display_len_collapsed_counts_only_headers() {
    let items = vec![pkg("app"), pkg("tool")];
    let deps = vec![dep("libfoo", &["app"]), dep("libbar", &["tool"])];
    let expanded = HashSet::new();
    let len = compute_display_items_len(&items, &deps, &expanded);
    assert_eq!(len, 2);
}

#[test]
/// What: Confirm file display counts add child rows only for expanded entries.
///
/// Inputs:
/// - File info for a package with two files and another with zero files.
///
/// Output:
/// - Collapsed count returns two (both packages shown); expanded count increases to four (2 headers + 2 files).
///
/// Details:
/// - Exercises the branch that toggles between header-only and expanded file listings.
fn file_display_len_respects_expansion_state() {
    let items = vec![pkg("pkg"), pkg("empty")];
    let info = vec![file_info("pkg", 2), file_info("empty", 0)];
    let mut expanded = HashSet::new();
    let collapsed = compute_file_display_items_len(&items, &info, &expanded);
    assert_eq!(collapsed, 2); // Both packages shown
    expanded.insert("pkg".to_string());
    let expanded_len = compute_file_display_items_len(&items, &info, &expanded);
    assert_eq!(expanded_len, 4); // 2 headers + 2 files from pkg
}

#[test]
/// What: Ensure file display item builder yields headers plus placeholder rows when expanded.
///
/// Inputs:
/// - File info with two entries for a single package and varying expansion sets.
///
/// Output:
/// - Collapsed result contains only the header; expanded result includes header plus two child slots.
///
/// Details:
/// - Helps guarantee alignment between item construction and length calculations.
fn build_file_items_match_expansion() {
    let items = vec![pkg("pkg")];
    let info = vec![file_info("pkg", 2)];
    let collapsed = build_file_display_items(&items, &info, &HashSet::new());
    assert_eq!(collapsed, vec![(true, "pkg".into())]);
    let mut expanded = HashSet::new();
    expanded.insert("pkg".to_string());
    let expanded_items = build_file_display_items(&items, &info, &expanded);
    assert_eq!(
        expanded_items,
        vec![
            (true, "pkg".into()),
            (false, String::new()),
            (false, String::new())
        ]
    );
}

/// What: Prepare an `AppState` with a seeded Preflight modal tailored for keyboard interaction tests.
///
/// Inputs:
/// - `tab`: Initial tab to display inside the Preflight modal.
/// - `dependency_info`: Pre-resolved dependency list to seed the modal state.
/// - `dep_selected`: Initial selection index for the dependency list.
/// - `dep_tree_expanded`: Set of package names that should start expanded.
///
/// Output:
/// - `AppState` instance whose `modal` field is pre-populated with consistent fixtures.
///
/// Details:
/// - Reduces duplication across tests that exercise navigation/expansion logic within the Preflight modal.
fn setup_preflight_app(
    tab: PreflightTab,
    dependency_info: Vec<DependencyInfo>,
    dep_selected: usize,
    dep_tree_expanded: HashSet<String>,
) -> AppState {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    app.modal = Modal::Preflight {
        items,
        action: PreflightAction::Install,
        tab,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info,
        dep_selected,
        dep_tree_expanded,
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };
    app
}

/// What: Build an `AppState` seeded with Services tab data for restart decision tests.
///
/// Inputs:
/// - `tab`: Initial tab to display inside the Preflight modal (expected to be Services).
/// - `service_info`: Collection of service impacts to expose through the modal.
/// - `service_selected`: Index that should be focused when the test begins.
///
/// Output:
/// - `AppState` populated with deterministic fixtures for Services tab interactions.
///
/// Details:
/// - Marks services as already loaded so handlers operate directly on the provided data.
fn setup_preflight_app_with_services(
    tab: PreflightTab,
    service_info: Vec<ServiceImpact>,
    service_selected: usize,
) -> AppState {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    app.modal = Modal::Preflight {
        items,
        action: PreflightAction::Install,
        tab,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info,
        service_selected,
        services_loaded: true,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };
    app
}

#[test]
/// What: Verify `Enter` toggles dependency expansion state within the preflight modal.
///
/// Inputs:
/// - Preflight modal focused on dependencies with an initial collapsed state.
///
/// Output:
/// - First `Enter` expands the target group; second `Enter` collapses it.
///
/// Details:
/// - Uses synthetic key events to mimic user interaction without rendering.
fn handle_enter_toggles_dependency_group() {
    let deps = vec![dep("libfoo", &["target"])];
    let mut app = setup_preflight_app(PreflightTab::Deps, deps, 0, HashSet::new());
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    handle_preflight_key(enter, &mut app);
    if let Modal::Preflight {
        dep_tree_expanded, ..
    } = &app.modal
    {
        assert!(dep_tree_expanded.contains("target"));
    } else {
        panic!("expected Preflight modal");
    }
    let enter_again = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
    handle_preflight_key(enter_again, &mut app);
    if let Modal::Preflight {
        dep_tree_expanded, ..
    } = &app.modal
    {
        assert!(!dep_tree_expanded.contains("target"));
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Ensure navigation does not move past the last visible dependency row when expanded.
///
/// Inputs:
/// - Preflight modal with expanded dependencies and repeated `Down` key events.
///
/// Output:
/// - Selection stops at the final row instead of wrapping or overshooting.
///
/// Details:
/// - Exercises selection bounds checking for keyboard navigation.
fn handle_down_stops_at_last_visible_dependency_row() {
    let deps = vec![dep("libfoo", &["target"]), dep("libbar", &["target"])];
    let mut expanded = HashSet::new();
    expanded.insert("target".to_string());
    let mut app = setup_preflight_app(PreflightTab::Deps, deps, 0, expanded);
    handle_preflight_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        &mut app,
    );
    handle_preflight_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        &mut app,
    );
    handle_preflight_key(
        KeyEvent::new(KeyCode::Down, KeyModifiers::empty()),
        &mut app,
    );
    if let Modal::Preflight { dep_selected, .. } = &app.modal {
        assert_eq!(*dep_selected, 2);
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Confirm spacebar toggles the service restart decision within the Services tab.
///
/// Inputs:
/// - Preflight modal focused on Services with a single restart-ready unit selected.
///
/// Output:
/// - First space press defers the restart; second space press restores the restart decision.
///
/// Details:
/// - Exercises the branch that flips `ServiceRestartDecision` without mutating selection.
fn handle_space_toggles_service_restart_decision() {
    let services = vec![svc("nginx.service", ServiceRestartDecision::Restart)];
    let mut app = setup_preflight_app_with_services(PreflightTab::Services, services, 0);
    handle_preflight_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
        &mut app,
    );
    if let Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info[0].restart_decision,
            ServiceRestartDecision::Defer
        );
    } else {
        panic!("expected Preflight modal");
    }
    handle_preflight_key(
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()),
        &mut app,
    );
    if let Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info[0].restart_decision,
            ServiceRestartDecision::Restart
        );
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Ensure dedicated shortcuts force service restart decisions regardless of current state.
///
/// Inputs:
/// - Services tab with one unit initially set to defer, then the `r` and `Shift+D` bindings.
///
/// Output:
/// - `r` enforces restart, `Shift+D` enforces defer on the focused service.
///
/// Details:
/// - Verifies that direct commands override any prior toggled state for the selected row.
fn handle_service_restart_shortcuts_force_decisions() {
    let services = vec![svc("postgresql.service", ServiceRestartDecision::Defer)];
    let mut app = setup_preflight_app_with_services(PreflightTab::Services, services, 0);
    handle_preflight_key(
        KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()),
        &mut app,
    );
    if let Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info[0].restart_decision,
            ServiceRestartDecision::Restart
        );
    } else {
        panic!("expected Preflight modal");
    }
    handle_preflight_key(
        KeyEvent::new(KeyCode::Char('D'), KeyModifiers::SHIFT),
        &mut app,
    );
    if let Modal::Preflight { service_info, .. } = &app.modal {
        assert_eq!(
            service_info[0].restart_decision,
            ServiceRestartDecision::Defer
        );
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Verify that installed dependencies are correctly included in the dependency list.
///
/// Inputs:
/// - Preflight modal with dependencies including installed ones.
///
/// Output:
/// - All dependencies, including installed ones, are present in dependency_info.
///
/// Details:
/// - Tests that installed dependencies are not filtered out and should display with checkmarks.
fn installed_dependencies_are_included_in_list() {
    let installed_dep = dep_with_status(
        "libinstalled",
        &["target"],
        DependencyStatus::Installed {
            version: "1.2.3".into(),
        },
    );
    let to_install_dep = dep("libnew", &["target"]);
    let deps = vec![installed_dep.clone(), to_install_dep.clone()];
    let app = setup_preflight_app(PreflightTab::Deps, deps.clone(), 0, HashSet::new());

    if let Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        assert_eq!(dependency_info.len(), 2);
        // Verify installed dependency is present
        assert!(
            dependency_info.iter().any(|d| d.name == "libinstalled"
                && matches!(d.status, DependencyStatus::Installed { .. }))
        );
        // Verify to-install dependency is present
        assert!(
            dependency_info
                .iter()
                .any(|d| d.name == "libnew" && matches!(d.status, DependencyStatus::ToInstall))
        );
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Verify that cached dependencies are correctly loaded when switching to Deps tab.
///
/// Inputs:
/// - AppState with cached dependencies in install_list_deps.
/// - Preflight modal initially on Summary tab, then switching to Deps tab.
///
/// Output:
/// - Cached dependencies are loaded into dependency_info when switching to Deps tab.
///
/// Details:
/// - Tests the tab switching logic that loads cached dependencies from app state.
fn cached_dependencies_load_on_tab_switch() {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    let cached_deps = vec![dep("libcached", &["target"])];
    app.install_list_deps = cached_deps.clone();
    app.modal = Modal::Preflight {
        items: items.clone(),
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };

    // Switch to Deps tab
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );

    if let Modal::Preflight {
        tab,
        dependency_info,
        ..
    } = &app.modal
    {
        assert_eq!(*tab, PreflightTab::Deps);
        assert_eq!(dependency_info.len(), 1);
        assert_eq!(dependency_info[0].name, "libcached");
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Verify that cached files are correctly loaded when switching to Files tab.
///
/// Inputs:
/// - AppState with cached files in install_list_files.
/// - Preflight modal initially on Summary tab, then switching to Files tab.
///
/// Output:
/// - Cached files are loaded into file_info when switching to Files tab.
///
/// Details:
/// - Tests the tab switching logic that loads cached files from app state.
fn cached_files_load_on_tab_switch() {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    let cached_files = vec![file_info("target", 3)];
    app.install_list_files = cached_files.clone();
    app.modal = Modal::Preflight {
        items: items.clone(),
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };

    // Switch to Files tab (Right twice: Summary -> Deps -> Files)
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );

    if let Modal::Preflight { tab, file_info, .. } = &app.modal {
        assert_eq!(*tab, PreflightTab::Files);
        assert_eq!(file_info.len(), 1);
        assert_eq!(file_info[0].name, "target");
        assert_eq!(file_info[0].files.len(), 3);
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Verify that dependencies with Installed status are counted correctly in display length.
///
/// Inputs:
/// - Dependency list containing both installed and to-install dependencies.
///
/// Output:
/// - Display length includes all dependencies regardless of status.
///
/// Details:
/// - Ensures installed dependencies are not excluded from the display count.
fn installed_dependencies_counted_in_display_length() {
    let items = vec![pkg("app")];
    let deps = vec![
        dep_with_status(
            "libinstalled",
            &["app"],
            DependencyStatus::Installed {
                version: "1.0.0".into(),
            },
        ),
        dep("libnew", &["app"]),
    ];
    let mut expanded = HashSet::new();
    expanded.insert("app".to_string());
    let len = compute_display_items_len(&items, &deps, &expanded);
    // Should count: 1 header + 2 dependencies = 3
    assert_eq!(len, 3);
}

#[test]
/// What: Verify that files are correctly displayed when file_info is populated.
///
/// Inputs:
/// - Preflight modal with file_info containing files for a package.
///
/// Output:
/// - File display length correctly counts files when package is expanded.
///
/// Details:
/// - Ensures files are not filtered out and are correctly counted for display.
fn files_displayed_when_file_info_populated() {
    let items = vec![pkg("pkg1"), pkg("pkg2")];
    let file_infos = vec![file_info("pkg1", 5), file_info("pkg2", 3)];
    let mut expanded = HashSet::new();
    expanded.insert("pkg1".to_string());
    let len = compute_file_display_items_len(&items, &file_infos, &expanded);
    // Should count: 2 headers + 5 files from pkg1 = 7
    assert_eq!(len, 7);

    // Expand both packages
    expanded.insert("pkg2".to_string());
    let len_expanded = compute_file_display_items_len(&items, &file_infos, &expanded);
    // Should count: 2 headers + 5 files + 3 files = 10
    assert_eq!(len_expanded, 10);
}

#[test]
/// What: Verify that empty file_info shows correct empty state.
///
/// Inputs:
/// - Preflight modal with empty file_info but packages in items.
///
/// Output:
/// - File display length returns 2 (all packages shown even if no file info).
///
/// Details:
/// - Ensures empty states are handled correctly without panicking.
fn empty_file_info_handled_correctly() {
    let items = vec![pkg("pkg1"), pkg("pkg2")];
    let file_infos = Vec::<PackageFileInfo>::new();
    let expanded = HashSet::new();
    let len = compute_file_display_items_len(&items, &file_infos, &expanded);
    // Should count: 2 headers (all packages shown even if no file info)
    assert_eq!(len, 2);
}

#[test]
/// What: Verify that dependencies are correctly filtered by required_by when loading from cache.
///
/// Inputs:
/// - AppState with cached dependencies, some matching current items and some not.
/// - Preflight modal switching to Deps tab.
///
/// Output:
/// - Only dependencies required by current items are loaded.
///
/// Details:
/// - Tests the filtering logic that ensures only relevant dependencies are shown.
fn dependencies_filtered_by_required_by_on_cache_load() {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    // Create dependencies: one for "target", one for "other" package
    let deps_for_target = dep("libtarget", &["target"]);
    let deps_for_other = dep("libother", &["other"]);
    app.install_list_deps = vec![deps_for_target.clone(), deps_for_other.clone()];
    app.modal = Modal::Preflight {
        items: items.clone(),
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };

    // Switch to Deps tab
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );

    if let Modal::Preflight {
        dependency_info, ..
    } = &app.modal
    {
        // Should only load dependency for "target", not "other"
        assert_eq!(dependency_info.len(), 1);
        assert_eq!(dependency_info[0].name, "libtarget");
        assert!(
            dependency_info[0]
                .required_by
                .iter()
                .any(|req| req == "target")
        );
    } else {
        panic!("expected Preflight modal");
    }
}

#[test]
/// What: Verify that files are correctly filtered by package name when loading from cache.
///
/// Inputs:
/// - AppState with cached files for multiple packages.
/// - Preflight modal with only some packages in items.
///
/// Output:
/// - Only files for packages in items are loaded.
///
/// Details:
/// - Tests the filtering logic that ensures only relevant files are shown.
fn files_filtered_by_package_name_on_cache_load() {
    let mut app = AppState::default();
    let items = vec![pkg("target")];
    let files_for_target = file_info("target", 2);
    let files_for_other = file_info("other", 3);
    app.install_list_files = vec![files_for_target.clone(), files_for_other.clone()];
    app.modal = Modal::Preflight {
        items: items.clone(),
        action: PreflightAction::Install,
        tab: PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: crate::state::modal::PreflightHeaderChips::default(),
        dependency_info: Vec::new(),
        dep_selected: 0,
        dep_tree_expanded: HashSet::new(),
        deps_error: None,
        file_info: Vec::new(),
        file_selected: 0,
        file_tree_expanded: HashSet::new(),
        files_error: None,
        service_info: Vec::new(),
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: Vec::new(),
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: CascadeMode::Basic,
    };

    // Switch to Files tab
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );
    handle_preflight_key(
        KeyEvent::new(KeyCode::Right, KeyModifiers::empty()),
        &mut app,
    );

    if let Modal::Preflight { file_info, .. } = &app.modal {
        // Should only load files for "target", not "other"
        assert_eq!(file_info.len(), 1);
        assert_eq!(file_info[0].name, "target");
        assert_eq!(file_info[0].files.len(), 2);
    } else {
        panic!("expected Preflight modal");
    }
}
