use axum::extract::{FromRef, FromRequestParts};
use http::header::AUTHORIZATION;
use http::request::Parts;
use uuid::Uuid;

use crate::api::state::AppState;
use crate::error::AppError;

/// Authenticated user injected by the jwt_auth middleware.
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

/// JWT claims for routes that must accept tokens from users without an
/// `api.users` row yet (waitlist sign-up). Validates the Bearer token
/// against the AuthKit JWKS and returns the identifying fields the
/// handler needs. `email` is sourced from the AuthKit access-token
/// claim emitted by the WorkOS JWT Template, so the signup path does
/// not need a second WorkOS round-trip.
#[derive(Debug, Clone)]
pub struct WorkosJwtClaims {
    pub workos_id: String,
    pub email: String,
}

impl<S: Send + Sync> FromRequestParts<S> for WorkosJwtClaims
where
    AppState: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| AppError::Unauthorized("Missing Bearer token".to_string()))?;

        let claims = app_state.auth.validate_jwt(token).await?;
        Ok(WorkosJwtClaims {
            workos_id: claims.sub,
            email: claims.email,
        })
    }
}
