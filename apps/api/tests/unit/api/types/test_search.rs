use tokenoverflow::api::types::SearchRequest;
use validator::Validate;

#[test]
fn validates_query_too_short() {
    let req = SearchRequest {
        query: "short".to_string(),
        tags: None,
        limit: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_query_too_long() {
    let req = SearchRequest {
        query: "x".repeat(10_001),
        tags: None,
        limit: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_too_many_tags() {
    let req = SearchRequest {
        query: "a valid query text".to_string(),
        tags: Some(vec!["tag".to_string(); 6]),
        limit: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_limit_too_low() {
    let req = SearchRequest {
        query: "a valid query text".to_string(),
        tags: None,
        limit: Some(0),
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_limit_too_high() {
    let req = SearchRequest {
        query: "a valid query text".to_string(),
        tags: None,
        limit: Some(11),
    };
    assert!(req.validate().is_err());
}

#[test]
fn accepts_valid_request() {
    let req = SearchRequest {
        query: "a valid query text".to_string(),
        tags: Some(vec!["rust".to_string()]),
        limit: Some(5),
    };
    assert!(req.validate().is_ok());
}

#[test]
fn accepts_request_without_optional_fields() {
    let req = SearchRequest {
        query: "a valid query text".to_string(),
        tags: None,
        limit: None,
    };
    assert!(req.validate().is_ok());
}
