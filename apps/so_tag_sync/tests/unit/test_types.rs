use so_tag_sync::types::{
    ApiErrorResponse, ApiResponse, ApiSynonym, ApiTag, StackOverflowSynonym, StackOverflowTag,
    SynonymsFile, TagsFile,
};

#[test]
fn tags_file_serialization_roundtrip() {
    let file = TagsFile {
        fetched_at: chrono::Utc::now(),
        tags: vec![
            StackOverflowTag {
                name: "rust".to_string(),
                count: 50000,
            },
            StackOverflowTag {
                name: "python".to_string(),
                count: 2000000,
            },
        ],
    };

    let json = serde_json::to_string(&file).unwrap();
    let parsed: TagsFile = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.tags.len(), 2);
    assert_eq!(parsed.tags[0].name, "rust");
    assert_eq!(parsed.tags[0].count, 50000);
    assert_eq!(parsed.tags[1].name, "python");
}

#[test]
fn synonyms_file_serialization_roundtrip() {
    let file = SynonymsFile {
        fetched_at: chrono::Utc::now(),
        synonyms: vec![StackOverflowSynonym {
            from: "js".to_string(),
            to: "javascript".to_string(),
        }],
    };

    let json = serde_json::to_string(&file).unwrap();
    let parsed: SynonymsFile = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.synonyms.len(), 1);
    assert_eq!(parsed.synonyms[0].from, "js");
    assert_eq!(parsed.synonyms[0].to, "javascript");
}

#[test]
fn tag_count_defaults_to_zero() {
    let json = r#"{"name": "rust"}"#;
    let tag: StackOverflowTag = serde_json::from_str(json).unwrap();
    assert_eq!(tag.name, "rust");
    assert_eq!(tag.count, 0);
}

#[test]
fn api_response_deserializes_tags() {
    let json = r#"{
        "items": [
            {"name": "rust", "count": 50000},
            {"name": "go", "count": 80000}
        ],
        "has_more": true,
        "backoff": 5,
        "quota_remaining": 9990
    }"#;

    let response: ApiResponse<ApiTag> = serde_json::from_str(json).unwrap();
    assert_eq!(response.items.len(), 2);
    assert!(response.has_more);
    assert_eq!(response.backoff, Some(5));
    assert_eq!(response.quota_remaining, Some(9990));
    assert_eq!(response.items[0].name, "rust");
    assert_eq!(response.items[1].count, 80000);
}

#[test]
fn api_response_optional_fields_default_to_none() {
    let json = r#"{
        "items": [],
        "has_more": false
    }"#;

    let response: ApiResponse<ApiTag> = serde_json::from_str(json).unwrap();
    assert!(response.items.is_empty());
    assert!(!response.has_more);
    assert!(response.backoff.is_none());
    assert!(response.quota_remaining.is_none());
}

#[test]
fn api_response_deserializes_synonyms() {
    let json = r#"{
        "items": [
            {"from_tag": "js", "to_tag": "javascript"},
            {"from_tag": "py", "to_tag": "python"}
        ],
        "has_more": false
    }"#;

    let response: ApiResponse<ApiSynonym> = serde_json::from_str(json).unwrap();
    assert_eq!(response.items.len(), 2);
    assert_eq!(response.items[0].from_tag, "js");
    assert_eq!(response.items[0].to_tag, "javascript");
}

#[test]
fn api_error_response_deserializes() {
    let json = r#"{
        "error_id": 403,
        "error_name": "access_denied",
        "error_message": "This method requires an access_token"
    }"#;

    let error: ApiErrorResponse = serde_json::from_str(json).unwrap();
    assert_eq!(error.error_id, Some(403));
    assert_eq!(error.error_name.unwrap(), "access_denied");
    assert_eq!(
        error.error_message.unwrap(),
        "This method requires an access_token"
    );
}

#[test]
fn api_error_response_handles_missing_fields() {
    let json = r#"{}"#;
    let error: ApiErrorResponse = serde_json::from_str(json).unwrap();
    assert!(error.error_id.is_none());
    assert!(error.error_name.is_none());
    assert!(error.error_message.is_none());
}

#[test]
fn stackoverflow_tag_clone() {
    let tag = StackOverflowTag {
        name: "rust".to_string(),
        count: 42,
    };
    let cloned = tag.clone();
    assert_eq!(cloned.name, "rust");
    assert_eq!(cloned.count, 42);
}

#[test]
fn stackoverflow_synonym_clone() {
    let synonym = StackOverflowSynonym {
        from: "k8s".to_string(),
        to: "kubernetes".to_string(),
    };
    let cloned = synonym.clone();
    assert_eq!(cloned.from, "k8s");
    assert_eq!(cloned.to, "kubernetes");
}
