use anyhow::Result;
use regex::Regex;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tracing::{debug, error, info};

use crate::{
    database,
    structs::{
        config::{DatabaseConfig, SubmitterTCPConfig},
        errors::SubmitError,
        flag::FlagStatus,
    },
};

static MSG_GREETINGS: [&str; 4] = ["hello", "hi", "greetings", "welcome"];
static MSG_READY: [&str; 3] = ["start", "begin", "enter your flags"];
static MSG_FLAG_SUCCESS: [&str; 2] = ["accepted", "earned"];
static MSG_FLAG_FAILURE: [&str; 3] = ["invalid", "too old", "your own"];

static FLAG_POINTS_REGEX: &str = "[\\d,.]+";

// Submit flags via TCP with database integration
pub async fn submit_flags_tcp(
    tcp_config: &SubmitterTCPConfig,
    db_config: &DatabaseConfig,
    flags: &[String],
) -> Result<(bool, f64), SubmitError> {
    // Initialize database
    let conn = database::init_db(db_config).map_err(SubmitError::SQLiteInit)?;

    // Store new flags in database
    let stored_count = database::store_flags(&conn, flags);

    if stored_count > 0 {
        debug!("Stored {} new flags in the database", stored_count);
    }

    // Get all pending flags for submission
    let pending_flags = database::get_pending_flags(&conn).map_err(SubmitError::RetrieveFlags)?;

    if pending_flags.is_empty() {
        return Ok((false, 0.0));
    }

    info!("Submitting {} pending flags", pending_flags.len());

    // Connect to TCP server
    let mut stream = TcpStream::connect((tcp_config.host, tcp_config.port))
        .await
        .map_err(SubmitError::ServiceConnection)?;

    let (rx, mut tx) = stream.split();

    // Read first line for greetings
    let mut line = String::new();
    let mut reader = tokio::io::BufReader::new(rx);

    reader
        .read_line(&mut line)
        .await
        .map_err(SubmitError::ServiceCommunication)?;

    if !MSG_GREETINGS
        .iter()
        .any(|&msg| line.trim().to_lowercase().contains(msg))
    {
        return Err(SubmitError::NoGreeting);
    }

    // Send team token
    tx.write(format!("{}\n", tcp_config.token).as_bytes())
        .await
        .map_err(SubmitError::ServiceCommunication)?;

    tx.flush()
        .await
        .map_err(SubmitError::ServiceCommunication)?;

    // Read for start message
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(SubmitError::ServiceCommunication)?;

    if !MSG_READY
        .iter()
        .any(|&msg| line.trim().to_lowercase().contains(msg))
    {
        return Err(SubmitError::NoReadyMessage);
    }

    let points_regex = Regex::new(FLAG_POINTS_REGEX).unwrap();
    let mut total_points = 0f64;

    // Submit each pending flag and update its status
    let pending_flags_len = pending_flags.len();
    for flag in pending_flags {
        // Send flag
        tx.write(format!("{flag}\n").as_bytes())
            .await
            .map_err(SubmitError::ServiceCommunication)?;

        tx.flush()
            .await
            .map_err(SubmitError::ServiceCommunication)?;

        // Read response
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(SubmitError::ServiceCommunication)?;

        // Check if response indicates success or failure
        let success = MSG_FLAG_SUCCESS
            .iter()
            .any(|&msg| line.trim().to_lowercase().contains(msg));
        let failure = MSG_FLAG_FAILURE
            .iter()
            .any(|&msg| line.trim().to_lowercase().contains(msg));

        let mut flag_points = 0.0;

        if success {
            // Extract points if flag was accepted
            if let Some(captures) = points_regex.captures(&line) {
                for capture in captures.iter().flatten() {
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
            .map_err(SubmitError::Database)?;

            info!("Flag {} accepted, earned {} points", flag, flag_points);
        } else if failure {
            // Update flag status to rejected
            database::update_flag_status(
                &conn,
                &flag,
                FlagStatus::Rejected,
                None,
                Some(&format!("Rejected: {line}")),
            )
            .map_err(SubmitError::Database)?;

            debug!("Flag {} rejected: {}", flag, line);
        } else {
            // Unexpected response
            let err_msg = format!("Unexpected response: {line}");
            database::update_flag_status(&conn, &flag, FlagStatus::Error, None, Some(&err_msg))
                .map_err(SubmitError::Database)?;

            error!("Flag {flag} failed with unexpected response: {line}");
        }
    }

    // Get total points from database (sanity check)
    let db_total_points = database::get_points_summary(&conn).map_err(SubmitError::Database)?;

    debug!("Total points in database: {}", db_total_points);

    Ok((pending_flags_len > 0, total_points))
}
