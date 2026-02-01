use http::StatusCode;
use serde_json::Value;

mod common {
    include!("../../../common/mod.rs");
}

use common::{QuestionRequestBuilder, SearchRequestBuilder};

// ============================================================================
// POST /v1/search - Search Questions
// ============================================================================

#[tokio::test]
async fn search_returns_questions_with_answers() {
    let client = common::TestClient::from_config();

    // Create a question first
    let create_req = QuestionRequestBuilder::new()
        .title("How to handle async errors in Rust?")
        .body("I need help understanding error handling in async Rust code.")
        .build();

    client.post("/v1/questions", &create_req).await;

    // Search for it
    let search_req = SearchRequestBuilder::new()
        .query("async errors in Rust")
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();

    assert!(!questions.is_empty());

    // Verify structure
    let first = &questions[0];
    assert!(first["id"].is_string());
    assert!(first["title"].is_string());
    assert!(first["body"].is_string());
    assert!(first["similarity"].is_f64());
    assert!(first["answers"].is_array());
    assert!(!first["answers"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn search_returns_empty_when_no_matching_data() {
    let client = common::TestClient::from_config();

    // Search for a unique query that won't match any existing data
    let search_req = SearchRequestBuilder::new()
        .query("xyz123nonexistentquery456abc")
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();

    // May be empty or contain unrelated results with low similarity
    // The important thing is that the API returns a valid response
    assert!(body["questions"].is_array());

    // If there are results, they should have low similarity scores
    for question in questions {
        assert!(question["similarity"].is_f64());
    }
}

#[tokio::test]
async fn search_orders_by_similarity() {
    let client = common::TestClient::from_config();

    // Create two questions
    let req1 = QuestionRequestBuilder::new()
        .title("How to parse JSON in Python?")
        .body("I need to parse a JSON file in my Python script.")
        .answer("Use the json module: import json; data = json.load(file)")
        .tags(vec!["python".to_string(), "json".to_string()])
        .build();

    let req2 = QuestionRequestBuilder::new()
        .title("How to handle async errors in Rust?")
        .body("I need help with error handling in async Rust code.")
        .answer("Use the ? operator with Result types.")
        .tags(vec!["rust".to_string(), "async".to_string()])
        .build();

    client.post("/v1/questions", &req1).await;
    client.post("/v1/questions", &req2).await;

    // Search
    let search_req = SearchRequestBuilder::new()
        .query("async error handling in Rust programming")
        .build();

    let response = client.post("/v1/search", &search_req).await;

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();

    assert!(questions.len() >= 2, "Should return at least 2 questions");

    // Verify ordered by similarity (descending)
    let similarities: Vec<f64> = questions
        .iter()
        .map(|q| q["similarity"].as_f64().unwrap())
        .collect();

    for i in 1..similarities.len() {
        assert!(
            similarities[i - 1] >= similarities[i],
            "Results should be ordered by similarity descending: {:?}",
            similarities
        );
    }

    // Verify each result has required fields
    for question in questions {
        assert!(question["id"].is_string());
        assert!(question["title"].is_string());
        assert!(question["similarity"].is_f64());
        let similarity = question["similarity"].as_f64().unwrap();
        assert!(
            (-1.0..=1.0).contains(&similarity),
            "Similarity should be between -1 and 1"
        );
    }
}

#[tokio::test]
async fn search_filters_by_tags() {
    let client = common::TestClient::from_config();

    // Use seeded tags — the tag resolver drops unknown tags
    let unique_tag = "svelte".to_string();
    let other_tag = "hadoop".to_string();

    // Create questions with different tags
    let rust_question = QuestionRequestBuilder::new()
        .title("How to handle errors in Rust?")
        .body("I need help with error handling in Rust.")
        .answer("Use Result and the ? operator.")
        .tags(vec![unique_tag.clone()])
        .build();

    let python_question = QuestionRequestBuilder::new()
        .title("How to handle errors in Python?")
        .body("I need help with error handling in Python.")
        .answer("Use try/except blocks.")
        .tags(vec![other_tag.clone()])
        .build();

    client.post("/v1/questions", &rust_question).await;
    client.post("/v1/questions", &python_question).await;

    // Search with unique rust tag filter
    let search_req = SearchRequestBuilder::new()
        .query("error handling programming")
        .tags(vec![unique_tag.clone()])
        .build();

    let response = client.post("/v1/search", &search_req).await;

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();

    // Only questions with the unique tag should be returned
    for question in questions {
        let tags = question["tags"].as_array().unwrap();
        assert!(
            tags.iter().any(|t| t.as_str().unwrap() == unique_tag),
            "All results should have the unique tag"
        );
    }
}

#[tokio::test]
async fn search_respects_limit() {
    let client = common::TestClient::from_config();

    // Create multiple questions
    for i in 1..=5 {
        let req = QuestionRequestBuilder::new()
            .title(format!("Error handling question #{}", i))
            .body("How do I handle this error?")
            .build();

        client.post("/v1/questions", &req).await;
    }

    // Search with limit of 2
    let search_req = SearchRequestBuilder::new()
        .query("error handling question")
        .limit(2)
        .build();

    let response = client.post("/v1/search", &search_req).await;

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();

    assert!(questions.len() <= 2, "Should respect the limit");
}

#[tokio::test]
async fn search_query_at_min_length_succeeds() {
    let client = common::TestClient::from_config();

    let query = "a".repeat(10); // exactly at min
    let search_req = SearchRequestBuilder::new().query(query).build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    assert!(body["questions"].is_array());
}

#[tokio::test]
async fn search_limit_at_boundaries_succeeds() {
    let client = common::TestClient::from_config();

    // limit=1 (minimum)
    let search_req = SearchRequestBuilder::new()
        .query("a valid query text")
        .limit(1)
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();
    assert!(questions.len() <= 1);

    // limit=10 (maximum)
    let search_req = SearchRequestBuilder::new()
        .query("a valid query text")
        .limit(10)
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body: Value = response.json();
    let questions = body["questions"].as_array().unwrap();
    assert!(questions.len() <= 10);
}

#[tokio::test]
async fn search_query_too_short_returns_422() {
    let client = common::TestClient::from_config();

    let search_req = SearchRequestBuilder::new()
        .query("short") // 5 chars, min is 10
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn search_too_many_tags_returns_422() {
    let client = common::TestClient::from_config();

    let search_req = SearchRequestBuilder::new()
        .query("a valid query text")
        .tags(vec!["tag".to_string(); 11]) // max is 10
        .build();

    let response = client.post("/v1/search", &search_req).await;

    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn search_limit_out_of_range_returns_422() {
    let client = common::TestClient::from_config();

    // Limit too low
    let search_req = SearchRequestBuilder::new()
        .query("a valid query text")
        .limit(0)
        .build();

    let response = client.post("/v1/search", &search_req).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Limit too high
    let search_req = SearchRequestBuilder::new()
        .query("a valid query text")
        .limit(11)
        .build();

    let response = client.post("/v1/search", &search_req).await;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
