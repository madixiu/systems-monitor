use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    Unauthorized(&'static str),
    NotFound,
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.to_string()),
            AppError::NotFound => (StatusCode::NOT_FOUND, "not found".into()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!("sqlx: {}", e);
        AppError::Internal("database error".into())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!("internal: {:#}", e);
        AppError::Internal("internal error".into())
    }
}
