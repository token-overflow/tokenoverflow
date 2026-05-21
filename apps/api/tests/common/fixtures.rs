#![allow(dead_code)]

use diesel::prelude::*;
use diesel_async::{AsyncPgConnection, RunQueryDsl};
use tokenoverflow::api::types::{CreateAnswerRequest, CreateQuestionRequest, SearchRequest};
use tokenoverflow::db::models::{NewUser, User};
use tokenoverflow::db::{users, waitlist};
use tokenoverflow::services::repository::{PgUserRepository, UserRepository};

/// Builder for CreateQuestionRequest with sensible defaults
pub struct QuestionRequestBuilder {
    title: String,
    body: String,
    answer: String,
    tags: Option<Vec<String>>,
}

impl Default for QuestionRequestBuilder {
    fn default() -> Self {
        Self {
            title: "How do I handle async errors in Rust?".to_string(),
            body: "I'm trying to handle errors from async functions but I keep getting compile errors about Send bounds.".to_string(),
            answer: "Use the ? operator with Box<dyn Error + Send + Sync> or the anyhow crate for easier error handling in async contexts.".to_string(),
            tags: Some(vec!["rust".to_string(), "async".to_string()]),
        }
    }
}

impl QuestionRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn answer(mut self, answer: impl Into<String>) -> Self {
        self.answer = answer.into();
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn no_tags(mut self) -> Self {
        self.tags = None;
        self
    }

    pub fn build(self) -> CreateQuestionRequest {
        CreateQuestionRequest {
            title: self.title,
            body: self.body,
            answer: self.answer,
            tags: self.tags,
        }
    }
}

/// Builder for CreateAnswerRequest with sensible defaults
pub struct AnswerRequestBuilder {
    body: String,
}

impl Default for AnswerRequestBuilder {
    fn default() -> Self {
        Self {
            body: "You can also try using tokio's JoinError handling for spawned tasks.".to_string(),
        }
    }
}

impl AnswerRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.body = body.into();
        self
    }

    pub fn build(self) -> CreateAnswerRequest {
        CreateAnswerRequest { body: self.body }
    }
}

/// Builder for SearchRequest with sensible defaults
pub struct SearchRequestBuilder {
    query: String,
    tags: Option<Vec<String>>,
    limit: Option<i32>,
}

impl Default for SearchRequestBuilder {
    fn default() -> Self {
        Self {
            query: "How do I handle async errors in Rust?".to_string(),
            tags: None,
            limit: None,
        }
    }
}

impl SearchRequestBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn build(self) -> SearchRequest {
        SearchRequest {
            query: self.query,
            tags: self.tags,
            limit: self.limit,
        }
    }
}

/// Seed an `api.users` row plus a matching approved `api.waitlist` row
/// so the integration test request flows through `jwt_auth_layer` exactly
/// the way prod handles a pre-approved returning applicant.
pub async fn seed_user_with_approval(
    conn: &mut AsyncPgConnection,
    workos_id: &str,
    github_id: i64,
    github_username: &str,
) -> User {
    let repo = PgUserRepository;
    let new_user = NewUser {
        workos_id: workos_id.to_string(),
        github_id: Some(github_id),
        username: github_username.to_string(),
    };
    let user = repo
        .create(conn, &new_user)
        .await
        .expect("seed_user_with_approval: user insert must succeed");

    diesel::insert_into(waitlist::table)
        .values((
            waitlist::github_id.eq(github_id),
            waitlist::github_username.eq(github_username),
            waitlist::email.eq(format!("{}@example.test", github_username)),
            waitlist::approved_at.eq(diesel::dsl::now),
            waitlist::user_id.eq(user.id),
        ))
        .on_conflict(waitlist::github_id)
        .do_update()
        .set((
            waitlist::approved_at.eq(diesel::dsl::now),
            waitlist::user_id.eq(user.id),
        ))
        .execute(conn)
        .await
        .expect("seed_user_with_approval: waitlist insert must succeed");

    user
}

/// Count `api.users` rows. Used by waitlist tests that assert the
/// signup endpoint does NOT provision a user row.
pub async fn count_users(conn: &mut AsyncPgConnection) -> i64 {
    users::table
        .count()
        .get_result(conn)
        .await
        .expect("count_users: select must succeed")
}
