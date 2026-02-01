#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

use tokenoverflow::api::types::{
    AnswerResponse, CreateQuestionResponse, QuestionWithAnswers, SearchResultQuestion,
};
use tokenoverflow::constants::SYSTEM_USER_ID;
use tokenoverflow::db::models::{NewUser, User};
use tokenoverflow::error::AppError;
use tokenoverflow::services::repository::{
    AnswerRepository, QuestionRepository, SearchRepository, TagRepository, UserRepository,
};

// ---------------------------------------------------------------------------
// ID generation for mocks
// ---------------------------------------------------------------------------

fn next_id() -> Uuid {
    Uuid::now_v7()
}

// ---------------------------------------------------------------------------
// Internal storage types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct StoredQuestion {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredAnswer {
    pub id: Uuid,
    pub question_id: Uuid,
    pub body: String,
    pub submitted_by: Uuid,
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct StoredVote {
    pub answer_id: Uuid,
    pub user_id: Uuid,
    pub value: i32,
}

#[derive(Clone, Debug)]
pub struct StoredQuestionTag {
    pub question_id: Uuid,
    pub tag_id: Uuid,
}

#[derive(Clone, Debug)]
pub struct StoredTag {
    pub id: Uuid,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct StoredSynonym {
    pub synonym: String,
    pub canonical: String,
}

// ---------------------------------------------------------------------------
// Shared in-memory store
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct MockStore {
    pub questions: Arc<Mutex<Vec<StoredQuestion>>>,
    pub answers: Arc<Mutex<Vec<StoredAnswer>>>,
    pub votes: Arc<Mutex<Vec<StoredVote>>>,
    pub question_tags: Arc<Mutex<Vec<StoredQuestionTag>>>,
    pub tags: Arc<Mutex<Vec<StoredTag>>>,
    pub synonyms: Arc<Mutex<Vec<StoredSynonym>>>,
}

impl MockStore {
    pub fn new() -> Self {
        Self {
            questions: Arc::new(Mutex::new(Vec::new())),
            answers: Arc::new(Mutex::new(Vec::new())),
            votes: Arc::new(Mutex::new(Vec::new())),
            question_tags: Arc::new(Mutex::new(Vec::new())),
            tags: Arc::new(Mutex::new(Vec::new())),
            synonyms: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a store pre-populated with the seed tags and synonyms
    /// (matching the migration seed data).
    pub fn with_seed_tags() -> Self {
        let store = Self::new();

        let seed_tags = vec![
            "javascript",
            "python",
            "java",
            "c#",
            "php",
            "android",
            "html",
            "jquery",
            "c++",
            "css",
            "ios",
            "mysql",
            "sql",
            "r",
            "node.js",
            "reactjs",
            "arrays",
            "c",
            "asp.net",
            "json",
            "ruby-on-rails",
            ".net",
            "sql-server",
            "swift",
            "python-3.x",
            "objective-c",
            "django",
            "angular",
            "excel",
            "regex",
            "pandas",
            "ruby",
            "linux",
            "ajax",
            "typescript",
            "xml",
            "vb.net",
            "spring",
            "database",
            "wordpress",
            "string",
            "mongodb",
            "postgresql",
            "windows",
            "git",
            "bash",
            "firebase",
            "algorithm",
            "docker",
            "list",
            "amazon-web-services",
            "azure",
            "spring-boot",
            "vue.js",
            "dataframe",
            "multithreading",
            "flutter",
            "api",
            "function",
            "image",
            "tensorflow",
            "numpy",
            "kotlin",
            "rest",
            "google-chrome",
            "maven",
            "selenium",
            "react-native",
            "eclipse",
            "performance",
            "macos",
            "powershell",
            "matplotlib",
            "dictionary",
            "unit-testing",
            "go",
            "scala",
            "class",
            "dart",
            "perl",
            "apache",
            "visual-studio",
            "nginx",
            "laravel",
            "express",
            "machine-learning",
            "css-selectors",
            "xcode",
            "google-maps",
            "rust",
            "graphql",
            "redis",
            "hadoop",
            "webpack",
            "xaml",
            "svelte",
            "next.js",
            "flask",
            "fastapi",
            "tailwindcss",
            "kubernetes",
            "github-actions",
            "terraform",
            "elasticsearch",
        ];

        {
            let mut tags_lock = store.tags.lock().unwrap();
            for name in seed_tags.iter() {
                tags_lock.push(StoredTag {
                    id: next_id(),
                    name: name.to_string(),
                });
            }
        }

        let seed_synonyms = vec![
            ("js", "javascript"),
            ("ecmascript", "javascript"),
            ("vanillajs", "javascript"),
            ("py", "python"),
            ("python3", "python"),
            ("ts", "typescript"),
            ("golang", "go"),
            ("k8s", "kubernetes"),
            ("postgres", "postgresql"),
            ("node", "node.js"),
            ("nodejs", "node.js"),
            ("react", "reactjs"),
            ("nextjs", "next.js"),
            ("vuejs", "vue.js"),
            ("vue", "vue.js"),
        ];

        {
            let mut synonyms_lock = store.synonyms.lock().unwrap();
            for (synonym, canonical) in seed_synonyms {
                synonyms_lock.push(StoredSynonym {
                    synonym: synonym.to_string(),
                    canonical: canonical.to_string(),
                });
            }
        }

        store
    }
}

impl Default for MockStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// MockQuestionRepository
// ---------------------------------------------------------------------------

pub struct MockQuestionRepository {
    store: MockStore,
}

impl MockQuestionRepository {
    pub fn new(store: MockStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl<Conn: Send + 'static> QuestionRepository<Conn> for MockQuestionRepository {
    async fn create(
        &self,
        _conn: &mut Conn,
        title: &str,
        body: &str,
        answer_body: &str,
        _embedding: Vec<f32>,
        submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError> {
        let question_id = next_id();
        let answer_id = next_id();
        let now = Utc::now();

        {
            let mut questions = self.store.questions.lock().unwrap();
            questions.push(StoredQuestion {
                id: question_id,
                title: title.to_string(),
                body: body.to_string(),
                tags: vec![],
                created_at: now,
            });
        }

        {
            let mut answers = self.store.answers.lock().unwrap();
            answers.push(StoredAnswer {
                id: answer_id,
                question_id,
                body: answer_body.to_string(),
                submitted_by,
                upvotes: 0,
                downvotes: 0,
                created_at: now,
            });
        }

        Ok(CreateQuestionResponse {
            question_id,
            answer_id,
        })
    }

    async fn get_by_id(
        &self,
        _conn: &mut Conn,
        id: Uuid,
    ) -> Result<QuestionWithAnswers, AppError> {
        let question = {
            let questions = self.store.questions.lock().unwrap();
            questions
                .iter()
                .find(|q| q.id == id)
                .cloned()
                .ok_or_else(|| AppError::NotFound(format!("Question {} not found", id)))?
        };

        // Collect tags from the question_tags join store
        let tag_names = {
            let qt = self.store.question_tags.lock().unwrap();
            let tags = self.store.tags.lock().unwrap();
            qt.iter()
                .filter(|qt| qt.question_id == id)
                .filter_map(|qt| tags.iter().find(|t| t.id == qt.tag_id))
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
        };

        let answers = {
            let answers = self.store.answers.lock().unwrap();
            answers
                .iter()
                .filter(|a| a.question_id == id)
                .map(|a| AnswerResponse {
                    id: a.id,
                    body: a.body.clone(),
                    upvotes: a.upvotes,
                    downvotes: a.downvotes,
                    created_at: a.created_at,
                })
                .collect()
        };

        Ok(QuestionWithAnswers {
            id: question.id,
            title: question.title,
            body: question.body,
            tags: tag_names,
            created_at: question.created_at,
            answers,
        })
    }

    async fn exists(&self, _conn: &mut Conn, id: Uuid) -> Result<bool, AppError> {
        let questions = self.store.questions.lock().unwrap();
        Ok(questions.iter().any(|q| q.id == id))
    }
}

// ---------------------------------------------------------------------------
// MockTagRepository
// ---------------------------------------------------------------------------

pub struct MockTagRepository {
    store: MockStore,
}

impl MockTagRepository {
    pub fn new(store: MockStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl<Conn: Send + 'static> TagRepository<Conn> for MockTagRepository {
    async fn load_synonyms(
        &self,
        _conn: &mut Conn,
    ) -> Result<HashMap<String, String>, AppError> {
        let synonyms = self.store.synonyms.lock().unwrap();
        Ok(synonyms
            .iter()
            .map(|s| (s.synonym.clone(), s.canonical.clone()))
            .collect())
    }

    async fn load_canonicals(&self, _conn: &mut Conn) -> Result<Vec<String>, AppError> {
        let tags = self.store.tags.lock().unwrap();
        Ok(tags.iter().map(|t| t.name.clone()).collect())
    }

    async fn find_tag_ids(
        &self,
        _conn: &mut Conn,
        names: &[String],
    ) -> Result<Vec<(String, Uuid)>, AppError> {
        let tags = self.store.tags.lock().unwrap();
        Ok(tags
            .iter()
            .filter(|t| names.contains(&t.name))
            .map(|t| (t.name.clone(), t.id))
            .collect())
    }

    async fn link_tags_to_question(
        &self,
        _conn: &mut Conn,
        question_id: Uuid,
        tag_ids: &[Uuid],
    ) -> Result<(), AppError> {
        let mut qt = self.store.question_tags.lock().unwrap();
        for &tag_id in tag_ids {
            // ON CONFLICT DO NOTHING equivalent
            if !qt
                .iter()
                .any(|r| r.question_id == question_id && r.tag_id == tag_id)
            {
                qt.push(StoredQuestionTag {
                    question_id,
                    tag_id,
                });
            }
        }
        Ok(())
    }

    async fn get_question_tags(
        &self,
        _conn: &mut Conn,
        question_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        let qt = self.store.question_tags.lock().unwrap();
        let tags = self.store.tags.lock().unwrap();
        Ok(qt
            .iter()
            .filter(|r| r.question_id == question_id)
            .filter_map(|r| tags.iter().find(|t| t.id == r.tag_id))
            .map(|t| t.name.clone())
            .collect())
    }
}

// ---------------------------------------------------------------------------
// MockAnswerRepository
// ---------------------------------------------------------------------------

pub struct MockAnswerRepository {
    store: MockStore,
}

impl MockAnswerRepository {
    pub fn new(store: MockStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl<Conn: Send + 'static> AnswerRepository<Conn> for MockAnswerRepository {
    async fn create(
        &self,
        _conn: &mut Conn,
        question_id: Uuid,
        body: &str,
        submitted_by: Uuid,
    ) -> Result<Uuid, AppError> {
        // Validate the question exists
        {
            let questions = self.store.questions.lock().unwrap();
            if !questions.iter().any(|q| q.id == question_id) {
                return Err(AppError::NotFound(format!(
                    "Question {} not found",
                    question_id
                )));
            }
        }

        let answer_id = next_id();
        let now = Utc::now();

        {
            let mut answers = self.store.answers.lock().unwrap();
            answers.push(StoredAnswer {
                id: answer_id,
                question_id,
                body: body.to_string(),
                submitted_by,
                upvotes: 0,
                downvotes: 0,
                created_at: now,
            });
        }

        Ok(answer_id)
    }

    async fn get_submitted_by(
        &self,
        _conn: &mut Conn,
        answer_id: Uuid,
    ) -> Result<Uuid, AppError> {
        let answers = self.store.answers.lock().unwrap();
        answers
            .iter()
            .find(|a| a.id == answer_id)
            .map(|a| a.submitted_by)
            .ok_or_else(|| AppError::NotFound(format!("Answer {} not found", answer_id)))
    }

    async fn upvote(
        &self,
        _conn: &mut Conn,
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.vote(answer_id, user_id, 1)
    }

    async fn downvote(
        &self,
        _conn: &mut Conn,
        answer_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), AppError> {
        self.vote(answer_id, user_id, -1)
    }

    async fn exists(&self, _conn: &mut Conn, id: Uuid) -> Result<bool, AppError> {
        let answers = self.store.answers.lock().unwrap();
        Ok(answers.iter().any(|a| a.id == id))
    }
}

impl MockAnswerRepository {
    fn vote(&self, answer_id: Uuid, user_id: Uuid, value: i32) -> Result<(), AppError> {
        // Validate the answer exists
        {
            let answers = self.store.answers.lock().unwrap();
            if !answers.iter().any(|a| a.id == answer_id) {
                return Err(AppError::NotFound(format!(
                    "Answer {} not found",
                    answer_id
                )));
            }
        }

        // Upsert vote
        {
            let mut votes = self.store.votes.lock().unwrap();
            if let Some(existing) = votes
                .iter_mut()
                .find(|v| v.answer_id == answer_id && v.user_id == user_id)
            {
                existing.value = value;
            } else {
                votes.push(StoredVote {
                    answer_id,
                    user_id,
                    value,
                });
            }
        }

        // Recalculate counts on the answer
        {
            let votes = self.store.votes.lock().unwrap();
            let upvotes = votes
                .iter()
                .filter(|v| v.answer_id == answer_id && v.value == 1)
                .count() as i32;
            let downvotes = votes
                .iter()
                .filter(|v| v.answer_id == answer_id && v.value == -1)
                .count() as i32;

            let mut answers = self.store.answers.lock().unwrap();
            if let Some(answer) = answers.iter_mut().find(|a| a.id == answer_id) {
                answer.upvotes = upvotes;
                answer.downvotes = downvotes;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MockSearchRepository
// ---------------------------------------------------------------------------

pub struct MockSearchRepository {
    store: MockStore,
}

impl MockSearchRepository {
    pub fn new(store: MockStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl<Conn: Send + 'static> SearchRepository<Conn> for MockSearchRepository {
    async fn search(
        &self,
        _conn: &mut Conn,
        _embedding: Vec<f32>,
        tags: Option<&[String]>,
        limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError> {
        let questions = self.store.questions.lock().unwrap();
        let answers = self.store.answers.lock().unwrap();
        let qt = self.store.question_tags.lock().unwrap();
        let tag_store = self.store.tags.lock().unwrap();

        let mut results: Vec<SearchResultQuestion> = questions
            .iter()
            .filter(|q| {
                // Filter by tags if provided (via question_tags join)
                if let Some(filter_tags) = tags {
                    let q_tag_names: Vec<String> = qt
                        .iter()
                        .filter(|r| r.question_id == q.id)
                        .filter_map(|r| tag_store.iter().find(|t| t.id == r.tag_id))
                        .map(|t| t.name.clone())
                        .collect();
                    filter_tags.iter().all(|t| q_tag_names.contains(t))
                } else {
                    true
                }
            })
            .map(|q| {
                let question_answers: Vec<AnswerResponse> = answers
                    .iter()
                    .filter(|a| a.question_id == q.id)
                    .map(|a| AnswerResponse {
                        id: a.id,
                        body: a.body.clone(),
                        upvotes: a.upvotes,
                        downvotes: a.downvotes,
                        created_at: a.created_at,
                    })
                    .collect();

                let question_tags: Vec<String> = qt
                    .iter()
                    .filter(|r| r.question_id == q.id)
                    .filter_map(|r| tag_store.iter().find(|t| t.id == r.tag_id))
                    .map(|t| t.name.clone())
                    .collect();

                SearchResultQuestion {
                    id: q.id,
                    title: q.title.clone(),
                    body: q.body.clone(),
                    tags: question_tags,
                    similarity: 0.95,
                    answers: question_answers,
                }
            })
            .collect();

        // Stable order, then apply limit
        results.truncate(limit as usize);

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Failing repositories (always return errors)
// ---------------------------------------------------------------------------

pub struct FailingQuestionRepository;

#[async_trait]
impl<Conn: Send + 'static> QuestionRepository<Conn> for FailingQuestionRepository {
    async fn create(
        &self,
        _conn: &mut Conn,
        _title: &str,
        _body: &str,
        _answer_body: &str,
        _embedding: Vec<f32>,
        _submitted_by: Uuid,
    ) -> Result<CreateQuestionResponse, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn get_by_id(
        &self,
        _conn: &mut Conn,
        _id: Uuid,
    ) -> Result<QuestionWithAnswers, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn exists(&self, _conn: &mut Conn, _id: Uuid) -> Result<bool, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
}

pub struct FailingAnswerRepository;

#[async_trait]
impl<Conn: Send + 'static> AnswerRepository<Conn> for FailingAnswerRepository {
    async fn create(
        &self,
        _conn: &mut Conn,
        _question_id: Uuid,
        _body: &str,
        _submitted_by: Uuid,
    ) -> Result<Uuid, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn get_submitted_by(
        &self,
        _conn: &mut Conn,
        _answer_id: Uuid,
    ) -> Result<Uuid, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn upvote(
        &self,
        _conn: &mut Conn,
        _answer_id: Uuid,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn downvote(
        &self,
        _conn: &mut Conn,
        _answer_id: Uuid,
        _user_id: Uuid,
    ) -> Result<(), AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn exists(&self, _conn: &mut Conn, _id: Uuid) -> Result<bool, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
}

pub struct FailingSearchRepository;

#[async_trait]
impl<Conn: Send + 'static> SearchRepository<Conn> for FailingSearchRepository {
    async fn search(
        &self,
        _conn: &mut Conn,
        _embedding: Vec<f32>,
        _tags: Option<&[String]>,
        _limit: i32,
    ) -> Result<Vec<SearchResultQuestion>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
}

// ---------------------------------------------------------------------------
// MockUserRepository
// ---------------------------------------------------------------------------

pub struct MockUserRepository {
    users: Arc<Mutex<Vec<User>>>,
}

impl MockUserRepository {
    pub fn new() -> Self {
        // Seed the system user with the well-known UUID constant
        let system_user = User {
            id: SYSTEM_USER_ID,
            workos_id: "system".to_string(),
            github_id: None,
            username: "system".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        Self {
            users: Arc::new(Mutex::new(vec![system_user])),
        }
    }

    /// Pre-seed a user so `find_by_workos_id` returns it without needing
    /// JIT provisioning (which requires the WorkOS API).
    pub fn seed_user(&self, workos_id: &str) {
        let mut users = self.users.lock().unwrap();
        if users.iter().any(|u| u.workos_id == workos_id) {
            return;
        }
        let id = next_id();
        let now = Utc::now();
        users.push(User {
            id,
            workos_id: workos_id.to_string(),
            github_id: None,
            username: workos_id.to_string(),
            created_at: now,
            updated_at: now,
        });
    }
}

#[async_trait]
impl<Conn: Send + 'static> UserRepository<Conn> for MockUserRepository {
    async fn find_by_workos_id(
        &self,
        _conn: &mut Conn,
        workos_id: &str,
    ) -> Result<Option<User>, AppError> {
        let users = self.users.lock().unwrap();
        Ok(users.iter().find(|u| u.workos_id == workos_id).cloned())
    }

    async fn create(
        &self,
        _conn: &mut Conn,
        new_user: &NewUser,
    ) -> Result<User, AppError> {
        let mut users = self.users.lock().unwrap();

        // Check for conflict (ON CONFLICT DO NOTHING equivalent)
        if let Some(existing) = users.iter().find(|u| u.workos_id == new_user.workos_id) {
            return Ok(existing.clone());
        }

        let id = next_id();
        let now = Utc::now();
        let user = User {
            id,
            workos_id: new_user.workos_id.clone(),
            github_id: new_user.github_id,
            username: new_user.username.clone(),
            created_at: now,
            updated_at: now,
        };

        users.push(user.clone());
        Ok(user)
    }
}

// ---------------------------------------------------------------------------
// Failing repositories (always return errors)
// ---------------------------------------------------------------------------

pub struct FailingUserRepository;

#[async_trait]
impl<Conn: Send + 'static> UserRepository<Conn> for FailingUserRepository {
    async fn find_by_workos_id(
        &self,
        _conn: &mut Conn,
        _workos_id: &str,
    ) -> Result<Option<User>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn create(&self, _conn: &mut Conn, _new_user: &NewUser) -> Result<User, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
}

pub struct FailingTagRepository;

#[async_trait]
impl<Conn: Send + 'static> TagRepository<Conn> for FailingTagRepository {
    async fn load_synonyms(
        &self,
        _conn: &mut Conn,
    ) -> Result<HashMap<String, String>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn load_canonicals(&self, _conn: &mut Conn) -> Result<Vec<String>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn find_tag_ids(
        &self,
        _conn: &mut Conn,
        _names: &[String],
    ) -> Result<Vec<(String, Uuid)>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn link_tags_to_question(
        &self,
        _conn: &mut Conn,
        _question_id: Uuid,
        _tag_ids: &[Uuid],
    ) -> Result<(), AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }

    async fn get_question_tags(
        &self,
        _conn: &mut Conn,
        _question_id: Uuid,
    ) -> Result<Vec<String>, AppError> {
        Err(AppError::Internal("mock repository failure".to_string()))
    }
}
