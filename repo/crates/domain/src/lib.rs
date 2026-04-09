//! Domain crate — pure business types and rules.
//!
//! Phase 1: only baseline identity/organisation entities are modelled.
//! Business modules (requisitions, orders, inventory, etc.) are introduced
//! in later phases and must live here, not in infrastructure.

pub mod identity;
pub mod organisation;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invariant violated: {0}")]
    Invariant(String),
}
