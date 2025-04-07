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
    let subnet = subnet.unwrap_or_else(|| {
        config
            .clone()
            .subnet
            .expect("Subnet is required (either as `--subnet` in the CLI, or in the config file.")
    });

    let mut all_flags: Vec<String> = vec![];
    let mut iteration = 1;

    loop {
        wait(r#loop, iteration);

        // Determine which scripts to run
        let scripts_to_run = get_exploits(&config, &script);

        // Set up the flag regex if available
        let flag_regex = config.flag_regex.as_ref().map(|re| Regex::new(re).unwrap());

        // Get a list of hosts from the subnet
        let hosts = get_hosts_from_subnet(&subnet);

        // Run exploits against all hosts and collect flags
        let flags = parallel_run(&hosts, &scripts_to_run, &flag_regex);

        // Process collected flags
        if !flags.is_empty() {
            info!("Your exploit captured the following flags:");

            for flag in &flags {
                // Allows to pipe the output of the exploit script
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

fn wait(r#loop: Option<u64>, iteration: u64) {
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

fn get_exploits(config: &Config, script: &Option<PathBuf>) -> Vec<PathBuf> {
    let scripts_to_run: Vec<PathBuf> = if let Some(script_path) = script.clone() {
        if script_path.is_dir() {
            get_dir_files(&script_path)
        } else {
            // Single file script
            vec![script_path]
        }
    } else if let Some(exploit_dir) = &config.exploit_dir {
        // No script provided, use exploit directory from config
        get_dir_files(exploit_dir)
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

fn get_dir_files(dir_path: &PathBuf) -> Vec<PathBuf> {
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

fn parallel_run(
    hosts: &Vec<String>,
    scripts: &Vec<PathBuf>,
    flag_regex: &Option<Regex>,
) -> Vec<String> {
    let scripts = Arc::new(scripts.clone());
    let flags = Arc::new(Mutex::new(Vec::new()));

    hosts.par_iter().for_each(|host| {
        let mut host_captures: Vec<String> = Vec::new();

        for script_path in scripts.iter() {
            let mut captured = run_exploit(host.clone(), script_path);
            host_captures.append(&mut captured);
        }

        // If enabled, filter flags using the regex
        if let Some(regex) = flag_regex {
            host_captures.retain(|flag| regex.is_match(flag));
        };

        if !host_captures.is_empty() {
            info!("Flag captured on {host}!");

            // Add the captured flags to the shared collection
            let mut flags = flags.lock().unwrap();
            flags.extend(host_captures);
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

fn run_exploit(remote: String, script: &PathBuf) -> Vec<String> {
    debug!("Running exploit {} on {}", script.display(), remote);

    let script_content =
        CString::new(std::fs::read_to_string(&script).expect("Failed to read exploit script"))
            .unwrap();
    let script_name = CString::new(
        script
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap()
            .to_string(),
    )
    .unwrap();

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
