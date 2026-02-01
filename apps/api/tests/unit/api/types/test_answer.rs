use tokenoverflow::api::types::CreateAnswerRequest;
use validator::Validate;

#[test]
fn validates_body_too_short() {
    let req = CreateAnswerRequest {
        body: "too short".to_string(), // 9 chars, min is 10
    };
    assert!(req.validate().is_err());
}

#[test]
fn validates_body_too_long() {
    let req = CreateAnswerRequest {
        body: "x".repeat(50_001),
    };
    assert!(req.validate().is_err());
}

#[test]
fn accepts_valid_request() {
    let req = CreateAnswerRequest {
        body: "a valid answer".to_string(),
    };
    assert!(req.validate().is_ok());
}
