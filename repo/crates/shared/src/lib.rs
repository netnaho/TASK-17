//! Shared crate — DTOs and small utilities used across HTTP, worker, and
//! WASM frontend. Keep this crate small and dependency-light; anything
//! HTTP-, DB-, or DOM-specific belongs in its respective layer.

use serde::{Deserialize, Serialize};

/// Standardized JSON error envelope returned by the REST API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: &'static str,
    pub version: &'static str,
}
