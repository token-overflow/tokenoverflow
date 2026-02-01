use std::sync::Arc;

use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::TagResolver;
use tokenoverflow::services::repository::{
    PgQuestionRepository, PgTagRepository, QuestionRepository, TagRepository,
};

use crate::test_db::IntegrationTestDb;

/// 256-dimensional embedding filled with a constant value.
fn test_embedding(value: f32) -> Vec<f32> {
    vec![value; 256]
}

#[tokio::test]
async fn create_returns_ids() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let mut conn = db.pool().get().await.unwrap();

    let result = repo
        .create(
            &mut *conn,
            "How do I use async/await in Rust?",
            "I need help understanding the async/await pattern.",
            "Use tokio as your runtime and mark functions with async.",
            test_embedding(0.1),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    // Both IDs should be valid UUIDs (non-nil)
    assert!(!result.question_id.is_nil());
    assert!(!result.answer_id.is_nil());
}

#[tokio::test]
async fn get_by_id_returns_question_with_answers() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let tag_repo = Arc::new(PgTagRepository);
    let mut conn = db.pool().get().await.unwrap();

    // Build resolver from DB seed data
    let resolver = TagResolver::new(tag_repo.as_ref(), &mut *conn)
        .await
        .expect("resolver init should succeed");

    let created = repo
        .create(
            &mut *conn,
            "What is ownership in Rust?",
            "Can someone explain Rust ownership rules?",
            "Ownership is a set of rules governing memory management.",
            test_embedding(0.2),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    // Link tags via the tag repository
    let resolved = resolver.resolve_tags(&["rust".to_string()]);
    let tag_pairs = tag_repo
        .find_tag_ids(&mut *conn, &resolved)
        .await
        .expect("find_tag_ids should succeed");
    let tag_ids: Vec<uuid::Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();
    tag_repo
        .link_tags_to_question(&mut *conn, created.question_id, &tag_ids)
        .await
        .expect("link_tags should succeed");

    let question = repo
        .get_by_id(&mut *conn, created.question_id)
        .await
        .expect("get_by_id should succeed");

    assert_eq!(question.id, created.question_id);
    assert_eq!(question.title, "What is ownership in Rust?");
    assert_eq!(question.body, "Can someone explain Rust ownership rules?");
    assert_eq!(question.tags, vec!["rust"]);
    assert_eq!(question.answers.len(), 1);
    assert_eq!(question.answers[0].id, created.answer_id);
    assert_eq!(
        question.answers[0].body,
        "Ownership is a set of rules governing memory management."
    );
}

#[tokio::test]
async fn get_by_id_nonexistent_returns_not_found() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let mut conn = db.pool().get().await.unwrap();

    let result = repo.get_by_id(&mut *conn, uuid::Uuid::nil()).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("not found"),
        "Expected NotFound error, got: {}",
        err
    );
}

#[tokio::test]
async fn exists_returns_true_for_existing() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let mut conn = db.pool().get().await.unwrap();

    let created = repo
        .create(
            &mut *conn,
            "Does this question exist?",
            "Testing the exists method on the question repository.",
            "Yes, this answer confirms it.",
            test_embedding(0.3),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let exists = repo
        .exists(&mut *conn, created.question_id)
        .await
        .expect("exists should succeed");

    assert!(exists);
}

#[tokio::test]
async fn exists_returns_false_for_nonexistent() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let mut conn = db.pool().get().await.unwrap();

    let exists = repo
        .exists(&mut *conn, uuid::Uuid::nil())
        .await
        .expect("exists should succeed");

    assert!(!exists);
}

#[tokio::test]
async fn create_with_no_tags_returns_empty() {
    let db = IntegrationTestDb::new().await;
    let repo = PgQuestionRepository;
    let mut conn = db.pool().get().await.unwrap();

    let created = repo
        .create(
            &mut *conn,
            "Question without any tags",
            "This question has no tags at all, which should be valid.",
            "Tags are optional so this should work fine.",
            test_embedding(0.5),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let question = repo
        .get_by_id(&mut *conn, created.question_id)
        .await
        .expect("get_by_id should succeed");

    assert!(question.tags.is_empty());
}
