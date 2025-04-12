use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Result, anyhow};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, error, info, warn};
use pyo3::{
    Python,
    types::{PyAnyMethods, PyModule},
};
use rand::Rng;
use regex::Regex;
use tokio::{signal, task, time};

use crate::{
    structs::{
        config::{AttackerLoopConfig, Config, SubmitterConfig},
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

/// Run an exploit and return the captured flags.
pub async fn attack(config: &Config) {
    let mut all_flags: Vec<String> = vec![];
    let mut iteration = 1;

    loop {
        // Check if we should wait
        if let Some(config) = &config.attacker.r#loop {
            wait(iteration, config).await;
        }

        // Determine which scripts to run
        let scripts_to_run = get_exploits(&config);

        // Set up the flag regex if available
        let flag_regex = Regex::new(&config.attacker.flag).expect("Invalid flag regex");

        // Run exploits against all hosts and collect flags
        let flags = parallel_run(&config.attacker.teams, &scripts_to_run, &flag_regex).await;

        // Process collected flags
        if !flags.is_empty() {
            info!("Your exploit captured the following flags:");
            for flag in &flags {
                println!("{}", flag);
            }
            all_flags.extend(flags.clone());
        }

        // Handle flag submission if enabled
        if let Some(submitter_config) = &config.submitter {
            submit_flags(submitter_config, flags).await;
        }

        iteration += 1;

        // If loop is not specified, break after first run
        if config.attacker.r#loop.is_none() {
            break;
        }
    }
}

async fn wait(iteration: u64, config: &AttackerLoopConfig) {
    // We don't need to wait on the very first iteration
    if iteration <= 1 {
        return;
    }

    info!(
        "Iteration {}: waiting {} seconds before next run...",
        iteration, config.every
    );

    // Progress bar for the waiting period
    let pb = progress_bar!(config.every);

    // Wait for the 'every' duration
    for _ in 0..config.every {
        // Check for CTRL+C signal and break if received
        tokio::select! {
            _ = time::sleep(Duration::from_secs(1)) => {
                pb.inc(1);
            }
            _ = signal::ctrl_c() => {
                warn!("Received CTRL+C. Exiting...");
                std::process::exit(0);
            }
        }
    }

    pb.finish_and_clear();

    // Apply random delay if specified
    if let Some(random_delay) = config.random {
        let mut rng = rand::rng();
        let delay = rng.random_range(0..=random_delay);

        let pb = progress_bar!(delay);

        info!(
            "Iteration {iteration}: applying random delay of {} seconds...",
            delay
        );

        for _ in 0..delay {
            // Check for CTRL+C signal and break if received
            tokio::select! {
                _ = time::sleep(Duration::from_secs(1)) => {
                    pb.inc(1);
                }
                _ = signal::ctrl_c() => {
                    warn!("Received CTRL+C. Exiting...");
                    std::process::exit(0);
                }
            }
        }
    }

    pb.finish_with_message("Ready for next iteration");
}

fn get_exploits(config: &Config) -> Vec<PathBuf> {
    let exploit_dir = &config.attacker.exploit;
    if exploit_dir.is_dir() {
        get_dir_files(exploit_dir)
    } else {
        vec![exploit_dir.clone()]
    }
}

fn get_dir_files(dir_path: &PathBuf) -> Vec<PathBuf> {
    fs::read_dir(dir_path)
        .expect(&format!("Failed to read directory: {}", dir_path.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

async fn parallel_run(
    teams: &HashMap<String, Team>,
    scripts: &Vec<PathBuf>,
    flag_regex: &Regex,
) -> Vec<String> {
    let flags = Arc::new(Mutex::new(Vec::new()));
    let tasks: Vec<_> = teams
        .iter()
        .map(|(name, team)| {
            let flags = Arc::clone(&flags);
            let scripts = scripts.clone();
            let flag_regex = flag_regex.clone();
            let name = name.clone();
            let team = team.clone();

            task::spawn(async move {
                let mut host_captures: Vec<String> = Vec::new();

                for script_path in scripts.iter() {
                    match run_exploit(&name, &team, script_path, &flag_regex) {
                        Ok(mut captures) => host_captures.append(&mut captures),
                        Err(e) => {
                            error!("Failed to run exploit on {} (\"{}\"): {e:#}", team.ip, name)
                        }
                    }
                }

                // If flags were captured, add them to the shared collection
                if !host_captures.is_empty() {
                    info!("Flag captured on {} (\"{}\")!", team.ip, name);

                    // Add the captured flags to the shared collection
                    let mut flags = flags.lock().unwrap();
                    flags.extend(host_captures);
                } else {
                    warn!("The exploit did not work on {} (\"{}\")", team.ip, name);
                }
            })
        })
        .collect();

    // Wait for all tasks to finish
    futures::future::join_all(tasks).await;

    // Get all flags from the mutex
    let flags = Arc::try_unwrap(flags)
        .expect("Unable to unwrap Arc")
        .into_inner()
        .expect("Unable to unwrap Mutex");

    // Deduplicate flags
    let mut deduplicated_flags = HashSet::new();
    for flag in flags {
        if deduplicated_flags.insert(flag.clone()) {
            deduplicated_flags.insert(flag);
        }
    }

    deduplicated_flags.into_iter().collect()
}

fn run_exploit(
    team_name: &String,
    team: &Team,
    script: &PathBuf,
    flag_regex: &Regex,
) -> Result<Vec<String>> {
    info!(
        "Running exploit {} on team {} (\"{}\")",
        script.display(),
        team.ip,
        team_name
    );

    let script_content =
        fs::read_to_string(&script).map_err(|e| anyhow!(AttackError::NoSuchExploitError(e)))?;

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

    // IMPORTANT: set `PWNLIB_NOTERM` to `1` or `pwntools` will try to attach to a curses-like terminal,
    //            which we don't have in our virtual interpreter environment.
    unsafe { std::env::set_var("PWNLIB_NOTERM", "1") }

    let flags: Vec<String> = Python::with_gil(|py| {
        let sys = py.import("sys").unwrap();
        let io = py.import("io").unwrap();
        let output = io.call_method0("StringIO").unwrap();
        sys.setattr("stdout", output.clone()).unwrap();

        // Load and execute the script
        let module = match PyModule::from_code(py, &script_content, &script_name, &script_name) {
            Ok(module) => module,
            Err(err) => {
                err.print(py);
                return vec![]; // Return empty vector if the script failed to load
            }
        };
        let args = (team.ip.clone(),); // Pass team IP as argument

        match module
            .getattr("exploit")
            .expect("Your exploit script does not contain an `exploit` function!")
            .call1(args)
        {
            Ok(_) => {
                // After script execution, capture everything printed to stdout
                let captured_output: String =
                    output.call_method0("getvalue").unwrap().extract().unwrap();

                debug!("Captured output:\n\n{}", captured_output);

                // Apply the regex to extract flags from stdout
                flag_regex
                    .captures_iter(&captured_output)
                    .map(|capture| capture[0].to_string())
                    .collect()
            }
            Err(err) => {
                error!(
                    "Error executing `{}` function: {}",
                    script_name.to_str().unwrap(),
                    err
                );
                vec![] // Return empty vector if there was an error executing the exploit
            }
        }
    });

    Ok(flags)
}

async fn submit_flags(config: &SubmitterConfig, flags: Vec<String>) {
    if flags.is_empty() {
        debug!("No flags to submit");
        return;
    }

    match config.r#type.as_str() {
        "tcp" => {
            if let Some(tcp_config) = &config.config.tcp {
                info!(
                    "Submitting {} flags to {}:{}",
                    flags.len(),
                    tcp_config.host,
                    tcp_config.port
                );

                match submitter::submit_flags_tcp(tcp_config, &config.database, flags) {
                    Ok(points) => {
                        if points > 0.0 {
                            info!("Flags submitted successfully and gained {points} points!");
                        } else {
                            warn!("Flags submitted successfully, but no points were gained.");
                        }
                    }
                    Err(e) => error!("Failed to submit flags: {}", e),
                }
            } else {
                error!("TCP configuration is missing but type is set to 'tcp'");
            }
        }
        "http" => {
            if let Some(http_config) = &config.config.http {
                info!("Submitting {} flags to {}", flags.len(), http_config.url);

                // Replace this with your HTTP submission implementation
                error!("HTTP submission not yet implemented!");
            } else {
                error!("HTTP configuration is missing but type is set to 'http'");
            }
        }
        unknown_type => {
            error!("Unknown submitter type: {}", unknown_type);
        }
    }
}
