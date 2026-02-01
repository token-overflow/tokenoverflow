//! Unit tests for QuestionService.
//!
//! Uses in-memory mock repositories with NoopConn -- no external dependencies.

use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::QuestionService;
use uuid::Uuid;

mod common {
    include!("../../common/mod.rs");
}

/// Test user ID for service-level tests (matches seeded system user)
const TEST_USER_ID: Uuid = SYSTEM_USER_ID;

#[tokio::test]
async fn create_returns_question_and_answer_ids() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver = tokenoverflow::services::TagResolver::from_data(
        std::collections::HashMap::new(),
        vec!["rust".to_string(), "async".to_string()],
    );

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "How to handle errors in async Rust code?",
        "I'm struggling with error handling in async functions. What's the best approach?",
        "Use the ? operator with Result types. You can also use anyhow for application code.",
        Some(&["rust".to_string(), "async".to_string()]),
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let response = result.unwrap();
    assert!(!response.question_id.is_nil());
    assert!(!response.answer_id.is_nil());
}

#[tokio::test]
async fn create_stores_tags_correctly() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver = tokenoverflow::services::TagResolver::from_data(
        {
            let mut m = std::collections::HashMap::new();
            m.insert("js".to_string(), "javascript".to_string());
            m
        },
        vec![
            "rust".to_string(),
            "javascript".to_string(),
            "async".to_string(),
        ],
    );

    let tags = vec!["rust".to_string(), "async".to_string()];

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "How to use tokio select macro?",
        "I need to wait on multiple async operations. How does select! work?",
        "Use tokio::select! to race multiple futures and handle the first one to complete.",
        Some(&tags),
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Fetch the question and verify tags (via join table in mock)
    let question = QuestionService::get_by_id(&mut conn, &repo, result.question_id)
        .await
        .unwrap();
    // Tags that exist in the mock seed data are linked
    assert!(!question.tags.is_empty());
}

#[tokio::test]
async fn create_without_tags_uses_empty_array() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "Simple question without tags",
        "This is a question body that doesn't need any tags for categorization.",
        "This is the answer to the question.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let question = QuestionService::get_by_id(&mut conn, &repo, result.question_id)
        .await
        .unwrap();
    assert!(question.tags.is_empty());
}

#[tokio::test]
async fn get_by_id_returns_question_with_answers() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);

    // Create a question
    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "What is the ownership system in Rust?",
        "I'm trying to understand how Rust manages memory without a garbage collector.",
        "Rust uses ownership rules: each value has one owner, and the value is dropped when the owner goes out of scope.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Fetch the question
    let question = QuestionService::get_by_id(&mut conn, &repo, result.question_id)
        .await
        .unwrap();

    assert_eq!(question.id, result.question_id);
    assert_eq!(question.title, "What is the ownership system in Rust?");
    assert_eq!(question.answers.len(), 1);
    assert_eq!(question.answers[0].id, result.answer_id);
}

#[tokio::test]
async fn get_by_id_returns_error_for_nonexistent_question() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockQuestionRepository::new(store.clone());

    let result = QuestionService::get_by_id(&mut conn, &repo, Uuid::nil()).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn exists_returns_true_for_existing_question() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "Does this question exist?",
        "Testing the exists function with a real question in the database.",
        "Yes, it exists after creation.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let exists = QuestionService::exists(&mut conn, &repo, result.question_id)
        .await
        .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn exists_returns_false_for_nonexistent_question() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockQuestionRepository::new(store.clone());

    let exists = QuestionService::exists(&mut conn, &repo, Uuid::nil())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn create_returns_error_when_embedding_fails() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::FailingMockEmbedding;
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "Test question with failing embedding",
        "This should fail because the embedding service is broken.",
        "Answer that should not be saved.",
        None,
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn create_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let repo = common::FailingQuestionRepository;
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);

    let result = QuestionService::create(
        &mut conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "Test question with broken pool",
        "This should fail because the database pool cannot provide connections.",
        "Answer that should not be saved.",
        None,
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn get_by_id_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingQuestionRepository;

    let result = QuestionService::get_by_id(&mut conn, &repo, Uuid::nil()).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn exists_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingQuestionRepository;

    let result = QuestionService::exists(&mut conn, &repo, Uuid::nil()).await;

    assert!(result.is_err());
}
