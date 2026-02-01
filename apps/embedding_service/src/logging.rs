// Logging initialization.

use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize logging with tracing subscriber.
///
/// Uses `try_init()` internally so it's safe to call multiple times
/// (subsequent calls are no-ops).
pub fn init() {
    let _ = try_init();
}

/// Try to initialize logging, returning error if already initialized.
pub fn try_init() -> Result<(), tracing_subscriber::util::TryInitError> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .with(fmt::layer())
        .try_init()
}
