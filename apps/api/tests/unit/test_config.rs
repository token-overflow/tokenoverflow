use std::env;
use std::path::PathBuf;
use std::sync::Mutex;
use tokenoverflow::config::Config;

// Mutex to ensure config tests run serially since they modify env vars
static ENV_MUTEX: Mutex<()> = Mutex::new(());

fn get_config_dir() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("config")
        .to_string_lossy()
        .to_string()
}

fn clean_env() {
    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        // Clean all TOKENOVERFLOW_* env vars to ensure test isolation
        env::remove_var("TOKENOVERFLOW_ENV");
        env::remove_var("TOKENOVERFLOW_CONFIG_DIR");
        env::remove_var("TOKENOVERFLOW_DATABASE_PASSWORD");
        env::remove_var("TOKENOVERFLOW_EMBEDDING_API_KEY");
        env::remove_var("TOKENOVERFLOW_WORKOS_API_KEY");
        // Clean env var overrides (double underscore separator)
        env::remove_var("TOKENOVERFLOW__DATABASE__HOST");
        env::remove_var("TOKENOVERFLOW__DATABASE__PORT");
        env::remove_var("TOKENOVERFLOW__EMBEDDING__BASE_URL");
    }
}

#[test]
fn test_load_unit_test_config() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.api.host, "127.0.0.1");
    assert_eq!(config.api.port, 8080);
    assert_eq!(config.api.base_url, "http://127.0.0.1:8080");
    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.database.port, 5432);
    assert_eq!(config.database.user, "tokenoverflow");
    assert_eq!(config.database.name, "tokenoverflow");
    assert!(config.embedding.base_url.is_none());
    assert_eq!(config.embedding.model, "voyage-code-3");
    assert_eq!(config.embedding.output_dimension, 256);
    assert_eq!(config.logging.level, "debug");
}

#[test]
fn test_load_local_config() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "local");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.api.host, "0.0.0.0");
    assert_eq!(config.api.port, 8080);
    assert_eq!(config.api.base_url, "http://localhost:8080");
    assert_eq!(config.database.host, "localhost");
    assert_eq!(config.database.port, 6432);
    assert_eq!(config.database.user, "tokenoverflow");
    assert_eq!(config.database.name, "tokenoverflow");
    assert_eq!(
        config.embedding.base_url,
        Some("http://localhost:3001/v1".to_string())
    );
    assert_eq!(config.embedding.model, "voyage-code-3");
    assert_eq!(config.embedding.output_dimension, 256);
    assert_eq!(config.logging.level, "debug");
}

#[test]
fn test_embedding_api_key_from_env() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
        env::set_var("TOKENOVERFLOW_EMBEDDING_API_KEY", "voy-test-key-123");
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.embedding.api_key(), Some("voy-test-key-123"));
}

#[test]
fn test_embedding_api_key_none_when_unset() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert!(config.embedding.api_key().is_none());
}

#[test]
fn test_database_url_without_password() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(
        config.database.url(),
        "postgres://tokenoverflow:@localhost:5432/tokenoverflow"
    );
}

#[test]
fn test_database_url_with_password() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
        env::set_var("TOKENOVERFLOW_DATABASE_PASSWORD", "secret123");
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(
        config.database.url(),
        "postgres://tokenoverflow:secret123@localhost:5432/tokenoverflow"
    );
}

#[test]
fn test_default_env_is_local() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        // Don't set TOKENOVERFLOW_ENV - should default to local
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    // Should load local.toml by default
    assert_eq!(config.api.base_url, "http://localhost:8080");
    assert_eq!(config.database.port, 6432);
}

#[test]
fn test_default_config_dir_fallback() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "local");
        // Intentionally NOT setting TOKENOVERFLOW_CONFIG_DIR so it falls
        // back to the default "config" relative path, which resolves to
        // the crate-root config/ directory.
    }

    let config = Config::load().expect("Should load from default config dir");
    assert_eq!(config.api.host, "0.0.0.0");
}

#[test]
fn test_missing_config_file_returns_error() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "nonexistent_env");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let result = Config::load();
    assert!(
        result.is_err(),
        "Expected error when config file is missing"
    );
}

#[test]
fn test_invalid_config_schema_returns_error() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // Write a TOML file that parses but doesn't match Config's schema
    let dir = env::temp_dir().join("tokenoverflow_config_test");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.toml"), "[wrong_section]\nkey = \"value\"").unwrap();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "bad");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", dir.to_str().unwrap());
    }

    let result = Config::load();
    assert!(
        result.is_err(),
        "Expected deserialization error for invalid schema"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_request_timeout_secs_loaded() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.api.request_timeout_secs, 30);
}

#[test]
fn test_env_var_override() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "local");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
        // Override database host via env var
        env::set_var("TOKENOVERFLOW__DATABASE__HOST", "custom-host");
    }

    let config = Config::load().expect("Failed to load config");

    // Should use env var override instead of TOML value
    assert_eq!(config.database.host, "custom-host");
    // Other values should still come from local.toml
    assert_eq!(config.database.port, 6432);
}

#[test]
fn test_auth_config_loaded_from_unit_test_toml() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.auth.workos_client_id, "client_test");
    assert_eq!(config.auth.workos_api_url, "http://localhost:8080");
    assert_eq!(
        config.auth.jwks_url,
        "file://tests/assets/auth/test_jwks.json"
    );
    assert_eq!(config.auth.jwks_cache_ttl_secs, 0);
    assert_eq!(config.auth.issuer, "tokenoverflow-test");
    assert_eq!(config.auth.audiences, vec!["http://localhost:8080"]);
}

#[test]
fn test_auth_config_file_protocol_accepted() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "local");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert!(
        config.auth.jwks_url.starts_with("file://"),
        "Local config should use file:// protocol for JWKS"
    );
}

#[test]
fn test_workos_api_key_from_env() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
        env::set_var("TOKENOVERFLOW_WORKOS_API_KEY", "sk_test_key_123");
    }

    let config = Config::load().expect("Failed to load config");

    assert_eq!(config.auth.workos_api_key(), Some("sk_test_key_123"));
}

#[test]
fn test_workos_api_key_none_when_unset() {
    let _lock = ENV_MUTEX.lock().unwrap();
    clean_env();

    // SAFETY: env vars are protected by mutex in this test binary
    unsafe {
        env::set_var("TOKENOVERFLOW_ENV", "unit_test");
        env::set_var("TOKENOVERFLOW_CONFIG_DIR", get_config_dir());
    }

    let config = Config::load().expect("Failed to load config");

    assert!(config.auth.workos_api_key().is_none());
}
