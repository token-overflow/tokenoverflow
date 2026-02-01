use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::services::repository::{
    AnswerRepository, PgAnswerRepository, PgQuestionRepository, PgSearchRepository,
    PgTagRepository, QuestionRepository, SearchRepository, TagRepository,
};

use crate::test_db::IntegrationTestDb;

/// Build a 256-dimensional embedding where all elements equal `value`.
fn uniform_embedding(value: f32) -> Vec<f32> {
    vec![value; 256]
}

/// Build a 256-dimensional embedding with a distinguishable pattern.
/// The first element is set to `marker` and the rest are filled with `base`.
fn marked_embedding(marker: f32, base: f32) -> Vec<f32> {
    let mut v = vec![base; 256];
    v[0] = marker;
    v
}

/// Helper: create a question and link tags to it.
#[allow(clippy::too_many_arguments)]
async fn create_question_with_tags(
    question_repo: &PgQuestionRepository,
    tag_repo: &PgTagRepository,
    conn: &mut diesel_async::AsyncPgConnection,
    title: &str,
    body: &str,
    answer: &str,
    tag_names: &[&str],
    embedding: Vec<f32>,
) -> uuid::Uuid {
    let created = question_repo
        .create(conn, title, body, answer, embedding, SYSTEM_USER_ID)
        .await
        .expect("create should succeed");

    if !tag_names.is_empty() {
        let names: Vec<String> = tag_names.iter().map(|s| s.to_string()).collect();
        let tag_pairs = tag_repo
            .find_tag_ids(conn, &names)
            .await
            .expect("find_tag_ids should succeed");
        let tag_ids: Vec<uuid::Uuid> = tag_pairs.into_iter().map(|(_, id)| id).collect();
        tag_repo
            .link_tags_to_question(conn, created.question_id, &tag_ids)
            .await
            .expect("link_tags should succeed");
    }

    created.question_id
}

#[tokio::test]
async fn search_returns_results() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let search_repo = PgSearchRepository;
    let mut conn = db.pool().get().await.unwrap();

    let embedding = uniform_embedding(0.1);
    create_question_with_tags(
        &question_repo,
        &tag_repo,
        &mut conn,
        "How to search with pgvector?",
        "I want to find semantically similar questions.",
        "Use cosine distance with the <=> operator.",
        &["postgresql"],
        embedding.clone(),
    )
    .await;

    // Search with the same embedding should return a high-similarity result
    let results = search_repo
        .search(&mut conn, embedding, None, 5)
        .await
        .expect("search should succeed");

    assert!(
        !results.is_empty(),
        "search should return at least one result"
    );
    assert_eq!(results[0].title, "How to search with pgvector?");
    // Identical embedding should yield similarity close to 1.0
    assert!(
        results[0].similarity > 0.99,
        "identical embedding should have near-perfect similarity, got {}",
        results[0].similarity
    );
}

#[tokio::test]
async fn search_with_tags_filters() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let search_repo = PgSearchRepository;
    let mut conn = db.pool().get().await.unwrap();

    let embedding = uniform_embedding(0.2);

    // Create two questions with different tags but similar embeddings
    create_question_with_tags(
        &question_repo,
        &tag_repo,
        &mut conn,
        "Rust question about lifetimes",
        "How do I understand lifetime annotations in Rust?",
        "Lifetimes are a way for the compiler to track references.",
        &["rust"],
        embedding.clone(),
    )
    .await;

    create_question_with_tags(
        &question_repo,
        &tag_repo,
        &mut conn,
        "Python question about typing",
        "How do I use type hints in Python?",
        "Use the typing module and annotate function signatures.",
        &["python"],
        embedding.clone(),
    )
    .await;

    // Search with tag filter for "rust" should only return the Rust question
    let tags = vec!["rust".to_string()];
    let results = search_repo
        .search(&mut conn, embedding, Some(&tags), 10)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust question about lifetimes");
    assert!(results[0].tags.contains(&"rust".to_string()));
}

#[tokio::test]
async fn search_respects_limit() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let search_repo = PgSearchRepository;
    let mut conn = db.pool().get().await.unwrap();

    let embedding = uniform_embedding(0.3);

    // Create 3 questions with the same embedding
    for i in 0..3 {
        create_question_with_tags(
            &question_repo,
            &tag_repo,
            &mut conn,
            &format!("Limit test question number {}", i),
            &format!("Body for limit test question {}.", i),
            &format!("Answer for limit test question {}.", i),
            &[],
            embedding.clone(),
        )
        .await;
    }

    // Search with limit=2 should return at most 2 results
    let results = search_repo
        .search(&mut conn, embedding, None, 2)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 2, "limit=2 should cap results at 2");
}

#[tokio::test]
async fn search_includes_answers() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let answer_repo = PgAnswerRepository;
    let tag_repo = PgTagRepository;
    let search_repo = PgSearchRepository;
    let mut conn = db.pool().get().await.unwrap();

    let embedding = uniform_embedding(0.4);

    let question_id = create_question_with_tags(
        &question_repo,
        &tag_repo,
        &mut conn,
        "Question with multiple answers",
        "This question will have extra answers added.",
        "First answer from question creation.",
        &[],
        embedding.clone(),
    )
    .await;

    // Add a second answer
    answer_repo
        .create(
            &mut conn,
            question_id,
            "A second answer added after creation.",
            SYSTEM_USER_ID,
        )
        .await
        .expect("second answer should succeed");

    let results = search_repo
        .search(&mut conn, embedding, None, 5)
        .await
        .expect("search should succeed");

    assert!(!results.is_empty());
    let question = &results[0];
    assert_eq!(
        question.answers.len(),
        2,
        "search results should include both answers"
    );
}

#[tokio::test]
async fn search_returns_empty_for_no_matches() {
    let db = IntegrationTestDb::new().await;
    let question_repo = PgQuestionRepository;
    let tag_repo = PgTagRepository;
    let search_repo = PgSearchRepository;
    let mut conn = db.pool().get().await.unwrap();

    // Create a question with one embedding pattern
    create_question_with_tags(
        &question_repo,
        &tag_repo,
        &mut conn,
        "Question with specific embedding",
        "This question has a distinctive embedding vector.",
        "Answer for the distinctive question.",
        &[],
        marked_embedding(1.0, 0.0),
    )
    .await;

    // Search with a tag that does not match any question
    let tags = vec!["rust".to_string()];
    let results = search_repo
        .search(&mut conn, uniform_embedding(0.5), Some(&tags), 5)
        .await
        .expect("search should succeed");

    assert!(
        results.is_empty(),
        "search with non-matching tag should return empty"
    );
}
