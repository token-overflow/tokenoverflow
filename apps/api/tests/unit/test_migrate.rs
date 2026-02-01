use tokenoverflow::migrate;

#[test]
fn test_run_pending_migrations_invalid_url_returns_error() {
    let result =
        migrate::run_pending_migrations("postgres://invalid:invalid@localhost:1/nonexistent");
    assert!(
        result.is_err(),
        "Expected error when connecting to invalid database URL"
    );
}

#[test]
fn test_migrations_are_embedded() {
    // Verify that MIGRATIONS constant compiles and is accessible.
    // If embed_migrations!() fails to find the migrations directory,
    // this module would not compile at all.
    let _migrations = migrate::MIGRATIONS;
}
