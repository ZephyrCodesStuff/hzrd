use std::{io::stdout, time::Duration};

use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState, Wrap},
    Terminal,
};
use rusqlite::{Connection, Result as SQLiteResult};

use crate::{
    database::{get_points_summary, init_db},
    structs::{config::Config, errors::DisplayError},
};

// Definition of a Flag struct for easier handling in the TUI
#[derive(Debug)]
struct Flag {
    id: i64,
    flag: String,
    status: String,
    points: f64,
    captured_at: u64,
    submitted_at: Option<u64>,
    error_message: Option<String>,
}

// Get all flags from the database
fn get_all_flags(conn: &Connection) -> SQLiteResult<Vec<Flag>> {
    let mut stmt = conn.prepare(
        "SELECT id, flag, status, points, captured_at, submitted_at, error_message FROM flags",
    )?;
    let flag_iter = stmt.query_map([], |row| {
        Ok(Flag {
            id: row.get(0)?,
            flag: row.get(1)?,
            status: row.get(2)?,
            points: row.get(3)?,
            captured_at: row.get(4)?,
            submitted_at: row.get(5).ok(),
            error_message: row.get(6).ok(),
        })
    })?;

    let mut flags = Vec::new();
    for flag in flag_iter {
        flags.push(flag?);
    }

    Ok(flags)
}

// Format timestamp as a human-readable date/time
fn format_timestamp(timestamp: Option<u64>) -> String {
    timestamp.map_or_else(
        || "N/A".to_string(),
        |ts| {
            let secs = ts as i64;
            let datetime = DateTime::from_timestamp(secs, 0).unwrap_or_default();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        },
    )
}

// Get status color based on flag status
fn get_status_color(status: &str) -> Color {
    match status {
        "Accepted" => Color::Green,
        "Rejected" => Color::Red,
        "Pending" => Color::Yellow,
        "Submitted" => Color::Blue,
        _ => Color::White,
    }
}

/// Use `ratatui` to create a TUI application to display all of the flags in the database.
pub fn print_flags(config: &Config) -> Result<(), DisplayError> {
    let submitter = config.submitter.as_ref().ok_or(DisplayError::NoSubmitter)?;
    let conn = init_db(&submitter.database).map_err(DisplayError::Rusqlite)?;

    // Set up terminal
    enable_raw_mode().map_err(DisplayError::Display)?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).map_err(DisplayError::Display)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(DisplayError::Display)?;

    // Get all flags from the database
    let mut flags = get_all_flags(&conn).map_err(DisplayError::Rusqlite)?;

    // Get total points
    let mut total_points = get_points_summary(&conn).map_err(DisplayError::Rusqlite)?;

    // App state
    let mut running = true;
    let mut table_state = TableState::default();
    table_state.select(Some(0)); // Start with the first row selected

    // Message and timer for reload
    let mut reload_message = String::new();
    let mut reload_message_timer = 0;

    // Main loop
    while running {
        terminal
            .draw(|f| {
                let size = f.area();

                // Create layout
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(2)
                    .constraints(
                        [
                            Constraint::Length(3),
                            Constraint::Min(0),
                            Constraint::Length(3),
                        ]
                        .as_ref(),
                    )
                    .split(size);

                // Title
                let title = Paragraph::new(vec![Line::styled(
                    "Flags Dashboard",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )])
                .block(Block::default().borders(Borders::ALL));
                f.render_widget(title, chunks[0]);

                // Table headers
                let header_cells = [
                    "ID",
                    "Flag",
                    "Status",
                    "Points",
                    "Captured At",
                    "Submitted At",
                    "Error Message",
                ]
                .iter()
                .map(|h| {
                    Cell::from(*h).style(
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    )
                });
                let header = Row::new(header_cells)
                    .style(Style::default().fg(Color::White))
                    .height(1);

                // Table rows
                let rows = flags.iter().map(|flag| {
                    let cells = [
                        Cell::from(flag.id.to_string()),
                        Cell::from(flag.flag.clone()),
                        Cell::from(flag.status.clone())
                            .style(Style::default().fg(get_status_color(&flag.status))),
                        Cell::from(format!("{:.2}", flag.points)),
                        Cell::from(format_timestamp(Some(flag.captured_at))),
                        Cell::from(format_timestamp(flag.submitted_at)),
                        Cell::from(
                            flag.error_message
                                .clone()
                                .unwrap_or_else(|| "N/A".to_string()),
                        ),
                    ];
                    Row::new(cells).height(1)
                });

                // Create table
                let table = Table::new(
                    rows,
                    &[
                        Constraint::Length(5),      // ID
                        Constraint::Percentage(25), // Flag
                        Constraint::Length(10),     // Status
                        Constraint::Length(8),      // Points
                        Constraint::Length(20),     // Captured At
                        Constraint::Length(20),     // Submitted At
                        Constraint::Min(10),        // Error Message
                    ],
                )
                .header(header)
                .block(Block::default().borders(Borders::ALL).title("Flags (↑/↓ to scroll, r to reload, q to quit)"))
                .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .column_spacing(1);
                
                // Render the table with state for scrolling
                f.render_stateful_widget(table, chunks[1], &mut table_state);

                // Footer with stats and instructions
                let footer_message = if reload_message.is_empty() {
                    format!("Total Points: {total_points} | Use ↑/↓ to scroll, R to reload data, Q to quit")
                } else {
                    format!("{reload_message} | Total Points: {total_points}")
                };

                let footer = Paragraph::new(vec![Line::styled(
                    footer_message,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )])
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
                f.render_widget(footer, chunks[2]);
            })
            .map_err(DisplayError::Display)?;

        // Handle input
        if event::poll(Duration::from_millis(100)).map_err(DisplayError::Display)? {
            if let Event::Key(key) = event::read().map_err(DisplayError::Display)? {
                match key.code {
                    KeyCode::Char('q') => {
                        running = false;
                    }
                    KeyCode::Char('r') => {
                        // Reload flags data from the database
                        flags = get_all_flags(&conn).map_err(DisplayError::Rusqlite)?;
                        total_points = get_points_summary(&conn).map_err(DisplayError::Rusqlite)?;

                        // Update selection to avoid out-of-bounds issues
                        if flags.is_empty() {
                            table_state.select(None);
                        } else if let Some(selected) = table_state.selected() {
                            if selected >= flags.len() {
                                table_state.select(Some(flags.len().saturating_sub(1)));
                            }
                        }

                        // Set reload message and timer
                        reload_message = format!(
                            "Data reloaded at {}! {} flags found",
                            Utc::now(),
                            flags.len()
                        );
                        reload_message_timer = 20; // Display for ~2 seconds
                    }
                    KeyCode::Down => {
                        // Move selection down
                        let next = table_state.selected().map_or(0, |i| {
                            if i >= flags.len().saturating_sub(1) {
                                0
                            } else {
                                i + 1
                            }
                        });
                        table_state.select(Some(next));
                    }
                    KeyCode::Up => {
                        // Move selection up
                        let next = table_state.selected().map_or(0, |i| {
                            if i == 0 {
                                flags.len().saturating_sub(1)
                            } else {
                                i - 1
                            }
                        });
                        table_state.select(Some(next));
                    }
                    _ => {}
                }
            }
        }

        // Update the reload message timer
        if reload_message_timer > 0 {
            reload_message_timer -= 1;
            if reload_message_timer == 0 {
                reload_message = String::new();
            }
        }
    }

    // Restore terminal
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
