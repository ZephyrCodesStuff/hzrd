use std::{
    fmt::Display,
    sync::{mpsc, Arc, Mutex},
};

use futures::FutureExt;
use ratatui::{prelude::CrosstermBackend, widgets::TableState, Terminal};
use tokio::runtime::Runtime;

use crate::{
    cli::Args,
    structs::{
        config::{Config, SubmitterConfig},
        errors::AttackError,
        team::Team,
    },
    utils::ui,
};

use super::{logging::LogManager, status::StatusBar, tabs::TabState};

/// Team attack states
#[derive(Clone, Debug)]
pub enum AttackState {
    Idle,
    Attacking,
    Submitting(Vec<String>),
    Success(usize), // Points earned
    Errored(Vec<AttackError>),
}

impl Display for AttackState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Attacking => write!(f, "Attacking"),
            Self::Submitting(flags) => write!(f, "Submitting {} flags", flags.len()),
            Self::Success(points) => write!(f, "Success ({} points)", points),
            Self::Errored(errors) => write!(f, "Errored ({})", errors.len()),
        }
    }
}

/// Status tracking for teams
#[derive(Clone, Debug)]
pub struct TeamStatus {
    pub name: String,
    pub ip: String,
    pub status: AttackState,
}

impl TeamStatus {
    /// Create a new TeamStatus from a team and its name
    pub fn from_team(name: &str, team: &Team) -> Self {
        Self {
            name: name.to_string(),
            ip: team.ip.to_string(),
            status: AttackState::Idle,
        }
    }
}

/// Main UI state container
pub struct AttackerUI {
    // Configuration
    pub args: Args,
    pub config: Config,
    pub submitter_config: Option<SubmitterConfig>,

    // Terminal and rendering
    pub terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    pub running: bool,

    // Team state
    pub teams: Arc<Mutex<Vec<TeamStatus>>>,
    pub table_state: TableState,

    // Exploit state
    pub exploit_state: TableState,

    // Auto-attack state
    pub auto_attack_enabled: bool,
    pub last_auto_attack: Option<std::time::Instant>,

    // Async runtime
    pub runtime: Runtime,

    // UI components state
    pub active_tab: TabState,
    pub logs: LogManager,
    pub status_bar: StatusBar,
}

impl AttackerUI {
    /// Create a new AttackerUI instance
    pub fn new(
        args: Args,
        config: &Config,
        terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> (Self, mpsc::Sender<String>) {
        // Set up log channel
        let (log_sender, log_receiver) = std::sync::mpsc::channel();

        // Collect teams from config
        let teams: Vec<TeamStatus> = config
            .attacker
            .teams
            .iter()
            .map(|(name, team)| TeamStatus::from_team(name, team))
            .collect();

        // Create the UI state
        let state = Self {
            args,
            config: config.clone(),
            submitter_config: config.submitter.clone(),
            terminal,
            running: true,
            teams: Arc::new(Mutex::new(teams)),
            table_state: TableState::default(),
            exploit_state: TableState::default(),
            auto_attack_enabled: false,
            last_auto_attack: None,
            runtime: Runtime::new().unwrap(),
            active_tab: TabState::Teams,
            logs: LogManager::new(log_receiver),
            status_bar: StatusBar::new(),
        };

        (state, log_sender)
    }

    /// Run a single frame of the UI
    pub fn tick(&mut self) {
        // Process new logs
        self.logs.process_new_logs();

        // Check if auto-attack is enabled and if it's time to attack again
        if self.auto_attack_enabled {
            let should_attack = match (self.last_auto_attack, &self.config.attacker.r#loop) {
                (Some(last_time), Some(loop_config)) => {
                    // Calculate the base wait time in seconds
                    let wait_time = loop_config.every;

                    // Add random jitter if configured
                    let jitter = loop_config.random.unwrap_or(0);

                    // Convert to duration, add random jitter between 0 and max jitter
                    let wait_duration = if jitter > 0 {
                        let random_jitter = rand::random::<u64>() % jitter;
                        std::time::Duration::from_secs(wait_time + random_jitter)
                    } else {
                        std::time::Duration::from_secs(wait_time)
                    };

                    // Check if enough time has elapsed
                    last_time.elapsed() >= wait_duration
                }
                // No last attack time or no loop config - default to 120s
                (None, _) => true,
                (Some(last_time), None) => {
                    // Default to 120 seconds if loop config is missing
                    last_time.elapsed() >= std::time::Duration::from_secs(120)
                }
            };

            // Run the attack if it's time
            if should_attack {
                super::input::attack_all_teams_loop(self);
                self.last_auto_attack = Some(std::time::Instant::now());
            }
        }

        // Update status based on team states - extract teams reference first to avoid recursive borrow
        let teams_ref = Arc::clone(&self.teams);
        self.status_bar.update_from_teams(&teams_ref);

        // Clone necessary state fields for rendering to avoid recursive borrow
        let teams_for_render = Arc::clone(&self.teams);
        let active_tab = self.active_tab;
        let logs = &self.logs;
        let status_bar = &self.status_bar;

        // Create temporary table states for rendering
        let mut team_table_state = TableState::new();
        let mut exploit_table_state = TableState::new();

        // Copy current selections to temporary states
        if let Some(selected) = self.table_state.selected() {
            team_table_state.select(Some(selected));
        }

        if let Some(selected) = self.exploit_state.selected() {
            exploit_table_state.select(Some(selected));
        }

        // Render the UI with cloned state to avoid recursive borrow
        if let Err(e) = self.terminal.draw(|f| {
            super::rendering::render_ui_with_state(
                f,
                &self.config,
                &teams_for_render,
                &mut team_table_state,
                &mut exploit_table_state,
                active_tab,
                logs,
                status_bar,
                self.auto_attack_enabled,
                self.last_auto_attack,
            )
        }) {
            tracing::error!("Failed to render UI: {}", e);
        }

        // Update the original table states from our temporary ones
        if let Some(selected) = team_table_state.selected() {
            self.table_state.select(Some(selected));
        }

        if let Some(selected) = exploit_table_state.selected() {
            self.exploit_state.select(Some(selected));
        }

        // Handle input - returns false when user wants to quit
        self.running = super::input::handle_input(self);

        // Check for CTRL+C signal non-blockingly
        if let Some(Ok(())) = tokio::signal::ctrl_c().now_or_never() {
            self.running = false;
        }
    }
}

/// Run the UI until the user quits
pub async fn run_ui(args: Args, config: &Config) {
    // Set up terminal
    let terminal = match ui::setup_terminal() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Failed to initialize terminal: {}", e);
            return;
        }
    };

    // Create UI state
    let (mut state, log_sender) = AttackerUI::new(args, config, terminal);

    // Scan for exploits
    let mut attacker_config = state.config.attacker.clone();
    crate::attacker::runner::scan_exploits(&mut attacker_config);
    state.config.attacker = attacker_config;

    // Initial table selection
    state.table_state.select(Some(0));
    if !state.config.attacker.exploits.is_empty() {
        state.exploit_state.select(Some(0));
    }

    // Clear the terminal
    let _ = state.terminal.clear();

    // Set up tracing
    let _guard = super::logging::LogManager::setup_tracing(log_sender);

    // Initial log messages
    tracing::info!("Attacker UI started successfully");
    tracing::debug!(
        "Loaded {} teams from configuration",
        state.teams.lock().unwrap().len()
    );

    // Main UI loop
    while state.running {
        // Process a single frame
        state.tick();
    }

    // Shutdown the runtime
    state.runtime.shutdown_background();

    // Clean up the terminal
    let _ = ui::cleanup_terminal(&mut state.terminal);
}
