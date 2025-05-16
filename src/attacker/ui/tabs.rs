use ratatui::text::Line;

const MAX_LINE_LENGTH: usize = 128;

/// Defines the different tabs available in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    Teams,
    Logs,
    Exploits,
}

impl TabState {
    /// Get tab titles, optionally showing the latest log message
    pub fn titles(&self, last_log: Option<&str>) -> Vec<Line<'static>> {
        match self {
            TabState::Teams => vec![
                Line::from("Teams"),
                // Last log preview for the Logs tab
                if let Some(log) = last_log {
                    // Truncate long messages
                    let preview = if log.len() > MAX_LINE_LENGTH {
                        format!("Logs ({}...)", &log[0..MAX_LINE_LENGTH])
                    } else {
                        format!("Logs ({})", log)
                    };
                    Line::from(preview)
                } else {
                    Line::from("Logs")
                },
                Line::from("Exploits"),
            ],
            TabState::Logs => vec![
                Line::from("Teams"),
                Line::from("Logs"),
                Line::from("Exploits"),
            ],
            TabState::Exploits => vec![
                Line::from("Teams"),
                Line::from("Logs"),
                Line::from("Exploits"),
            ],
        }
    }

    /// Get the index of the current tab
    pub fn index(&self) -> usize {
        match self {
            TabState::Teams => 0,
            TabState::Logs => 1,
            TabState::Exploits => 2,
        }
    }

    /// Convert an index to a TabState
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => TabState::Teams,
            1 => TabState::Logs,
            2 => TabState::Exploits,
            _ => TabState::Teams, // Default
        }
    }
}
