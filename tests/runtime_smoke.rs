#![cfg(test)]
// End-to-end runtime smoke test (headless)
// - Starts pacsea::app::run(true) in the background.
// - Runs with PACSEA_TEST_HEADLESS=1 to bypass raw TTY setup/restore.
// - Waits briefly to allow initialization and a render cycle.
// - Asserts the task does not panic. If it finishes, it must return Ok(()).
// - If still running after the wait, aborts the task and asserts the join was a clean cancel.

use std::io::Write;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn runtime_smoke_headless_initializes_and_runs_without_panic() {
    // Ensure terminal raw mode/alternate screen are bypassed during this test
    unsafe {
        std::env::set_var("PACSEA_TEST_HEADLESS", "1");
    }

    // Spawn the runtime in the background. Use dry-run to avoid any real install actions.
    let handle = tokio::spawn(async { pacsea::app::run(true).await });

    // Allow a short window for initialization + first render + background tasks to spin up.
    tokio::time::sleep(Duration::from_millis(400)).await;

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
