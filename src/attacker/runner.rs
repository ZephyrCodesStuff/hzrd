use std::{ffi::CString, fs, path::PathBuf, process::Stdio};

use anyhow::Result;
use futures::future::join_all;
use regex::Regex;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::{
    structs::{
        config::{AttackerConfig, SubmitterConfig},
        errors::AttackError,
        team::Team,
    },
    submitter,
};

/// Shortcut to create a default-styled progress-bar accepting
/// a maximum size, with manual incrementing
#[macro_export]
macro_rules! progress_bar {
    ($size:expr) => {
        ProgressBar::new($size).with_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} seconds",
                )
                .expect("Failed to set progress bar template")
                .progress_chars("#>-"),
        )
    };
}

/// Result of a single attack
///
/// - Either an array of captured flags
/// - Or an error
pub type AttackResult = Result<Vec<String>, AttackError>;

/// Attack a single team with exploits running in parallel using Tokio tasks
pub async fn attack_team_parallel(team: &Team, config: &AttackerConfig) -> Vec<AttackResult> {
    let scripts_to_run = get_exploits(config);
    let flag_regex = Regex::new(&config.flag).expect("Invalid flag regex");

    // Find the team details
    let Some((team_name, team_details)) = config
        .teams
        .iter()
        .find(|(_, t)| t.ip == team.ip)
        .map(|(name, t_details)| (name.clone(), t_details.clone()))
    else {
        error!("The specified team does not exist ({})", team.ip);
        return vec![Err(AttackError::NoSuchTeam(team.ip.to_string()))];
    };

    // Determine Python executable and PYTHONHOME
    let python_executable: String;
    let python_home_env: Option<String>;

    if let Ok(venv_path_str) = std::env::var("VIRTUAL_ENV") {
        let venv_path_buf = PathBuf::from(venv_path_str.clone());
        python_executable = venv_path_buf
            .join("bin")
            .join("python3")
            .to_string_lossy()
            .into_owned();
        python_home_env = Some(venv_path_str);
        info!("Using Python from venv: {}", python_executable);
        if let Some(ph) = &python_home_env {
            info!("Setting PYTHONHOME to: {}", ph);
        }
    } else {
        python_executable = "python3".to_string();
        python_home_env = None;
        warn!("VIRTUAL_ENV not set. Using system '{}'. PYTHONHOME not set. This might cause issues if scripts rely on a specific venv.", python_executable);
    }

    let mut futures = Vec::new();

    for script_path in scripts_to_run {
        let tn_clone = team_name.clone();
        let td_clone = team_details.clone();
        let sp_clone = script_path.clone();
        let fr_clone = flag_regex.clone();
        let pe_clone = python_executable.clone();
        let phe_clone = python_home_env.clone();

        futures.push(tokio::spawn(async move {
            run_exploit(
                &tn_clone,
                &td_clone,
                &sp_clone,
                &fr_clone,
                &pe_clone,
                phe_clone.as_deref(),
            )
            .await
        }));
    }

    let results_from_join: Vec<Result<AttackResult, tokio::task::JoinError>> =
        join_all(futures).await;

    results_from_join
        .into_iter()
        .map(|join_result| match join_result {
            Ok(attack_result) => attack_result,
            Err(join_error) => {
                error!("A spawned attack task failed to complete: {}", join_error);
                Err(AttackError::ScriptExecutionError {
                    script: "Unknown (task join error)".to_string(),
                    message: format!("Task execution failed: {}", join_error),
                })
            }
        })
        .collect()
}

/// Scan for available exploits and populate the exploits list
pub fn scan_exploits(config: &mut AttackerConfig) {
    let exploit_dir = &config.exploit;
    let mut exploits = Vec::new();

    if exploit_dir.is_dir() {
        // Scan directory for exploit files
        if let Ok(entries) = fs::read_dir(exploit_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    // Check if this exploit is already in our list (to preserve enabled status)
                    let existing = config.exploits.iter().find(|e| e.path == path);

                    if let Some(existing) = existing {
                        // Keep the existing exploit info with its enabled status
                        exploits.push(existing.clone());
                    } else {
                        // Add new exploit info
                        exploits.push(crate::structs::config::ExploitInfo::new(path));
                    }
                }
            }
        }
    } else if exploit_dir.is_file() {
        // Single exploit file
        let existing = config.exploits.iter().find(|e| e.path == *exploit_dir);

        if let Some(existing) = existing {
            exploits.push(existing.clone());
        } else {
            exploits.push(crate::structs::config::ExploitInfo::new(
                exploit_dir.clone(),
            ));
        }
    }

    // Sort exploits by name for consistent display
    exploits.sort_by(|a, b| a.name.cmp(&b.name));

    // Update the config
    config.exploits = exploits;

    info!("Scanned {} exploits", config.exploits.len());
}

fn get_exploits(config: &AttackerConfig) -> Vec<PathBuf> {
    // Filter to only use enabled exploits
    config
        .exploits
        .iter()
        .filter(|exploit| exploit.enabled)
        .map(|exploit| exploit.path.clone())
        .collect()
}

/// Run a single exploit script against a team using tokio::process::Command.
async fn run_exploit(
    team_name: &String,
    team: &Team,
    script_path: &PathBuf,
    flag_regex: &Regex,
    python_executable: &str,
    python_home_val: Option<&str>,
) -> Result<Vec<String>, AttackError> {
    let script_file_name = script_path
        .file_name()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
        .to_string();
    info!(
        "Running exploit {} on team {} (\"{}\") using Python interpreter: {}",
        script_path.display(),
        team.ip,
        team_name,
        python_executable
    );

    let mut command = Command::new(python_executable);
    command
        .arg(script_path)
        .arg(team.ip.to_string())
        .env("PWNLIB_NOTERM", "1")
        .env("NO_COLOR", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(ph_val) = python_home_val {
        command.env("PYTHONHOME", ph_val);
        debug!("PYTHONHOME set for script {}: {}", script_file_name, ph_val);
    } else {
        debug!("PYTHONHOME not set for script {}", script_file_name);
    }

    let child_process = command.spawn().map_err(|e| {
        error!(
            "Failed to spawn script process for {}: {}",
            script_path.display(),
            e
        );
        AttackError::ScriptExecutionError {
            script: script_file_name.clone(),
            message: format!("Failed to spawn script process: {}", e),
        }
    })?;

    let output = child_process.wait_with_output().await.map_err(|e| {
        error!(
            "Error waiting for script {} to finish: {}",
            script_path.display(),
            e
        );
        AttackError::ScriptExecutionError {
            script: script_file_name.clone(),
            message: format!("Error waiting for script execution: {}", e),
        }
    })?;

    if !output.status.success() {
        let stderr_output = String::from_utf8_lossy(&output.stderr);
        error!(
            "Exploit script {} for team {} (IP: {}) failed with status {:?}. Stderr:\n{}",
            script_path.display(),
            team_name,
            team.ip,
            output.status,
            stderr_output
        );
        return Err(AttackError::ScriptExecutionError {
            script: script_file_name,
            message: format!(
                "Script execution failed (status: {:?}): {}",
                output.status,
                stderr_output.trim_end()
            ),
        });
    }

    let captured_stdout = String::from_utf8_lossy(&output.stdout).to_string();
    debug!(
        "Captured stdout from {}:\n---\n{}\n---",
        script_path.display(),
        captured_stdout
    );

    let flags: Vec<String> = flag_regex
        .captures_iter(&captured_stdout)
        .map(|capture| capture[0].to_string())
        .collect();

    if flags.is_empty() && output.status.success() {
        debug!(
            "Exploit {} on {} completed successfully but found no flags matching regex.",
            script_path.display(),
            team.ip
        );
    }

    Ok(flags)
}

pub async fn submit_flags(config: &SubmitterConfig, flags: Vec<String>) -> usize {
    if flags.is_empty() {
        debug!("No flags to submit");
        return 0;
    }

    match submitter::submit_flags(config, flags).await {
        Ok((submitted, points)) => {
            if !submitted {
                info!("No new flags captured");
                return 0;
            } else if points > 0.0 {
                info!("Flags submitted successfully and gained {points} points!");
                return points as usize;
            } else {
                warn!("Flags submitted successfully, but no points were gained.");
                return 0;
            }
        }
        Err(e) => {
            error!("Failed to submit flags: {}", e);
            return 0;
        }
    }
}
