use tokenoverflow::api::types::CreateQuestionRequest;
use validator::Validate;

#[test]
fn validates_title_too_short() {
    let req = CreateQuestionRequest {
        title: "short".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_title_too_long() {
    let req = CreateQuestionRequest {
        title: "x".repeat(151),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_body_too_short() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "short".to_string(),
        answer: "a valid answer".to_string(),
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_body_too_long() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "x".repeat(1_501),
        answer: "a valid answer".to_string(),
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_answer_too_short() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "too short".to_string(), // 9 chars, min is 10
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_answer_too_long() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "x".repeat(50_001),
        tags: None,
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_too_many_tags() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: Some(vec!["tag".to_string(); 6]),
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_tag_too_long() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: Some(vec!["x".repeat(36)]),
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_empty_tag() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: Some(vec!["".to_string()]),
    };
    assert!(req.validate().is_err());
}

#[test]
fn accepts_valid_request() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: Some(vec!["rust".to_string()]),
    };
    assert!(req.validate().is_ok());
}

#[test]
fn accepts_request_without_tags() {
    let req = CreateQuestionRequest {
        title: "a valid title text".to_string(),
        body: "a valid body text here".to_string(),
        answer: "a valid answer".to_string(),
        tags: None,
    };
    assert!(req.validate().is_ok());
}
