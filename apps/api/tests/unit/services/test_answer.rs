//! Unit tests for AnswerService.
//!
//! Uses in-memory mock repositories with NoopConn -- no external dependencies.

use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::error::AppError;
use tokenoverflow::services::{AnswerService, QuestionService};
use uuid::Uuid;

mod common {
    include!("../../common/mod.rs");
}

/// Test user ID for service-level tests (matches seeded system user)
const TEST_USER_ID: Uuid = SYSTEM_USER_ID;

/// Separate voter identity so happy-path vote tests do not trigger the self-vote guard
const TEST_VOTER_ID: Uuid = common::TEST_VOTER_ID;

/// Helper to create a question and return its ID via mock repos.
async fn create_test_question(conn: &mut common::NoopConn, store: &common::MockStore) -> Uuid {
    let repo = common::MockQuestionRepository::new(store.clone());
    let tag_repo = common::MockTagRepository::new(store.clone());
    let embedding = common::MockEmbedding::new();
    let tag_resolver =
        tokenoverflow::services::TagResolver::from_data(std::collections::HashMap::new(), vec![]);
    let result = QuestionService::create(
        conn,
        &repo,
        &tag_repo,
        &embedding,
        &tag_resolver,
        "Test question for answer tests",
        "This is a test question body used for testing the answer service.",
        "Initial answer provided during question creation.",
        None,
        TEST_USER_ID,
    )
    .await
    .unwrap();
    result.question_id
}

#[tokio::test]
async fn create_returns_answer_id() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());

    let result = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "This is a new answer to the question. It provides additional information.",
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    let answer_id = result.unwrap();
    assert!(!answer_id.is_nil());
}

#[tokio::test]
async fn create_returns_error_for_nonexistent_question() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockAnswerRepository::new(store.clone());

    let result = AnswerService::create(
        &mut conn,
        &repo,
        Uuid::nil(),
        "This answer is for a question that doesn't exist.",
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn upvote_succeeds_for_existing_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "Answer to test upvoting functionality.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let result = AnswerService::upvote(&mut conn, &repo, answer_id, TEST_VOTER_ID).await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
}

#[tokio::test]
async fn upvote_is_idempotent() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test idempotent upvoting.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Upvote twice
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    // Verify the answer only has 1 upvote
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 1);
}

#[tokio::test]
async fn upvote_returns_error_for_nonexistent_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockAnswerRepository::new(store.clone());

    let result = AnswerService::upvote(&mut conn, &repo, Uuid::nil(), TEST_USER_ID).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn downvote_succeeds_for_existing_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "Answer to test downvoting functionality.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let result = AnswerService::downvote(&mut conn, &repo, answer_id, TEST_VOTER_ID).await;
    assert!(result.is_ok(), "Expected Ok, got {:?}", result);
}

#[tokio::test]
async fn downvote_is_idempotent() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test idempotent downvoting.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Downvote twice
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    // Verify the answer only has 1 downvote
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.downvotes, 1);
}

#[tokio::test]
async fn downvote_returns_error_for_nonexistent_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockAnswerRepository::new(store.clone());

    let result = AnswerService::downvote(&mut conn, &repo, Uuid::nil(), TEST_USER_ID).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn switching_vote_updates_counts() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test vote switching behavior.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // First upvote
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    // Check upvote count is 1
    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 1);
    assert_eq!(answer.downvotes, 0);

    // Switch to downvote
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    // Check downvote count is 1 and upvote is 0
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 0);
    assert_eq!(answer.downvotes, 1);
}

#[tokio::test]
async fn switching_downvote_to_upvote_updates_counts() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test reverse vote switching.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // First downvote
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 0);
    assert_eq!(answer.downvotes, 1);

    // Switch to upvote
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 1);
    assert_eq!(answer.downvotes, 0);
}

#[tokio::test]
async fn flip_then_revote_is_idempotent() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test flip then revote idempotency.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    // Upvote -> flip to downvote -> downvote again (no-op)
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, TEST_VOTER_ID)
        .await
        .unwrap();

    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 0);
    assert_eq!(answer.downvotes, 1);
}

#[tokio::test]
async fn multiple_voters_have_independent_counts() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let answer_repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &answer_repo,
        question_id,
        "Answer to test multiple independent voters.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let voter_a = TEST_VOTER_ID;
    let voter_b = Uuid::from_u128(0xBBBB_BBBB_BBBB_BBBB_BBBB_BBBB_BBBB_BBBB);

    // Voter A upvotes, voter B downvotes
    AnswerService::upvote(&mut conn, &answer_repo, answer_id, voter_a)
        .await
        .unwrap();
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, voter_b)
        .await
        .unwrap();

    let question_repo = common::MockQuestionRepository::new(store.clone());
    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 1);
    assert_eq!(answer.downvotes, 1);

    // Voter A flips to downvote
    AnswerService::downvote(&mut conn, &answer_repo, answer_id, voter_a)
        .await
        .unwrap();

    let question = QuestionService::get_by_id(&mut conn, &question_repo, question_id)
        .await
        .unwrap();
    let answer = question.answers.iter().find(|a| a.id == answer_id).unwrap();
    assert_eq!(answer.upvotes, 0);
    assert_eq!(answer.downvotes, 2);
}

#[tokio::test]
async fn exists_returns_true_for_existing_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "Answer to test exists functionality.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let exists = AnswerService::exists(&mut conn, &repo, answer_id)
        .await
        .unwrap();
    assert!(exists);
}

#[tokio::test]
async fn exists_returns_false_for_nonexistent_answer() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let repo = common::MockAnswerRepository::new(store.clone());

    let exists = AnswerService::exists(&mut conn, &repo, Uuid::nil())
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
async fn create_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingAnswerRepository;

    let result = AnswerService::create(
        &mut conn,
        &repo,
        Uuid::nil(),
        "Answer with broken pool.",
        TEST_USER_ID,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn upvote_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingAnswerRepository;

    let result = AnswerService::upvote(&mut conn, &repo, Uuid::nil(), TEST_USER_ID).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn downvote_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingAnswerRepository;

    let result = AnswerService::downvote(&mut conn, &repo, Uuid::nil(), TEST_USER_ID).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn exists_returns_error_when_repo_fails() {
    let mut conn = common::NoopConn;
    let repo = common::FailingAnswerRepository;

    let result = AnswerService::exists(&mut conn, &repo, Uuid::nil()).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn self_upvote_returns_forbidden() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "Answer authored by test user.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let result = AnswerService::upvote(&mut conn, &repo, answer_id, TEST_USER_ID).await;
    assert!(
        matches!(result, Err(AppError::Forbidden(_))),
        "Expected Forbidden, got {:?}",
        result
    );
}

#[tokio::test]
async fn self_downvote_returns_forbidden() {
    let mut conn = common::NoopConn;
    let store = common::MockStore::new();
    let question_id = create_test_question(&mut conn, &store).await;
    let repo = common::MockAnswerRepository::new(store.clone());
    let answer_id = AnswerService::create(
        &mut conn,
        &repo,
        question_id,
        "Answer authored by test user.",
        TEST_USER_ID,
    )
    .await
    .unwrap();

    let result = AnswerService::downvote(&mut conn, &repo, answer_id, TEST_USER_ID).await;
    assert!(
        matches!(result, Err(AppError::Forbidden(_))),
        "Expected Forbidden, got {:?}",
        result
    );
}
