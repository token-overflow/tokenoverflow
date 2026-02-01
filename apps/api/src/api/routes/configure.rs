use axum::Router;
use axum::routing::{get, post};

use crate::api::middleware;
use crate::api::routes::answers;
use crate::api::routes::health;
use crate::api::routes::oauth_proxy;
use crate::api::routes::questions;
use crate::api::routes::search;
use crate::api::routes::well_known;
use crate::api::state::AppState;

/// Configure all routes for the application.
///
/// Public routes (no auth): /health, /.well-known/*, /oauth2/*
/// Protected routes (jwt_auth middleware): /v1/*
///
// Declarative .route() wiring — no branching logic to test.
// E2E: tests/e2e/api/routes/ exercises every route end-to-end.
#[cfg_attr(coverage_nightly, coverage(off))]
pub fn configure(state: AppState) -> Router<AppState> {
    // Protected routes require JWT authentication.
    // State is passed to the middleware so it can validate JWTs and resolve users.
    let protected = Router::new()
        .route("/v1/search", post(search::search))
        .route("/v1/questions", post(questions::create_question))
        .route("/v1/questions/{id}", get(questions::get_question))
        .route("/v1/questions/{id}/answers", post(questions::add_answer))
        .route("/v1/answers/{id}/upvote", post(answers::upvote))
        .route("/v1/answers/{id}/downvote", post(answers::downvote))
        .route_layer(axum::middleware::from_fn_with_state(
            state,
            middleware::jwt_auth_layer,
        ));

    let public = Router::new()
        .route("/health", get(health::health_check))
        .route(
            "/.well-known/oauth-protected-resource",
            get(well_known::oauth_protected_resource),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            get(well_known::oauth_authorization_server),
        )
        .route("/oauth2/authorize", get(oauth_proxy::authorize))
        .route("/oauth2/token", post(oauth_proxy::token))
        .route("/oauth2/register", post(oauth_proxy::register));

    public.merge(protected)
}
