#![allow(dead_code)]

use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

/// Generate a test JWT signed with the test private key.
///
/// The token uses RS256 and the `test-key-1` kid matching the test JWKS.
/// Claims match the test config: issuer=`tokenoverflow-test`, audience=`http://localhost:8080`.
pub fn generate_test_jwt(sub: &str, expires_in_secs: u64) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).expect("test private key must be valid PEM");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());

    let now = jsonwebtoken::get_current_timestamp();
    let claims = serde_json::json!({
        "sub": sub,
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": now + expires_in_secs,
        "iat": now,
    });

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
}

/// Generate a test JWT with custom issuer and audience.
pub fn generate_test_jwt_custom(
    sub: &str,
    issuer: &str,
    audience: &str,
    expires_in_secs: u64,
) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).expect("test private key must be valid PEM");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("test-key-1".to_string());

    let now = jsonwebtoken::get_current_timestamp();
    let claims = serde_json::json!({
        "sub": sub,
        "iss": issuer,
        "aud": audience,
        "exp": now + expires_in_secs,
        "iat": now,
    });

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
}

/// Generate a test JWT with a custom kid.
pub fn generate_test_jwt_with_kid(sub: &str, kid: &str, expires_in_secs: u64) -> String {
    let private_key = include_bytes!("../assets/auth/test_private_key.pem");
    let key = EncodingKey::from_rsa_pem(private_key).expect("test private key must be valid PEM");

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());

    let now = jsonwebtoken::get_current_timestamp();
    let claims = serde_json::json!({
        "sub": sub,
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": now + expires_in_secs,
        "iat": now,
    });

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
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
        "iss": "tokenoverflow-test",
        "aud": "http://localhost:8080",
        "exp": now - 3600, // expired 1 hour ago
        "iat": now - 7200,
    });

    encode(&header, &claims, &key).expect("JWT encoding must succeed with test key")
}
