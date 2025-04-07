use std::{
    ffi::CString,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use log::{debug, error, info};
use pyo3::{
    Python,
    types::{PyAnyMethods, PyModule},
};
use rayon::prelude::*;
use regex::Regex;

use crate::{config::Config, subnet::Subnet};

/// Run an exploit and return the captured flags.
pub fn run(config: Config, script: Option<PathBuf>, subnet: Option<Subnet>, r#loop: Option<u64>) {
    let subnet = get_subnet(config.clone(), subnet);

    let mut all_flags: Vec<String> = vec![];
    let mut iteration = 1;

    loop {
        handle_iteration_delay(r#loop, iteration);

        // Determine which scripts to run
        let scripts_to_run = determine_scripts_to_run(&config, &script);

        // Set up the flag regex if available
        let flag_regex = config.flag_regex.as_ref().map(|re| Regex::new(re).unwrap());

        // Get a list of hosts from the subnet
        let hosts = get_hosts_from_subnet(&subnet);

        // Run exploits against all hosts and collect flags
        let flags = run_exploits_in_parallel(&hosts, &scripts_to_run, &flag_regex);

        // Process collected flags
        if !flags.is_empty() {
            info!("Your exploit captured the following flags:");

            for flag in &flags {
                println!("{}", flag);
            }

            all_flags.extend(flags);
        }

        iteration += 1;

        // If loop is not specified, break after first run
        if r#loop.is_none() {
            break;
        }
    }
}

fn get_subnet(config: Config, subnet: Option<Subnet>) -> Subnet {
    subnet.unwrap_or_else(|| {
        config
            .subnet
            .expect("Subnet is required (either as `--subnet` in the CLI, or in the config file.")
    })
}

fn handle_iteration_delay(r#loop: Option<u64>, iteration: u64) {
    if let Some(loop_seconds) = r#loop {
        if iteration > 1 {
            info!(
                "Iteration {}: waiting {} seconds before next run...",
                iteration, loop_seconds
            );
            thread::sleep(Duration::from_secs(loop_seconds));
        }
    }
}

fn determine_scripts_to_run(config: &Config, script: &Option<PathBuf>) -> Vec<PathBuf> {
    let scripts_to_run: Vec<PathBuf> = if let Some(script_path) = script.clone() {
        if script_path.is_dir() {
            get_scripts_from_directory(&script_path)
        } else {
            // Single file script
            vec![script_path]
        }
    } else if let Some(exploit_dir) = &config.exploit_dir {
        // No script provided, use exploit directory from config
        get_scripts_from_directory(exploit_dir)
    } else {
        error!("No script provided and no exploit directory configured");
        std::process::exit(1);
    };

    if scripts_to_run.is_empty() {
        error!("No exploit scripts found to run");
        std::process::exit(1);
    }

    scripts_to_run
}

fn get_scripts_from_directory(dir_path: &PathBuf) -> Vec<PathBuf> {
    fs::read_dir(dir_path)
        .expect(&format!("Failed to read directory: {}", dir_path.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect()
}

fn get_hosts_from_subnet(subnet: &Subnet) -> Vec<String> {
    subnet.0.hosts().map(|h| h.to_string()).collect()
}

fn run_exploits_in_parallel(
    hosts: &Vec<String>,
    scripts: &Vec<PathBuf>,
    flag_regex: &Option<Regex>,
) -> Vec<String> {
    let scripts = Arc::new(scripts.clone());
    let flags = Arc::new(Mutex::new(Vec::new()));

    hosts.par_iter().for_each(|host| {
        let host_flags = run_exploits_on_host(host, &scripts, flag_regex);

        if !host_flags.is_empty() {
            info!("Flag captured on {host}!");

            // Add the captured flags to the shared collection
            let mut flags = flags.lock().unwrap();
            flags.extend(host_flags);
        } else {
            debug!("The exploit did not work on {host}.");
        }
    });

    // Get all flags from the mutex
    Arc::try_unwrap(flags)
        .expect("Unable to unwrap Arc")
        .into_inner()
        .expect("Unable to unwrap Mutex")
}

fn run_exploits_on_host(
    host: &String,
    scripts: &Arc<Vec<PathBuf>>,
    flag_regex: &Option<Regex>,
) -> Vec<String> {
    let mut host_captures: Vec<String> = Vec::new();

    for script_path in scripts.iter() {
        let mut captured = run_exploit(host.clone(), script_path);
        host_captures.append(&mut captured);
    }

    // If enabled, filter flags using the regex
    if let Some(regex) = flag_regex {
        host_captures.retain(|flag| regex.is_match(flag));
    }

    host_captures
}

fn run_exploit(remote: String, script: &PathBuf) -> Vec<String> {
    debug!("Running exploit {} on {}", script.display(), remote);

    let script_content = load_script_content(script);
    let script_name = extract_script_name(script);

    // Load the exploit script into a Python module
    Python::with_gil(|py| {
        let module = PyModule::from_code(py, &script_content, &script_name, &script_name).unwrap();
        let args = (remote,);

        module
            .getattr("exploit")
            .expect("Your exploit script does not contain an `exploit` function!")
            .call1(args)
            .expect("The `exploit` function failed to execute! Make sure it's defined as: `def exploit(subnet: str)`")
            .extract()
            .expect("Failed to get the result of `exploit`!")
    })
}

fn load_script_content(script: &PathBuf) -> CString {
    CString::new(std::fs::read_to_string(&script).expect("Failed to read exploit script")).unwrap()
}

fn extract_script_name(script: &PathBuf) -> CString {
    CString::new(
        script
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap()
            .to_string(),
    )
    .unwrap()
}
