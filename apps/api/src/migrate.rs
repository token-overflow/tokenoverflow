use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness};
use tracing::info;

pub const MIGRATIONS: EmbeddedMigrations = diesel_migrations::embed_migrations!("migrations");

/// Run all pending Diesel migrations against the database.
///
/// Uses a synchronous `PgConnection` because `diesel_migrations` does not
/// support async connections. The connection URL is built from the same
/// `Config` the server uses, so environment overrides (e.g.
/// `TOKENOVERFLOW__DATABASE__HOST=localhost` for the SSM tunnel) work
/// identically.
pub fn run_pending_migrations(
    database_url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Connecting to database for migrations");
    let mut conn = PgConnection::establish(database_url)?;

    info!("Running pending migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .map_err(|e| format!("Failed to run migrations: {e}"))?;

    info!("Migrations completed successfully");
    Ok(())
}
