use std::{
    sync::{Arc, Mutex, RwLock},
    time::{self, Instant},
};

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap,
    },
    Frame,
};

use crate::structs::config::{AttackerLoopConfig, Config};

use super::tabs::TabState;
use super::{logging::LogManager, state::TeamStatus, status::StatusBar};

/// Create the main layout
fn create_layout(size: Rect) -> Vec<Rect> {
    // Help text defines the height of that section
    let help_text = get_help_text();

    Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(3),                                         // Title
                Constraint::Min(10),                                           // Main content area
                Constraint::Length(3),                                         // Tabs
                Constraint::Length((help_text.len() + 2).try_into().unwrap()), // Help + borders
                Constraint::Length(3),                                         // Status bar
            ]
            .as_ref(),
        )
        .split(size)
        .to_vec()
}

/// Render the title bar
fn render_title(
    f: &mut Frame,
    area: Rect,
    auto_attack_enabled: bool,
    auto_attack_last_at: Option<Instant>,
    attacker_loop_config: &Option<AttackerLoopConfig>,
) {
    // Get current time formatted as HH:MM:SS
    let current_time = chrono::Local::now().format("%H:%M:%S").to_string();

    // Create title text with auto-attack status
    let loop_every = attacker_loop_config
        .as_ref()
        .map(|config| config.every)
        .unwrap_or(120);

    let auto_attack_status = if auto_attack_enabled {
        format!(
            " [Auto: ON, next in: {}s]",
            if let Some(last) = auto_attack_last_at {
                let next_time = last + time::Duration::from_secs(loop_every);
                let remaining = next_time
                    .saturating_duration_since(Instant::now())
                    .as_secs();

                remaining.to_string()
            } else {
                "N/A".to_string()
            }
        )
    } else {
        " [Auto: OFF]".to_string()
    };

    let title_text = format!("Attacker Panel - {}{}", current_time, auto_attack_status);

    let title = Paragraph::new(vec![Line::styled(
        title_text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )])
    .block(Block::default().borders(Borders::ALL));

    f.render_widget(title, area);
}

/// Get appropriate style for team status
fn get_status_style(team: &TeamStatus) -> Style {
    use super::state::AttackState;

    match &team.status {
        AttackState::Idle => Style::default().fg(Color::White),
        AttackState::Attacking => Style::default().fg(Color::Yellow),
        AttackState::Submitting(_) => Style::default().fg(Color::Blue),
        AttackState::Success(_) => Style::default().fg(Color::Green),
        AttackState::Errored(_) => Style::default().fg(Color::Red),
    }
}

/// Get the help text lines showing keyboard shortcuts
fn get_help_text() -> Vec<Line<'static>> {
    vec![
        Line::from("Available Commands:"),
        Line::from("↑/↓: Navigate teams/logs/exploits/settings"),
        Line::from("Enter/Space: Attack selected team or toggle exploit"),
        Line::from("a: Attack all teams in parallel"),
        Line::from("1/2/3/4: Switch tabs (Teams/Logs/Exploits/Settings)"),
        Line::from("PageUp/PageDown: Scroll logs"),
        Line::from("r: Reload configuration and scan exploits"),
        Line::from("q: Quit"),
    ]
}

/// Render the help area with keyboard shortcuts
fn render_help(f: &mut Frame, area: Rect) {
    let help_text = get_help_text();

    let help_block = Block::default().borders(Borders::ALL).title("Help");

    let help_paragraph = Paragraph::new(help_text)
        .block(help_block)
        .wrap(Wrap { trim: true });

    f.render_widget(help_paragraph, area);
}

/// Render the UI with individual state components to avoid recursive borrow issues
pub fn render_ui_with_state(
    f: &mut Frame,
    config: &Config,
    teams: &Arc<RwLock<Vec<TeamStatus>>>,
    team_state: &mut TableState,
    exploit_state: &mut TableState,
    active_tab: TabState,
    logs: &LogManager,
    status_bar: &StatusBar,
    auto_attack_enabled: bool,
    auto_attack_last_at: Option<Instant>,
) {
    // Layout
    let size = f.area();
    let chunks = create_layout(size);

    // Render components
    render_title(
        f,
        chunks[0],
        auto_attack_enabled,
        auto_attack_last_at,
        &config.attacker.r#loop,
    );

    // Render content based on active tab
    match active_tab {
        TabState::Teams => render_teams_tab_with_state(f, teams, team_state, chunks[1]),
        TabState::Logs => render_logs_tab_with_state(f, logs, chunks[1]),
        TabState::Exploits => {
            render_exploits_tab_with_state(f, &config.attacker.exploits, exploit_state, chunks[1])
        }
        TabState::Settings => {
            render_settings_tab_with_state(f, config, auto_attack_enabled, status_bar, chunks[1])
        }
    };

    // Render other UI components
    render_tabs_with_state(f, active_tab, logs, chunks[2]);
    render_help(f, chunks[3]);
    render_status_bar_with_state(f, status_bar, chunks[4]);
}

/// Render the teams tab with state components
fn render_teams_tab_with_state(
    f: &mut Frame,
    teams: &Arc<RwLock<Vec<TeamStatus>>>,
    table_state: &mut TableState,
    area: Rect,
) {
    // Table headers
    let header = Row::new(vec![
        Cell::from("Team"),
        Cell::from("IP"),
        Cell::from("Status"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    // Table rows
    let teams_lock = teams.read().unwrap();
    let rows = teams_lock.iter().map(|team| {
        let status_style = get_status_style(&team);

        Row::new(vec![
            Cell::from(team.name.clone()),
            Cell::from(team.ip.clone()),
            Cell::from(team.status.to_string()).style(status_style),
        ])
    });

    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(30),
            Constraint::Percentage(40),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title("Teams"))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, table_state);
}

/// Render the logs tab with state components
fn render_logs_tab_with_state(f: &mut Frame, logs: &LogManager, area: Rect) {
    let total_logs = logs.messages.len();
    let logs_per_page = area.height as usize - 2; // Account for borders

    // Calculate visible range based on scroll position
    let visible_logs = if total_logs > 0 {
        let start_idx = logs.scroll_position.min(total_logs.saturating_sub(1));
        let end_idx = (start_idx + logs_per_page).min(total_logs);
        &logs.messages[start_idx..end_idx]
    } else {
        &[]
    };

    // Create colored log items
    let log_items: Vec<ListItem> = visible_logs
        .iter()
        .map(|msg| {
            let color = if msg.contains(" [ERROR] ") {
                Color::Red
            } else if msg.contains(" [WARN] ") {
                Color::Yellow
            } else if msg.contains(" [INFO] ") {
                Color::Green
            } else if msg.contains(" [DEBUG] ") {
                Color::Blue
            } else {
                Color::White
            };

            ListItem::new(Line::from(Span::styled(msg, Style::default().fg(color))))
        })
        .collect();

    let scroll_info = format!(
        "Logs ({}/{})",
        if total_logs > 0 {
            logs.scroll_position + 1
        } else {
            0
        },
        total_logs
    );

    let log_list =
        List::new(log_items).block(Block::default().borders(Borders::ALL).title(scroll_info));

    f.render_widget(log_list, area);
}

/// Render the tabs bar with state components
fn render_tabs_with_state(f: &mut Frame, active_tab: TabState, logs: &LogManager, area: Rect) {
    let last_log = logs.latest_message();
    let tab_titles = active_tab.titles(last_log);

    let tabs = Tabs::new(tab_titles)
        .select(active_tab.index())
        .block(Block::default().title("Tabs").borders(Borders::ALL))
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(tabs, area);
}

/// Render the status bar with state components
fn render_status_bar_with_state(f: &mut Frame, status_bar: &StatusBar, area: Rect) {
    let status_style = Style::default().fg(status_bar.status_type.color());

    let elapsed = status_bar.start_time.elapsed();
    let status_text = format!(
        "Status: {} | Elapsed: {:.1}s",
        status_bar.message,
        elapsed.as_secs_f32()
    );

    let status_line = Paragraph::new(status_text)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .style(status_style);

    f.render_widget(status_line, area);
}

/// Render the exploits tab with the list of available exploits
fn render_exploits_tab_with_state(
    f: &mut Frame,
    exploits: &[crate::structs::config::ExploitInfo],
    exploit_state: &mut TableState,
    area: Rect,
) {
    // Table headers
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Status"),
        Cell::from("Path"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    // Table rows
    let rows = exploits.iter().map(|exploit| {
        let status_style = if exploit.enabled {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Red)
        };

        Row::new(vec![
            Cell::from(exploit.name.clone()),
            Cell::from(if exploit.enabled {
                "Enabled"
            } else {
                "Disabled"
            })
            .style(status_style),
            Cell::from(exploit.path.display().to_string()),
        ])
    });

    let table = Table::new(
        rows,
        &[
            Constraint::Percentage(30),
            Constraint::Percentage(20),
            Constraint::Percentage(50),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Exploits ({})", exploits.len())),
    )
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, area, exploit_state);
}

/// Render the settings tab with the current configuration
fn render_settings_tab_with_state(
    f: &mut Frame,
    config: &Config,
    auto_attack_enabled: bool,
    status_bar: &StatusBar,
    area: Rect,
) {
    // Create layout for settings sections
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage(40), // Attacker settings
                Constraint::Percentage(40), // Submitter settings
                Constraint::Percentage(20), // General settings
            ]
            .as_ref(),
        )
        .split(area);

    // --- Attacker Settings Section ---
    let attacker_settings = vec![
        format!(
            "Auto Attack: {}",
            if auto_attack_enabled { "On" } else { "Off" }
        ),
        format!(
            "Attack Interval: {} seconds",
            if let Some(config) = &config.attacker.r#loop {
                format!(
                    "Every {}s, Random delay up to {}s",
                    config.every,
                    config.random.unwrap_or(0)
                )
            } else {
                "N/A".to_string()
            }
        ),
        format!("Flag Regex: {}", config.attacker.flag),
        format!("Exploits Directory: {}", config.attacker.exploit.display()),
        format!(
            "Enabled Exploits: {}/{}",
            config
                .attacker
                .exploits
                .iter()
                .filter(|e| e.enabled)
                .count(),
            config.attacker.exploits.len()
        ),
    ];

    let attacker_paragraph = Paragraph::new(attacker_settings.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Attacker Settings"),
        )
        .style(Style::default().fg(Color::Cyan))
        .wrap(Wrap { trim: true });

    // --- Submitter Settings Section ---
    let mut submitter_settings = Vec::new();

    if let Some(submitter) = config.submitter.as_ref() {
        submitter_settings.extend([
            format!("Type: {:?}", submitter.r#type),
            format!("Mode: {:?}", submitter.mode),
            format!("Database: {}", submitter.database.file),
        ]);

        // Add type-specific configuration
        match submitter.r#type {
            crate::structs::config::SubmitterType::Tcp => {
                if let Some(tcp_cfg) = &submitter.config.tcp {
                    submitter_settings.extend([
                        format!("TCP Host: {}", tcp_cfg.host),
                        format!("TCP Port: {}", tcp_cfg.port),
                        format!(
                            "TCP Token: {}",
                            if tcp_cfg.token == "changeme" {
                                "⚠️ Default token! Change it!".to_string()
                            } else {
                                "****".to_string()
                            }
                        ),
                    ]);
                }
            }
            crate::structs::config::SubmitterType::Http => {
                if let Some(http_cfg) = &submitter.config.http {
                    submitter_settings.extend([
                        format!("HTTP URL: {}", http_cfg.url),
                        format!("Insecure TLS: {}", http_cfg.insecure),
                        format!("Timeout: {}s", http_cfg.timeout.0),
                    ]);
                }
            }
        }
    } else {
        submitter_settings.push("Submitter not configured".to_string());
    }

    let submitter_paragraph = Paragraph::new(submitter_settings.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Submitter Settings"),
        )
        .style(Style::default().fg(Color::Blue))
        .wrap(Wrap { trim: true });

    // --- General Settings Section ---
    let general_settings = vec![
        "Press 'r' to reload configuration".to_string(),
        "Press 'a' to attack all teams".to_string(),
        format!("Teams configured: {}", config.attacker.teams.len()),
        format!(
            "Runtime: {:.1}s",
            status_bar.start_time.elapsed().as_secs_f32()
        ),
    ];

    let general_paragraph = Paragraph::new(general_settings.join("\n"))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("General Information"),
        )
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });

    // Render all sections
    f.render_widget(attacker_paragraph, sections[0]);
    f.render_widget(submitter_paragraph, sections[1]);
    f.render_widget(general_paragraph, sections[2]);
}
