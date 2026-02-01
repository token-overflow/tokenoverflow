use std::time::Duration;
use tokio::net::TcpListener;

use tokenoverflow::api::server::serve_until_shutdown;

/// Verify that the server shuts down cleanly when the shutdown signal fires.
#[tokio::test]
async fn server_shuts_down_on_signal() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind");
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let server_handle = tokio::spawn(async move {
        let app = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));
        serve_until_shutdown(listener, app, async {
            rx.await.ok();
        })
        .await
        .expect("Server should shut down cleanly");
    });

    // Verify the server is accepting connections
    let client = reqwest::Client::new();
    let url = format!("http://{}/health", addr);

    // Wait briefly for the server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response = client
        .get(&url)
        .send()
        .await
        .expect("Request should succeed");
    assert!(response.status().is_success());

    // Trigger shutdown
    tx.send(()).expect("Shutdown signal should send");

    // Server should complete within a reasonable time
    tokio::time::timeout(Duration::from_secs(5), server_handle)
        .await
        .expect("Server should shut down within 5 seconds")
        .expect("Server task should not panic");
}
