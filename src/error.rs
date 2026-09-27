use axum::Json;
use axum::extract::multipart::MultipartError;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::imaging::ImageError;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("missing or malformed admin token")]
    Unauthorized,
    #[error("invalid admin token")]
    Forbidden,
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    PayloadTooLarge(String),
    #[error("{0}")]
    UnsupportedMedia(String),
    #[error("server storage is full, try again later")]
    InsufficientStorage,
    /// Logged in full, reported to the client as a generic message.
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::Internal(e.to_string())
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::PayloadTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            Self::UnsupportedMedia(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::InsufficientStorage => StatusCode::INSUFFICIENT_STORAGE,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let message = match &self {
            Self::Internal(detail) => {
                tracing::error!(%detail, "internal error");
                "internal server error".to_owned()
            }
            other => other.to_string(),
        };
        let mut res = (status, Json(json!({ "error": message }))).into_response();
        if status == StatusCode::UNAUTHORIZED {
            res.headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        res
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(format!("database: {e}"))
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        Self::Internal(format!("io: {e}"))
    }
}

impl From<MultipartError> for AppError {
    fn from(e: MultipartError) -> Self {
        if e.status() == StatusCode::PAYLOAD_TOO_LARGE {
            Self::PayloadTooLarge("request body is too large".into())
        } else {
            Self::BadRequest(format!("invalid multipart body: {}", e.body_text()))
        }
    }
}

impl From<ImageError> for AppError {
    fn from(e: ImageError) -> Self {
        match e {
            ImageError::Unsupported => Self::UnsupportedMedia(e.to_string()),
            ImageError::TooLarge => Self::PayloadTooLarge(e.to_string()),
            ImageError::Internal(msg) => Self::Internal(format!("image: {msg}")),
        }
    }
}

/// True if the error is a UNIQUE constraint violation.
pub fn is_unique_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .is_some_and(|d| d.is_unique_violation())
}

/// True if the error is a FOREIGN KEY constraint violation.
pub fn is_fk_violation(e: &sqlx::Error) -> bool {
    e.as_database_error()
        .is_some_and(|d| d.is_foreign_key_violation())
}
