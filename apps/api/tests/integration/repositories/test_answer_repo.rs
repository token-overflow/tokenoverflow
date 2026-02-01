use std::sync::atomic::{AtomicU64, Ordering};

use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::repository::{
    AnswerRepository, PgAnswerRepository, PgQuestionRepository, QuestionRepository,
};
use uuid::Uuid;

use crate::test_db::IntegrationTestDb;

/// 256-dimensional embedding filled with a constant value.
fn test_embedding(value: f32) -> Vec<f32> {
    vec![value; 256]
}

/// Helper: create a question and return its ID.
async fn create_question(
    repo: &PgQuestionRepository,
    conn: &mut diesel_async::AsyncPgConnection,
) -> Uuid {
    repo.create(
        conn,
        "Helper question for answer tests",
        "This question exists so we can attach answers to it.",
        "Initial answer provided during question creation.",
        test_embedding(0.1),
        SYSTEM_USER_ID,
    )
    .await
    .expect("question creation should succeed")
    .question_id
}

#[tokio::test]
async fn create_returns_id() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;

    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "A second answer to the question.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    assert!(!answer_id.is_nil());
}

#[tokio::test]
async fn create_with_nonexistent_question_returns_error() {
    let db = IntegrationTestDb::new().await;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let result = answer_repo
        .create(
            &mut conn,
            Uuid::nil(),
            "This answer targets a nonexistent question.",
            SYSTEM_USER_ID,
        )
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("not found"),
        "Expected NotFound from FK violation, got: {}",
        err
    );
}

#[tokio::test]
async fn upvote_increments_count() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(&mut conn, question_id, "Answer to upvote.", SYSTEM_USER_ID)
        .await
        .expect("create should succeed");

    let voter = insert_test_user(db.pool()).await;

    answer_repo
        .upvote(&mut conn, answer_id, voter)
        .await
        .expect("upvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");

    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");

    assert_eq!(answer.upvotes, 1);
    assert_eq!(answer.downvotes, 0);
}

#[tokio::test]
async fn downvote_increments_downvote_count() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "Answer to downvote.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let voter = insert_test_user(db.pool()).await;

    answer_repo
        .downvote(&mut conn, answer_id, voter)
        .await
        .expect("downvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");

    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");

    assert_eq!(answer.upvotes, 0);
    assert_eq!(answer.downvotes, 1);
}

#[tokio::test]
async fn upvote_is_idempotent() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "Answer for idempotent upvote test.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let voter = insert_test_user(db.pool()).await;

    answer_repo
        .upvote(&mut conn, answer_id, voter)
        .await
        .expect("first upvote should succeed");
    answer_repo
        .upvote(&mut conn, answer_id, voter)
        .await
        .expect("second upvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");

    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");

    assert_eq!(answer.upvotes, 1, "duplicate upvote should be idempotent");
    assert_eq!(answer.downvotes, 0);
}

#[tokio::test]
async fn vote_switch_updates_counts() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "Answer to test vote switching.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let voter = insert_test_user(db.pool()).await;

    answer_repo
        .upvote(&mut conn, answer_id, voter)
        .await
        .expect("upvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");
    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");
    assert_eq!(answer.upvotes, 1);
    assert_eq!(answer.downvotes, 0);

    answer_repo
        .downvote(&mut conn, answer_id, voter)
        .await
        .expect("downvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");
    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");
    assert_eq!(
        answer.upvotes, 0,
        "upvote should be removed after switching"
    );
    assert_eq!(
        answer.downvotes, 1,
        "downvote should be recorded after switching"
    );
}

#[tokio::test]
async fn exists_returns_true_for_existing() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "Answer to check existence.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let exists = answer_repo
        .exists(&mut conn, answer_id)
        .await
        .expect("exists should succeed");

    assert!(exists);
}

#[tokio::test]
async fn exists_returns_false_for_nonexistent() {
    let db = IntegrationTestDb::new().await;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let exists = answer_repo
        .exists(&mut conn, Uuid::nil())
        .await
        .expect("exists should succeed");

    assert!(!exists);
}

#[tokio::test]
async fn downvote_is_idempotent() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;
    let answer_id = answer_repo
        .create(
            &mut conn,
            question_id,
            "Answer for idempotent downvote test.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("create should succeed");

    let voter = insert_test_user(db.pool()).await;

    // Downvote twice with the same user
    answer_repo
        .downvote(&mut conn, answer_id, voter)
        .await
        .expect("first downvote should succeed");
    answer_repo
        .downvote(&mut conn, answer_id, voter)
        .await
        .expect("second downvote should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");

    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");

    // Should still be 1 downvote, not 2
    assert_eq!(
        answer.downvotes, 1,
        "duplicate downvote should be idempotent"
    );
    assert_eq!(answer.upvotes, 0);
}

#[tokio::test]
async fn downvote_nonexistent_answer_returns_not_found() {
    let db = IntegrationTestDb::new().await;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let voter = insert_test_user(db.pool()).await;

    let result = answer_repo.downvote(&mut conn, Uuid::nil(), voter).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        format!("{}", err).contains("not found"),
        "Expected NotFound from FK violation, got: {}",
        err
    );
}

#[tokio::test]
async fn create_answer_stores_correct_fields() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let mut conn = db.pool().get().await.unwrap();

    let question_id = create_question(&question_repo, &mut conn).await;

    let body = "This is a specific answer body for field verification.";
    let answer_id = answer_repo
        .create(&mut conn, question_id, body, SYSTEM_USER_ID)
        .await
        .expect("create should succeed");

    let question = question_repo
        .get_by_id(&mut conn, question_id)
        .await
        .expect("get_by_id should succeed");

    let answer = question
        .answers
        .iter()
        .find(|a| a.id == answer_id)
        .expect("answer should be in results");

    assert_eq!(answer.body, body, "answer body should match input");
}

static TEST_USER_COUNTER: AtomicU64 = AtomicU64::new(100);

/// Insert a minimal user row so FK constraints on votes are satisfied.
async fn insert_test_user(pool: &tokenoverflow::db::DbPool) -> Uuid {
    use diesel::sql_types::Text;
    use diesel_async::RunQueryDsl;

    let mut conn = pool.get().await.expect("Failed to get connection");

    let n = TEST_USER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let workos_id = format!("user_test_{}", n);

    #[derive(diesel::QueryableByName)]
    struct Row {
        #[diesel(sql_type = diesel::sql_types::Uuid)]
        id: Uuid,
    }

    let row: Row = diesel::sql_query(
        "INSERT INTO api.users (workos_id, username) VALUES ($1, $1) RETURNING id",
    )
    .bind::<Text, _>(&workos_id)
    .get_result(&mut conn)
    .await
    .expect("Failed to insert test user");

    row.id
}
