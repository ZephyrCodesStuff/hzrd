use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str;

use regex::Regex;

use crate::config::SubmitConfig;

static MSG_GREETINGS: [&str; 4] = ["hello", "hi", "greetings", "welcome"];
static MSG_START: [&str; 3] = ["start", "begin", "enter your flags"];
static MSG_FLAG_SUCCESS: [&str; 2] = ["accepted", "earned"];
static MSG_FLAG_FAILURE: [&str; 3] = ["invalid", "too old", "your own"];

static FLAG_POINTS_REGEX: &str = "[\\d,.]+";

pub fn submit_flags(config: SubmitConfig, flags: Vec<String>) -> Result<f64, String> {
    let stream = TcpStream::connect((config.host, config.port)).map_err(|e| e.to_string())?;
    let mut stream = stream;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);

    // Read first line for greetings
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    line = line.trim().to_lowercase();

    if !MSG_GREETINGS.iter().any(|&msg| line.contains(msg)) {
        return Err("Expected greeting not found".to_string());
    }

    // Send team token
    writeln!(stream, "{}", config.token).map_err(|e| e.to_string())?;
    stream.flush().map_err(|e| e.to_string())?;

    // Read for start message
    line.clear();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    line = line.trim().to_lowercase();

    if !MSG_START.iter().any(|&msg| line.contains(msg)) {
        return Err("Expected start message not found".to_string());
    }

    let points_regex = Regex::new(FLAG_POINTS_REGEX).unwrap();
    let mut points = 0f64;

    // Send flags and check responses
    for flag in flags {
        // Send flag
        writeln!(stream, "{}", flag).map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;

        // Read response
        line.clear();
        reader.read_line(&mut line).map_err(|e| e.to_string())?;
        line = line.trim().to_lowercase();

        // Check if response indicates success or failure
        let success = MSG_FLAG_SUCCESS.iter().any(|&msg| line.contains(msg));
        let failure = MSG_FLAG_FAILURE.iter().any(|&msg| line.contains(msg));

        if !success && !failure {
            return Err(format!("Unexpected response for flag: {}", line));
        }

        if failure {
            return Err(format!("Flag submission failed: {}", line));
        }

        // Scan how many points we gathered
        if let Some(captures) = points_regex.captures(&line) {
            captures.iter().filter_map(|c| c).for_each(|c| {
                points += c.as_str().parse::<f64>().unwrap();
            });
        }
    }

    Ok(points)
}
