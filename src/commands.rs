use std::{ffi::CString, net::Ipv4Addr, path::PathBuf};

use log::debug;
use pyo3::{
    Python,
    types::{PyAnyMethods, PyModule},
};

/// Run an exploit and return the captured flags.
pub fn run(script: PathBuf, remote: Ipv4Addr) -> Vec<String> {
    debug!("Running exploit {} on {}", script.display(), remote);

    // Read the exploit script from the file system
    let script_file =
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
    return Python::with_gil(|py| {
        let module = PyModule::from_code(py, &script_file, &script_name, &script_name).unwrap();

        // Call the `exploit()` function
        let args = (remote.to_string(),);
        let flags: Vec<String> = module
            .getattr("exploit")
            .expect("Your exploit script does not contain an `exploit` function!")
            .call1(args)
            .expect("The `exploit` function failed to execute! Make sure it's defined as: `def exploit(subnet: str)`")
            .extract()
            .expect("Failed to get the result of `exploit`!");

        return flags;
    });
}
