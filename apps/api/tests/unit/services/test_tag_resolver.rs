//! Unit tests for TagResolver.
//!
//! Uses TagResolver::from_data — no external dependencies.

use std::collections::HashMap;
use tokenoverflow::services::TagResolver;

mod common {
    include!("../../common/mod.rs");
}

fn test_resolver() -> TagResolver {
    let mut synonyms = HashMap::new();
    synonyms.insert("js".to_string(), "javascript".to_string());
    synonyms.insert("ecmascript".to_string(), "javascript".to_string());
    synonyms.insert("vanillajs".to_string(), "javascript".to_string());
    synonyms.insert("py".to_string(), "python".to_string());
    synonyms.insert("ts".to_string(), "typescript".to_string());
    synonyms.insert("react".to_string(), "reactjs".to_string());

    let canonicals = vec![
        "javascript".to_string(),
        "python".to_string(),
        "typescript".to_string(),
        "reactjs".to_string(),
        "react-native".to_string(),
        "rust".to_string(),
        "r".to_string(),
    ];

    TagResolver::from_data(synonyms, canonicals)
}

// --- resolve() ---

#[test]
fn resolves_known_synonym() {
    let resolver = test_resolver();
    assert_eq!(resolver.resolve("js"), Some("javascript".to_string()));
}

#[test]
fn resolves_canonical() {
    let resolver = test_resolver();
    assert_eq!(
        resolver.resolve("javascript"),
        Some("javascript".to_string())
    );
}

#[test]
fn resolves_typo_via_similarity() {
    let resolver = test_resolver();
    // "javascrip" is close to "javascript" (JW ~0.98)
    assert_eq!(
        resolver.resolve("javascrip"),
        Some("javascript".to_string())
    );
}

#[test]
fn drops_unknown() {
    let resolver = test_resolver();
    assert_eq!(resolver.resolve("xyzgarbage"), None);
}

#[test]
fn short_canonical_exact_match() {
    let resolver = test_resolver();
    // "r" is a canonical tag
    assert_eq!(resolver.resolve("r"), Some("r".to_string()));
}

#[test]
fn no_false_positive_similar_tags() {
    let resolver = test_resolver();
    // "react" is a synonym for "reactjs", should not match "redux" or similar
    // (and it should resolve via synonym, not JW)
    let result = resolver.resolve("react");
    assert_eq!(result, Some("reactjs".to_string()));
}

#[test]
fn synonym_takes_priority_over_jw() {
    let resolver = test_resolver();
    // "js" is a synonym -- should hit layer 1, not fall through to JW
    assert_eq!(resolver.resolve("js"), Some("javascript".to_string()));
}

#[test]
fn canonical_takes_priority_over_jw() {
    let resolver = test_resolver();
    // "rust" is canonical -- should hit layer 2, not fall through to JW
    assert_eq!(resolver.resolve("rust"), Some("rust".to_string()));
}

#[test]
fn resolve_tags_with_invalid_chars() {
    let resolver = test_resolver();
    // "foo@bar" normalizes to "foobar", which won't match anything
    let result = resolver.resolve_tags(&["foo@bar".to_string()]);
    assert!(result.is_empty());
}

// --- resolve_tags() ---

#[test]
fn deduplicates_synonym_and_canonical() {
    let resolver = test_resolver();
    let result = resolver.resolve_tags(&["js".to_string(), "javascript".to_string()]);
    assert_eq!(result, vec!["javascript"]);
}

#[test]
fn deduplicates_multiple_synonyms() {
    let resolver = test_resolver();
    let result = resolver.resolve_tags(&["js".to_string(), "ecmascript".to_string()]);
    assert_eq!(result, vec!["javascript"]);
}

#[test]
fn normalizes_before_resolving() {
    let resolver = test_resolver();
    // "JS" normalizes to "js", which is a synonym for "javascript"
    let result = resolver.resolve_tags(&["JS".to_string()]);
    assert_eq!(result, vec!["javascript"]);
}

#[test]
fn drops_all_unknown() {
    let resolver = test_resolver();
    let result = resolver.resolve_tags(&["xyzgarbage".to_string(), "asdfqwer".to_string()]);
    assert!(result.is_empty());
}

#[test]
fn preserves_order() {
    let resolver = test_resolver();
    // "react" is a synonym for "reactjs", "js" is a synonym for "javascript"
    let result = resolver.resolve_tags(&["react".to_string(), "js".to_string()]);
    assert_eq!(result, vec!["reactjs", "javascript"]);
}

#[test]
fn mixed_resolve_and_drop() {
    let resolver = test_resolver();
    let result = resolver.resolve_tags(&[
        "js".to_string(),
        "xyzgarbage".to_string(),
        "react".to_string(),
    ]);
    assert_eq!(result, vec!["javascript", "reactjs"]);
}

#[test]
fn empty_input() {
    let resolver = test_resolver();
    let result = resolver.resolve_tags(&[]);
    assert!(result.is_empty());
}

#[test]
fn similarity_threshold_boundary_above() {
    // "typescrip" vs "typescript" should be above 0.85
    let resolver = test_resolver();
    let result = resolver.resolve("typescrip");
    assert_eq!(result, Some("typescript".to_string()));
}

#[test]
fn similarity_threshold_boundary_below() {
    // A short unrelated string should score below 0.85
    let resolver = test_resolver();
    let result = resolver.resolve("zzz");
    assert_eq!(result, None);
}

// --- refresh() ---

#[tokio::test]
async fn refresh_reloads_data() {
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockTagRepository::new(store.clone());

    // Start with empty data
    let resolver = TagResolver::from_data(HashMap::new(), vec![]);
    assert_eq!(resolver.resolve("javascript"), None);

    // Refresh loads seed data from the mock repository
    let mut conn = common::NoopConn;
    resolver.refresh(&repo, &mut conn).await.unwrap();

    assert_eq!(
        resolver.resolve("javascript"),
        Some("javascript".to_string())
    );
    assert_eq!(resolver.resolve("js"), Some("javascript".to_string()));
}

#[tokio::test]
async fn refresh_replaces_old_data() {
    let store = common::MockStore::new();
    // Pre-populate with one tag
    {
        let mut tags = store.tags.lock().unwrap();
        tags.push(common::mock_repository::StoredTag {
            id: uuid::Uuid::now_v7(),
            name: "old-tag".to_string(),
        });
    }
    let repo = common::MockTagRepository::new(store.clone());

    let resolver = TagResolver::from_data(HashMap::new(), vec![]);
    let mut conn = common::NoopConn;
    resolver.refresh(&repo, &mut conn).await.unwrap();
    assert_eq!(resolver.resolve("old-tag"), Some("old-tag".to_string()));

    // Update the store and refresh again
    {
        let mut tags = store.tags.lock().unwrap();
        tags.clear();
        tags.push(common::mock_repository::StoredTag {
            id: uuid::Uuid::now_v7(),
            name: "new-tag".to_string(),
        });
    }
    resolver.refresh(&repo, &mut conn).await.unwrap();
    assert_eq!(resolver.resolve("old-tag"), None);
    assert_eq!(resolver.resolve("new-tag"), Some("new-tag".to_string()));
}

// --- new() (async constructor) ---

#[tokio::test]
async fn new_loads_from_repository() {
    let store = common::MockStore::with_seed_tags();
    let repo = common::MockTagRepository::new(store);

    let mut conn = common::NoopConn;
    let resolver = TagResolver::new(&repo, &mut conn).await.unwrap();

    assert_eq!(
        resolver.resolve("javascript"),
        Some("javascript".to_string())
    );
    assert_eq!(resolver.resolve("js"), Some("javascript".to_string()));
    assert_eq!(resolver.resolve("rust"), Some("rust".to_string()));
}
