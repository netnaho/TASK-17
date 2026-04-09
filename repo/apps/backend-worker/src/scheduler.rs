use anyhow::Result;
use sqlx::PgPool;
use std::time::Duration;

use crate::jobs::JobRegistry;

/// Minimal interval-based scheduler. Phase 1 deliberately avoids a full
/// cron crate; the registry already exposes the boundary that a richer
/// scheduler can plug into in later phases.
pub async fn run_forever(pool: PgPool, registry: JobRegistry) -> Result<()> {
    let mut ticker = tokio::time::interval(Duration::from_secs(60));
    loop {
        ticker.tick().await;
        for job in registry.iter() {
            tracing::debug!(job = job.name(), "tick");
            if let Err(err) = job.maybe_run(&pool).await {
                tracing::error!(job = job.name(), error = %err, "job failed");
            }
        }
    }
}
