use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str;

use anyhow::{Result, anyhow};
use log::{debug, error};
use regex::Regex;

use crate::database;
use crate::structs::config::{DatabaseConfig, SubmitterTCPConfig};
use crate::structs::errors::SubmitError;
use crate::structs::flag::FlagStatus;

static MSG_GREETINGS: [&str; 4] = ["hello", "hi", "greetings", "welcome"];
static MSG_READY: [&str; 3] = ["start", "begin", "enter your flags"];
static MSG_FLAG_SUCCESS: [&str; 2] = ["accepted", "earned"];
static MSG_FLAG_FAILURE: [&str; 3] = ["invalid", "too old", "your own"];

static FLAG_POINTS_REGEX: &str = "[\\d,.]+";

// Submit flags via TCP with database integration
pub fn submit_flags_tcp(
    tcp_config: &SubmitterTCPConfig,
    db_config: &DatabaseConfig,
    flags: Vec<String>,
) -> Result<f64> {
    // Initialize database
    let conn = database::init_db(db_config).map_err(|e| SubmitError::SQLiteInitError(e))?;

    // Store new flags in database
    let stored_count =
        database::store_flags(&conn, &flags).map_err(|e| SubmitError::StoreFlagsError(e))?;

    if stored_count > 0 {
        debug!("Stored {} new flags in the database", stored_count);
    }

    // Get all pending flags for submission
    let pending_flags =
        database::get_pending_flags(&conn).map_err(|e| SubmitError::RetrieveFlagsError(e))?;

    if pending_flags.is_empty() {
        return Ok(0.0);
    }

    debug!("Found {} pending flags to submit", pending_flags.len());

    // Connect to TCP server
    let stream = TcpStream::connect((tcp_config.host, tcp_config.port))
        .map_err(|e| SubmitError::ServiceConnectionError(e))?;

    let mut stream = stream;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(|e| SubmitError::ServiceCommunicationError(e))?,
    );

    // Read first line for greetings
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| SubmitError::ServiceCommunicationError(e))?;
    line = line.trim().to_lowercase();

    if !MSG_GREETINGS.iter().any(|&msg| line.contains(msg)) {
        return Err(anyhow!(SubmitError::NoGreetingError));
    }

    // Send team token
    writeln!(stream, "{}", tcp_config.token)
        .map_err(|e| SubmitError::ServiceCommunicationError(e))?;

    stream
        .flush()
        .map_err(|e| SubmitError::ServiceCommunicationError(e))?;

    // Read for start message
    line.clear();
    reader
        .read_line(&mut line)
        .map_err(|e| SubmitError::ServiceCommunicationError(e))?;
    line = line.trim().to_lowercase();

    if !MSG_READY.iter().any(|&msg| line.contains(msg)) {
        return Err(anyhow!(SubmitError::NoReadyMessageError));
    }

    let points_regex = Regex::new(FLAG_POINTS_REGEX).unwrap();
    let mut total_points = 0f64;

    // Submit each pending flag and update its status
    for flag in pending_flags {
        // Send flag
        writeln!(stream, "{}", flag).map_err(|e| SubmitError::ServiceCommunicationError(e))?;
        stream
            .flush()
            .map_err(|e| SubmitError::ServiceCommunicationError(e))?;

        // Read response
        line.clear();
        reader
            .read_line(&mut line)
            .map_err(|e| SubmitError::ServiceCommunicationError(e))?;
        line = line.trim().to_lowercase();

        // Check if response indicates success or failure
        let success = MSG_FLAG_SUCCESS.iter().any(|&msg| line.contains(msg));
        let failure = MSG_FLAG_FAILURE.iter().any(|&msg| line.contains(msg));

        let mut flag_points = 0.0;

        if success {
            // Extract points if flag was accepted
            if let Some(captures) = points_regex.captures(&line) {
                for capture in captures.iter().filter_map(|c| c) {
                    let parsed_points = capture.as_str().parse::<f64>().unwrap_or(0.0);
                    flag_points += parsed_points;
                    total_points += parsed_points;
                }
            }

            // Update flag status to accepted with points
            database::update_flag_status(
                &conn,
                &flag,
                FlagStatus::Accepted,
                Some(flag_points),
                None,
            )
            .map_err(|e| SubmitError::DatabaseError(e))?;

            log::info!("Flag {} accepted, earned {} points", flag, flag_points);
        } else if failure {
            // Update flag status to rejected
            database::update_flag_status(
                &conn,
                &flag,
                FlagStatus::Rejected,
                None,
                Some(&format!("Rejected: {}", line)),
            )
            .map_err(|e| SubmitError::DatabaseError(e))?;

            debug!("Flag {} rejected: {}", flag, line);
        } else {
            // Unexpected response
            let err_msg = format!("Unexpected response: {}", line);
            database::update_flag_status(&conn, &flag, FlagStatus::Error, None, Some(&err_msg))
                .map_err(|e| SubmitError::DatabaseError(e))?;

            error!("Flag {} failed with unexpected response: {}", flag, line);
        }
    }

    // Get total points from database (sanity check)
    let db_total_points =
        database::get_points_summary(&conn).map_err(|e| SubmitError::DatabaseError(e))?;

    debug!("Total points in database: {}", db_total_points);

    Ok(total_points)
}
