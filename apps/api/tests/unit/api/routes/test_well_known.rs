use axum::Router;
use axum::routing::get;
use tokenoverflow::api::routes::well_known::{
    oauth_authorization_server, oauth_protected_resource,
};

mod common {
    include!("../../../common/mod.rs");
}

use common::{get_request, read_json};

#[tokio::test]
async fn protected_resource_points_to_api_base_url() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource),
        )
        .with_state(app_state);

    let response = get_request(app, "/.well-known/oauth-protected-resource").await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    let servers = json["authorization_servers"]
        .as_array()
        .expect("authorization_servers should be an array");

    assert_eq!(servers.len(), 1);
    // Mock state uses api_base_url = "http://localhost:8080"
    assert_eq!(servers[0], "http://localhost:8080");
}

#[tokio::test]
async fn protected_resource_returns_api_base_url_as_resource() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route(
            "/.well-known/oauth-protected-resource",
            get(oauth_protected_resource),
        )
        .with_state(app_state);

    let response = get_request(app, "/.well-known/oauth-protected-resource").await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    assert_eq!(json["resource"], "http://localhost:8080");
}

#[tokio::test]
async fn authorization_server_metadata_has_proxy_urls() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server),
        )
        .with_state(app_state);

    let response = get_request(app, "/.well-known/oauth-authorization-server").await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    // All three proxy endpoints should point to the API base URL
    assert_eq!(
        json["authorization_endpoint"],
        "http://localhost:8080/oauth2/authorize"
    );
    assert_eq!(json["token_endpoint"], "http://localhost:8080/oauth2/token");
    assert_eq!(
        json["registration_endpoint"],
        "http://localhost:8080/oauth2/register"
    );
}

#[tokio::test]
async fn authorization_server_metadata_has_authkit_issuer() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server),
        )
        .with_state(app_state);

    let response = get_request(app, "/.well-known/oauth-authorization-server").await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    // Issuer should point to authkit_url (tokens are issued by AuthKit)
    assert_eq!(json["issuer"], "http://localhost:8080");
}

#[tokio::test]
async fn authorization_server_metadata_has_authkit_jwks() {
    let app_state = common::create_mock_app_state();
    let app: Router = Router::new()
        .route(
            "/.well-known/oauth-authorization-server",
            get(oauth_authorization_server),
        )
        .with_state(app_state);

    let response = get_request(app, "/.well-known/oauth-authorization-server").await;

    assert_eq!(response.status().as_u16(), 200);

    let json = read_json(response).await;
    // JWKS should point to authkit_url (keys belong to AuthKit)
    assert_eq!(json["jwks_uri"], "http://localhost:8080/oauth2/jwks");
}
