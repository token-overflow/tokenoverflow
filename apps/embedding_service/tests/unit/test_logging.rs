use embedding_service::logging;

#[test]
fn init_is_idempotent() {
    // init() should be safe to call multiple times
    logging::init();
    logging::init();
    // No panic means success
}

#[test]
fn try_init_returns_error_on_second_call() {
    // First call might succeed or fail depending on test order
    let first_result = logging::try_init();

    // Second call should always return an error (already initialized)
    let second_result = logging::try_init();

    // If first succeeded, second must fail
    // If first failed (another test initialized), second also fails
    if first_result.is_ok() {
        assert!(
            second_result.is_err(),
            "Second try_init should fail after successful first init"
        );
    }
}
