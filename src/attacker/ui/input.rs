use super::state::AttackerUI;
use super::tabs::TabState;
use crate::attacker::runner::{self, AttackResult};
use crate::structs::config::Config;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use futures::future::join_all;
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
            let teams_len = state.teams.read().unwrap().len();
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
            let teams_len = state.teams.read().unwrap().len();
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

            if let Ok(mut state_teams) = state.teams.write() {
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
        let mut teams = state.teams.write().unwrap();
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
        let results = runner::attack_team_parallel(&team_to_attack, &config.attacker).await;

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
        if let Ok(mut teams) = teams_arc.write() {
            if let Some(team) = teams.get_mut(idx) {
                team.status = final_status;
            }
        }

        // Submit flags if we have them and a submitter is configured
        if let Some(submitter_config) = submitter_config {
            if flags.is_empty() {
                tracing::warn!("No flags to submit for team {}", team_name);
                return;
            }

            tracing::debug!("Submitting {} flags from team {}", flags.len(), team_name);
            let points = runner::submit_flags(&submitter_config, flags).await;

            if let Ok(mut teams) = teams_arc.write() {
                if let Some(team) = teams.get_mut(idx) {
                    team.status = AttackState::Success(points);
                }
            }
        }
    });
}

/// Attack all teams in parallel with flag submission based on the configured mode
pub fn attack_all_teams(state: &mut AttackerUI) {
    let teams_to_process = {
        let mut teams_guard = state.teams.write().unwrap();

        // Update all teams to Attacking status first
        for team_status in teams_guard.iter_mut() {
            team_status.status = super::state::AttackState::Attacking;
        }

        // Clone all teams for async processing
        teams_guard.clone()
    };

    tracing::info!(
        "Starting parallel attack on all {} teams",
        teams_to_process.len()
    );

    // Clone needed config objects for async task
    let config = state.config.clone();
    let submitter_config_opt = state.submitter_config.clone();
    let teams_arc = Arc::clone(&state.teams);

    // Spawn a single Tokio task to manage all team attacks
    state.runtime.spawn(async move {
        use super::state::AttackState;

        let mut attack_futures = Vec::new();

        for (idx, team_status) in teams_to_process.iter().enumerate() {
            // Convert to Team struct for the runner
            let team_to_attack_runner = crate::structs::team::Team {
                ip: team_status
                    .ip
                    .parse()
                    .expect("Invalid IP address in team status"),
                nop: None,
            };

            let config_clone = config.clone();
            let team_name_clone = team_status.name.clone();

            // Each team's attack is a future
            attack_futures.push(async move {
                let results_for_team =
                    runner::attack_team_parallel(&team_to_attack_runner, &config_clone.attacker)
                        .await;
                (
                    idx,
                    team_name_clone,
                    team_status.ip.clone(),
                    results_for_team,
                )
            });
        }

        // Execute all team attacks concurrently
        let completed_team_attacks: Vec<(usize, String, String, Vec<AttackResult>)> =
            join_all(attack_futures).await;

        // Process the results and handle flag submission
        let mut all_flags_batch = Vec::new();

        for (idx, team_name, _team_ip, team_specific_results) in completed_team_attacks {
            let mut current_team_flags: Vec<String> = Vec::new();
            let mut current_team_errors: Vec<crate::structs::errors::AttackError> = Vec::new();
            let mut success_script_count_for_team = 0;

            for attack_result in team_specific_results {
                match attack_result {
                    Ok(flags_from_script) => {
                        if !flags_from_script.is_empty() {
                            current_team_flags.extend(flags_from_script);
                        }
                        success_script_count_for_team += 1; // Count successful scripts even if no flags
                    }
                    Err(e) => {
                        current_team_errors.push(e.clone()); // Clone the error
                                                             // Log individual script error (already done in runner, but can do here too)
                                                             // tracing::error!("Exploit script failed for {}: {}", team_name, e);
                    }
                }
            }

            let total_scripts_for_team = success_script_count_for_team + current_team_errors.len();
            // This success_count is for scripts, not if flags were found
            let overall_success_count_for_team_scripts = success_script_count_for_team;
            let overall_error_count_for_team_scripts = current_team_errors.len();

            tracing::info!(
                "Attack on team {} completed: {}/{} scripts succeeded.",
                team_name,
                overall_success_count_for_team_scripts,
                total_scripts_for_team
            );

            if !current_team_flags.is_empty() {
                tracing::info!(
                    "Captured {} flags from team {}",
                    current_team_flags.len(),
                    team_name
                );

                if let Some(ref submitter_config_inner) = submitter_config_opt {
                    match submitter_config_inner.mode {
                        crate::structs::config::SubmitMode::Grouped => {
                            tracing::info!(
                                "Submitting {} flags from team {} (Grouped mode)",
                                current_team_flags.len(),
                                team_name
                            );
                            let points = runner::submit_flags(
                                submitter_config_inner,
                                current_team_flags.clone(),
                            )
                            .await;
                            if let Ok(mut teams_guard) = teams_arc.write() {
                                if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                                    team_status_mut.status = AttackState::Success(points);
                                }
                            }
                            tracing::info!(
                                "Team {} flags submitted, earned {} points",
                                team_name,
                                points
                            );
                        }
                        crate::structs::config::SubmitMode::Batch => {
                            all_flags_batch.extend(current_team_flags.clone()); // Keep using current_team_flags
                            if let Ok(mut teams_guard) = teams_arc.write() {
                                if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                                    // Store the actual flags for potential display in UI if needed
                                    team_status_mut.status =
                                        AttackState::Submitting(current_team_flags.clone());
                                }
                            }
                        }
                        crate::structs::config::SubmitMode::Instant => {
                            tracing::warn!(
                                "Instant mode processing in attack_all_teams is unexpected."
                            );
                            let points = runner::submit_flags(
                                submitter_config_inner,
                                current_team_flags.clone(),
                            )
                            .await;
                            if let Ok(mut teams_guard) = teams_arc.write() {
                                if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                                    team_status_mut.status = AttackState::Success(points);
                                }
                            }
                        }
                    }
                } else {
                    // No submitter config
                    if let Ok(mut teams_guard) = teams_arc.write() {
                        if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                            team_status_mut.status = if current_team_flags.is_empty() {
                                AttackState::Idle
                            } else {
                                AttackState::Success(0)
                            }; // Success with 0 points if flags but no submitter
                        }
                    }
                }
            } else if overall_success_count_for_team_scripts > 0 {
                // No flags, but some scripts succeeded
                tracing::warn!(
                    "No flags captured from team {} despite {} successful scripts",
                    team_name,
                    overall_success_count_for_team_scripts
                );
                if let Ok(mut teams_guard) = teams_arc.write() {
                    if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                        team_status_mut.status = AttackState::Idle;
                    }
                }
            } else {
                // All scripts for this team failed (i.e., current_team_errors contains all results)
                tracing::error!("All exploit scripts failed for team {}.", team_name);
                if let Ok(mut teams_guard) = teams_arc.write() {
                    if let Some(team_status_mut) = teams_guard.get_mut(idx) {
                        team_status_mut.status = AttackState::Errored(current_team_errors);
                    }
                }
            }
        }

        if !all_flags_batch.is_empty()
            && submitter_config_opt.as_ref().map_or(false, |cfg| {
                cfg.mode == crate::structs::config::SubmitMode::Batch
            })
        {
            if let Some(sc_inner) = &submitter_config_opt {
                tracing::info!(
                    "Submitting all {} flags in a single batch (Batch mode)",
                    all_flags_batch.len()
                );
                let total_points = runner::submit_flags(sc_inner, all_flags_batch).await;
                if let Ok(mut teams_guard) = teams_arc.write() {
                    for team_status_mut in teams_guard.iter_mut() {
                        if let AttackState::Submitting(_) = &team_status_mut.status {
                            team_status_mut.status = AttackState::Success(total_points);
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
