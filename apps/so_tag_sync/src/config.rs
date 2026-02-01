use anyhow::{Context, Result};

pub fn get_database_url() -> Result<String> {
    // Explicit URL override takes priority
    if let Ok(url) = std::env::var("TOKENOVERFLOW_DATABASE_URL") {
        return Ok(url);
    }

    let env = std::env::var("TOKENOVERFLOW_ENV").unwrap_or_else(|_| "local".to_string());
    let config_dir =
        std::env::var("TOKENOVERFLOW_CONFIG_DIR").unwrap_or_else(|_| "apps/api/config".to_string());

    let settings = config::Config::builder()
        .add_source(config::File::with_name(&format!("{}/{}", config_dir, env)).required(false))
        // Use __ separator to match the API app convention (TOKENOVERFLOW__DATABASE__HOST)
        .add_source(
            config::Environment::with_prefix("TOKENOVERFLOW")
                .separator("__")
                .try_parsing(true),
        )
        .build()
        .context("Failed to load configuration")?;

    let host = settings
        .get_string("database.host")
        .unwrap_or_else(|_| "localhost".to_string());
    let port = settings.get_int("database.port").unwrap_or(6432);
    let name = settings
        .get_string("database.name")
        .unwrap_or_else(|_| "tokenoverflow".to_string());
    let user = settings
        .get_string("database.user")
        .unwrap_or_else(|_| "tokenoverflow".to_string());
    // Env var takes priority, then toml config
    let password = std::env::var("TOKENOVERFLOW_DATABASE_PASSWORD")
        .or_else(|_| settings.get_string("database.password"))
        .unwrap_or_default();

    if password.is_empty() {
        Ok(format!("postgresql://{}@{}:{}/{}", user, host, port, name))
    } else {
        Ok(format!(
            "postgresql://{}:{}@{}:{}/{}",
            user, password, host, port, name
        ))
    }
}
