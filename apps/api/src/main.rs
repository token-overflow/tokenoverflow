#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use tokenoverflow::api::server;
use tokenoverflow::config::Config;
use tokenoverflow::logging;
use tokenoverflow::migrate;

// Entry point: runtime bootstrap only, no testable logic.
// E2E: all tests in tests/e2e/ exercise the full server startup path.
#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    logging::init(&config.logging.level);

    if std::env::args().any(|arg| arg == "--migrate") {
        migrate::run_pending_migrations(&config.database.url())?;
        return Ok(());
    }

    server::run()
}
