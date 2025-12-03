#![cfg(test)]
//! End-to-end runtime smoke test (headless)
//!
//! Tests cover:
//! - Application initialization without panicking
//! - Headless mode operation with `PACSEA_TEST_HEADLESS=1`
//! - Task cancellation handling

use std::io::Write;
use std::time::Duration;

#[tokio::test]
/// What: Test end-to-end runtime initialization and execution in headless mode.
///
/// Inputs:
/// - `PACSEA_TEST_HEADLESS=1` environment variable to bypass raw TTY setup/restore.
/// - `pacsea::app::run(true)` called with dry-run flag.
///
/// Output:
/// - Application initializes without panicking.
/// - Task either completes successfully or can be cleanly cancelled.
///
/// Details:
/// - Starts `pacsea::app::run(true)` in the background.
/// - Waits briefly (50ms) to allow initialization and a render cycle.
/// - If task finishes early, asserts it returned `Ok(())`.
/// - If still running, aborts the task and verifies clean cancellation.
/// - Clears screen output for `--nocapture` runs.
/// - In headless mode, slow operations (pacman calls, network) are skipped.
async fn runtime_smoke_headless_initializes_and_runs_without_panic() {
    // Ensure terminal raw mode/alternate screen are bypassed during this test
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    // Note: Mouse position reports (^[[<35;...]) may appear in test output when moving
    // the mouse over the terminal if mouse reporting was enabled elsewhere (e.g., by
    // Fish shell or the terminal emulator itself). The setup_terminal() function now
    // explicitly disables mouse reporting in headless mode to prevent this.

    // Spawn the runtime in the background. Use dry-run to avoid any real install actions.
    let handle = tokio::spawn(async { pacsea::app::run(true).await });

    // Allow a minimal window for initialization - just enough to verify it starts without panicking
    // In headless mode, we skip slow operations (pacman calls, network), so this should be fast
    tokio::time::sleep(Duration::from_millis(50)).await;

    // If it already finished, it must have returned Ok(()) and not panicked.
    if handle.is_finished() {
        match handle.await {
            Ok(run_result) => {
                if let Err(e) = run_result {
                    panic!("app::run returned error early: {e:?}");
                }
                // Returned Ok(()): good enough as a smoke test.
                // Clear the screen for --nocapture runs to avoid leaving the TUI in the output
                print!("\x1b[2J\x1b[H");
                let _ = std::io::stdout().flush();
                return;
            }
            Err(join_err) => {
                // If it finished with a panic, this will be a JoinError (not cancelled).
                panic!("app::run task panicked: {join_err}");
            }
        }
    }

    // Otherwise, abort it and ensure it did not panic (i.e., the join error is 'cancelled').
    handle.abort();
    match handle.await {
        Ok(run_result) => {
            // Rare race: the task may have completed right before abort. Require Ok(()).
            if let Err(e) = run_result {
                panic!("app::run completed with error on abort race: {e:?}");
            }
        }
        Err(join_err) => {
            assert!(
                join_err.is_cancelled(),
                "app::run join error should be cancellation, got: {join_err}"
            );
        }
    }
    // Clear the screen at end of test (useful with --nocapture)
    print!("\x1b[2J\x1b[H");
    let _ = std::io::stdout().flush();
}
