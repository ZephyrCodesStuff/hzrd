use ratatui::{
    prelude::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::structs::{
    config::{AttackerConfig, SubmitMode, SubmitterConfig, SubmitterType},
    errors::AttackError,
};

/// Possible settings that can be modified
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingType {
    ExploitPath,
    FlagRegex,
    SubmitEnabled,
    SubmitterType,
    SubmitMode,
}

/// Represents the current state of the settings tab
pub struct SettingsState {
    /// List of settings that can be modified
    pub items: Vec<SettingType>,

    /// Current selected setting
    pub state: ListState,

    /// Whether the user is currently editing a setting
    pub editing: bool,

    /// Temporary value for the setting being edited
    pub edit_value: String,
}

impl Default for SettingsState {
    fn default() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));

        Self {
            items: vec![
                SettingType::ExploitPath,
                SettingType::FlagRegex,
                SettingType::SubmitEnabled,
                SettingType::SubmitterType,
                SettingType::SubmitMode,
            ],
            state,
            editing: false,
            edit_value: String::new(),
        }
    }
}

impl SettingsState {
    /// Move the selection up
    pub fn previous(&mut self) {
        if !self.editing {
            let i = match self.state.selected() {
                Some(i) => {
                    if i == 0 {
                        self.items.len() - 1
                    } else {
                        i - 1
                    }
                }
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    /// Move the selection down
    pub fn next(&mut self) {
        if !self.editing {
            let i = match self.state.selected() {
                Some(i) => {
                    if i >= self.items.len() - 1 {
                        0
                    } else {
                        i + 1
                    }
                }
                None => 0,
            };
            self.state.select(Some(i));
        }
    }

    /// Enter edit mode for the current setting
    pub fn start_edit(
        &mut self,
        attacker_config: &AttackerConfig,
        submitter_config: &Option<SubmitterConfig>,
    ) {
        if let Some(selected) = self.state.selected() {
            self.editing = true;
            match self.items[selected] {
                SettingType::ExploitPath => {
                    self.edit_value = attacker_config.exploit.to_string_lossy().to_string();
                }
                SettingType::FlagRegex => {
                    self.edit_value = attacker_config.flag.clone();
                }
                SettingType::SubmitEnabled => {
                    self.edit_value = if submitter_config.is_some() {
                        "enabled"
                    } else {
                        "disabled"
                    }
                    .to_string();
                }
                SettingType::SubmitterType => {
                    if let Some(config) = submitter_config {
                        self.edit_value = format!("{}", config.r#type);
                    } else {
                        self.edit_value = "tcp".to_string();
                    }
                }
                SettingType::SubmitMode => {
                    if let Some(config) = submitter_config {
                        self.edit_value = match config.mode {
                            SubmitMode::Batch => "batch",
                            SubmitMode::Grouped => "grouped",
                            SubmitMode::Instant => "instant",
                        }
                        .to_string();
                    } else {
                        self.edit_value = "batch".to_string();
                    }
                }
            }
        }
    }

    /// Cancel editing the current setting
    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.edit_value.clear();
    }

    /// Apply the edited value to the configuration
    pub fn apply_edit(
        &mut self,
        attacker_config: &mut AttackerConfig,
        submitter_config: &mut Option<SubmitterConfig>,
        errors: &mut Vec<AttackError>,
    ) {
        if let Some(selected) = self.state.selected() {
            match self.items[selected] {
                SettingType::ExploitPath => {
                    let path = PathBuf::from(&self.edit_value);
                    if path.exists() {
                        attacker_config.exploit = path;
                    } else {
                        errors.push(AttackError::Other(format!(
                            "Path does not exist: {}",
                            self.edit_value
                        )));
                    }
                }
                SettingType::FlagRegex => {
                    // Check if regex is valid
                    match regex::Regex::new(&self.edit_value) {
                        Ok(_) => attacker_config.flag = self.edit_value.clone(),
                        Err(err) => {
                            errors.push(AttackError::Other(format!("Invalid regex: {}", err)))
                        }
                    }
                }
                SettingType::SubmitEnabled => match self.edit_value.to_lowercase().as_str() {
                    "enabled" | "true" | "yes" | "y" | "1" => {
                        if submitter_config.is_none() {
                            *submitter_config = Some(SubmitterConfig::default());
                        }
                    }
                    "disabled" | "false" | "no" | "n" | "0" => {
                        *submitter_config = None;
                    }
                    _ => {
                        errors.push(AttackError::Other(
                            "Invalid value for submit enabled. Use 'enabled' or 'disabled'".into(),
                        ));
                    }
                },
                SettingType::SubmitterType => {
                    if let Some(config) = submitter_config.as_mut() {
                        match self.edit_value.to_lowercase().as_str() {
                            "tcp" => config.r#type = SubmitterType::Tcp,
                            "http" => config.r#type = SubmitterType::Http,
                            _ => {
                                errors.push(AttackError::Other(
                                    "Invalid submitter type. Use 'tcp' or 'http'".into(),
                                ));
                            }
                        }
                    } else {
                        errors.push(AttackError::Other(
                            "Cannot set submitter type when submission is disabled".into(),
                        ));
                    }
                }
                SettingType::SubmitMode => {
                    if let Some(config) = submitter_config.as_mut() {
                        match self.edit_value.to_lowercase().as_str() {
                            "batch" => config.mode = SubmitMode::Batch,
                            "grouped" => config.mode = SubmitMode::Grouped,
                            "instant" => config.mode = SubmitMode::Instant,
                            _ => {
                                errors.push(AttackError::Other(
                                    "Invalid submit mode. Use 'batch', 'grouped', or 'instant'"
                                        .into(),
                                ));
                            }
                        }
                    } else {
                        errors.push(AttackError::Other(
                            "Cannot set submit mode when submission is disabled".into(),
                        ));
                    }
                }
            }
        }
        self.editing = false;
        self.edit_value.clear();
    }

    /// Handle keyboard input for editing a setting value
    pub fn handle_edit_input(&mut self, c: char) {
        if c == '\n' {
            // Enter key handling is done elsewhere
            return;
        } else if c == '\x08' || c == '\x7f' {
            // Backspace
            self.edit_value.pop();
        } else {
            self.edit_value.push(c);
        }
    }
}

/// Default implementation for SubmitterConfig
impl Default for SubmitterConfig {
    fn default() -> Self {
        Self {
            r#type: SubmitterType::Tcp,
            database: crate::structs::config::DatabaseConfig {
                file: "flags.sqlite".to_string(),
            },
            config: crate::structs::config::SubmissionConfig {
                tcp: Some(crate::structs::config::SubmitterTCPConfig {
                    host: "127.0.0.1".parse().unwrap(),
                    port: 1337,
                    token: "changeme".to_string(),
                }),
                http: None,
            },
            mode: SubmitMode::Batch,
        }
    }
}

/// Render the settings tab UI
pub fn render_settings(
    area: Rect,
    attacker_config: &AttackerConfig,
    submitter_config: &Option<SubmitterConfig>,
    settings_state: &mut SettingsState,
) -> Vec<ListItem> {
    let items: Vec<ListItem> = settings_state
        .items
        .iter()
        .map(|item| match item {
            SettingType::ExploitPath => {
                let value = attacker_config.exploit.to_string_lossy().to_string();
                create_setting_item("Exploit Path", &value)
            }
            SettingType::FlagRegex => {
                let value = attacker_config.flag.clone();
                create_setting_item("Flag Regex", &value)
            }
            SettingType::SubmitEnabled => {
                let value = if submitter_config.is_some() {
                    "enabled"
                } else {
                    "disabled"
                };
                create_setting_item("Submit Flags", value)
            }
            SettingType::SubmitterType => {
                let value = if let Some(config) = submitter_config {
                    format!("{}", config.r#type)
                } else {
                    "N/A".to_string()
                };
                create_setting_item("Submitter Type", &value)
            }
            SettingType::SubmitMode => {
                let value = if let Some(config) = submitter_config {
                    match config.mode {
                        SubmitMode::Batch => "batch",
                        SubmitMode::Grouped => "grouped",
                        SubmitMode::Instant => "instant",
                    }
                } else {
                    "N/A"
                };
                create_setting_item("Submit Mode", value)
            }
        })
        .collect();

    items
}

/// Create a formatted setting item for the list
fn create_setting_item(name: &str, value: &str) -> ListItem {
    let name_span = Span::styled(name, Style::default().fg(Color::Yellow));

    let value_span = Span::styled(format!(": {}", value), Style::default().fg(Color::White));

    ListItem::new(Line::from(vec![name_span, value_span]))
}

/// Render an editing field for the selected setting
pub fn render_edit_field(area: Rect, settings_state: &SettingsState) -> Paragraph<'static> {
    let selected = settings_state.state.selected().unwrap_or(0);
    let setting_type = settings_state.items[selected];

    let title = match setting_type {
        SettingType::ExploitPath => "Edit Exploit Path",
        SettingType::FlagRegex => "Edit Flag Regex",
        SettingType::SubmitEnabled => "Toggle Submission (enabled/disabled)",
        SettingType::SubmitterType => "Edit Submitter Type (tcp/http)",
        SettingType::SubmitMode => "Edit Submit Mode (batch/grouped/instant)",
    };

    Paragraph::new(settings_state.edit_value.clone()).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    )
}
