mod jobs;
mod scheduler;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .json()
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = infrastructure::db::connect(&database_url).await?;

    tracing::info!("backend-worker started");

    let registry = jobs::register();
    scheduler::run_forever(pool, registry).await
}
