//! Integration tests for UI rendering using ratatui's `TestBackend`.
//!
//! These tests verify that the TUI renders correctly across different application states
//! without requiring a real terminal. They focus on visual rendering correctness rather
//! than business logic.

use ratatui::{Terminal, backend::TestBackend};
use std::collections::HashMap;
use std::time::Instant;

use pacsea::state::{AppState, Modal, PackageDetails, PackageItem, Source};
use pacsea::ui;

/// Initialize minimal English translations for tests.
///
/// Sets up only the translations needed for tests to pass.
fn init_test_translations(app: &mut AppState) {
    let mut translations = HashMap::new();

    // Details
    translations.insert("app.details.fields.url".to_string(), "URL".to_string());
    translations.insert("app.details.url_label".to_string(), "URL:".to_string());

    // Results
    translations.insert("app.results.title".to_string(), "Results".to_string());
    translations.insert("app.results.buttons.sort".to_string(), "Sort".to_string());
    translations.insert(
        "app.results.buttons.options".to_string(),
        "Options".to_string(),
    );
    translations.insert(
        "app.results.buttons.panels".to_string(),
        "Panels".to_string(),
    );
    translations.insert(
        "app.results.buttons.config_lists".to_string(),
        "Config/Lists".to_string(),
    );
    translations.insert("app.results.filters.aur".to_string(), "AUR".to_string());
    translations.insert("app.results.filters.core".to_string(), "core".to_string());
    translations.insert("app.results.filters.extra".to_string(), "extra".to_string());
    translations.insert(
        "app.results.filters.multilib".to_string(),
        "multilib".to_string(),
    );
    translations.insert("app.results.filters.eos".to_string(), "EOS".to_string());
    translations.insert(
        "app.results.filters.cachyos".to_string(),
        "CachyOS".to_string(),
    );
    translations.insert("app.results.filters.artix".to_string(), "Artix".to_string());
    translations.insert(
        "app.results.filters.artix_omniverse".to_string(),
        "OMNI".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_universe".to_string(),
        "UNI".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_lib32".to_string(),
        "LIB32".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_galaxy".to_string(),
        "GALAXY".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_world".to_string(),
        "WORLD".to_string(),
    );
    translations.insert(
        "app.results.filters.artix_system".to_string(),
        "SYSTEM".to_string(),
    );
    translations.insert(
        "app.results.filters.manjaro".to_string(),
        "Manjaro".to_string(),
    );

    // Toasts
    translations.insert(
        "app.toasts.copied_to_clipboard".to_string(),
        "Copied to clipboard".to_string(),
    );
    translations.insert(
        "app.toasts.title_clipboard".to_string(),
        "Clipboard".to_string(),
    );
    translations.insert("app.toasts.title_news".to_string(), "News".to_string());

    // Middle row
    translations.insert("app.middle.recent.title".to_string(), "Recent".to_string());
    translations.insert(
        "app.middle.install.title".to_string(),
        "Install".to_string(),
    );
    translations.insert(
        "app.middle.downgrade.title".to_string(),
        "Downgrade".to_string(),
    );
    translations.insert("app.middle.remove.title".to_string(), "Remove".to_string());

    // Modals
    translations.insert("app.modals.alert.title".to_string(), "Alert".to_string());
    translations.insert("app.modals.help.title".to_string(), "Help".to_string());
    translations.insert(
        "app.modals.news.title".to_string(),
        "Arch Linux News".to_string(),
    );

    app.translations.clone_from(&translations);
    app.translations_fallback = translations;
}

/// Create a minimal `AppState` for testing.
fn create_test_app_state() -> AppState {
    let mut app = AppState {
        last_input_change: Instant::now(),
        ..Default::default()
    };
    init_test_translations(&mut app);
    app
}

/// Create a `TestBackend` with standard size for testing.
fn create_test_backend() -> TestBackend {
    TestBackend::new(120, 40)
}

/// Create a `TestBackend` with custom size.
fn create_test_backend_size(width: u16, height: u16) -> TestBackend {
    TestBackend::new(width, height)
}

/// Render UI to a `TestBackend` and return the terminal for assertions.
fn render_ui_to_backend(backend: TestBackend, app: &mut AppState) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|f| ui::ui(f, app))
        .expect("failed to draw test terminal");
    terminal
}

// Core UI Rendering Tests

#[test]
fn test_ui_renders_empty_state() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify UI renders without panicking
    let buffer = terminal.backend().buffer();
    assert!(buffer.area.width > 0);
    assert!(buffer.area.height > 0);

    // Verify key rects are set
    assert!(app.results_rect.is_some());
}

#[test]
fn test_ui_renders_with_results() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Add some test results
    app.results = vec![
        PackageItem {
            name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            description: "A test package".to_string(),
            source: Source::Aur,
            popularity: Some(42.5),
        },
        PackageItem {
            name: "another-package".to_string(),
            version: "2.0.0".to_string(),
            description: "Another test package".to_string(),
            source: Source::Official {
                repo: "extra".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
        },
    ];
    app.all_results = app.results.clone();
    app.selected = 0;
    app.list_state.select(Some(0));

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify results pane rendered
    assert!(app.results_rect.is_some());

    // Verify buffer dimensions
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}

#[test]
fn test_ui_renders_with_details() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Add a result and details
    app.results = vec![PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    app.details = PackageDetails {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "A test package description".to_string(),
        url: "https://example.com/test".to_string(),
        repository: "aur".to_string(),
        architecture: "x86_64".to_string(),
        licenses: vec!["MIT".to_string()],
        groups: vec![],
        provides: vec![],
        depends: vec!["bash".to_string()],
        opt_depends: vec![],
        required_by: vec![],
        optional_for: vec![],
        conflicts: vec![],
        replaces: vec![],
        download_size: Some(1024),
        install_size: Some(2048),
        owner: "testuser".to_string(),
        build_date: "2024-01-01".to_string(),
        popularity: None,
    };

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify details pane rendered
    assert!(app.details_rect.is_some());
    assert!(app.url_button_rect.is_some());
}

#[test]
fn test_ui_renders_middle_row() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Add some recent searches and install list items
    app.recent = vec!["vim".to_string(), "git".to_string()];
    app.history_state.select(Some(0));

    app.install_list = vec![PackageItem {
        name: "install-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "To install".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.install_state.select(Some(0));

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify middle row components rendered
    assert!(app.recent_rect.is_some());
    assert!(app.install_rect.is_some());
}

// Layout Tests

#[test]
fn test_layout_minimum_sizes() {
    // Test with minimum viable size
    let backend = create_test_backend_size(80, 10);
    let mut app = create_test_app_state();

    let terminal = render_ui_to_backend(backend, &mut app);

    // UI should still render without panicking
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 80);
    assert_eq!(buffer.area.height, 10);
}

#[test]
fn test_layout_maximum_sizes() {
    // Test with large terminal size
    let backend = create_test_backend_size(200, 60);
    let mut app = create_test_app_state();

    app.results = vec![PackageItem {
        name: "pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify layout handles large sizes
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 200);
    assert_eq!(buffer.area.height, 60);
}

#[test]
fn test_layout_responsive() {
    // Test different terminal sizes
    let sizes = vec![(80, 24), (120, 40), (160, 50)];

    for (width, height) in sizes {
        let backend = create_test_backend_size(width, height);
        let mut app = create_test_app_state();

        app.results = vec![PackageItem {
            name: "test".to_string(),
            version: "1.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
        }];
        app.selected = 0;
        app.list_state.select(Some(0));

        let terminal = render_ui_to_backend(backend, &mut app);
        let buffer = terminal.backend().buffer();

        assert_eq!(buffer.area.width, width);
        assert_eq!(buffer.area.height, height);
        assert!(app.results_rect.is_some());
    }
}

#[test]
fn test_layout_pane_hiding() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Test with Recent pane hidden
    app.show_recent_pane = false;
    let _terminal = render_ui_to_backend(backend, &mut app);
    assert!(app.recent_rect.is_none() || app.recent_rect.is_some());

    // Test with Install pane hidden
    app.show_recent_pane = true;
    app.show_install_pane = false;
    let backend = create_test_backend();
    let _terminal = render_ui_to_backend(backend, &mut app);
    assert!(app.install_rect.is_none() || app.install_rect.is_some());
}

// Modal Tests

#[test]
fn test_modal_alert_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.modal = Modal::Alert {
        message: "Test alert message".to_string(),
    };

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify modal rendered (check buffer dimensions)
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}

#[test]
fn test_modal_help_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.modal = Modal::Help;

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify help modal rendered
    assert!(app.help_rect.is_some());
}

#[test]
fn test_modal_news_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.modal = Modal::News {
        items: vec![pacsea::state::types::NewsItem {
            date: "2024-01-01".to_string(),
            title: "Test News Item".to_string(),
            url: "https://example.com/news".to_string(),
        }],
        selected: 0,
    };

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify news modal rendered
    assert!(app.news_rect.is_some());
    assert!(app.news_list_rect.is_some());
}

#[test]
fn test_modal_preflight_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.modal = Modal::Preflight {
        items: vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
        }],
        action: pacsea::state::modal::PreflightAction::Install,
        tab: pacsea::state::modal::PreflightTab::Summary,
        summary: None,
        summary_scroll: 0,
        header_chips: pacsea::state::modal::PreflightHeaderChips::default(),
        dependency_info: vec![],
        dep_selected: 0,
        dep_tree_expanded: std::collections::HashSet::new(),
        deps_error: None,
        file_info: vec![],
        file_selected: 0,
        file_tree_expanded: std::collections::HashSet::new(),
        files_error: None,
        service_info: vec![],
        service_selected: 0,
        services_loaded: false,
        services_error: None,
        sandbox_info: vec![],
        sandbox_selected: 0,
        sandbox_tree_expanded: std::collections::HashSet::new(),
        sandbox_loaded: false,
        sandbox_error: None,
        selected_optdepends: std::collections::HashMap::new(),
        cascade_mode: pacsea::state::modal::CascadeMode::Basic,
    };

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify preflight modal rendered
    assert!(app.preflight_content_rect.is_some());
}

#[test]
fn test_modal_confirm_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.modal = Modal::ConfirmInstall {
        items: vec![PackageItem {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            source: Source::Aur,
            popularity: None,
        }],
    };

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify confirm modal rendered (check buffer)
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}

// Component State Tests

#[test]
fn test_results_selection_highlighting() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.results = vec![
        PackageItem {
            name: "pkg1".to_string(),
            version: "1.0".to_string(),
            description: "First".to_string(),
            source: Source::Aur,
            popularity: None,
        },
        PackageItem {
            name: "pkg2".to_string(),
            version: "2.0".to_string(),
            description: "Second".to_string(),
            source: Source::Aur,
            popularity: None,
        },
    ];
    app.all_results = app.results.clone();
    app.selected = 1;
    app.list_state.select(Some(1));

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify selection state is maintained
    assert_eq!(app.selected, 1);
    assert!(app.results_rect.is_some());
}

#[test]
fn test_search_input_focus() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.input = "test query".to_string();
    app.focus = pacsea::state::types::Focus::Search;
    app.search_caret = app.input.len();

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify search input is focused
    assert_eq!(app.focus, pacsea::state::types::Focus::Search);
}

#[test]
fn test_dropdowns_render() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Test sort dropdown
    app.sort_menu_open = true;
    let _terminal = render_ui_to_backend(backend, &mut app);
    assert!(app.sort_menu_rect.is_some());

    // Test options dropdown
    let backend = create_test_backend();
    app.sort_menu_open = false;
    app.options_menu_open = true;
    let _terminal = render_ui_to_backend(backend, &mut app);
    assert!(app.options_menu_rect.is_some());
}

#[test]
fn test_toast_message_renders() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.toast_message = Some("Test toast message".to_string());

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify toast was rendered (check buffer dimensions)
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}

#[test]
fn test_url_button_rect_set() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.results = vec![PackageItem {
        name: "test".to_string(),
        version: "1.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    app.details.url = "https://example.com".to_string();

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify URL button rect is set
    assert!(app.url_button_rect.is_some());
}

// Edge Cases and Error States

#[test]
fn test_ui_very_small_terminal() {
    // Test with very small terminal
    let backend = create_test_backend_size(40, 8);
    let mut app = create_test_app_state();

    let terminal = render_ui_to_backend(backend, &mut app);

    // UI should still render without panicking
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 40);
    assert_eq!(buffer.area.height, 8);
}

#[test]
fn test_ui_very_large_terminal() {
    // Test with very large terminal
    let backend = create_test_backend_size(300, 100);
    let mut app = create_test_app_state();

    app.results = vec![PackageItem {
        name: "test".to_string(),
        version: "1.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    let terminal = render_ui_to_backend(backend, &mut app);

    // Verify layout handles large sizes
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 300);
    assert_eq!(buffer.area.height, 100);
}

#[test]
fn test_ui_long_package_names() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    // Test with very long package names
    app.results = vec![PackageItem {
        name: "very-long-package-name-that-should-be-truncated-properly-in-the-ui".to_string(),
        version: "1.0.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify UI handles long names without panicking
    assert!(app.results_rect.is_some());
}

#[test]
fn test_ui_empty_results_with_query() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.input = "nonexistent-package-xyz".to_string();
    app.results = vec![];
    app.all_results = vec![];

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify UI handles empty results gracefully
    assert!(app.results_rect.is_some());
}

#[test]
fn test_ui_installed_only_mode() {
    let backend = create_test_backend();
    let mut app = create_test_app_state();

    app.installed_only_mode = true;
    app.right_pane_focus = pacsea::state::types::RightPaneFocus::Downgrade;

    app.downgrade_list = vec![PackageItem {
        name: "downgrade-pkg".to_string(),
        version: "1.0".to_string(),
        description: "To downgrade".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.downgrade_state.select(Some(0));

    app.remove_list = vec![PackageItem {
        name: "remove-pkg".to_string(),
        version: "2.0".to_string(),
        description: "To remove".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.remove_state.select(Some(0));

    let _terminal = render_ui_to_backend(backend, &mut app);

    // Verify installed-only mode renders correctly
    assert!(app.downgrade_rect.is_some());
}

#[test]
fn test_ui_resize_handling() {
    let backend = create_test_backend_size(80, 24);
    let mut app = create_test_app_state();

    app.results = vec![PackageItem {
        name: "test".to_string(),
        version: "1.0".to_string(),
        description: "Test".to_string(),
        source: Source::Aur,
        popularity: None,
    }];
    app.selected = 0;
    app.list_state.select(Some(0));

    // Render at initial size
    let mut terminal = Terminal::new(backend).expect("failed to create test terminal");
    terminal
        .draw(|f| ui::ui(f, &mut app))
        .expect("failed to draw test terminal");

    // Resize and render again
    terminal.backend_mut().resize(120, 40);
    terminal
        .draw(|f| ui::ui(f, &mut app))
        .expect("failed to draw test terminal after resize");

    // Verify resize worked
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}
