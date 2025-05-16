use std::{ffi::CString, fs, path::PathBuf};

use anyhow::Result;
use pyo3::{
    types::{PyAnyMethods, PyModule},
    PyErr, Python,
};
use regex::Regex;
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

/// Attack a single team with exploits running in parallel using Rayon
pub async fn attack_team_parallel(team: &Team, config: &AttackerConfig) -> Vec<AttackResult> {
    use rayon::prelude::*;

    let scripts_to_run = get_exploits(config);
    let flag_regex = Regex::new(&config.flag).expect("Invalid flag regex");

    // Find the team details
    let Some((team_name, team)) = config
        .teams
        .iter()
        .find(|(_, t)| t.ip == team.ip)
        .map(|(name, team)| (name.clone(), team.clone())) else
    {
        error!("The specified team does not exist ({})", team.ip);
        return vec![Err(AttackError::NoSuchTeam(team.ip.to_string()))];
    };

    // Run exploits in parallel against the team
    let results: Vec<AttackResult> = scripts_to_run
        .par_iter() // Use par_iter for parallel execution
        .map(|script_path| {
            let flag_regex = flag_regex.clone();
            let team_name = team_name.clone();
            let team = team.clone();

            // Release the GIL for each exploit
            Python::with_gil(|py| {
                match run_exploit(py, &team_name, &team, &script_path, &flag_regex) {
                    Ok(captures) => {
                        debug!("Captured flags: {:?}", captures);

                        if captures.is_empty() {
                            warn!(
                                "The exploit {} did not work on {} (\"{}\")",
                                script_path.display(),
                                team.ip,
                                team_name
                            );

                            Err(AttackError::NoCaptures)
                        } else {
                            info!("Flag captured on {} (\"{}\")!", team.ip, team_name);

                            // Return the captured flags
                            Ok(captures)
                        }
                    }
                    Err(e) => {
                        error!(
                            "Failed to run exploit on {} (\"{}\"): {e:#}",
                            team.ip, team_name
                        );

                        Err(e)
                    }
                }
            })
        })
        .collect();

    results
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

/// Run a single exploit script against a team.
fn run_exploit(
    py: Python<'_>,
    team_name: &String,
    team: &Team,
    script: &PathBuf,
    flag_regex: &Regex,
) -> Result<Vec<String>, AttackError> {
    info!(
        "Running exploit {} on team {} (\"{}\")",
        script.display(),
        team.ip,
        team_name
    );

    let script_content =
        fs::read_to_string(script).map_err(|_| AttackError::NoSuchExploit(script.clone()))?;

    let script_content = CString::new(script_content).unwrap();
    let script_name = CString::new(
        script
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap()
            .to_string(),
    )
    .unwrap();

    // IMPORTANT: set `PWNLIB_NOTERM` and `NO_COLOR` to `1` or pwntools will try to attach to a curses-like terminal,
    //            which we don't have in our virtual interpreter environment.
    unsafe {
        std::env::set_var("PWNLIB_NOTERM", "1");
        std::env::set_var("NO_COLOR", "1");
    }

    // Redirect stdout to capture output
    let sys = py.import("sys").unwrap();
    let io = py.import("io").unwrap();
    let output = io.call_method0("StringIO").unwrap();
    sys.setattr("stdout", output.clone()).unwrap();

    // Set the venv path from the current environment
    if let Ok(venv_path) = std::env::var("VIRTUAL_ENV") {
        std::env::set_var("PYTHONHOME", venv_path);
        info!(
            "Using virtual environment at {}",
            std::env::var("PYTHONHOME").unwrap()
        );
    } else {
        warn!("VIRTUAL_ENV not set, using system Python. This will *definitely* cause issues.");
    }

    // Load and execute the script with proper exception handling
    let result = PyModule::from_code(py, &script_content, &script_name, &script_name);
    let module = match result {
        Ok(module) => module,
        Err(err) => {
            // Clear the Python exception state
            if PyErr::occurred(py) {
                PyErr::fetch(py);
            }

            let error_message = err.to_string();
            error!(
                "Failed to load script {}: {}",
                script.display(),
                error_message
            );
            return Err(AttackError::ScriptExecutionError {
                script: script
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap()
                    .to_string(),
                message: error_message,
            });
        }
    };

    // Execute the exploit function with proper error handling
    match module.getattr("exploit") {
        Ok(exploit_fn) => {
            let args = (team.ip.to_string(),);
            match exploit_fn.call1(args) {
                Ok(_) => {
                    // Function executed successfully
                }
                Err(err) => {
                    // Clear the Python exception state
                    if PyErr::occurred(py) {
                        PyErr::fetch(py);
                    }

                    let error_message = err.to_string();
                    error!("Error executing {}: {}", script.display(), error_message);
                    return Err(AttackError::ScriptExecutionError {
                        script: script
                            .file_name()
                            .unwrap_or_default()
                            .to_str()
                            .unwrap()
                            .to_string(),
                        message: error_message,
                    });
                }
            }
        }
        Err(_) => {
            // Clear the Python exception state
            if PyErr::occurred(py) {
                PyErr::fetch(py);
            }

            error!(
                "Script {} does not have an 'exploit' function",
                script.display()
            );
            return Err(AttackError::ScriptExecutionError {
                script: script
                    .file_name()
                    .unwrap_or_default()
                    .to_str()
                    .unwrap()
                    .to_string(),
                message: "Missing 'exploit' function".to_string(),
            });
        }
    }

    // After script execution, capture everything printed to stdout
    let captured_output: String = output.call_method0("getvalue").unwrap().extract().unwrap();
    debug!("Captured output:\n\n{}", captured_output);

    // Apply the regex to extract flags from stdout
    Ok(flag_regex
        .captures_iter(&captured_output)
        .map(|capture| capture[0].to_string())
        .collect())
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
