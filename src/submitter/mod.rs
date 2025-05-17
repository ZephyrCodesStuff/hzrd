pub mod http;
pub mod tcp;

use anyhow::Result;

use crate::structs::config::SubmitterType;
use crate::structs::errors::SubmitError;

// Public function to submit flags (dispatcher)
pub async fn submit_flags(
    config: &crate::structs::config::SubmitterConfig,
    flags: Vec<String>,
) -> Result<(bool, f64), SubmitError> {
    match &config.r#type {
        SubmitterType::Tcp => {
            let tcp_config = config
                .config
                .tcp
                .as_ref()
                .ok_or_else(|| SubmitError::NoSubmitter)?;

            tcp::submit_flags_tcp(tcp_config, &config.database, &flags)
        }
        SubmitterType::Http => {
            let http_config = config
                .config
                .http
                .as_ref()
                .ok_or_else(|| SubmitError::NoSubmitter)?;

            http::submit_flags_http(http_config, &config.database, flags).await
        }
    }
}
