use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, TokenData, Validation, decode, decode_header};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use serde::Deserialize;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::AuthConfig;
use crate::db::models::{NewUser, User};
use crate::error::AppError;
use crate::services::repository::{UserRepository, WaitlistRepository};

/// Validated JWT claims extracted from the token.
///
/// `email` is required: AuthKit access tokens carry it via the JWT
/// Template configured in the WorkOS Dashboard (Authentication →
/// Features → JWT Template), which injects `{"email": "{{ user.email }}"}`
/// into every access token.
#[derive(Debug)]
pub struct JwtClaims {
    pub sub: String,
    pub iss: String,
    pub aud: StringOrVec,
    pub email: String,
}

/// Wire-level claims as parsed by `jsonwebtoken::decode`. `email` is
/// optional here so we can produce a precise "missing email claim"
/// error instead of a generic deserialization failure.
#[derive(Debug, Deserialize)]
struct RawJwtClaims {
    sub: String,
    iss: String,
    aud: StringOrVec,
    #[serde(default)]
    email: Option<String>,
}

/// WorkOS returns `aud` as either a string or array; handle both.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum StringOrVec {
    Single(String),
    Multiple(Vec<String>),
}

/// JWKS key entry as returned by the JWKS endpoint.
#[derive(Debug, Clone, Deserialize)]
struct JwksKey {
    kid: String,
    n: String,
    e: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Jwks {
    keys: Vec<JwksKey>,
}

/// Cached JWKS keyset with expiry tracking.
struct JwksCache {
    keys: Vec<JwksKey>,
    loaded_at: Instant,
}

/// WorkOS identity entry from the identities endpoint.
#[derive(Debug, Deserialize)]
struct WorkosIdentity {
    idp_id: String,
    provider: String,
}

/// GitHub user profile from the public users API.
#[derive(Debug, Deserialize)]
struct GithubUser {
    login: String,
}

/// AuthService encapsulates JWKS loading, JWT validation, and user resolution.
///
/// The same code path runs in all environments (cloud, local, test).
/// The only difference is the JWKS source: `file://` for test/local, `https://` for production.
pub struct AuthService {
    config: AuthConfig,
    cache: RwLock<Option<JwksCache>>,
    http_client: reqwest_middleware::ClientWithMiddleware,
}

impl AuthService {
    pub fn new(config: AuthConfig) -> Self {
        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_secs(1), Duration::from_secs(4))
            .build_with_max_retries(2);

        let base_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");

        let http_client = ClientBuilder::new(base_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            config,
            cache: RwLock::new(None),
            http_client,
        }
    }

    /// Validate a JWT and return the claims.
    ///
    /// Steps:
    /// 1. Decode header to get `kid`
    /// 2. Look up `kid` in cached JWKS
    /// 3. If not found, refresh JWKS and retry
    /// 4. Validate signature, issuer, audience, expiry
    /// 5. Return claims including `sub` (WorkOS user ID)
    pub async fn validate_jwt(&self, token: &str) -> Result<JwtClaims, AppError> {
        let header = decode_header(token)
            .map_err(|e| AppError::Unauthorized(format!("Invalid JWT header: {}", e)))?;

        let kid = header
            .kid
            .ok_or_else(|| AppError::Unauthorized("JWT missing kid header".to_string()))?;

        // Try with cached keys first
        if let Some(claims) = self.try_validate_with_kid(token, &kid).await? {
            return Ok(claims);
        }

        // kid not found in cache -- refresh JWKS and retry
        self.refresh_jwks().await?;

        self.try_validate_with_kid(token, &kid)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Unknown signing key".to_string()))
    }

    /// Resolve the local user from the JWT sub claim.
    ///
    /// Branches:
    /// 1. `api.users` row exists → return it.
    /// 2. Else if `require_waitlist_approval` is false → JIT-provision
    ///    via the WorkOS-backed identity helper.
    /// 3. Else → look up `api.waitlist` by GitHub id and branch on
    ///    `approved_at`: approved provisions a user, pending returns
    ///    `WaitlistPending`, missing returns `WaitlistRequired`.
    pub async fn resolve_user<Conn: Send>(
        &self,
        user_repo: &(dyn UserRepository<Conn> + Sync),
        waitlist_repo: &(dyn WaitlistRepository<Conn> + Sync),
        conn: &mut Conn,
        workos_id: &str,
    ) -> Result<User, AppError> {
        // Fast path: user already exists
        if let Some(user) = user_repo.find_by_workos_id(conn, workos_id).await? {
            return Ok(user);
        }

        let (github_id, github_login) = self
            .resolve_github_identity(user_repo, conn, workos_id)
            .await?;

        // Rollout-deprecation: when the gate is off, revert to plain
        // find-or-create.
        if !self.config.require_waitlist_approval {
            let new_user = NewUser {
                workos_id: workos_id.to_string(),
                github_id: Some(github_id),
                username: github_login,
            };
            return user_repo.create(conn, &new_user).await;
        }

        // Gate ON: applicant must have an approved waitlist row.
        match waitlist_repo.find_by_github_id(conn, github_id).await? {
            Some(entry) if entry.approved_at.is_some() => {
                let new_user = NewUser {
                    workos_id: workos_id.to_string(),
                    github_id: Some(github_id),
                    username: github_login,
                };
                let user = user_repo.create(conn, &new_user).await?;
                // Best-effort audit: the user creation is the source of
                // truth, so a `set_user_id` failure does not roll back the
                // user. We still surface it as Internal so the operator
                // notices a partial state.
                waitlist_repo.set_user_id(conn, github_id, user.id).await?;
                Ok(user)
            }
            Some(_) => Err(AppError::WaitlistPending),
            None => Err(AppError::WaitlistRequired),
        }
    }

    /// Map a `workos_id` to `(github_id, github_login)`.
    ///
    /// Tries the local `api.users` row first so any returning prod user
    /// skips the WorkOS round-trip. Falls through to the WorkOS Management
    /// API for callers without a db entry.
    pub async fn resolve_github_identity<Conn: Send>(
        &self,
        user_repo: &(dyn UserRepository<Conn> + Sync),
        conn: &mut Conn,
        workos_id: &str,
    ) -> Result<(i64, String), AppError> {
        if let Some(user) = user_repo.find_by_workos_id(conn, workos_id).await?
            && let Some(github_id) = user.github_id
        {
            return Ok((github_id, user.username));
        }

        self.resolve_github_identity_via_workos(workos_id).await
    }

    /// Try to validate the JWT against the cached JWKS for the given kid.
    async fn try_validate_with_kid(
        &self,
        token: &str,
        kid: &str,
    ) -> Result<Option<JwtClaims>, AppError> {
        let keys = self.get_jwks().await?;

        let matching_key = keys.iter().find(|k| k.kid == kid);
        let key = match matching_key {
            Some(k) => k,
            None => return Ok(None),
        };

        let decoding_key = DecodingKey::from_rsa_components(&key.n, &key.e)
            .map_err(|e| AppError::Internal(format!("Invalid JWKS key components: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&self.config.issuer]);
        // Audience validation: WorkOS AuthKit does not support RFC 8707 Resource
        // Indicators, so the `aud` claim is always the WorkOS environment-level
        // client ID (e.g., `client_01KKZDZQ26HJSBXSWQRSWABFMX`), not the
        // canonical resource URI (`https://api.tokenoverflow.io`).
        //
        // The MCP spec requires servers to validate `aud` against the resource
        // URI (RFC 8707 S2), but this is impossible when the AS ignores the
        // `resource` parameter. We validate against the WorkOS client ID, which
        // is the only audience WorkOS produces. This matches WorkOS's own MCP
        // documentation:
        // https://workos.com/docs/authkit/mcp
        //
        // MCP clients still send `resource` per the spec, but WorkOS silently
        // ignores it. When WorkOS adds RFC 8707 support, add the canonical
        // resource URI to the `audiences` list in config to accept both.
        let aud_refs: Vec<&str> = self.config.audiences.iter().map(|s| s.as_str()).collect();
        validation.set_audience(&aud_refs);
        validation.set_required_spec_claims(&["sub", "iss", "aud", "exp"]);

        let token_data: TokenData<RawJwtClaims> = decode(token, &decoding_key, &validation)
            .map_err(|e| AppError::Unauthorized(format!("JWT validation failed: {}", e)))?;

        let raw = token_data.claims;
        let email = raw
            .email
            .ok_or_else(|| AppError::Unauthorized("missing email claim".to_string()))?;

        Ok(Some(JwtClaims {
            sub: raw.sub,
            iss: raw.iss,
            aud: raw.aud,
            email,
        }))
    }

    /// Get JWKS keys, loading from source if cache is empty or expired.
    async fn get_jwks(&self) -> Result<Vec<JwksKey>, AppError> {
        let cache = self.cache.read().await;
        if let Some(ref cached) = *cache {
            let ttl = Duration::from_secs(self.config.jwks_cache_ttl_secs);
            if ttl.is_zero() || cached.loaded_at.elapsed() < ttl {
                return Ok(cached.keys.clone());
            }
        }
        drop(cache);

        self.refresh_jwks().await
    }

    /// Refresh the JWKS cache from the configured URL.
    async fn refresh_jwks(&self) -> Result<Vec<JwksKey>, AppError> {
        let jwks = self.load_jwks().await?;
        let keys = jwks.keys;

        let mut cache = self.cache.write().await;
        *cache = Some(JwksCache {
            keys: keys.clone(),
            loaded_at: Instant::now(),
        });

        info!(key_count = keys.len(), "JWKS cache refreshed");

        Ok(keys)
    }

    /// Load JWKS from the configured URL. Supports `file://` protocol for test/local.
    async fn load_jwks(&self) -> Result<Jwks, AppError> {
        if let Some(path) = self.config.jwks_url.strip_prefix("file://") {
            let content = std::fs::read_to_string(path).map_err(|e| {
                AppError::Internal(format!("Failed to read JWKS file '{}': {}", path, e))
            })?;
            let jwks: Jwks = serde_json::from_str(&content)
                .map_err(|e| AppError::Internal(format!("Failed to parse JWKS file: {}", e)))?;
            Ok(jwks)
        } else {
            let response = self
                .http_client
                .get(&self.config.jwks_url)
                .send()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to fetch JWKS: {}", e)))?;

            let jwks: Jwks = response
                .json()
                .await
                .map_err(|e| AppError::Internal(format!("Failed to parse JWKS response: {}", e)))?;
            Ok(jwks)
        }
    }

    /// Fetch GitHub identity and username via the WorkOS Management API.
    ///
    /// The user is already authenticated via JWT (WorkOS verified them),
    /// so we skip fetching the WorkOS user profile and go straight to:
    /// 1. Fetch WorkOS identities to get the GitHub numeric user ID.
    /// 2. Fetch GitHub user profile to get the actual login handle.
    async fn resolve_github_identity_via_workos(
        &self,
        workos_id: &str,
    ) -> Result<(i64, String), AppError> {
        let api_key = self.config.workos_api_key().ok_or_else(|| {
            AppError::Internal(
                "TOKENOVERFLOW_WORKOS_API_KEY not configured; cannot resolve GitHub identity"
                    .to_string(),
            )
        })?;

        // Step 1: Fetch WorkOS identities to get GitHub numeric user ID
        // workos_id is a ULID (Crockford base32, URL-safe by construction);
        // no urlencoding needed.
        let identities_url = format!(
            "{}/user_management/users/{}/identities",
            self.config.workos_api_url, workos_id
        );

        let identities_response = self
            .http_client
            .get(&identities_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("WorkOS identities request failed: {}", e)))?;

        if !identities_response.status().is_success() {
            let status = identities_response.status();
            let body = identities_response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "WorkOS identities API returned {}: {}",
                status, body
            )));
        }

        let identities: Vec<WorkosIdentity> = identities_response.json().await.map_err(|e| {
            AppError::Internal(format!("Failed to parse WorkOS identities response: {}", e))
        })?;

        let github_identity = identities
            .iter()
            .find(|i| i.provider == "GithubOAuth")
            .ok_or_else(|| {
                AppError::Internal(format!("No GitHub identity found for user {}", workos_id))
            })?;

        let github_id: i64 = github_identity.idp_id.parse().map_err(|_| {
            AppError::Internal(format!("Invalid GitHub ID: {}", github_identity.idp_id))
        })?;

        // Step 2: Fetch GitHub username via GitHub Users API
        let github_url = format!("{}/user/{}", self.config.github_api_url, github_id);

        let mut request = self
            .http_client
            .get(&github_url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "TokenOverflow API");

        if let Some((client_id, client_secret)) = self.config.github_oauth_credentials() {
            request = request.basic_auth(client_id, Some(client_secret));
        } else {
            warn!(
                "GitHub OAuth App credentials not configured; using unauthenticated GitHub API (60 req/hour limit)"
            );
        }

        let github_response = request
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("GitHub API request failed: {}", e)))?;

        if !github_response.status().is_success() {
            let status = github_response.status();
            let body = github_response.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "GitHub API returned {}: {}",
                status, body
            )));
        }

        let github_user: GithubUser = github_response.json().await.map_err(|e| {
            AppError::Internal(format!("Failed to parse GitHub user response: {}", e))
        })?;

        Ok((github_id, github_user.login))
    }
}

/// Create an AuthService from config, wrapped in Arc for shared ownership.
pub fn create_auth_service(config: &AuthConfig) -> Arc<AuthService> {
    Arc::new(AuthService::new(config.clone()))
}
