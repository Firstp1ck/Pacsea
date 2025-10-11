use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub fn setup_terminal() -> Result<()> {
    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    Ok(())
}

pub fn restore_terminal() -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen)?;
    Ok(())
}
