use embedding_service::model::{Embedder, EmbeddingModel, OUTPUT_DIMENSION};

#[test]
fn new_creates_model_successfully() {
    let model = EmbeddingModel::new();
    assert!(model.is_ok(), "EmbeddingModel::new() should succeed");
}

#[test]
fn embed_returns_correct_dimension() {
    let model = EmbeddingModel::new().expect("Failed to create model");
    let texts = vec!["test input".to_string()];

    let embeddings = model.embed(&texts).expect("Failed to embed");

    assert_eq!(embeddings.len(), 1);
    assert_eq!(embeddings[0].len(), OUTPUT_DIMENSION);
}

#[test]
fn embed_handles_multiple_texts() {
    let model = EmbeddingModel::new().expect("Failed to create model");
    let texts = vec![
        "first text".to_string(),
        "second text".to_string(),
        "third text".to_string(),
    ];

    let embeddings = model.embed(&texts).expect("Failed to embed");

    assert_eq!(embeddings.len(), 3);
    for emb in &embeddings {
        assert_eq!(emb.len(), OUTPUT_DIMENSION);
    }
}

#[test]
fn embed_produces_deterministic_results() {
    let model = EmbeddingModel::new().expect("Failed to create model");
    let texts = vec!["deterministic test".to_string()];

    let emb1 = model.embed(&texts).expect("Failed to embed first time");
    let emb2 = model.embed(&texts).expect("Failed to embed second time");

    assert_eq!(emb1, emb2, "Same input should produce identical embeddings");
}

#[test]
fn embed_produces_different_results_for_different_inputs() {
    let model = EmbeddingModel::new().expect("Failed to create model");
    let texts1 = vec!["rust programming".to_string()];
    let texts2 = vec!["cooking recipes".to_string()];

    let emb1 = model.embed(&texts1).expect("Failed to embed first");
    let emb2 = model.embed(&texts2).expect("Failed to embed second");

    assert_ne!(
        emb1, emb2,
        "Different inputs should produce different embeddings"
    );
}

#[test]
fn embed_handles_empty_input() {
    let model = EmbeddingModel::new().expect("Failed to create model");
    let texts: Vec<String> = vec![];

    let embeddings = model.embed(&texts).expect("Failed to embed empty input");

    assert_eq!(embeddings.len(), 0);
}
