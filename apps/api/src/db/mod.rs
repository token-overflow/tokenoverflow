pub mod models;
mod pool;
// Diesel CLI auto-generated table definitions — no hand-written logic.
// E2E: all database-backed tests indirectly exercise the schema.
#[cfg_attr(coverage_nightly, coverage(off))]
mod schema;

pub use pool::DbPool;
pub use pool::init;
pub use schema::api::*;
pub use schema::*;
