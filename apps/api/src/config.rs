use config::{Config as ConfigBuilder, ConfigError, Environment, File};
use serde::Deserialize;
use std::env;

/// Application configuration loaded from TOML files with secrets from env vars.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub api: ApiConfig,
    pub auth: AuthConfig,
    pub database: DatabaseConfig,
    pub embedding: EmbeddingConfig,
    pub logging: LoggingConfig,
    pub mcp: McpConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub base_url: String,
    #[serde(default = "default_request_timeout_secs")]
    pub request_timeout_secs: u64,
}

fn default_request_timeout_secs() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub workos_client_id: String,
    pub workos_api_url: String,
    pub jwks_url: String,
    #[serde(default = "default_jwks_cache_ttl")]
    pub jwks_cache_ttl_secs: u64,
    pub issuer: String,
    /// Accepted JWT audience values.
    ///
    /// WorkOS AuthKit sets `aud` to the environment-level client ID, not
    /// the canonical resource URI, because it does not support RFC 8707.
    /// See the comment in `auth.rs:try_validate_with_kid` for details.
    pub audiences: Vec<String>,
    pub authkit_url: String,
    pub github_api_url: String,
    #[serde(default)]
    pub github_client_id: Option<String>,
    #[serde(skip_deserializing)]
    workos_api_key: Option<String>,
    #[serde(skip_deserializing)]
    github_client_secret: Option<String>,
}

fn default_jwks_cache_ttl() -> u64 {
    3600
}

impl AuthConfig {
    pub fn workos_api_key(&self) -> Option<&str> {
        self.workos_api_key.as_deref()
    }

    /// Returns both client_id and client_secret if both are configured.
    pub fn github_oauth_credentials(&self) -> Option<(&str, &str)> {
        match (
            self.github_client_id.as_deref(),
            self.github_client_secret.as_deref(),
        ) {
            (Some(id), Some(secret)) => Some((id, secret)),
            _ => None,
        }
    }

    /// Create an AuthConfig programmatically without TOML deserialization.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        workos_client_id: String,
        workos_api_url: String,
        jwks_url: String,
        jwks_cache_ttl_secs: u64,
        issuer: String,
        audiences: Vec<String>,
        authkit_url: String,
        github_api_url: String,
    ) -> Self {
        Self {
            workos_client_id,
            workos_api_url,
            jwks_url,
            jwks_cache_ttl_secs,
            issuer,
            audiences,
            authkit_url,
            github_api_url,
            github_client_id: None,
            workos_api_key: None,
            github_client_secret: None,
        }
    }

    pub fn set_jwks_url(&mut self, url: String) {
        self.jwks_url = url;
    }

    pub fn set_workos_api_key_for_test(&mut self, key: String) {
        self.workos_api_key = Some(key);
    }

    pub fn set_github_oauth_for_test(&mut self, client_id: String, client_secret: String) {
        self.github_client_id = Some(client_id);
        self.github_client_secret = Some(client_secret);
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub name: String,
    #[serde(default)]
    password: Option<String>,
}

impl DatabaseConfig {
    pub fn url(&self) -> String {
        let password = self.password.as_deref().unwrap_or("");
        format!(
            "postgres://{}:{}@{}:{}/{}",
            self.user, password, self.host, self.port, self.name
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingConfig {
    pub base_url: Option<String>,
    pub model: String,
    pub output_dimension: u32,
    #[serde(skip_deserializing)]
    api_key: Option<String>,
}

impl EmbeddingConfig {
    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    pub base_url: String,
}

impl Config {
    /// Load configuration from TOML files based on TOKENOVERFLOW_ENV.
    ///
    /// Configuration is loaded in layers:
    /// 1. Environment-specific config (e.g., config/local.toml)
    /// 2. Optional local overrides (config/local.override.toml)
    /// 3. Environment variable overrides (TOKENOVERFLOW__SECTION__KEY)
    /// 4. Secrets from environment variables
    pub fn load() -> Result<Self, ConfigError> {
        let env_name = env::var("TOKENOVERFLOW_ENV").unwrap_or_else(|_| "local".into());

        let config_dir = env::var("TOKENOVERFLOW_CONFIG_DIR").unwrap_or_else(|_| "config".into());

        let mut config: Config = ConfigBuilder::builder()
            // Load environment-specific config
            .add_source(File::with_name(&format!("{}/{}", config_dir, env_name)))
            // Load local overrides (optional)
            .add_source(File::with_name(&format!("{}/local.override", config_dir)).required(false))
            // Load environment variable overrides (TOKENOVERFLOW__DATABASE__HOST, etc.)
            .add_source(
                Environment::with_prefix("TOKENOVERFLOW")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?
            .try_deserialize()?;

        // Load secrets from env vars (override toml value when set)
        if let Ok(pw) = env::var("TOKENOVERFLOW_DATABASE_PASSWORD") {
            config.database.password = Some(pw);
        }
        config.embedding.api_key = env::var("TOKENOVERFLOW_EMBEDDING_API_KEY").ok();
        config.auth.workos_api_key = env::var("TOKENOVERFLOW_WORKOS_API_KEY").ok();
        config.auth.github_client_secret = env::var("TOKENOVERFLOW_GITHUB_CLIENT_SECRET").ok();

        Ok(config)
    }
}
