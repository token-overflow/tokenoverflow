use std::env;
use std::error::Error;

/// Initialize tracing with JSON output for structured logging.
///
/// Uses the given `default_level` unless overridden by the
/// TOKENOVERFLOW_LOG_LEVEL env var.
/// Safe to call multiple times (subsequent calls are no-ops).
pub fn init(default_level: &str) {
    let _ = try_init(default_level);
}

/// Try to initialize logging, returning error if already initialized.
///
/// Uses the given `default_level` unless overridden by the
/// TOKENOVERFLOW_LOG_LEVEL env var.
pub fn try_init(default_level: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    let log_level =
        env::var("TOKENOVERFLOW_LOG_LEVEL").unwrap_or_else(|_| default_level.to_string());
    let level: tracing::Level = log_level.parse().unwrap_or(tracing::Level::INFO);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into()),
        )
        .json()
        .try_init()
}
