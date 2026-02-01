use tokenoverflow::db::models::NewUser;
use tokenoverflow::services::repository::{PgUserRepository, UserRepository};

use crate::test_db::IntegrationTestDb;

fn test_new_user(workos_id: &str) -> NewUser {
    NewUser {
        workos_id: workos_id.to_string(),
        github_id: Some(12345),
        username: "testuser".to_string(),
    }
}

#[tokio::test]
async fn find_by_workos_id_returns_none_for_unknown() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let result = repo
        .find_by_workos_id(&mut *conn, "user_nonexistent")
        .await
        .expect("find_by_workos_id should not error");

    assert!(result.is_none());
}

#[tokio::test]
async fn create_returns_user_with_all_fields() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let new_user = test_new_user("user_create_test");
    let user = repo
        .create(&mut *conn, &new_user)
        .await
        .expect("create should succeed");

    assert!(!user.id.is_nil());
    assert_eq!(user.workos_id, "user_create_test");
    assert_eq!(user.github_id, Some(12345));
    assert_eq!(user.username, "testuser");
}

#[tokio::test]
async fn create_then_find_returns_same_user() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let new_user = test_new_user("user_find_test");
    let created = repo
        .create(&mut *conn, &new_user)
        .await
        .expect("create should succeed");

    let found = repo
        .find_by_workos_id(&mut *conn, "user_find_test")
        .await
        .expect("find should succeed")
        .expect("user should exist");

    assert_eq!(found.id, created.id);
    assert_eq!(found.workos_id, created.workos_id);
}

#[tokio::test]
async fn create_with_duplicate_workos_id_returns_existing() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let new_user = test_new_user("user_dup_test");
    let first = repo
        .create(&mut *conn, &new_user)
        .await
        .expect("first create should succeed");

    // Second insert with same workos_id should return the existing user
    let second = repo
        .create(&mut *conn, &new_user)
        .await
        .expect("second create should succeed (conflict handled)");

    assert_eq!(first.id, second.id);
}

#[tokio::test]
async fn create_with_nullable_fields() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let new_user = NewUser {
        workos_id: "user_nullable_test".to_string(),
        github_id: None,
        username: "nullable_user".to_string(),
    };

    let user = repo
        .create(&mut *conn, &new_user)
        .await
        .expect("create should succeed");

    assert_eq!(user.workos_id, "user_nullable_test");
    assert!(user.github_id.is_none());
    assert_eq!(user.username, "nullable_user");
}

#[tokio::test]
async fn system_user_exists_with_workos_id_system() {
    let db = IntegrationTestDb::new().await;
    let repo = PgUserRepository;
    let mut conn = db.pool().get().await.unwrap();

    let system_user = repo
        .find_by_workos_id(&mut *conn, "system")
        .await
        .expect("find should succeed")
        .expect("system user should exist from migration");

    assert_eq!(system_user.id, tokenoverflow::constants::SYSTEM_USER_ID);
    assert_eq!(system_user.workos_id, "system");
    assert_eq!(system_user.username, "system");
}
