use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use subtle::ConstantTimeEq;

use crate::AppState;
use crate::error::AppError;

/// Extractor guarding admin-only handlers: requires
/// `Authorization: Bearer <ADMIN_TOKEN>`. Put it first in the handler's
/// argument list so the check runs before the request body is read.
pub struct RequireAdmin;

impl FromRequestParts<AppState> for RequireAdmin {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
            .ok_or(AppError::Unauthorized)?;

        if bool::from(token.as_bytes().ct_eq(state.settings.admin_token.as_bytes())) {
            Ok(RequireAdmin)
        } else {
            Err(AppError::Forbidden)
        }
    }
}
