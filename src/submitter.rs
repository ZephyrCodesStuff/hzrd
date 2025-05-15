use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::str;
use std::time::Duration;

use anyhow::{anyhow, Result};
use regex::Regex;
use reqwest::{header, ClientBuilder};
use tracing::info;
use tracing::{debug, error};

use crate::database;
use crate::structs::config::{
    DatabaseConfig, SubmitterHTTPConfig, SubmitterTCPConfig, SubmitterType,
};
use crate::structs::errors::SubmitError;
use crate::structs::flag::{FlagStatus, SubmitterHTTPResponse};

static MSG_GREETINGS: [&str; 4] = ["hello", "hi", "greetings", "welcome"];
static MSG_READY: [&str; 3] = ["start", "begin", "enter your flags"];
static MSG_FLAG_SUCCESS: [&str; 2] = ["accepted", "earned"];
static MSG_FLAG_FAILURE: [&str; 3] = ["invalid", "too old", "your own"];

static FLAG_POINTS_REGEX: &str = "[\\d,.]+";

// Submit flags via TCP with database integration
pub fn submit_flags_tcp(
    tcp_config: &SubmitterTCPConfig,
    db_config: &DatabaseConfig,
    flags: &[String],
) -> Result<(bool, f64)> {
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

    debug!("Found {} pending flags to submit", pending_flags.len());

    // Connect to TCP server
    let stream = TcpStream::connect((tcp_config.host, tcp_config.port))
        .map_err(SubmitError::ServiceConnection)?;

    let mut stream = stream;
    let mut reader = BufReader::new(
        stream
            .try_clone()
            .map_err(SubmitError::ServiceCommunication)?,
    );

    // Read first line for greetings
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(SubmitError::ServiceCommunication)?;
    line = line.trim().to_lowercase();

    if !MSG_GREETINGS.iter().any(|&msg| line.contains(msg)) {
        return Err(anyhow!(SubmitError::NoGreeting));
    }

    // Send team token
    writeln!(stream, "{}", tcp_config.token).map_err(SubmitError::ServiceCommunication)?;

    stream.flush().map_err(SubmitError::ServiceCommunication)?;

    // Read for start message
    line.clear();
    reader
        .read_line(&mut line)
        .map_err(SubmitError::ServiceCommunication)?;
    line = line.trim().to_lowercase();

    if !MSG_READY.iter().any(|&msg| line.contains(msg)) {
        return Err(anyhow!(SubmitError::NoReadyMessage));
    }

    let points_regex = Regex::new(FLAG_POINTS_REGEX).unwrap();
    let mut total_points = 0f64;

    // Submit each pending flag and update its status
    let pending_flags_len = pending_flags.len();
    for flag in pending_flags {
        // Send flag
        writeln!(stream, "{flag}").map_err(SubmitError::ServiceCommunication)?;
        stream.flush().map_err(SubmitError::ServiceCommunication)?;

        // Read response
        line.clear();
        reader
            .read_line(&mut line)
            .map_err(SubmitError::ServiceCommunication)?;
        line = line.trim().to_lowercase();

        // Check if response indicates success or failure
        let success = MSG_FLAG_SUCCESS.iter().any(|&msg| line.contains(msg));
        let failure = MSG_FLAG_FAILURE.iter().any(|&msg| line.contains(msg));

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

// Submit flags via HTTP with database integration
pub async fn submit_flags_http(
    http_config: &SubmitterHTTPConfig,
    db_config: &DatabaseConfig,
    flags: Vec<String>,
) -> Result<(bool, f64)> {
    // Initialize database
    let conn = database::init_db(db_config).map_err(SubmitError::SQLiteInit)?;

    // Store new flags in database
    let stored_count = database::store_flags(&conn, &flags);

    if stored_count > 0 {
        debug!("Stored {} new flags in the database", stored_count);
    }

    // Get all pending flags for submission
    let pending_flags = database::get_pending_flags(&conn).map_err(SubmitError::RetrieveFlags)?;

    if pending_flags.is_empty() {
        return Ok((false, 0.0));
    }

    debug!("Found {} pending flags to submit", pending_flags.len());

    // Set up headers
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "X-Team-Token",
        header::HeaderValue::from_str(&http_config.token).map_err(|e| {
            SubmitError::ServiceCommunication(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                e,
            ))
        })?,
    );
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );

    // Create HTTP client
    let client = ClientBuilder::new()
        .timeout(Duration::from_secs(http_config.timeout.0 as u64))
        .danger_accept_invalid_certs(http_config.insecure)
        .default_headers(headers)
        .build()
        .map_err(|e| SubmitError::ServiceCommunication(std::io::Error::other(e)))?;

    // Create JSON body with array of flags
    let json_body = serde_json::json!(pending_flags);

    // Send PUT request
    let response = client
        .put(http_config.url.clone())
        .json(&json_body)
        .send()
        .await
        .map_err(|e| SubmitError::ServiceCommunication(std::io::Error::other(e)))?;

    // Check response status and body
    if response.status().is_success() {
        let responses: Vec<SubmitterHTTPResponse> = response
            .json()
            .await
            .map_err(|e| SubmitError::ServiceCommunication(std::io::Error::other(e)))?;

        for (i, result) in responses.iter().enumerate() {
            if i >= pending_flags.len() {
                break;
            }

            let flag = &pending_flags[i];
            let flag_status = FlagStatus::from(result.status.clone());

            // Update flag status in database
            database::update_flag_status(&conn, flag, flag_status, None, Some(&result.message))
                .map_err(SubmitError::Database)?;

            if flag_status == FlagStatus::Accepted {
                info!("Flag {} accepted!", result.flag);
            } else {
                debug!("Flag {} rejected: {}", result.flag, result.message);
            }
        }
    } else {
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| SubmitError::ServiceCommunication(std::io::Error::other(e)))?;

        let error_msg = format!("HTTP error: {status} - {response_text}");
        error!("{}", error_msg);

        for flag in &pending_flags {
            database::update_flag_status(&conn, flag, FlagStatus::Error, None, Some(&error_msg))
                .map_err(SubmitError::Database)?;
        }

        error!("Flag submission failed: {}", error_msg);
    }

    // Get total points from database (sanity check)
    let db_total_points = database::get_points_summary(&conn).map_err(SubmitError::Database)?;

    debug!("Total points in database: {}", db_total_points);

    Ok((!pending_flags.is_empty(), db_total_points))
}

// Public function to submit flags (dispatcher)
pub async fn submit_flags(
    config: &crate::structs::config::SubmitterConfig,
    flags: Vec<String>,
) -> Result<(bool, f64)> {
    match &config.r#type {
        SubmitterType::Tcp => {
            let tcp_config = config
                .config
                .tcp
                .as_ref()
                .ok_or_else(|| anyhow!("TCP config required for TCP submitter"))?;
            submit_flags_tcp(tcp_config, &config.database, &flags)
        }
        SubmitterType::Http => {
            let http_config = config
                .config
                .http
                .as_ref()
                .ok_or_else(|| anyhow!("HTTP config required for HTTP submitter"))?;
            submit_flags_http(http_config, &config.database, flags).await
        }
    }
}
