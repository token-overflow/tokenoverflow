use tokenoverflow::logging;

#[test]
fn init_is_idempotent() {
    // init() should be safe to call multiple times
    logging::init("info");
    logging::init("info");
    // No panic means success
}

#[test]
fn try_init_returns_error_on_second_call() {
    // First call might succeed or fail depending on test order
    let first_result = logging::try_init("info");

    // Second call should always return an error (already initialized)
    let second_result = logging::try_init("info");

    // If first succeeded, second must fail
    // If first failed (another test initialized), second also fails
    if first_result.is_ok() {
        assert!(
            second_result.is_err(),
            "Second try_init should fail after successful first init"
        );
    }
}

#[test]
fn init_accepts_debug_level() {
    // Should not panic with "debug" level (used in production debug mode)
    logging::init("debug");
}

#[test]
fn init_accepts_warn_level() {
    // Should not panic with "warn" level (default production level)
    logging::init("warn");
}

#[test]
fn init_falls_back_on_invalid_level() {
    // Invalid level should fall back to INFO without panicking
    logging::init("not_a_real_level");
}
