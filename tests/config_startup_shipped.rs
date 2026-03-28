//! Startup regression smoke test using pristine shipped config files.
//!
//! This integration test mutates global environment variables (`HOME`,
//! `XDG_CONFIG_HOME`, `PACSEA_TEST_HEADLESS`) and should run with
//! `cargo test -- --test-threads=1` to avoid cross-test interference.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use tempfile::TempDir;

/// What: Guard and restore environment variables needed for startup integration tests.
///
/// Inputs:
/// - `temp_home`: Temporary HOME directory path to enforce isolated config lookup.
///
/// Output:
/// - A guard that restores original env vars in `Drop`.
struct EnvGuard {
    /// Original HOME value.
    home: Option<std::ffi::OsString>,
    /// Original `XDG_CONFIG_HOME` value.
    xdg_config_home: Option<std::ffi::OsString>,
    /// Original `PACSEA_TEST_HEADLESS` value.
    pacsea_test_headless: Option<std::ffi::OsString>,
}

impl EnvGuard {
    /// What: Configure process environment for isolated headless startup testing.
    ///
    /// Inputs:
    /// - `temp_home`: Temporary home directory path.
    ///
    /// Output:
    /// - Initialized `EnvGuard`.
    fn new(temp_home: &Path) -> Self {
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

/// What: Return path to `$HOME/.config/pacsea`.
///
/// Inputs:
/// - `home_dir`: Temporary HOME path.
///
/// Output:
/// - Pacsea config directory path under HOME.
fn pacsea_config_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".config").join("pacsea")
}

/// What: Copy shipped config examples from repository config directory into a test HOME.
///
/// Inputs:
/// - `target_config_dir`: Destination directory (`$HOME/.config/pacsea`).
///
/// Output:
/// - No return value; writes `settings.conf`, `theme.conf`, and `keybinds.conf`.
///
/// Details:
/// - Sources from `CARGO_MANIFEST_DIR/config`.
fn copy_shipped_config_files(target_config_dir: &Path) {
    let source_config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
    std::fs::create_dir_all(target_config_dir).expect("target config dir should be creatable");

    for file_name in ["settings.conf", "theme.conf", "keybinds.conf"] {
        let source = source_config_dir.join(file_name);
        let target = target_config_dir.join(file_name);
        let bytes = std::fs::read(&source).unwrap_or_else(|error| {
            panic!(
                "failed reading shipped config {}: {error}",
                source.display()
            )
        });
        std::fs::write(&target, bytes).unwrap_or_else(|error| {
            panic!("failed writing copied config {}: {error}", target.display())
        });
    }
}

#[tokio::test]
/// What: Verify pristine shipped config files do not prevent startup.
///
/// Inputs:
/// - Isolated temporary HOME.
/// - Unmodified copies of repo `config/settings.conf`, `config/theme.conf`, `config/keybinds.conf`.
///
/// Output:
/// - Startup does not panic.
/// - Runtime either exits cleanly (`Ok`) or can be cleanly cancelled.
///
/// Details:
/// - Executes full startup path through `pacsea::app::run(true)` in headless mode.
/// - Uses dedicated integration test binary for fresh process-level theme initialization.
async fn startup_with_shipped_configs_does_not_prevent_boot() {
    let temp_home = TempDir::new().expect("temp home dir should be creatable");
    let _env_guard = EnvGuard::new(temp_home.path());
    let target_config_dir = pacsea_config_dir(temp_home.path());
    copy_shipped_config_files(&target_config_dir);

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
