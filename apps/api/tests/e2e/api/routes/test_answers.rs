use http::StatusCode;
use serde_json::Value;

mod common {
    include!("../../../common/mod.rs");
}

use common::{QuestionRequestBuilder, TestClient};

/// Helper to create a question and return (question_id, answer_id)
async fn create_test_question(client: &TestClient) -> (String, String) {
    let req = QuestionRequestBuilder::new().build();

    let response = client.post("/v1/questions", &req).await;

    let body: Value = response.json();
    (
        body["question_id"].as_str().unwrap().to_string(),
        body["answer_id"].as_str().unwrap().to_string(),
    )
}

/// Helper to get answer vote counts from a question
async fn get_answer_votes(client: &TestClient, question_id: &str, answer_id: &str) -> (i64, i64) {
    let response = client.get(&format!("/v1/questions/{}", question_id)).await;

    let body: Value = response.json();
    let answers = body["answers"].as_array().unwrap();

    for answer in answers {
        if answer["id"].as_str().unwrap() == answer_id {
            return (
                answer["upvotes"].as_i64().unwrap(),
                answer["downvotes"].as_i64().unwrap(),
            );
        }
    }
    panic!("Answer not found");
}

// ============================================================================
// POST /v1/answers/{id}/upvote - Upvote Answer
// ============================================================================

#[tokio::test]
async fn upvote_increments_count() {
    let client = TestClient::from_config();
    let voter = TestClient::voter();

    let (question_id, answer_id) = create_test_question(&client).await;

    // Verify initial state
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 0);
    assert_eq!(downvotes, 0);

    // Upvote as a different user
    let response = voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["status"], "upvoted");

    // Verify vote count
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 1);
    assert_eq!(downvotes, 0);
}

#[tokio::test]
async fn upvote_is_idempotent() {
    let client = TestClient::from_config();
    let voter = TestClient::voter();

    let (question_id, answer_id) = create_test_question(&client).await;

    // Upvote twice as a different user
    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    // Verify still only 1 upvote (idempotent)
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 1);
    assert_eq!(downvotes, 0);
}

#[tokio::test]
async fn upvote_not_found_returns_404() {
    let client = common::TestClient::from_config();

    let response = client
        .post_empty("/v1/answers/00000000-0000-0000-0000-000000000099/upvote")
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upvote_invalid_id_returns_422() {
    let client = common::TestClient::from_config();

    let response = client.post_empty("/v1/answers/not-a-valid-id/upvote").await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// POST /v1/answers/{id}/downvote - Downvote Answer
// ============================================================================

#[tokio::test]
async fn downvote_increments_count() {
    let client = TestClient::from_config();
    let voter = TestClient::voter();

    let (question_id, answer_id) = create_test_question(&client).await;

    // Downvote as a different user
    let response = voter
        .post_empty(&format!("/v1/answers/{}/downvote", answer_id))
        .await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["status"], "downvoted");

    // Verify vote count
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 0);
    assert_eq!(downvotes, 1);
}

#[tokio::test]
async fn downvote_replaces_upvote() {
    let client = TestClient::from_config();
    let voter = TestClient::voter();

    let (question_id, answer_id) = create_test_question(&client).await;

    // First upvote as a different user
    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    // Verify upvote recorded
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 1);
    assert_eq!(downvotes, 0);

    // Then downvote (should replace the upvote)
    voter
        .post_empty(&format!("/v1/answers/{}/downvote", answer_id))
        .await;

    // Verify downvote replaced upvote
    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(upvotes, 0);
    assert_eq!(downvotes, 1);
}

#[tokio::test]
async fn downvote_not_found_returns_404() {
    let client = common::TestClient::from_config();

    let response = client
        .post_empty("/v1/answers/00000000-0000-0000-0000-000000000099/downvote")
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn downvote_invalid_id_returns_422() {
    let client = common::TestClient::from_config();

    let response = client
        .post_empty("/v1/answers/not-a-valid-id/downvote")
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// User Story: Voting Flow
// ============================================================================

#[tokio::test]
async fn voting_flow_is_idempotent() {
    let client = TestClient::from_config();
    let voter = TestClient::voter();

    let (question_id, answer_id) = create_test_question(&client).await;

    // Step 1: Upvote
    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!((upvotes, downvotes), (1, 0), "After first upvote");

    // Step 2: Upvote again (idempotent)
    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(
        (upvotes, downvotes),
        (1, 0),
        "After second upvote (idempotent)"
    );

    // Step 3: Change to downvote
    voter
        .post_empty(&format!("/v1/answers/{}/downvote", answer_id))
        .await;

    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(
        (upvotes, downvotes),
        (0, 1),
        "After downvote replaces upvote"
    );

    // Step 4: Downvote again (idempotent)
    voter
        .post_empty(&format!("/v1/answers/{}/downvote", answer_id))
        .await;

    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(
        (upvotes, downvotes),
        (0, 1),
        "After second downvote (idempotent)"
    );

    // Step 5: Change back to upvote
    voter
        .post_empty(&format!("/v1/answers/{}/upvote", answer_id))
        .await;

    let (upvotes, downvotes) = get_answer_votes(&client, &question_id, &answer_id).await;
    assert_eq!(
        (upvotes, downvotes),
        (1, 0),
        "After upvote replaces downvote"
    );
}
