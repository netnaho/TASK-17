use anyhow::Result;
use sqlx::PgPool;

/// Migrations are embedded from the workspace-level `migrations/` folder so
/// that the API binary can run them on startup without any host-side tooling.
/// This keeps the one-command startup contract intact.
static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("../../migrations");

pub async fn run(pool: &PgPool) -> Result<()> {
    tracing::info!("running database migrations");
    MIGRATOR.run(pool).await?;
    tracing::info!("database migrations complete");
    Ok(())
}
