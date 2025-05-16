//! Utility functions for the application.

/// UI utilities.
pub mod ui {
    use crate::structs::errors::DisplayError;
    use crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};

    /// Set up the terminal for the UI.
    ///
    /// - Enable raw mode
    /// - Enter alternate screen
    /// - Enable mouse capture
    ///
    /// Returns a `Terminal` instance.
    pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, DisplayError> {
        enable_raw_mode().map_err(DisplayError::Display)?;

        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(DisplayError::Display)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend).map_err(DisplayError::Display)?;

        Ok(terminal)
    }

    /// Clean up the terminal.
    ///
    /// - Disable raw mode
    /// - Leave alternate screen
    /// - Disable mouse capture
    ///
    /// This should be called when exiting the UI, even in case of errors or panics.
    pub fn cleanup_terminal(
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> Result<(), DisplayError> {
        disable_raw_mode().map_err(DisplayError::Display)?;

        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(DisplayError::Display)?;

        terminal.show_cursor().map_err(DisplayError::Display)?;

        Ok(())
    }
}
