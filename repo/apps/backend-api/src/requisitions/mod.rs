//! Requisition workflow: state machine, approval engine, and the
//! transactional finalization that creates the issue record + deducts
//! inventory in a single DB transaction.
//!
//! Reviewer map:
//!   state.rs   — lifecycle enum + central transition validation
//!   engine.rs  — pure rule evaluator (configurable, NOT hardcoded)
//!   service.rs — repository + use cases (all writes go through here)

pub mod engine;
pub mod service;
pub mod state;
