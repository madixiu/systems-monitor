use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::AppState;

pub fn hash_token(token: &str) -> String {
    let mut h = Sha256::new();
    h.update(token.as_bytes());
    hex::encode(h.finalize())
}

pub fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn bearer(parts: &Parts) -> Option<&str> {
    parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
}

pub struct AgentAuth {
    pub agent_id: Uuid,
}

#[async_trait]
impl FromRequestParts<AppState> for AgentAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = bearer(parts).ok_or(AppError::Unauthorized("missing bearer token"))?;
        let h = hash_token(token);
        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT agent_id FROM agents WHERE token_hash = $1 AND revoked_at IS NULL"
        )
        .bind(&h)
        .fetch_optional(&state.pool)
        .await?;
        let (agent_id,) = row.ok_or(AppError::Unauthorized("invalid agent token"))?;
        Ok(AgentAuth { agent_id })
    }
}

pub struct ApiAuth;

#[async_trait]
impl FromRequestParts<AppState> for ApiAuth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let key = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized("missing X-API-Key header"))?;
        verify_api_key(&state.pool, key).await?;
        Ok(ApiAuth)
    }
}

pub async fn verify_api_key(pool: &PgPool, key: &str) -> Result<(), AppError> {
    let h = hash_token(key);
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT key_hash FROM api_keys WHERE key_hash = $1 AND revoked_at IS NULL"
    )
    .bind(&h)
    .fetch_optional(pool)
    .await?;
    row.ok_or(AppError::Unauthorized("invalid API key")).map(|_| ())
}
