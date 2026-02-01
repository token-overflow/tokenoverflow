//! Testcontainers-based database helper for so_tag_sync integration tests.
//!
//! Starts a single pgvector/pgvector:0.8.2-pg18 container and creates a template
//! database with the api schema and tag tables. Each test gets its own
//! database cloned from the template.

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

static DB_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEST_INFRA: OnceCell<TestInfra> = OnceCell::const_new();

/// Container ID for process-exit cleanup via `#[dtor]`.
static CONTAINER_ID: OnceLock<String> = OnceLock::new();

struct TestInfra {
    _container: ContainerAsync<GenericImage>,
    admin_url: String,
    base_url: String,
}

async fn get_infra() -> &'static TestInfra {
    TEST_INFRA
        .get_or_init(|| async {
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
            Ok(output) => {
                if !output.status.success() {
                    eprintln!(
                        "Failed to remove test container {} with `docker rm -f`: status={}, stderr={}",
                        id,
                        output.status,
                        std::string::String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
            Err(err) => {
                eprintln!(
                    "Failed to run `docker rm -f {}` during test cleanup: {}",
                    id, err
                );
            }
        }
    }
}

async fn create_template_database(admin_url: &str) {
    let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(admin_url);
    let pool = Pool::builder()
        .max_size(1)
        .build(config)
        .await
        .expect("Failed to connect to admin database");

    let mut conn = pool.get().await.expect("Failed to get admin connection");

    diesel::sql_query("CREATE DATABASE template_sync")
        .execute(&mut *conn)
        .await
        .expect("Failed to create template database");

    let template_url = format!(
        "{}/template_sync",
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

    // Create the api schema and tag tables (subset of the full migration)
    diesel::sql_query("CREATE EXTENSION IF NOT EXISTS vector")
        .execute(&mut *template_conn)
        .await
        .expect("Failed to create vector extension");

    diesel::sql_query("CREATE SCHEMA IF NOT EXISTS api")
        .execute(&mut *template_conn)
        .await
        .expect("Failed to create api schema");

    diesel::sql_query(
        "CREATE TABLE api.tags (
            id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
            name VARCHAR(35) UNIQUE NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(&mut *template_conn)
    .await
    .expect("Failed to create tags table");

    diesel::sql_query(
        "CREATE TABLE api.tag_synonyms (
            id UUID NOT NULL DEFAULT uuidv7() PRIMARY KEY,
            synonym VARCHAR(35) UNIQUE NOT NULL,
            tag_id UUID NOT NULL REFERENCES api.tags(id) ON DELETE CASCADE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )",
    )
    .execute(&mut *template_conn)
    .await
    .expect("Failed to create tag_synonyms table");

    drop(template_conn);
    drop(template_pool);

    diesel::sql_query(
        "UPDATE pg_database SET datistemplate = true WHERE datname = 'template_sync'",
    )
    .execute(&mut *conn)
    .await
    .expect("Failed to mark database as template");
}

/// A per-test database cloned from the template.
pub struct TestDb {
    pub url: String,
}

impl TestDb {
    pub async fn new() -> Self {
        let infra = get_infra().await;

        let db_index = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!("sync_test_{}", db_index);

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
            "CREATE DATABASE {} TEMPLATE template_sync",
            db_name
        ))
        .execute(&mut *admin_conn)
        .await
        .unwrap_or_else(|e| panic!("Failed to create test database {}: {}", db_name, e));

        drop(admin_conn);
        drop(admin_pool);

        let url = format!("{}/{}", infra.base_url, db_name);
        TestDb { url }
    }
}
