use std::sync::Mutex;

use so_tag_sync::config::get_database_url;

// Config tests must run serially because they mutate env vars.
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn clear_env() {
    // SAFETY: env var mutation in test context, protected by ENV_LOCK
    unsafe {
        std::env::remove_var("TOKENOVERFLOW_ENV");
        std::env::remove_var("TOKENOVERFLOW_DATABASE_URL");
        std::env::remove_var("TOKENOVERFLOW_DATABASE_PASSWORD");
        std::env::remove_var("TOKENOVERFLOW__DATABASE__HOST");
        std::env::remove_var("TOKENOVERFLOW__DATABASE__PORT");
        std::env::remove_var("TOKENOVERFLOW__DATABASE__NAME");
        std::env::remove_var("TOKENOVERFLOW__DATABASE__USER");
        std::env::set_var("TOKENOVERFLOW_CONFIG_DIR", "/nonexistent/path");
    }
}

fn restore_env() {
    unsafe {
        std::env::remove_var("TOKENOVERFLOW_CONFIG_DIR");
        std::env::remove_var("TOKENOVERFLOW_DATABASE_URL");
        std::env::remove_var("TOKENOVERFLOW_DATABASE_PASSWORD");
    }
}

#[test]
fn missing_config_returns_default_url() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env();

    let url = get_database_url().unwrap();
    assert_eq!(
        url,
        "postgresql://tokenoverflow@localhost:6432/tokenoverflow"
    );

    restore_env();
}

#[test]
fn explicit_database_url_takes_priority() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env();
    unsafe {
        std::env::set_var(
            "TOKENOVERFLOW_DATABASE_URL",
            "postgresql://custom@myhost:9999/mydb",
        );
    }

    let url = get_database_url().unwrap();
    assert_eq!(url, "postgresql://custom@myhost:9999/mydb");

    restore_env();
}

#[test]
fn password_env_var_included_in_url() {
    let _lock = ENV_LOCK.lock().unwrap();
    clear_env();
    unsafe {
        std::env::set_var("TOKENOVERFLOW_DATABASE_PASSWORD", "secret123");
    }

    let url = get_database_url().unwrap();
    assert_eq!(
        url,
        "postgresql://tokenoverflow:secret123@localhost:6432/tokenoverflow"
    );

    restore_env();
}
