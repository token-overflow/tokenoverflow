use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::repository::{
    PgQuestionRepository, PgTagRepository, QuestionRepository, TagRepository,
};

use crate::test_db::IntegrationTestDb;

/// 256-dimensional embedding filled with a constant value.
fn test_embedding(value: f32) -> Vec<f32> {
    vec![value; 256]
}

#[tokio::test]
async fn loads_synonyms() {
    let db = IntegrationTestDb::new().await;
    let repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let synonyms = repo
        .load_synonyms(&mut *conn)
        .await
        .expect("load_synonyms should succeed");

    // The migration seeds 15 synonyms
    assert!(
        synonyms.len() >= 15,
        "Expected at least 15 seeded synonyms, got {}",
        synonyms.len()
    );
    assert_eq!(synonyms.get("js"), Some(&"javascript".to_string()));
    assert_eq!(synonyms.get("py"), Some(&"python".to_string()));
    assert_eq!(synonyms.get("k8s"), Some(&"kubernetes".to_string()));
}

#[tokio::test]
async fn loads_canonicals() {
    let db = IntegrationTestDb::new().await;
    let repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let canonicals = repo
        .load_canonicals(&mut *conn)
        .await
        .expect("load_canonicals should succeed");

    // The migration seeds 100 canonical tags
    assert!(
        canonicals.len() >= 100,
        "Expected at least 100 seeded tags, got {}",
        canonicals.len()
    );
    assert!(canonicals.contains(&"javascript".to_string()));
    assert!(canonicals.contains(&"rust".to_string()));
    assert!(canonicals.contains(&"python".to_string()));
}

#[tokio::test]
async fn find_tag_ids_existing() {
    let db = IntegrationTestDb::new().await;
    let repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let names = vec!["javascript".to_string(), "rust".to_string()];
    let pairs = repo
        .find_tag_ids(&mut *conn, &names)
        .await
        .expect("find_tag_ids should succeed");

    assert_eq!(pairs.len(), 2);
    let found_names: Vec<&str> = pairs.iter().map(|(n, _)| n.as_str()).collect();
    assert!(found_names.contains(&"javascript"));
    assert!(found_names.contains(&"rust"));
}

#[tokio::test]
async fn find_tag_ids_missing() {
    let db = IntegrationTestDb::new().await;
    let repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let names = vec!["nonexistenttag123".to_string()];
    let pairs = repo
        .find_tag_ids(&mut *conn, &names)
        .await
        .expect("find_tag_ids should succeed");

    assert!(pairs.is_empty());
}

#[tokio::test]
async fn find_tag_ids_partial() {
    let db = IntegrationTestDb::new().await;
    let repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let names = vec!["javascript".to_string(), "nonexistenttag123".to_string()];
    let pairs = repo
        .find_tag_ids(&mut *conn, &names)
        .await
        .expect("find_tag_ids should succeed");

    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0].0, "javascript");
}

#[tokio::test]
async fn links_tags_to_question() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let created = question_repo
        .create(
            &mut *conn,
            "Test question for tag linking",
            "This tests the link_tags_to_question method.",
            "Answer for the test question.",
            test_embedding(0.1),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let tag_pairs = tag_repo
        .find_tag_ids(&mut *conn, &["rust".to_string(), "python".to_string()])
        .await
        .expect("find_tag_ids should succeed");
    let tag_ids: Vec<uuid::Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();

    tag_repo
        .link_tags_to_question(&mut *conn, created.question_id, &tag_ids)
        .await
        .expect("link_tags should succeed");

    let tags = tag_repo
        .get_question_tags(&mut *conn, created.question_id)
        .await
        .expect("get_question_tags should succeed");

    assert_eq!(tags.len(), 2);
    assert!(tags.contains(&"rust".to_string()));
    assert!(tags.contains(&"python".to_string()));
}

#[tokio::test]
async fn links_tags_idempotent() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let created = question_repo
        .create(
            &mut *conn,
            "Test question for idempotent linking",
            "This tests that linking same tags twice doesn't error.",
            "Answer for the idempotent test.",
            test_embedding(0.2),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let tag_pairs = tag_repo
        .find_tag_ids(&mut *conn, &["rust".to_string()])
        .await
        .expect("find_tag_ids should succeed");
    let tag_ids: Vec<uuid::Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();

    // Link twice
    tag_repo
        .link_tags_to_question(&mut *conn, created.question_id, &tag_ids)
        .await
        .expect("first link should succeed");
    tag_repo
        .link_tags_to_question(&mut *conn, created.question_id, &tag_ids)
        .await
        .expect("second link should succeed (idempotent)");

    let tags = tag_repo
        .get_question_tags(&mut *conn, created.question_id)
        .await
        .expect("get_question_tags should succeed");

    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0], "rust");
}

#[tokio::test]
async fn get_question_tags_empty() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let mut conn = db.pool().get().await.unwrap();

    let created = question_repo
        .create(
            &mut *conn,
            "Question with no tags linked",
            "This question should have zero tags.",
            "Answer with no tags.",
            test_embedding(0.3),
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let tags = tag_repo
        .get_question_tags(&mut *conn, created.question_id)
        .await
        .expect("get_question_tags should succeed");

    assert!(tags.is_empty());
}
