//! Application crate — orchestrates use cases over domain ports.
//!
//! Phase 1 only declares the port traits the infrastructure crate will
//! implement. Use-case structs are added in later phases as the business
//! modules come online.

use async_trait::async_trait;
use domain::identity::User;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("repository error: {0}")]
    Repository(String),
}

pub type AppResult<T> = Result<T, AppError>;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> AppResult<Option<User>>;
}
