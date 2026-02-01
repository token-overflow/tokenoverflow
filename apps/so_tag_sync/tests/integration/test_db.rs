use so_tag_sync::db;
use so_tag_sync::types::{StackOverflowSynonym, StackOverflowTag};

use super::test_db_infra::TestDb;

// ============================================================================
// create_pool
// ============================================================================

#[tokio::test]
async fn create_pool_succeeds() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await;
    assert!(pool.is_ok());
}

#[tokio::test]
async fn create_pool_invalid_url_fails_on_use() {
    // bb8 pool creation is lazy; actual connection failure surfaces on .get()
    let pool = db::create_pool("postgresql://invalid:invalid@127.0.0.1:1/nope")
        .await
        .unwrap();
    let result = pool.get().await;
    assert!(result.is_err());
}

// ============================================================================
// upsert_tags
// ============================================================================

#[tokio::test]
async fn upsert_tags_inserts_new_tags() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![
        StackOverflowTag {
            name: "rust".to_string(),
            count: 50000,
        },
        StackOverflowTag {
            name: "python".to_string(),
            count: 2000000,
        },
    ];

    let count = db::upsert_tags(&pool, &tags).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn upsert_tags_updates_existing() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![StackOverflowTag {
        name: "rust".to_string(),
        count: 50000,
    }];

    db::upsert_tags(&pool, &tags).await.unwrap();
    // Upsert again — should update rather than fail
    let count = db::upsert_tags(&pool, &tags).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_tags_empty_input() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let count = db::upsert_tags(&pool, &[]).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn upsert_tags_escapes_single_quotes() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![StackOverflowTag {
        name: "c#".to_string(),
        count: 100,
    }];

    let count = db::upsert_tags(&pool, &tags).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_tags_handles_large_batch() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    // Create 1500 tags to trigger chunking (chunks of 1000)
    let tags: Vec<StackOverflowTag> = (0..1500)
        .map(|i| StackOverflowTag {
            name: format!("tag-{}", i),
            count: i,
        })
        .collect();

    let count = db::upsert_tags(&pool, &tags).await.unwrap();
    assert_eq!(count, 1500);
}

#[tokio::test]
async fn upsert_tags_deduplicates_within_batch() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![
        StackOverflowTag {
            name: "rust".to_string(),
            count: 50000,
        },
        StackOverflowTag {
            name: "rust".to_string(),
            count: 50001,
        },
        StackOverflowTag {
            name: "python".to_string(),
            count: 2000000,
        },
    ];

    let count = db::upsert_tags(&pool, &tags).await.unwrap();
    assert_eq!(count, 2);
}

// ============================================================================
// upsert_synonyms
// ============================================================================

#[tokio::test]
async fn upsert_synonyms_inserts_new() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    // Insert target tags first
    let tags = vec![
        StackOverflowTag {
            name: "javascript".to_string(),
            count: 1,
        },
        StackOverflowTag {
            name: "python".to_string(),
            count: 1,
        },
    ];
    db::upsert_tags(&pool, &tags).await.unwrap();

    let synonyms = vec![
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
        StackOverflowSynonym {
            from: "py".to_string(),
            to: "python".to_string(),
        },
    ];

    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn upsert_synonyms_updates_existing() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![StackOverflowTag {
        name: "javascript".to_string(),
        count: 1,
    }];
    db::upsert_tags(&pool, &tags).await.unwrap();

    let synonyms = vec![StackOverflowSynonym {
        from: "js".to_string(),
        to: "javascript".to_string(),
    }];

    db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    // Upsert again — should update
    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_synonyms_empty_input() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let count = db::upsert_synonyms(&pool, &[]).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn upsert_synonyms_deduplicates_within_batch() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![StackOverflowTag {
        name: "javascript".to_string(),
        count: 1,
    }];
    db::upsert_tags(&pool, &tags).await.unwrap();

    let synonyms = vec![
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
    ];

    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_synonyms_skips_missing_target_tags() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    // Insert only one target tag — the other synonym references a missing tag
    let tags = vec![StackOverflowTag {
        name: "javascript".to_string(),
        count: 1,
    }];
    db::upsert_tags(&pool, &tags).await.unwrap();

    let synonyms = vec![
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
        StackOverflowSynonym {
            from: "py".to_string(),
            to: "nonexistent-tag".to_string(),
        },
    ];

    // Pre-filters synonyms with missing targets; only valid ones are inserted
    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn upsert_synonyms_all_targets_missing() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    // No tags inserted — all synonym targets are missing
    let synonyms = vec![
        StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        },
        StackOverflowSynonym {
            from: "py".to_string(),
            to: "python".to_string(),
        },
    ];

    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn upsert_synonyms_mixed_batch_across_chunks() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    // Insert 600 tags so we exercise chunking (chunks of 500)
    let tags: Vec<StackOverflowTag> = (0..600)
        .map(|i| StackOverflowTag {
            name: format!("tag-{}", i),
            count: i,
        })
        .collect();
    db::upsert_tags(&pool, &tags).await.unwrap();

    // 600 valid synonyms + 10 with missing targets, spread across chunks
    let mut synonyms: Vec<StackOverflowSynonym> = (0..600)
        .map(|i| StackOverflowSynonym {
            from: format!("syn-{}", i),
            to: format!("tag-{}", i),
        })
        .collect();
    for i in 0..10 {
        synonyms.push(StackOverflowSynonym {
            from: format!("bad-syn-{}", i),
            to: format!("missing-tag-{}", i),
        });
    }

    let count = db::upsert_synonyms(&pool, &synonyms).await.unwrap();
    assert_eq!(count, 600);
}

// ============================================================================
// get_last_sync_date
// ============================================================================

#[tokio::test]
async fn get_last_sync_date_empty_db() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let date = db::get_last_sync_date(&pool).await.unwrap();
    assert!(date.is_none());
}

#[tokio::test]
async fn get_last_sync_date_with_tags() {
    let test_db = TestDb::new().await;
    let pool = db::create_pool(&test_db.url).await.unwrap();

    let tags = vec![StackOverflowTag {
        name: "rust".to_string(),
        count: 1,
    }];
    db::upsert_tags(&pool, &tags).await.unwrap();

    let date = db::get_last_sync_date(&pool).await.unwrap();
    assert!(date.is_some());
}
