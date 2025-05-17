use super::state::AttackerUI;
use super::tabs::TabState;
use crate::structs::config::Config;
use crate::{attacker::runner, structs::config::SubmitMode};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::sync::Arc;

/// Handle keyboard inputs in the UI
pub fn handle_input(state: &mut AttackerUI) -> bool {
    // Return value indicates if we should continue running
    let mut continue_running = true;

    // Handle input events with a small timeout for responsiveness
    if event::poll(std::time::Duration::from_millis(100)).unwrap_or(false) {
        if let Ok(Event::Key(key)) = event::read() {
            match (key.code, key.modifiers) {
                (KeyCode::Char('q'), _) => {
                    continue_running = false;
                }
                (KeyCode::Char('a'), _) => {
                    attack_all_teams(state);
                }
                (KeyCode::Enter, _) => match state.active_tab {
                    TabState::Teams => {
                        if let Some(idx) = state.table_state.selected() {
                            attack_team(state, idx);
                        }
                    }
                    TabState::Exploits => {
                        toggle_selected_exploit(state);
                    }
                    _ => {}
                },
                (KeyCode::Char(' '), _) => match state.active_tab {
                    TabState::Exploits => {
                        toggle_selected_exploit(state);
                    }
                    TabState::Teams => {
                        toggle_auto_attack(state);
                    }
                    _ => {}
                },
                (KeyCode::Char('c'), _) => {
                    clear_logs(state);
                }
                (KeyCode::Char('r') | KeyCode::Char('R'), _) => {
                    reload_config(state);
                }
                (KeyCode::Char('1'), _) => {
                    state.active_tab = TabState::Teams;
                }
                (KeyCode::Char('2'), _) => {
                    state.active_tab = TabState::Logs;
                }
                (KeyCode::Char('3'), _) => {
                    state.active_tab = TabState::Exploits;
                }
                (KeyCode::Char('4'), _) => {
                    state.active_tab = TabState::Settings;
                }
                (KeyCode::Tab, KeyModifiers::CONTROL) => {
                    // Switch to the next tab
                    let new_index = (state.active_tab.index() + 1) % 3; // Now with 3 tabs
                    state.active_tab = TabState::from_index(new_index);
                }
                // Handle tab-specific controls
                (KeyCode::Down, _) => handle_down_key(state),
                (KeyCode::Up, _) => handle_up_key(state),
                (KeyCode::PageDown, _) => handle_page_down(state),
                (KeyCode::PageUp, _) => handle_page_up(state),
                _ => {}
            }
        }
    }

    continue_running
}

/// Clear the logs
fn clear_logs(state: &mut AttackerUI) {
    state.logs.clear();
}

/// Handle the Down arrow key based on the active tab
fn handle_down_key(state: &mut AttackerUI) {
    match state.active_tab {
        TabState::Teams => {
            let teams_len = state.teams.lock().unwrap().len();
            if teams_len > 0 {
                let next = match state.table_state.selected() {
                    Some(i) => {
                        if i >= teams_len - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                state.table_state.select(Some(next));
            }
        }
        TabState::Logs => {
            state.logs.scroll_down(1);
        }
        TabState::Exploits => {
            let exploits_len = state.config.attacker.exploits.len();
            if exploits_len > 0 {
                let next = match state.exploit_state.selected() {
                    Some(i) => {
                        if i >= exploits_len - 1 {
                            0
                        } else {
                            i + 1
                        }
                    }
                    None => 0,
                };
                state.exploit_state.select(Some(next));
            }
        }
        TabState::Settings => {
            // No action for Down key in Settings tab
        }
    }
}

/// Handle the Up arrow key based on the active tab
fn handle_up_key(state: &mut AttackerUI) {
    match state.active_tab {
        TabState::Teams => {
            let teams_len = state.teams.lock().unwrap().len();
            if teams_len > 0 {
                let next = match state.table_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            teams_len - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                state.table_state.select(Some(next));
            }
        }
        TabState::Logs => {
            state.logs.scroll_up(1);
        }
        TabState::Exploits => {
            let exploits_len = state.config.attacker.exploits.len();
            if exploits_len > 0 {
                let next = match state.exploit_state.selected() {
                    Some(i) => {
                        if i == 0 {
                            exploits_len - 1
                        } else {
                            i - 1
                        }
                    }
                    None => 0,
                };
                state.exploit_state.select(Some(next));
            }
        }
        TabState::Settings => {
            // No action for Up key in Settings tab
        }
    }
}

// Toggle the enabled state of the currently selected exploit
fn toggle_selected_exploit(state: &mut AttackerUI) {
    if let Some(idx) = state.exploit_state.selected() {
        if idx < state.config.attacker.exploits.len() {
            // Toggle the exploit's enabled status
            state.config.attacker.exploits[idx].toggle();

            // Update UI to show the change
            let exploit_name = &state.config.attacker.exploits[idx].name;
            let status = if state.config.attacker.exploits[idx].enabled {
                "enabled"
            } else {
                "disabled"
            };
            tracing::info!("Exploit '{}' is now {}", exploit_name, status);
        }
    }
}

/// Handle PageDown key for scrolling logs
fn handle_page_down(state: &mut AttackerUI) {
    match state.active_tab {
        TabState::Teams => {
            for _ in 0..10 {
                handle_down_key(state);
            }
        }
        TabState::Logs => {
            state.logs.scroll_down(10); // Scroll by a page (10 lines)
        }
        _ => {}
    }
}

/// Handle PageUp key for scrolling logs
fn handle_page_up(state: &mut AttackerUI) {
    match state.active_tab {
        TabState::Teams => {
            for _ in 0..10 {
                handle_up_key(state);
            }
        }
        TabState::Logs => {
            state.logs.scroll_up(10); // Scroll by a page (10 lines)
        }
        _ => {}
    }
}

/// Reload the config from file
pub fn reload_config(state: &mut AttackerUI) {
    tracing::info!("Reloading configuration");

    let result = Config::from_sources(&state.args);

    match result {
        Ok(config) => {
            let mut teams = config
                .attacker
                .teams
                .iter()
                .map(|(name, team)| super::state::TeamStatus::from_team(name, team))
                .collect::<Vec<_>>();

            teams.sort_by(|a, b| a.ip.cmp(&b.ip));

            if let Ok(mut state_teams) = state.teams.lock() {
                *state_teams = teams;
            }

            // Keep the old exploits list to preserve enabled status
            let old_exploits = state.config.attacker.exploits.clone();

            // Update the config
            state.config = config.clone();
            state.submitter_config = config.submitter.clone();

            // Preserve the old exploits list
            state.config.attacker.exploits = old_exploits;

            // Scan for any new exploits
            crate::attacker::runner::scan_exploits(&mut state.config.attacker);

            tracing::info!("Configuration reloaded successfully");
        }
        Err(err) => {
            tracing::error!("Failed to reload configuration: {}", err);
        }
    }
}

/// Attack a single team by index
pub fn attack_team(state: &mut AttackerUI, team_idx: usize) {
    let team;
    {
        let mut teams = state.teams.lock().unwrap();
        if team_idx >= teams.len() {
            tracing::error!("Attempted to attack team at invalid index: {}", team_idx);
            return;
        }
        team = teams[team_idx].clone();

        // Update status immediately to show we're attacking
        teams[team_idx].status = super::state::AttackState::Attacking;
    }

    tracing::info!("Starting attack on team {} ({})", team.name, team.ip);

    // Convert to Team struct for the runner
    let team_to_attack = crate::structs::team::Team {
        ip: team.ip.parse().unwrap(),
        nop: None,
    };

    // Clone needed config
    let config = state.config.clone();
    let submitter_config = state.submitter_config.clone();

    // Create a clone of teams reference for the async task
    let teams_arc = Arc::clone(&state.teams);
    let team_name = team.name.clone();
    let team_ip = team.ip.clone();
    let idx = team_idx;

    // Spawn the attack task
    state.runtime.spawn(async move {
        tracing::debug!(
            "Running attack scripts against team {} ({})",
            team_name,
            team_ip
        );
        let results = runner::attack_team_parallel(&team_to_attack, &config.attacker);

        // Count successes and errors
        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let error_count = results.iter().filter(|r| r.is_err()).count();

        // Log errors specifically
        for (i, result) in results.iter().enumerate() {
            if let Err(err) = result {
                tracing::error!("Exploit {} failed against {}: {}", i + 1, team_name, err);
            }
        }

        let flags = results
            .iter()
            .filter_map(|r| r.clone().ok())
            .flat_map(|r| r)
            .collect::<Vec<_>>();

        tracing::info!(
            "Attack on team {} completed: {} scripts succeeded, {} scripts failed",
            team_name,
            success_count,
            error_count
        );

        if !flags.is_empty() {
            tracing::info!("Captured {} flags from team {}", flags.len(), team_name);
        } else if success_count > 0 {
            tracing::warn!(
                "No flags captured from team {} despite {} successful scripts",
                team_name,
                success_count
            );
        }

        // Determine final status
        use super::state::AttackState;
        let final_status = if results.iter().all(|r| r.is_err()) {
            AttackState::Errored(results.iter().filter_map(|r| r.clone().err()).collect())
        } else {
            if flags.is_empty() {
                AttackState::Idle
            } else {
                AttackState::Submitting(flags.clone())
            }
        };

        // Update the team status directly
        if let Ok(mut teams) = teams_arc.lock() {
            if let Some(team) = teams.get_mut(idx) {
                team.status = final_status;
            }
        }

        // Submit flags if we have them and a submitter is configured
        if let Some(submitter_config) = submitter_config {
            // Grouped: submit for each team, after all exploits
            if submitter_config.mode != SubmitMode::Grouped {
                return;
            }

            if flags.is_empty() {
                tracing::warn!("No flags to submit for team {}", team_name);
                return;
            }

            tracing::debug!("Submitting {} flags from team {}", flags.len(), team_name);
            let points = runner::submit_flags(&submitter_config, flags).await;

            if let Ok(mut teams) = teams_arc.lock() {
                if let Some(team) = teams.get_mut(idx) {
                    team.status = AttackState::Success(points);
                }
            }
        }
    });
}

/// Attack all teams in parallel with flag submission based on the configured mode
pub fn attack_all_teams(state: &mut AttackerUI) {
    let teams_to_attack = {
        let mut teams = state.teams.lock().unwrap();

        // Update all teams to Attacking status first
        for team in teams.iter_mut() {
            team.status = super::state::AttackState::Attacking;
        }

        // Clone all teams for parallel processing
        teams.clone()
    };

    tracing::info!(
        "Starting parallel attack on all {} teams",
        teams_to_attack.len()
    );

    // Clone needed config objects for async task
    let config = state.config.clone();
    let submitter_config = state.submitter_config.clone();
    let teams_arc = Arc::clone(&state.teams);

    // Spawn a task to handle the attack and flag submission
    state.runtime.spawn(async move {
        use super::state::AttackState;

        // For batch mode only: collect all flags from all teams
        let mut all_flags = Vec::new();

        // Process teams in parallel with rayon
        let results: Vec<_> = teams_to_attack
            .par_iter()
            .enumerate()
            .map(|(idx, team)| {
                // Convert to Team struct for the runner
                let team_to_attack = crate::structs::team::Team {
                    ip: team.ip.parse().unwrap(),
                    nop: None,
                };

                let team_name = team.name.clone();
                let team_ip = team.ip.clone();

                let results = runner::attack_team_parallel(&team_to_attack, &config.attacker);

                (idx, team_name, team_ip, results)
            })
            .collect();

        // Process the results and handle flag submission per team if in Grouped mode
        for (idx, team_name, _, results) in results {
            // Count successes and errors
            let success_count = results.iter().filter(|r| r.is_ok()).count();
            let error_count = results.iter().filter(|r| r.is_err()).count();

            // Log errors specifically
            for (i, result) in results.iter().enumerate() {
                if let Err(err) = result {
                    tracing::error!("Exploit {} failed against {}: {}", i + 1, team_name, err);
                }
            }

            // Collect flags
            let flags = results
                .iter()
                .filter_map(|r| r.clone().ok())
                .flat_map(|r| r)
                .collect::<Vec<_>>();

            tracing::info!(
                "Attack on team {} completed: {} scripts succeeded, {} scripts failed",
                team_name,
                success_count,
                error_count
            );

            if !flags.is_empty() {
                tracing::info!("Captured {} flags from team {}", flags.len(), team_name);

                // In 'Grouped' mode, immediately submit flags for this team
                if let Some(ref submitter_config) = submitter_config {
                    match submitter_config.mode {
                        crate::structs::config::SubmitMode::Grouped => {
                            tracing::info!(
                                "Submitting {} flags from team {} (Grouped mode)",
                                flags.len(),
                                team_name
                            );

                            // Submit flags and update team status
                            let points =
                                runner::submit_flags(submitter_config, flags.clone()).await;

                            // Update the team status
                            if let Ok(mut teams) = teams_arc.lock() {
                                if let Some(team) = teams.get_mut(idx) {
                                    team.status = AttackState::Success(points);
                                }
                            }

                            tracing::info!(
                                "Team {} flags submitted, earned {} points",
                                team_name,
                                points
                            );
                        }
                        crate::structs::config::SubmitMode::Batch => {
                            // In Batch mode, collect flags for later submission
                            all_flags.extend(flags.clone());

                            // Update team status to indicate pending submission
                            if let Ok(mut teams) = teams_arc.lock() {
                                if let Some(team) = teams.get_mut(idx) {
                                    team.status = AttackState::Submitting(flags);
                                }
                            }
                        }
                        crate::structs::config::SubmitMode::Instant => {
                            // Instant mode should not be handled here
                            // (it should be handled directly in attack_team_parallel)
                            // This is here for completeness
                            tracing::warn!(
                                "Instant mode found in attack_all_teams - this shouldn't happen"
                            );
                            all_flags.extend(flags.clone());
                        }
                    }
                } else {
                    // No submitter config, just update status
                    if let Ok(mut teams) = teams_arc.lock() {
                        if let Some(team) = teams.get_mut(idx) {
                            team.status = AttackState::Idle;
                        }
                    }
                }
            } else if success_count > 0 {
                tracing::warn!(
                    "No flags captured from team {} despite {} successful scripts",
                    team_name,
                    success_count
                );
                // Update team status - no flags captured
                if let Ok(mut teams) = teams_arc.lock() {
                    if let Some(team) = teams.get_mut(idx) {
                        team.status = AttackState::Idle;
                    }
                }
            } else {
                // All exploits failed
                let errors = results.iter().filter_map(|r| r.clone().err()).collect();
                if let Ok(mut teams) = teams_arc.lock() {
                    if let Some(team) = teams.get_mut(idx) {
                        team.status = AttackState::Errored(errors);
                    }
                }
            }
        }

        // For Batch mode: submit all collected flags at the end
        if !all_flags.is_empty()
            && submitter_config.as_ref().map_or(false, |cfg| {
                cfg.mode == crate::structs::config::SubmitMode::Batch
            })
        {
            if let Some(submitter_config) = &submitter_config {
                tracing::info!(
                    "Submitting all {} flags in a single batch (Batch mode)",
                    all_flags.len()
                );

                // Submit all flags at once
                let total_points = runner::submit_flags(submitter_config, all_flags).await;

                // Update all teams that have flags with success status
                if let Ok(mut teams) = teams_arc.lock() {
                    for team in teams.iter_mut() {
                        if let AttackState::Submitting(_) = &team.status {
                            team.status = AttackState::Success(total_points);
                        }
                    }
                }

                tracing::info!(
                    "Batch flag submission completed with {} total points",
                    total_points
                );
            }
        }

        tracing::info!("All team attacks and flag submissions completed");
    });
}

// Toggle auto-attack mode for all teams
pub fn toggle_auto_attack(state: &mut AttackerUI) {
    // Toggle the auto-attack status
    state.auto_attack_enabled = !state.auto_attack_enabled;

    // Log the state change
    let status = if state.auto_attack_enabled {
        "enabled"
    } else {
        "disabled"
    };
    tracing::info!("Auto-attack for all teams is now {}", status);

    // If enabling auto-attack, immediately start the first attack and set timestamp
    if state.auto_attack_enabled {
        attack_all_teams_loop(state);
        state.last_auto_attack = Some(std::time::Instant::now());
    }
}

/// Attack all teams in parallel - used by the auto-attack loop
pub fn attack_all_teams_loop(state: &mut AttackerUI) {
    // Set status bar message to indicate auto-attack
    if state.auto_attack_enabled {
        let loop_info = match &state.config.attacker.r#loop {
            Some(loop_config) => format!(
                "Auto-attack running (every {}s{})",
                loop_config.every,
                if let Some(random) = loop_config.random {
                    format!(" +0-{}s random", random)
                } else {
                    "".to_string()
                }
            ),
            None => "Auto-attack running (default: every 120s)".to_string(),
        };
        tracing::info!("{}", loop_info);
    }

    // Just use the normal attack_all_teams function
    attack_all_teams(state);
}
