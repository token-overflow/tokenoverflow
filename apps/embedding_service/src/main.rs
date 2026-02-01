#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use embedding_service::api::server;
use embedding_service::logging;

// Entry point: runtime bootstrap only, no testable logic.
#[cfg_attr(coverage_nightly, coverage(off))]
fn main() -> std::io::Result<()> {
    logging::init();
    server::run()
}
