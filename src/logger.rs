/// Initialises pretty tracing
pub fn init_logging() {
    use tracing_subscriber::{
        fmt::{self, format::FmtSpan, time::UtcTime},
        prelude::*,
        EnvFilter,
    };

    // Get log level from environment or use INFO as default
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Create a formatting layer for nice console output
    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_timer(UtcTime::rfc_3339())
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(true) // Colorized output
        .pretty(); // Use pretty formatter for multi-line records

    // Compose the layers and initialize the subscriber
    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .init();
}
