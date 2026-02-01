use std::path::PathBuf;

use tokenoverflow::config::AuthConfig;
use tokenoverflow::services::AuthService;

mod common {
    include!("../../common/mod.rs");
}

use common::test_jwt::{
    generate_expired_test_jwt, generate_test_jwt, generate_test_jwt_custom,
    generate_test_jwt_with_kid,
};

fn test_auth_config() -> AuthConfig {
    let jwks_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/assets/auth/test_jwks.json")
        .to_string_lossy()
        .to_string();

    AuthConfig::new(
        "client_test".to_string(),
        "http://localhost:8080".to_string(),
        format!("file://{}", jwks_path),
        0,
        "tokenoverflow-test".to_string(),
        vec!["http://localhost:8080".to_string()],
        "http://localhost:8080".to_string(),
        "http://localhost:8080".to_string(),
    )
}

#[tokio::test]
async fn validate_jwt_accepts_valid_token() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt("user_valid", 3600);

    let claims = service
        .validate_jwt(&token)
        .await
        .expect("valid token should be accepted");

    assert_eq!(claims.sub, "user_valid");
}

#[tokio::test]
async fn validate_jwt_rejects_expired_token() {
    let service = AuthService::new(test_auth_config());
    let token = generate_expired_test_jwt("user_expired");

    let result = service.validate_jwt(&token).await;

    assert!(result.is_err(), "expired token should be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Unauthorized"),
        "error should be Unauthorized, got: {}",
        err
    );
}

#[tokio::test]
async fn validate_jwt_rejects_wrong_issuer() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt_custom(
        "user_wrong_iss",
        "wrong-issuer",
        "http://localhost:8080",
        3600,
    );

    let result = service.validate_jwt(&token).await;

    assert!(
        result.is_err(),
        "token with wrong issuer should be rejected"
    );
}

#[tokio::test]
async fn validate_jwt_rejects_wrong_audience() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt_custom(
        "user_wrong_aud",
        "tokenoverflow-test",
        "https://wrong-audience.com",
        3600,
    );

    let result = service.validate_jwt(&token).await;

    assert!(
        result.is_err(),
        "token with wrong audience should be rejected"
    );
}

#[tokio::test]
async fn validate_jwt_rejects_unknown_kid() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt_with_kid("user_unknown_kid", "nonexistent-key", 3600);

    let result = service.validate_jwt(&token).await;

    assert!(result.is_err(), "token with unknown kid should be rejected");
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("Unknown signing key"),
        "error should mention unknown key, got: {}",
        err
    );
}

#[tokio::test]
async fn validate_jwt_rejects_malformed_token() {
    let service = AuthService::new(test_auth_config());

    let result = service.validate_jwt("not-a-jwt").await;

    assert!(result.is_err(), "malformed token should be rejected");
}

#[tokio::test]
async fn validate_jwt_extracts_sub_claim() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt("user_workos_12345", 3600);

    let claims = service
        .validate_jwt(&token)
        .await
        .expect("valid token should be accepted");

    assert_eq!(claims.sub, "user_workos_12345");
    assert_eq!(claims.iss, "tokenoverflow-test");
}

#[tokio::test]
async fn file_protocol_jwks_loads_successfully() {
    let service = AuthService::new(test_auth_config());
    let token = generate_test_jwt("user_file_jwks", 3600);

    // This implicitly tests file:// JWKS loading
    let result = service.validate_jwt(&token).await;
    assert!(
        result.is_ok(),
        "file:// JWKS should load and validate successfully"
    );
}

#[tokio::test]
async fn invalid_jwks_path_returns_error() {
    let mut config = test_auth_config();
    config.set_jwks_url("file:///nonexistent/path/jwks.json".to_string());

    let service = AuthService::new(config);
    let token = generate_test_jwt("user_bad_path", 3600);

    let result = service.validate_jwt(&token).await;
    assert!(result.is_err(), "invalid JWKS path should return error");
}
