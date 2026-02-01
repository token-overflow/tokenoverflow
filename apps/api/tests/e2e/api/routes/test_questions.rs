use http::StatusCode;
use serde_json::{Value, json};

mod common {
    include!("../../../common/mod.rs");
}

use common::{AnswerRequestBuilder, QuestionRequestBuilder};

// ============================================================================
// POST /v1/questions - Create Question
// ============================================================================

#[tokio::test]
async fn create_question_returns_201_with_ids() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new().build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = response.json();
    assert!(body["question_id"].is_string());
    assert!(body["answer_id"].is_string());

    // Verify they are valid UUID strings
    let question_id = body["question_id"].as_str().unwrap();
    let answer_id = body["answer_id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(question_id).is_ok());
    assert!(uuid::Uuid::parse_str(answer_id).is_ok());
}

#[tokio::test]
async fn create_question_with_tags_stores_tags() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .tags(vec!["rust".to_string(), "python".to_string()])
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = response.json();
    let question_id = body["question_id"].as_str().unwrap();

    // Retrieve the question and verify tags
    let get_response = client.get(&format!("/v1/questions/{}", question_id)).await;

    assert_eq!(get_response.status(), StatusCode::OK);
    let question: Value = get_response.json();

    let tags = question["tags"].as_array().unwrap();
    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&json!("rust")));
    assert!(tags.contains(&json!("python")));
}

#[tokio::test]
async fn create_question_without_tags_succeeds() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new().no_tags().build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_question_title_too_short_returns_422() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .title("short") // 5 chars, min is 10
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_body_too_short_returns_422() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .body("short") // 5 chars, min is 10
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_answer_too_short_returns_422() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .answer("short") // 5 chars, min is 10
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_too_many_tags_returns_422() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .tags(vec!["tag".to_string(); 11]) // max is 10
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_tag_too_long_returns_422() {
    let client = common::TestClient::from_config();

    // 36 chars exceeds the 35-char tag limit
    let long_tag = "a".repeat(36);
    let req = QuestionRequestBuilder::new().tags(vec![long_tag]).build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_empty_tag_returns_422() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .tags(vec!["".to_string()])
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_with_unicode_succeeds() {
    let client = common::TestClient::from_config();

    let req = QuestionRequestBuilder::new()
        .title("Comment gerer les erreurs en Rust?")
        .body("Je veux comprendre la gestion d'erreurs avec les traits.")
        .answer("Utilisez le trait std::error::Error et l'operateur ? pour propager.")
        .build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = response.json();
    let question_id = body["question_id"].as_str().unwrap();

    // GET to verify round-trip
    let get_response = client.get(&format!("/v1/questions/{}", question_id)).await;

    assert_eq!(get_response.status(), StatusCode::OK);

    let question: Value = get_response.json();
    assert_eq!(question["title"], "Comment gerer les erreurs en Rust?");
    assert_eq!(
        question["body"],
        "Je veux comprendre la gestion d'erreurs avec les traits."
    );
    let answers = question["answers"].as_array().unwrap();
    assert_eq!(answers.len(), 1);
    assert_eq!(
        answers[0]["body"],
        "Utilisez le trait std::error::Error et l'operateur ? pour propager."
    );
}

#[tokio::test]
async fn create_question_title_at_max_length_succeeds() {
    let client = common::TestClient::from_config();

    let title = "a".repeat(150); // exactly at max
    let req = QuestionRequestBuilder::new().title(title).build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_question_title_over_max_returns_422() {
    let client = common::TestClient::from_config();

    let title = "a".repeat(151); // one over max
    let req = QuestionRequestBuilder::new().title(title).build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn create_question_body_at_max_length_succeeds() {
    let client = common::TestClient::from_config();

    let body = "a".repeat(1500); // exactly at max
    let req = QuestionRequestBuilder::new().body(body).build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_question_body_over_max_returns_422() {
    let client = common::TestClient::from_config();

    let body = "a".repeat(1501); // one over max
    let req = QuestionRequestBuilder::new().body(body).build();

    let response = client.post("/v1/questions", &req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// GET /v1/questions/{id} - Get Question
// ============================================================================

#[tokio::test]
async fn get_question_returns_question_with_answers() {
    let client = common::TestClient::from_config();

    // Create a question first
    let req = QuestionRequestBuilder::new()
        .title("How to handle errors in Rust?")
        .build();

    let create_response = client.post("/v1/questions", &req).await;

    let create_body: Value = create_response.json();
    let question_id = create_body["question_id"].as_str().unwrap();

    // Get the question
    let response = client.get(&format!("/v1/questions/{}", question_id)).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    assert_eq!(body["id"].as_str().unwrap(), question_id);
    assert_eq!(body["title"], "How to handle errors in Rust?");
    assert!(body["answers"].is_array());
    assert_eq!(body["answers"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn get_question_not_found_returns_404() {
    let client = common::TestClient::from_config();

    let response = client
        .get("/v1/questions/00000000-0000-0000-0000-000000000099")
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn get_question_invalid_id_returns_422() {
    let client = common::TestClient::from_config();

    let response = client.get("/v1/questions/not-a-valid-id").await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn get_question_numeric_id_returns_422() {
    let client = common::TestClient::from_config();

    // Old-style numeric ID should be rejected
    let response = client.get("/v1/questions/1").await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// POST /v1/questions/{id}/answers - Add Answer
// ============================================================================

#[tokio::test]
async fn add_answer_returns_201_with_id() {
    let client = common::TestClient::from_config();

    // Create a question first
    let create_req = QuestionRequestBuilder::new().build();
    let create_response = client.post("/v1/questions", &create_req).await;

    let create_body: Value = create_response.json();
    let question_id = create_body["question_id"].as_str().unwrap();

    // Add a new answer
    let answer_req = AnswerRequestBuilder::new()
        .body("Here's another approach to solve this problem.")
        .build();

    let response = client
        .post(
            &format!("/v1/questions/{}/answers", question_id),
            &answer_req,
        )
        .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let body: Value = response.json();
    assert!(body["id"].is_string());

    // Verify it's a valid UUID
    let answer_id = body["id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(answer_id).is_ok());
}

#[tokio::test]
async fn add_answer_question_not_found_returns_404() {
    let client = common::TestClient::from_config();

    let answer_req = AnswerRequestBuilder::new().build();

    let response = client
        .post(
            "/v1/questions/00000000-0000-0000-0000-000000000099/answers",
            &answer_req,
        )
        .await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn add_answer_body_too_short_returns_422() {
    let client = common::TestClient::from_config();

    // Create a question first
    let create_req = QuestionRequestBuilder::new().build();
    let create_response = client.post("/v1/questions", &create_req).await;

    let create_body: Value = create_response.json();
    let question_id = create_body["question_id"].as_str().unwrap();

    // Try to add an answer with body too short
    let answer_req = AnswerRequestBuilder::new()
        .body("too short") // 9 chars, min is 10
        .build();

    let response = client
        .post(
            &format!("/v1/questions/{}/answers", question_id),
            &answer_req,
        )
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn add_answer_invalid_question_id_returns_422() {
    let client = common::TestClient::from_config();

    let answer_req = AnswerRequestBuilder::new().build();

    let response = client
        .post("/v1/questions/not-a-valid-id/answers", &answer_req)
        .await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

// ============================================================================
// User Story: Question Submission and Answer Flow
// ============================================================================

#[tokio::test]
async fn question_submission_and_answer_flow() {
    let client = common::TestClient::from_config();

    // Step 1: Create a question with initial answer
    let create_req = QuestionRequestBuilder::new()
        .title("How to implement async iterators in Rust?")
        .body("I need to iterate over async data streams but I'm not sure how to structure this.")
        .answer("Use the futures crate's Stream trait. You can implement Stream for custom types.")
        .tags(vec![
            "rust".to_string(),
            "async".to_string(),
            "streams".to_string(),
        ])
        .build();

    let create_response = client.post("/v1/questions", &create_req).await;

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_body: Value = create_response.json();
    let question_id = create_body["question_id"].as_str().unwrap();
    let first_answer_id = create_body["answer_id"].as_str().unwrap();

    // Step 2: Retrieve the question and verify initial state
    let get_response = client.get(&format!("/v1/questions/{}", question_id)).await;

    assert_eq!(get_response.status(), StatusCode::OK);
    let question: Value = get_response.json();

    assert_eq!(
        question["title"],
        "How to implement async iterators in Rust?"
    );
    assert_eq!(question["answers"].as_array().unwrap().len(), 1);
    assert_eq!(
        question["answers"][0]["id"].as_str().unwrap(),
        first_answer_id
    );

    // Step 3: Add another answer
    let second_answer_req = AnswerRequestBuilder::new()
        .body("Consider using async-stream crate for a more ergonomic syntax with yield.")
        .build();

    let second_answer_response = client
        .post(
            &format!("/v1/questions/{}/answers", question_id),
            &second_answer_req,
        )
        .await;

    assert_eq!(second_answer_response.status(), StatusCode::CREATED);

    // Step 4: Verify the question now has two answers
    let final_response = client.get(&format!("/v1/questions/{}", question_id)).await;

    let final_question: Value = final_response.json();
    assert_eq!(final_question["answers"].as_array().unwrap().len(), 2);
}
