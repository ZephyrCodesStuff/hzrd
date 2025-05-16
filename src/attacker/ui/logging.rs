use std::sync::mpsc;
use tracing_subscriber::{layer::SubscriberExt, registry::LookupSpan, Layer};

/// Custom layer to capture logs for our UI
pub struct UiLogLayer {
    pub sender: mpsc::Sender<String>,
}

impl<S> Layer<S> for UiLogLayer
where
    S: tracing::Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        // Extract fields from the event
        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        // Format the log message with timestamp, level, and message
        let now = chrono::Local::now().format("%H:%M:%S%.3f");
        let level = event.metadata().level();
        let target = event.metadata().target();

        // Color based on log level
        let level_str = match *level {
            tracing::Level::ERROR => format!("ERROR"),
            tracing::Level::WARN => format!("WARN"),
            tracing::Level::INFO => format!("INFO"),
            tracing::Level::DEBUG => format!("DEBUG"),
            tracing::Level::TRACE => format!("TRACE"),
        };

        let log_message = format!(
            "[{now}] [{level_str}] [{target}]: {}",
            visitor
                .message
                .unwrap_or_else(|| String::from("<no message>"))
        );

        // Send the formatted message to our UI
        let _ = self.sender.send(log_message);
    }
}

/// Visitor to extract the message from the event
#[derive(Default)]
pub struct LogVisitor {
    pub message: Option<String>,
}

impl tracing::field::Visit for LogVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" && self.message.is_none() {
            self.message = Some(format!("{:?}", value));
        }
    }
}

/// Log management
pub struct LogManager {
    pub messages: Vec<String>,
    pub receiver: mpsc::Receiver<String>,
    pub scroll_position: usize,
}

impl LogManager {
    /// Create a new log manager
    pub fn new(receiver: mpsc::Receiver<String>) -> Self {
        Self {
            messages: Vec::new(),
            receiver,
            scroll_position: 0,
        }
    }

    /// Process any new log messages
    pub fn process_new_logs(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            self.messages.insert(0, message);
            // Keep the log at a reasonable size
            if self.messages.len() > 1000 {
                self.messages.pop();
            }
        }
    }

    /// Get the most recent log message, if any
    pub fn latest_message(&self) -> Option<&str> {
        self.messages.first().map(String::as_str)
    }

    /// Scroll up in the log view
    pub fn scroll_up(&mut self, amount: usize) {
        if self.scroll_position > 0 {
            self.scroll_position = self.scroll_position.saturating_sub(amount);
        }
    }

    /// Scroll down in the log view
    pub fn scroll_down(&mut self, amount: usize) {
        if !self.messages.is_empty() && self.scroll_position < self.messages.len() - 1 {
            self.scroll_position = (self.scroll_position + amount).min(self.messages.len() - 1);
        }
    }

    /// Set up tracing with our custom logger
    pub fn setup_tracing(sender: mpsc::Sender<String>) {
        // Create a custom subscriber that only sends logs to our UI
        let subscriber = tracing_subscriber::registry()
            .with(UiLogLayer { sender })
            .with(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            );

        // Set our subscriber as the global default
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set global default subscriber")
    }

    /// Clear the log messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_position = 0;
    }
}
