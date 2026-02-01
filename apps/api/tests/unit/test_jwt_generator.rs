use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode};

mod common {
    include!("../common/mod.rs");
}

use common::test_jwt::{
    generate_expired_test_jwt, generate_test_jwt, generate_test_jwt_custom,
    generate_test_jwt_with_kid,
};

/// Load the test JWKS and extract the DecodingKey.
fn test_decoding_key() -> DecodingKey {
    let jwks_bytes = include_bytes!("../assets/auth/test_jwks.json");
    let jwks: serde_json::Value = serde_json::from_slice(jwks_bytes).unwrap();
    let key = &jwks["keys"][0];

    DecodingKey::from_rsa_components(key["n"].as_str().unwrap(), key["e"].as_str().unwrap())
        .expect("test JWKS must produce a valid decoding key")
}

fn test_validation() -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["tokenoverflow-test"]);
    validation.set_audience(&["http://localhost:8080"]);
    validation.set_required_spec_claims(&["sub", "iss", "aud", "exp"]);
    validation
}

#[test]
fn generate_test_jwt_produces_valid_token() {
    let token = generate_test_jwt("user_test123", 3600);

    let key = test_decoding_key();
    let validation = test_validation();

    let token_data: TokenData<serde_json::Value> =
        decode(&token, &key, &validation).expect("token should validate against test JWKS");

    assert_eq!(token_data.claims["sub"], "user_test123");
    assert_eq!(token_data.claims["iss"], "tokenoverflow-test");
    assert_eq!(token_data.claims["aud"], "http://localhost:8080");
    assert!(token_data.claims["exp"].is_number());
    assert!(token_data.claims["iat"].is_number());
}

#[test]
fn generate_test_jwt_uses_correct_kid() {
    let token = generate_test_jwt("user_kid_test", 3600);

    // Decode header without verification to check kid
    let header = jsonwebtoken::decode_header(&token).expect("header should decode");
    assert_eq!(header.kid, Some("test-key-1".to_string()));
    assert_eq!(header.alg, Algorithm::RS256);
}

#[test]
fn generate_test_jwt_custom_uses_custom_claims() {
    let token = generate_test_jwt_custom("user_custom", "custom-issuer", "custom-audience", 3600);

    let key = test_decoding_key();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&["custom-issuer"]);
    validation.set_audience(&["custom-audience"]);
    validation.set_required_spec_claims(&["sub", "iss", "aud", "exp"]);

    let token_data: TokenData<serde_json::Value> =
        decode(&token, &key, &validation).expect("custom token should validate");

    assert_eq!(token_data.claims["sub"], "user_custom");
    assert_eq!(token_data.claims["iss"], "custom-issuer");
    assert_eq!(token_data.claims["aud"], "custom-audience");
}

#[test]
fn generate_test_jwt_with_kid_uses_specified_kid() {
    let token = generate_test_jwt_with_kid("user_test", "my-custom-kid", 3600);

    let header = jsonwebtoken::decode_header(&token).expect("header should decode");
    assert_eq!(header.kid, Some("my-custom-kid".to_string()));
}

#[test]
fn generate_expired_test_jwt_is_rejected() {
    let token = generate_expired_test_jwt("user_expired");

    let key = test_decoding_key();
    let validation = test_validation();

    let result = decode::<serde_json::Value>(&token, &key, &validation);
    assert!(result.is_err(), "expired token should be rejected");
}

#[test]
fn token_with_wrong_issuer_is_rejected() {
    let token = generate_test_jwt_custom(
        "user_wrong_iss",
        "wrong-issuer",
        "http://localhost:8080",
        3600,
    );

    let key = test_decoding_key();
    let validation = test_validation(); // expects tokenoverflow-test issuer

    let result = decode::<serde_json::Value>(&token, &key, &validation);
    assert!(
        result.is_err(),
        "token with wrong issuer should be rejected"
    );
}

#[test]
fn token_with_wrong_audience_is_rejected() {
    let token = generate_test_jwt_custom(
        "user_wrong_aud",
        "tokenoverflow-test",
        "wrong-audience",
        3600,
    );

    let key = test_decoding_key();
    let validation = test_validation(); // expects http://localhost:8080 audience

    let result = decode::<serde_json::Value>(&token, &key, &validation);
    assert!(
        result.is_err(),
        "token with wrong audience should be rejected"
    );
}
