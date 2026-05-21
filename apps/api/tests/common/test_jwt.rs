#![allow(dead_code)]

use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

/// Default email claim for tests that do not need a custom value.
const TEST_EMAIL: &str = "test@example.test";

/// Generate a test JWT signed with the test private key.
///
/// The token uses RS256 and the `test-key-1` kid matching the test JWKS.
/// Claims match the test config: issuer=`tokenoverflow-test`,
/// audience=`http://localhost:8080`, plus the `email` claim that the
/// API requires (sourced in prod from the AuthKit `email` scope).
pub fn generate_test_jwt(sub: &str, expires_in_secs: u64) -> String {
    generate_test_jwt_inner(
        sub,
        Some(TEST_EMAIL),
        "tokenoverflow-test",
        "http://localhost:8080",
        "test-key-1",
        expires_in_secs,
        0,
    )
}

/// Generate a test JWT with custom issuer and audience.
pub fn generate_test_jwt_custom(
    sub: &str,
    issuer: &str,
    audience: &str,
    expires_in_secs: u64,
) -> String {
    generate_test_jwt_inner(
        sub,
        Some(TEST_EMAIL),
        issuer,
        audience,
        "test-key-1",
        expires_in_secs,
        0,
    )
}

/// Generate a test JWT with a custom kid.
pub fn generate_test_jwt_with_kid(sub: &str, kid: &str, expires_in_secs: u64) -> String {
    generate_test_jwt_inner(
        sub,
        Some(TEST_EMAIL),
        "tokenoverflow-test",
        "http://localhost:8080",
        kid,
        expires_in_secs,
        0,
    )
}

/// Generate a test JWT that is already expired.
pub fn generate_expired_test_jwt(sub: &str) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).expect("test private key must be valid PEM");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());

    let now = jsonwebtoken::get_current_timestamp();
    let claims = serde_json::json!({
        "sub": sub,
        "email": TEST_EMAIL,
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": now - 3600, // expired 1 hour ago
        "iat": now - 7200,
    });

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
}

/// Generate a test JWT without the `email` claim. Used to assert the
/// API surfaces 401 when AuthKit issues a token from an OAuth flow that
/// failed to request the `email` scope.
pub fn generate_test_jwt_without_email(sub: &str, expires_in_secs: u64) -> String {
    generate_test_jwt_inner(
        sub,
        None,
        "tokenoverflow-test",
        "http://localhost:8080",
        "test-key-1",
        expires_in_secs,
        0,
    )
}

fn generate_test_jwt_inner(
    sub: &str,
    email: Option<&str>,
    issuer: &str,
    audience: &str,
    kid: &str,
    expires_in_secs: u64,
    issued_offset_secs: i64,
) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).expect("test private key must be valid PEM");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());

    let now = jsonwebtoken::get_current_timestamp() as i64;
    let iat = now + issued_offset_secs;
    let exp = iat + expires_in_secs as i64;

    let mut claims = serde_json::json!({
        "sub": sub,
        "iss": issuer,
        "aud": audience,
        "exp": exp,
        "iat": iat,
    });
    if let Some(value) = email {
        claims["email"] = serde_json::Value::String(value.to_string());
    }

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
}
