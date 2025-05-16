use std::time::Duration;

use anyhow::Result;
use reqwest::{header, ClientBuilder};
use tracing::{debug, error, info};

use crate::{
    database,
    structs::{
        config::{DatabaseConfig, SubmitterHTTPConfig},
        errors::SubmitError,
        flag::{FlagStatus, SubmitterHTTPResponse},
    },
};

// Submit flags via HTTP with database integration
pub async fn submit_flags_http(
    http_config: &SubmitterHTTPConfig,
    db_config: &DatabaseConfig,
    flags: Vec<String>,
) -> Result<(bool, f64), SubmitError> {
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

    info!("Submitting {} pending flags", pending_flags.len());

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
