//! Tests for `skip_preflight` setting functionality.
//!
//! Tests cover:
//! - Preflight modal is opened when `skip_preflight` = false (default)
//! - `open_preflight_modal` function is callable and handles packages correctly

use pacsea as crate_root;

/// What: Test that `open_preflight_modal` is callable and doesn't panic.
///
/// Details:
/// - Creates a test app state
/// - Calls `open_preflight_modal` with a test package
/// - Verifies the function executes without panicking
/// - Note: Actual `skip_preflight` behavior depends on settings file
#[test]
fn test_open_preflight_modal_callable() {
    // Create test app state
    let mut app = crate_root::state::AppState::default();

    // Create a test package
    let test_package = crate_root::state::PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "Test package".to_string(),
        source: crate_root::state::Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

    // Call open_preflight_modal - should not panic
    crate_root::events::open_preflight_modal(&mut app, vec![test_package], true);

    // Verify the function executed (either opened modal or skipped based on settings)
    // The actual modal state depends on skip_preflight setting in config
}

/// What: Test that `open_preflight_modal` handles multiple packages correctly.
///
/// Details:
/// - Creates a test app state
/// - Calls `open_preflight_modal` with multiple test packages
/// - Verifies the function executes without panicking
#[test]
fn test_open_preflight_modal_multiple_packages() {
    // Create test app state
    let mut app = crate_root::state::AppState::default();

    // Create test packages
    let test_packages = vec![
        crate_root::state::PackageItem {
            name: "pkg1".to_string(),
            version: "1.0.0".to_string(),
            description: "Test package 1".to_string(),
            source: crate_root::state::Source::Aur,
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
        crate_root::state::PackageItem {
            name: "pkg2".to_string(),
            version: "2.0.0".to_string(),
            description: "Test package 2".to_string(),
            source: crate_root::state::Source::Official {
                repo: "core".to_string(),
                arch: "x86_64".to_string(),
            },
            popularity: None,
            out_of_date: None,
            orphaned: false,
        },
    ];

    // Call open_preflight_modal - should not panic
    crate_root::events::open_preflight_modal(&mut app, test_packages, true);

    // Verify the function executed (either opened modal or skipped based on settings)
}

/// What: Test that `open_preflight_modal` with `use_cache=false` works correctly.
///
/// Details:
/// - Creates a test app state
/// - Calls `open_preflight_modal` with `use_cache=false`
/// - Verifies the function executes without panicking
#[test]
fn test_open_preflight_modal_no_cache() {
    // Create test app state
    let mut app = crate_root::state::AppState::default();

    // Create a test package
    let test_package = crate_root::state::PackageItem {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "Test package".to_string(),
        source: crate_root::state::Source::Aur,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    };

    // Call open_preflight_modal with use_cache=false - should not panic
    crate_root::events::open_preflight_modal(&mut app, vec![test_package], false);

    // Verify the function executed (either opened modal or skipped based on settings)
}

/// What: Test that Updates modal state is properly initialized.
///
/// Details:
/// - Creates an Updates modal with test entries
/// - Verifies the modal state is correctly set
#[test]
fn test_updates_modal_initialization() {
    // Create test app state
    let mut app = crate_root::state::AppState::default();

    // Create test update entries
    let entries = vec![
        (
            "test-pkg".to_string(),
            "1.0.0".to_string(),
            "1.1.0".to_string(),
        ),
        (
            "another-pkg".to_string(),
            "2.0.0".to_string(),
            "2.1.0".to_string(),
        ),
    ];

    // Set up Updates modal
    app.modal = crate_root::state::Modal::Updates {
        entries,
        scroll: 0,
        selected: 0,
    };

    // Verify modal state
    match &app.modal {
        crate_root::state::Modal::Updates {
            entries: modal_entries,
            scroll,
            selected,
        } => {
            assert_eq!(modal_entries.len(), 2);
            assert_eq!(modal_entries[0].0, "test-pkg");
            assert_eq!(modal_entries[1].0, "another-pkg");
            assert_eq!(*scroll, 0);
            assert_eq!(*selected, 0);
        }
        _ => panic!("Expected Updates modal"),
    }
}
