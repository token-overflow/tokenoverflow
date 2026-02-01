mod interface;
mod postgres;

pub use interface::AnswerRepository;
pub use interface::QuestionRepository;
pub use interface::SearchRepository;
pub use interface::TagRepository;
pub use interface::UserRepository;
pub use postgres::PgAnswerRepository;
pub use postgres::PgQuestionRepository;
pub use postgres::PgSearchRepository;
pub use postgres::PgTagRepository;
pub use postgres::PgUserRepository;
