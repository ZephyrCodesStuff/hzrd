use super::state::{AttackState, TeamStatus};
use ratatui::style::Color;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Status types for the status bar with color coding
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusType {
    Idle,
    Attacking,
    Submitting,
    Success,
}

impl StatusType {
    /// Get the appropriate color for this status type
    pub fn color(&self) -> Color {
        match self {
            StatusType::Idle => Color::White,
            StatusType::Attacking => Color::Yellow,
            StatusType::Submitting => Color::Blue,
            StatusType::Success => Color::Green,
        }
    }
}

/// Status bar state information
pub struct StatusBar {
    pub message: String,
    pub status_type: StatusType,
    pub start_time: Instant,
}

impl StatusBar {
    /// Create a new status bar with default values
    pub fn new() -> Self {
        Self {
            message: "Idle - Ready to attack".to_string(),
            status_type: StatusType::Idle,
            start_time: Instant::now(),
        }
    }

    /// Update the status message and type
    pub fn update(&mut self, message: &str, status_type: StatusType) {
        self.message = message.to_string();
        self.status_type = status_type;
        self.start_time = Instant::now();
    }

    /// Update status based on team states
    pub fn update_from_teams(&mut self, teams: &Arc<Mutex<Vec<TeamStatus>>>) {
        let teams_lock = teams.lock().unwrap();
        let (idle, attacking, submitting, success) = self.get_team_stats(&teams_lock);
        let total = idle + attacking + submitting + success;

        if attacking > 0 {
            self.update(
                &format!("Attacking {} teams", attacking),
                StatusType::Attacking,
            );
        } else if submitting > 0 {
            self.update(
                &format!("Submitting flags from {} teams", submitting),
                StatusType::Submitting,
            );
        } else if success > 0 && success == total {
            self.update("All attacks completed successfully", StatusType::Success);
        } else if idle == total {
            self.update("Idle - Ready to attack", StatusType::Idle);
        } else {
            // Mixed state
            self.update(
                &format!(
                    "Mixed state: {} idle, {} attacking, {} submitting, {} completed",
                    idle, attacking, submitting, success
                ),
                if submitting > 0 {
                    StatusType::Submitting
                } else if attacking > 0 {
                    StatusType::Attacking
                } else {
                    StatusType::Idle
                },
            );
        }
    }

    /// Calculate how many teams are in each status state
    fn get_team_stats(&self, teams: &[TeamStatus]) -> (usize, usize, usize, usize) {
        let mut idle = 0;
        let mut attacking = 0;
        let mut submitting = 0;
        let mut success = 0;

        for team in teams.iter() {
            match team.status {
                AttackState::Idle => idle += 1,
                AttackState::Attacking => attacking += 1,
                AttackState::Submitting(_) => submitting += 1,
                AttackState::Success(_) => success += 1,
                // Count errored teams as idle for simplicity
                AttackState::Errored(_) => idle += 1,
            }
        }

        (idle, attacking, submitting, success)
    }
}
