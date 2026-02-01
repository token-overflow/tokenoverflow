use std::time::Duration;

use diesel_async::AsyncPgConnection;
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;

pub type DbPool = Pool<AsyncPgConnection>;

/// Initialize the database connection pool
///
/// Uses bb8 with a single connection because PgBouncer handles connection pooling.
/// The application only needs a single connection to PgBouncer.
/// Lambda runs one request at a time, so max_size=1 is correct.
// Thin bb8 pool setup — tests use their own per-test pool via testcontainers.
// E2E: tests/e2e/api/routes/test_health.rs exercises the production pool path.
#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn init(database_url: &str) -> Result<DbPool, Box<dyn std::error::Error + Send + Sync>> {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(database_url);
    let pool = Pool::builder()
        .max_size(1)
        // Fail fast if pool.get() cannot return a connection
        .connection_timeout(Duration::from_secs(5))
        .build(config)
        .await?;
    Ok(pool)
}
