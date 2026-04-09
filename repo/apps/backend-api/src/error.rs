use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use shared::ApiError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiAppError {
    #[error("not found")]
    NotFound,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("rate limited (retry after {0}s)")]
    RateLimited(u64),
    #[error("internal: {0}")]
    Internal(String),
}

impl ResponseError for ApiAppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let (code, message) = match self {
            Self::NotFound => ("not_found".into(), self.to_string()),
            Self::BadRequest(m) => ("bad_request".into(), m.clone()),
            Self::Conflict(m) => ("conflict".into(), m.clone()),
            Self::Unauthorized(m) => ("unauthorized".into(), m.clone()),
            Self::Forbidden(m) => ("forbidden".into(), m.clone()),
            Self::RateLimited(s) => ("rate_limited".into(), format!("retry after {s}s")),
            Self::Internal(m) => ("internal".into(), m.clone()),
        };
        let mut resp = HttpResponse::build(self.status_code());
        if let Self::RateLimited(s) = self {
            resp.insert_header(("Retry-After", s.to_string()));
        }
        resp.json(ApiError { code, message })
    }
}

impl From<sqlx::Error> for ApiAppError {
    fn from(e: sqlx::Error) -> Self {
        ApiAppError::Internal(e.to_string())
    }
}

impl From<anyhow::Error> for ApiAppError {
    fn from(e: anyhow::Error) -> Self {
        ApiAppError::Internal(e.to_string())
    }
}
