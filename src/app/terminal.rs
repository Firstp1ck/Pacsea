use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// What: Enter raw mode and switch to the alternate screen with mouse capture enabled.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Ok(())` if the terminal was prepared; `Err` on I/O or terminal backend failure.
pub fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

/// What: Restore terminal to normal mode, leave the alternate screen, and disable mouse capture.
///
/// Inputs:
/// - None
///
/// Output:
/// - `Ok(())` when restoration succeeds; `Err` if underlying terminal operations fail.
pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}
