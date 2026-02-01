use axum::extract::FromRequestParts;
use http::request::Parts;
use uuid::Uuid;

use crate::error::AppError;

/// Authenticated user injected by the jwt_auth middleware.
///
/// Extracted from request extensions. Only present on routes protected
/// by the jwt_auth middleware layer.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// Primary key from the users table
    pub id: Uuid,
    /// WorkOS user ID (from JWT sub claim)
    pub workos_id: String,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| AppError::Unauthorized("Authentication required".to_string()))
    }
}
