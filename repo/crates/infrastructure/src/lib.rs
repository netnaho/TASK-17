//! Infrastructure crate — adapters for databases and external systems.
//!
//! Phase 1 wires up the Postgres connection pool, the migration runner, and
//! the seed runner. Concrete repositories implementing the application
//! ports arrive in later phases alongside their business modules.

pub mod crypto;
pub mod db;
pub mod migrations;
pub mod seed;
