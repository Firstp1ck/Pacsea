//! Public-boundary regression tests for the arch-toolkit migration.

#![cfg(test)]

use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use pacsea::install::{build_install_command_for_executor, build_remove_command_for_executor};
use pacsea::state::{PackageItem, Source, modal::CascadeMode};

/// What: Override privilege-tool test hooks and restore them on drop.
///
/// Inputs:
/// - No explicit inputs; captures the current integration-test override variables.
///
/// Output:
/// - A guard exposing both sudo and doas to debug-only test resolution.
///
/// Details:
/// - A module-local mutex serializes this override; the full suite also runs with one test thread.
struct PrivilegeOverride {
    /// Original integration-test context marker.
    original_integration_test: Option<OsString>,
    /// Original privilege availability override.
    original_available: Option<OsString>,
    /// Module-local lock serializing privilege environment overrides.
    _env_lock: MutexGuard<'static, ()>,
}

/// What: Serialize process-wide privilege environment overrides in this test binary.
///
/// Inputs: None.
///
/// Output:
/// - Lazily initialized mutex.
///
/// Details:
/// - No other install integration module mutates these variables.
static PRIVILEGE_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

impl PrivilegeOverride {
    /// What: Enable deterministic privilege-tool availability for one test.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Guard that restores prior variables on drop.
    ///
    /// Details:
    /// - Both supported tools are exposed so any cached valid privilege mode can resolve.
    fn new() -> Self {
        let env_lock = PRIVILEGE_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let original_integration_test = std::env::var_os("PACSEA_INTEGRATION_TEST");
        let original_available = std::env::var_os("PACSEA_TEST_PRIVILEGE_AVAILABLE");
        unsafe {
            std::env::set_var("PACSEA_INTEGRATION_TEST", "1");
            std::env::set_var("PACSEA_TEST_PRIVILEGE_AVAILABLE", "sudo,doas");
        }
        Self {
            original_integration_test,
            original_available,
            _env_lock: env_lock,
        }
    }
}

impl Drop for PrivilegeOverride {
    /// What: Restore privilege-tool test override variables.
    ///
    /// Inputs: None.
    ///
    /// Output:
    /// - Process environment side effects only.
    ///
    /// Details:
    /// - Restoration is panic-safe while this guard still owns the module-local mutex.
    fn drop(&mut self) {
        unsafe {
            restore_env_var(
                "PACSEA_INTEGRATION_TEST",
                self.original_integration_test.as_ref(),
            );
            restore_env_var(
                "PACSEA_TEST_PRIVILEGE_AVAILABLE",
                self.original_available.as_ref(),
            );
        }
    }
}

/// What: Restore one process environment variable from a captured value.
///
/// Inputs:
/// - `key`: Environment variable name.
/// - `value`: Original value, or `None` when the variable was absent.
///
/// Output:
/// - Process environment side effect only.
///
/// Details:
/// - Centralizes the unsafe Rust 2024 environment mutation required by the test guard.
unsafe fn restore_env_var(key: &str, value: Option<&OsString>) {
    if let Some(value) = value {
        unsafe { std::env::set_var(key, value) };
    } else {
        unsafe { std::env::remove_var(key) };
    }
}

/// What: Create a deterministic package fixture for executor command tests.
///
/// Inputs:
/// - `name`: Package operand under test.
/// - `source`: Official or AUR package source.
///
/// Output:
/// - A package row with stable nonessential fields.
///
/// Details:
/// - Construction performs no host or network queries.
fn package(name: &str, source: Source) -> PackageItem {
    PackageItem {
        name: name.to_string(),
        version: String::new(),
        description: String::new(),
        source,
        popularity: None,
        out_of_date: None,
        orphaned: false,
    }
}

/// What: Verify unsafe package operands fail through Pacsea's public executor boundary.
///
/// Inputs:
/// - An official package named like a pacman option.
///
/// Output:
/// - An actionable validation error and no command string.
///
/// Details:
/// - Ensures toolkit validation cannot be bypassed by the executor adapter.
/// - A read-only installed-package probe may run, but its result cannot affect rejection.
#[test]
fn executor_install_rejects_unsafe_package_name() {
    let item = package(
        "-Syu",
        Source::Official {
            repo: "extra".to_string(),
            arch: "x86_64".to_string(),
        },
    );

    let error = build_install_command_for_executor(&[item], None, true)
        .expect_err("option-like package name should be rejected");

    assert!(
        error.contains("-Syu"),
        "error should identify the operand: {error}"
    );
}

/// What: Verify removal dry-run output preserves the toolkit plan without executing it.
///
/// Inputs:
/// - One package, config-pruning cascade mode, and dry-run enabled.
///
/// Output:
/// - A single quoted echo containing the exact neutral removal plan.
///
/// Details:
/// - Exercises validation, cascade conversion, privilege wrapping, and dry-run rendering together.
#[test]
fn executor_remove_dry_run_wraps_exact_toolkit_plan() {
    let _privilege_override = PrivilegeOverride::new();
    let command = build_remove_command_for_executor(
        &["ripgrep".to_string()],
        None,
        CascadeMode::CascadeWithConfigs,
        true,
    )
    .expect("dry-run removal command should build");

    assert!(command.starts_with("echo DRY RUN: '"));
    assert!(command.contains("pacman -Rns --noconfirm -- ripgrep"));
    assert!(
        command.contains("sudo pacman -Rns") || command.contains("doas pacman -Rns"),
        "dry-run removal must retain privilege wrapping: {command}"
    );
    assert!(!command.starts_with("echo DRY RUN: 'pacman "));
    assert!(command.ends_with('\''));
}

/// What: Verify public dependency comparison retains Arch relational semantics.
///
/// Inputs:
/// - Relational, epoch-bearing, unconstrained, and unknown-operator requirements.
///
/// Output:
/// - The expected compatibility truth table.
///
/// Details:
/// - Tests the public Pacsea wrapper rather than the private toolkit adapter.
#[test]
fn dependency_version_contract_survives_migration() {
    let cases = [
        ("2.0", ">=1.5", true),
        ("1.6", "<1.5", false),
        ("1.5", "=1.5", true),
        ("2:1.0-1", ">=9.9", true),
        ("2.0", "", true),
        ("2.0", "~1.5", true),
    ];

    for (version, requirement, expected) in cases {
        assert_eq!(
            pacsea::logic::deps::version_satisfies(version, requirement),
            expected,
            "unexpected result for {version} against {requirement}"
        );
    }
}

/// What: Verify public sandbox parsers retain representative legacy behavior.
///
/// Inputs:
/// - Multiline dependency arrays, appended build dependencies, and normalized conflicts.
///
/// Output:
/// - Stable dependency categories and bare conflict names.
///
/// Details:
/// - Crosses the public compatibility layer that preflight callers use after migration.
#[test]
fn sandbox_parser_contract_survives_migration() {
    let pkgbuild = r"
        depends=(
            'foo'
            'bar>=1.2'
            'libc.so'
        )
        makedepends+=(cmake ninja)
        conflicts=('old-pkg<2.0' 'libdbus-1.so=1-64')
    ";

    let (depends, make, check, optional) = pacsea::logic::sandbox::parse_pkgbuild_deps(pkgbuild);
    let conflicts = pacsea::logic::sandbox::parse_pkgbuild_conflicts(pkgbuild);

    assert_eq!(depends, vec!["foo", "bar>=1.2"]);
    assert_eq!(make, vec!["cmake", "ninja"]);
    assert!(check.is_empty());
    assert!(optional.is_empty());
    assert_eq!(conflicts, vec!["old-pkg"]);
}
