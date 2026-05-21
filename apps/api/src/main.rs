#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use tokenoverflow::api::openapi::ApiDoc;
use tokenoverflow::api::server;
use tokenoverflow::config::Config;
use tokenoverflow::logging;
use tokenoverflow::migrate;
use utoipa::OpenApi;

// Entry point: runtime bootstrap only, no testable logic.
// E2E: all tests in tests/e2e/ exercise the full server startup path.
#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = std::env::args().collect();

    // CLI mode: emit the OpenAPI spec to stdout. The TS-side codegen
    // pipeline calls this without booting a server.
    if args.iter().any(|a| a == "--openapi-json") {
        let json = serde_json::to_string_pretty(&ApiDoc::openapi())?;
        println!("{}", json);
        return Ok(());
    }

    let config = Config::load()?;
    logging::init(&config.logging.level);

    if args.iter().any(|a| a == "--migrate") {
        migrate::run_pending_migrations(&config.database.url())?;
        return Ok(());
    }

    server::run()
}
