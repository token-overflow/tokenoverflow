//! Unit tests for SearchService.
//!
//! Uses in-memory mock repositories with NoopConn -- no external dependencies.

use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::{QuestionService, SearchService, TagResolver};
use uuid::Uuid;

mod common {
    include!("../../common/mod.rs");
}

/// Test user ID for service-level tests (matches seeded system user)
const TEST_USER_ID: Uuid = SYSTEM_USER_ID;

/// Create a resolver with the seed tags for search tests.
fn seed_resolver() -> TagResolver {
    let store = common::MockStore::with_seed_tags();
    let synonyms = {
        let s = store.synonyms.lock().unwrap();
        s.iter()
            .map(|s| (s.synonym.clone(), s.canonical.clone()))
            .collect()
    };
    let canonicals = {
        let t = store.tags.lock().unwrap();
        t.iter().map(|t| t.name.clone()).collect()
    };
    TagResolver::from_data(synonyms, canonicals)
}

#[tokio::test]
async fn search_returns_empty_when_no_questions() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "How to handle errors in Rust?",
        None,
        5,
    )
    .await
    .unwrap();

    assert!(results.is_empty());
}

#[tokio::test]
async fn search_returns_matching_questions() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create a question
    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "How to handle async errors in Rust?",
        "I'm having trouble with error handling in async code. What's the best approach?",
        "Use the ? operator with anyhow or thiserror for better error handling.",
        Some(&["rust".to_string(), "async".to_string()]),
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Search for related questions
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "Error handling in Rust async functions",
        None,
        5,
    )
    .await
    .unwrap();

    assert!(!results.is_empty());
    assert!(results[0].title.contains("async") || results[0].title.contains("error"));
}

#[tokio::test]
async fn search_respects_limit() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create multiple questions
    for i in 0..5 {
        QuestionService::create(
            &mut conn,
            &question_repo,
            &tag_repo,
            &embedding,
            &resolver,
            &format!("Question {} about Rust programming", i),
            &format!(
                "This is question number {} about Rust programming concepts.",
                i
            ),
            &format!("Answer to question {} about Rust.", i),
            None,
            TEST_USER_ID,
        )
        .await
        .unwrap();
    }

    // Search with limit of 2
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "Rust programming questions",
        None,
        2,
    )
    .await
    .unwrap();

    assert!(results.len() <= 2);
}

#[tokio::test]
async fn search_results_ordered_by_similarity_descending() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create questions with different topics
    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "How to use tokio runtime in Rust?",
        "I want to understand the tokio async runtime for Rust applications.",
        "Tokio provides an async runtime with task scheduling and I/O.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "Best Python web frameworks comparison",
        "Comparing Flask, Django, and FastAPI for web development.",
        "Django is full-featured, Flask is minimal, FastAPI is async.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Search for tokio
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "tokio async runtime Rust",
        None,
        10,
    )
    .await
    .unwrap();

    // Mock returns all with 0.95, so just check we got results
    if results.len() >= 2 {
        assert!(
            results[0].similarity >= results[1].similarity,
            "Expected results ordered by similarity: {} >= {}",
            results[0].similarity,
            results[1].similarity
        );
    }
}

#[tokio::test]
async fn search_includes_answers_with_results() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create a question (which includes an initial answer)
    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "What is borrowing in Rust?",
        "I'm trying to understand the borrowing rules in Rust's ownership system.",
        "Borrowing lets you reference a value without taking ownership, using & for immutable and &mut for mutable.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Search for the question
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "Rust borrowing ownership rules",
        None,
        5,
    )
    .await
    .unwrap();

    assert!(!results.is_empty());
    assert!(!results[0].answers.is_empty());
}

#[tokio::test]
async fn search_filters_by_tags_when_provided() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create questions with different tags
    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "How to use async/await in Rust?",
        "Understanding async programming patterns in Rust language.",
        "Use async fn and .await for async operations.",
        Some(&["rust".to_string()]),
        TEST_USER_ID,
    )
    .await
    .unwrap();

    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "How to use async/await in Python?",
        "Understanding async programming patterns in Python language.",
        "Use async def and await for async operations.",
        Some(&["python".to_string()]),
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Search with rust tag filter
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "async await programming",
        Some(&["rust".to_string()]),
        10,
    )
    .await
    .unwrap();

    // Should only return Rust question
    for result in &results {
        assert!(
            result.tags.contains(&"rust".to_string()),
            "Expected results to have 'rust' tag, got {:?}",
            result.tags
        );
    }
}

#[tokio::test]
async fn search_similarity_scores_are_valid() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    // Create a question
    QuestionService::create(
        &mut conn,
        &question_repo,
        &tag_repo,
        &embedding,
        &resolver,
        "Learning Rust programming language",
        "I'm new to Rust and want to learn the basics of the language.",
        "Start with the Rust book and practice with small projects.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Search
    let results = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "Learning Rust programming",
        None,
        5,
    )
    .await
    .unwrap();

    assert!(!results.is_empty());

    // Similarity should be between -1 and 1 (cosine similarity)
    for result in &results {
        assert!(
            result.similarity >= -1.0 && result.similarity <= 1.0,
            "Similarity {} out of valid range [-1, 1]",
            result.similarity
        );
    }
}

#[tokio::test]
async fn search_returns_error_when_embedding_fails() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::with_seed_tags();
    let search_repo = common::MockSearchRepository::new(store.clone());
    let embedding = common::FailingMockEmbedding;
    let resolver = seed_resolver();

    let result = SearchService::search(
        &mut conn,
        &search_repo,
        &embedding,
        &resolver,
        "This query should fail",
        None,
        5,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn search_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingSearchRepository;
    let embedding = common::MockEmbedding::new();
    let resolver = seed_resolver();

    let result = SearchService::search(
        &mut conn,
        &repo,
        &embedding,
        &resolver,
        "Query with broken pool",
        None,
        5,
    )
    .await;

    assert!(result.is_err());
}
