use axum::Router;
use axum::error_handling::HandleErrorLayer;
use axum::extract::DefaultBodyLimit;
use http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use http::{Method, StatusCode};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower::timeout::TimeoutLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::info;

use crate::api::middleware;

use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::tower::{
    StreamableHttpServerConfig, StreamableHttpService,
};

use crate::api::routes;
use crate::api::state::AppState;
use crate::config::Config;
use crate::db;
use crate::external::embedding::VoyageClient;
use crate::mcp::TokenOverflowServer;
use crate::services::TagResolver;
use crate::services::auth::create_auth_service;
use crate::services::repository::{
    PgAnswerRepository, PgQuestionRepository, PgSearchRepository, PgTagRepository, PgUserRepository,
};

// Tokio runtime bootstrap — needs a running server to exercise.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async_run())
}

// Server startup: config loading, TCP bind, and service wiring.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn async_run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = Config::load()?;
    let app_state = create_app_state(&config).await?;

    let mcp_app_state = app_state.clone();
    let mcp_config = StreamableHttpServerConfig::default()
        .with_stateful_mode(false)
        .with_json_response(true);
    let mcp_service = StreamableHttpService::new(
        move || Ok(TokenOverflowServer::new(mcp_app_state.clone())),
        Arc::new(NeverSessionManager::default()),
        mcp_config,
    );

    let timeout = std::time::Duration::from_secs(config.api.request_timeout_secs);

    // MCP sub-router: jwt_auth_layer enforces Bearer token on /mcp.
    // Separate from the /v1/* protected routes because nest_service requires
    // its own Router to attach route_layer middleware.
    let mcp_router = Router::new().nest_service("/mcp", mcp_service).route_layer(
        axum::middleware::from_fn_with_state(app_state.clone(), middleware::jwt_auth_layer),
    );

    let app = routes::configure(app_state.clone())
        .merge(mcp_router)
        .with_state(app_state)
        .layer(
            ServiceBuilder::new()
                // 1. Trace ID: extract from Lambda request context or generate UUID
                .layer(axum::middleware::from_fn(middleware::trace_id))
                // 2. HandleErrorLayer must be above TimeoutLayer so it can
                // convert Elapsed errors into HTTP 408 responses.
                .layer(HandleErrorLayer::new(|_: tower::BoxError| async {
                    StatusCode::REQUEST_TIMEOUT
                }))
                // 3. Timeout
                .layer(TimeoutLayer::new(timeout))
                // 4. Body size limit (100KB)
                .layer(DefaultBodyLimit::max(100 * 1024))
                // 5. Security headers
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::overriding(
                    http::header::X_FRAME_OPTIONS,
                    HeaderValue::from_static("DENY"),
                ))
                // 6. CORS
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods([Method::GET, Method::POST])
                        .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                        .max_age(Duration::from_secs(3600)),
                ),
        );

    if std::env::var("AWS_LAMBDA_RUNTIME_API").is_ok() {
        info!("Running in AWS Lambda mode");
        lambda_http::run(app).await?;
        Ok(())
    } else {
        let bind_addr = format!("{}:{}", config.api.host, config.api.port);
        info!("Starting server on {}", bind_addr);
        let listener = TcpListener::bind(&bind_addr).await?;
        serve_until_shutdown(listener, app, shutdown_signal()).await
    }
}

// Production state wiring: real DB pool + real embedding client + Pg repositories.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn create_app_state(
    config: &Config,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let pool = db::init(&config.database.url()).await?;
    let embedding = Arc::new(VoyageClient::new(
        config.embedding.base_url.as_deref(),
        &config.embedding.model,
        config.embedding.output_dimension,
        config.embedding.api_key().unwrap_or(""),
    )?);

    let questions = Arc::new(PgQuestionRepository);
    let answers = Arc::new(PgAnswerRepository);
    let search = Arc::new(PgSearchRepository);
    let tag_repo = Arc::new(PgTagRepository);
    let users = Arc::new(PgUserRepository);

    // Load tag cache at startup
    let tag_resolver = {
        let mut conn = pool.get().await?;
        Arc::new(TagResolver::new(tag_repo.as_ref(), &mut *conn).await?)
    };

    let auth = create_auth_service(&config.auth);

    Ok(AppState::new(
        pool,
        embedding,
        questions,
        answers,
        search,
        tag_repo,
        users,
        tag_resolver,
        auth,
        config.auth.clone(),
        config.api.base_url.clone(),
    ))
}

/// Run the server until the provided shutdown signal completes.
///
/// Separated from `async_run` so that tests can provide their own
/// shutdown trigger (e.g., a cancellation token) without needing real
/// OS signals.
pub async fn serve_until_shutdown(
    listener: TcpListener,
    app: axum::Router,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Wait for a shutdown signal (SIGTERM on Unix, Ctrl+C everywhere).
///
/// This function is not testable in unit/integration tests because it
/// blocks on OS signals. The graceful shutdown behavior is tested via
/// `serve_until_shutdown` with a synthetic shutdown future.
// OS signal listener -- cannot be exercised without sending real signals.
// E2E: tests/e2e/api/ exercises the full server via Docker Compose.
#[cfg_attr(coverage_nightly, coverage(off))]
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {
            info!("Received Ctrl+C, initiating graceful shutdown");
        }
        () = terminate => {
            info!("Received SIGTERM, initiating graceful shutdown");
        }
    }
}
