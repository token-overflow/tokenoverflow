//! Testcontainers-based database helper for integration tests.
//!
//! Starts a single pgvector/pgvector:pg17 container per test run and creates
//! a template database with migrations applied. Each test gets its own
//! database cloned from the template, providing full isolation without the
//! cost of running migrations per test.

#![allow(dead_code)]

use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::pooled_connection::bb8::Pool;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use testcontainers::core::IntoContainerPort;
use testcontainers::core::wait::{LogWaitStrategy, WaitFor};
use testcontainers::runners::AsyncRunner;
use testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::OnceCell;

use tokenoverflow::db::DbPool;

/// Monotonically increasing counter for unique database names.
static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Global test infrastructure -- started once per test run.
static TEST_INFRA: OnceCell<TestInfra> = OnceCell::const_new();

/// Container ID for process-exit cleanup via `#[dtor]`.
static CONTAINER_ID: OnceLock<String> = OnceLock::new();

/// Holds the running container and connection metadata.
struct TestInfra {
    /// Keep the container alive for the entire test run.
    _container: ContainerAsync<GenericImage>,
    /// Admin connection URL (points at the `postgres` database).
    admin_url: String,
    /// Base URL without a database path component.
    base_url: String,
}

/// Build and return the global test infrastructure.
async fn get_infra() -> &'static TestInfra {
    TEST_INFRA
        .get_or_init(|| async {
            // Postgres logs "ready to accept connections" twice during startup:
            // once during the init script and once after the final restart.
            // Waiting for the second occurrence ensures the database is fully ready.
            // Using stdout_or_stderr because Docker runtimes (e.g. OrbStack) may
            // merge the two streams.
            let ready_condition = WaitFor::log(
                LogWaitStrategy::stdout_or_stderr("database system is ready to accept connections")
                    .with_times(2),
            );

            let container = GenericImage::new("pgvector/pgvector", "0.8.2-pg18")
                .with_exposed_port(5432.tcp())
                .with_wait_for(ready_condition)
                .with_env_var("POSTGRES_USER", "postgres")
                .with_env_var("POSTGRES_PASSWORD", "postgres")
                .with_env_var("POSTGRES_DB", "postgres")
                .start()
                .await
                .expect("Failed to start pgvector container");

            CONTAINER_ID.set(container.id().to_string()).ok();

            let host_port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("Failed to get mapped port");

            let base_url = format!("postgresql://postgres:postgres@127.0.0.1:{}", host_port);
            let admin_url = format!("{}/postgres", base_url);

            create_template_database(&admin_url).await;

            TestInfra {
                _container: container,
                admin_url,
                base_url,
            }
        })
        .await
}

/// Force-remove the testcontainer on normal process exit.
/// The `watchdog` feature handles signal-based kills (SIGTERM/SIGINT/SIGQUIT).
#[ctor::dtor]
unsafe fn cleanup() {
    if let Some(id) = CONTAINER_ID.get() {
        match std::process::Command::new("docker")
            .args(["rm", "-fv", id])
            .output()
        {
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                eprintln!(
                    "Failed to remove test container {}: docker rm exited with status {}. stderr: {} stdout: {}",
                    id,
                    output.status,
                    stderr.trim(),
                    stdout.trim()
                );
            }
            Ok(_) => {}
            Err(err) => {
                eprintln!("Failed to run docker rm for test container {}: {}", id, err);
            }
        }
    }
}

/// Create the template database with pgvector extension and migrations.
async fn create_template_database(admin_url: &str) {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(admin_url);
    let pool = Pool::builder()
        .max_size(1)
        .build(config)
        .await
        .expect("Failed to connect to admin database");

    let mut conn = pool.get().await.expect("Failed to get admin connection");

    diesel::sql_query("CREATE DATABASE template_tokenoverflow")
        .execute(&mut *conn)
        .await
        .expect("Failed to create template database");

    // Connect to the template database to install extension and run migrations.
    // Only replace the trailing database name, not "/postgres" in the authority.
    let template_url = format!(
        "{}/template_tokenoverflow",
        admin_url
            .rsplit_once('/')
            .expect("admin_url must have a path")
            .0
    );
    let template_config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&template_url);
    let template_pool = Pool::builder()
        .max_size(1)
        .build(template_config)
        .await
        .expect("Failed to connect to template database");

    let mut template_conn = template_pool
        .get()
        .await
        .expect("Failed to get template connection");

    // Create the tokenoverflow role so GRANT statements in migrations succeed.
    diesel::sql_query("CREATE ROLE tokenoverflow WITH LOGIN")
        .execute(&mut *conn)
        .await
        .expect("Failed to create tokenoverflow role");

    run_migrations(&mut template_conn).await;

    // Disconnect from template before marking it as a template
    drop(template_conn);
    drop(template_pool);

    // Mark as template so CREATE DATABASE ... TEMPLATE works
    diesel::sql_query(
        "UPDATE pg_database SET datistemplate = true WHERE datname = 'template_tokenoverflow'",
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to mark database as template");
}

/// Run all migration SQL files against a connection.
///
/// Discovers migrations dynamically by scanning the migrations directory,
/// sorted by directory name (which encodes timestamps) to ensure correct order.
async fn run_migrations(conn: &mut AsyncPgConnection) {
    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");

    let mut migration_dirs: Vec<_> = std::fs::read_dir(&migrations_dir)
        .unwrap_or_else(|e| panic!("Failed to read migrations dir {:?}: {}", migrations_dir, e))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() && path.join("up.sql").exists() {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    migration_dirs.sort();

    for dir in migration_dirs {
        let up_sql_path = dir.join("up.sql");
        let migration_sql = std::fs::read_to_string(&up_sql_path)
            .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", up_sql_path, e));

        for statement in split_sql_statements(&migration_sql) {
            let sql: String = statement
                .lines()
                .skip_while(|line| line.trim().is_empty() || line.trim().starts_with("--"))
                .collect::<Vec<_>>()
                .join("\n");

            let trimmed = sql.trim();
            if !trimmed.is_empty() {
                // ALTER DATABASE tokenoverflow targets the production DB name;
                // in testcontainers the DB has a different name so we skip it.
                if trimmed.starts_with("ALTER DATABASE") {
                    let _ = diesel::sql_query(trimmed).execute(conn).await;
                    continue;
                }
                diesel::sql_query(trimmed)
                    .execute(conn)
                    .await
                    .unwrap_or_else(|e| {
                        panic!("Migration statement failed: {}\nSQL: {}", e, trimmed)
                    });
            }
        }
    }
}

/// Split SQL text on `;` while preserving `$$`-quoted blocks intact.
fn split_sql_statements(sql: &str) -> Vec<&str> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut in_dollar_quote = false;
    let bytes = sql.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'$' {
            in_dollar_quote = !in_dollar_quote;
            i += 2;
        } else if bytes[i] == b';' && !in_dollar_quote {
            statements.push(&sql[start..i]);
            start = i + 1;
            i += 1;
        } else {
            i += 1;
        }
    }

    if start < bytes.len() {
        statements.push(&sql[start..]);
    }

    statements
}

/// A per-test database cloned from the template.
///
/// Each instance owns a connection pool to a unique database that was created
/// with `CREATE DATABASE test_N TEMPLATE template_tokenoverflow`.
pub struct IntegrationTestDb {
    pool: DbPool,
}

impl IntegrationTestDb {
    /// Create a new isolated test database from the template.
    pub async fn new() -> Self {
        let infra = get_infra().await;

        let db_index = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!("test_{}", db_index);

        // Use admin connection to create the per-test database
        let admin_config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&infra.admin_url);
        let admin_pool = Pool::builder()
            .max_size(1)
            .build(admin_config)
            .await
            .expect("Failed to connect for database creation");

        let mut admin_conn = admin_pool
            .get()
            .await
            .expect("Failed to get admin connection");

        diesel::sql_query(format!(
            "CREATE DATABASE {} TEMPLATE template_tokenoverflow",
            db_name
        ))
        .execute(&mut *admin_conn)
        .await
        .unwrap_or_else(|e| panic!("Failed to create test database {}: {}", db_name, e));

        drop(admin_conn);
        drop(admin_pool);

        // Connect to the per-test database
        let db_url = format!("{}/{}", infra.base_url, db_name);
        let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&db_url);
        let pool = Pool::builder()
            .max_size(2)
            .build(config)
            .await
            .unwrap_or_else(|e| panic!("Failed to create pool for {}: {}", db_name, e));

        IntegrationTestDb { pool }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
