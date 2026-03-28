//! Startup regression smoke test with isolated config state.
//!
//! This integration test intentionally mutates process environment variables (`HOME`,
//! `XDG_CONFIG_HOME`, and `PACSEA_TEST_HEADLESS`) and should be run with
//! `cargo test -- --test-threads=1` to avoid cross-test interference.

use std::path::PathBuf;
use std::time::Duration;

use tempfile::TempDir;

/// What: Guard and restore startup-related environment variables for config startup tests.
///
/// Inputs:
/// - `temp_home`: Temporary HOME directory path to use for test isolation.
///
/// Output:
/// - A guard that restores original variable values when dropped.
///
/// Details:
/// - Sets `HOME` to `temp_home`.
/// - Removes `XDG_CONFIG_HOME` so path resolution follows HOME-based defaults.
/// - Sets `PACSEA_TEST_HEADLESS=1` to avoid terminal raw-mode setup in tests.
struct EnvGuard {
    /// Original HOME value before test mutation.
    home: Option<std::ffi::OsString>,
    /// Original `XDG_CONFIG_HOME` value before test mutation.
    xdg_config_home: Option<std::ffi::OsString>,
    /// Original `PACSEA_TEST_HEADLESS` value before test mutation.
    pacsea_test_headless: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// What: Create a new environment guard and switch process env to isolated test mode.
    ///
    /// Inputs:
    /// - `temp_home`: Temporary HOME directory path.
    ///
    /// Output:
    /// - Initialized `EnvGuard`.
    ///
    /// Details:
    /// - Captures original environment values for restoration in `Drop`.
    fn new(temp_home: &std::path::Path) -> Self {
        let home = std::env::var_os("HOME");
        let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME");
        let pacsea_test_headless = std::env::var_os("PACSEA_TEST_HEADLESS");

        unsafe {
            std::env::set_var("HOME", temp_home);
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::set_var("PACSEA_TEST_HEADLESS", "1");
        }

        Self {
            home,
            xdg_config_home,
            pacsea_test_headless,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            if let Some(home) = self.home.as_ref() {
                std::env::set_var("HOME", home);
            } else {
                std::env::remove_var("HOME");
            }

            if let Some(xdg) = self.xdg_config_home.as_ref() {
                std::env::set_var("XDG_CONFIG_HOME", xdg);
            } else {
                std::env::remove_var("XDG_CONFIG_HOME");
            }

            if let Some(headless) = self.pacsea_test_headless.as_ref() {
                std::env::set_var("PACSEA_TEST_HEADLESS", headless);
            } else {
                std::env::remove_var("PACSEA_TEST_HEADLESS");
            }
        }
    }
}

/// What: Build the canonical pacsea config directory path under a temporary HOME.
///
/// Inputs:
/// - `home_dir`: Temporary home directory path.
///
/// Output:
/// - Path to `$HOME/.config/pacsea`.
fn pacsea_config_dir(home_dir: &std::path::Path) -> PathBuf {
    home_dir.join(".config").join("pacsea")
}

#[tokio::test]
/// What: Verify startup remains stable with missing files, missing keys, and invalid config values.
///
/// Inputs:
/// - Isolated temporary HOME directory.
/// - `settings.conf` containing intentionally invalid/unknown values and missing keys.
/// - Missing `theme.conf` and `keybinds.conf`.
///
/// Output:
/// - Startup does not panic.
/// - Runtime either exits cleanly (`Ok`) or can be safely aborted (`JoinError::is_cancelled`).
///
/// Details:
/// - Exercises the real startup path (`app::run`) in headless mode.
/// - Covers config failure classes in one run:
///   - missing files (`theme.conf`, `keybinds.conf` absent),
///   - missing keys (partial `settings.conf`),
///   - invalid values (`layout_left_pct = not_a_number`, unknown sort mode).
async fn startup_with_invalid_and_missing_config_files_does_not_prevent_boot() {
    let temp_home = TempDir::new().expect("temp home dir should be creatable");
    let _env_guard = EnvGuard::new(temp_home.path());
    let config_dir = pacsea_config_dir(temp_home.path());
    std::fs::create_dir_all(&config_dir).expect("config dir should be creatable");

    let broken_settings = "\
# intentionally incomplete + partially invalid startup config
layout_left_pct = not_a_number
sort_mode = definitely_not_a_valid_sort_mode
show_recent_pane = maybe
unknown_future_key = future_value
";
    std::fs::write(config_dir.join("settings.conf"), broken_settings)
        .expect("broken settings.conf should be writable");

    let handle = tokio::spawn(async { pacsea::app::run(true).await });
    tokio::time::sleep(Duration::from_millis(50)).await;

    if handle.is_finished() {
        match handle.await {
            Ok(run_result) => {
                if let Err(error) = run_result {
                    panic!("app::run returned early error: {error:?}");
                }
            }
            Err(join_error) => {
                panic!("app::run task panicked: {join_error}");
            }
        }
        return;
    }

    handle.abort();
    match handle.await {
        Ok(run_result) => {
            if let Err(error) = run_result {
                panic!("app::run completed with error during abort race: {error:?}");
            }
        }
        Err(join_error) => {
            assert!(
                join_error.is_cancelled(),
                "app::run join should be cancellation, got: {join_error}"
            );
        }
    }
}
