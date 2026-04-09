use anyhow::Result;
use sqlx::PgPool;

pub async fn is_blacklisted(pool: &PgPool, ip: &str) -> Result<bool> {
    let row: Option<(String,)> = sqlx::query_as("SELECT ip FROM ip_blacklist WHERE ip = $1")
        .bind(ip)
        .fetch_optional(pool)
        .await?;
    Ok(row.is_some())
}

pub async fn add(pool: &PgPool, ip: &str, reason: &str, by: Option<uuid::Uuid>) -> Result<()> {
    sqlx::query(
        "INSERT INTO ip_blacklist (ip, reason, created_by) VALUES ($1, $2, $3)
         ON CONFLICT (ip) DO UPDATE SET reason = EXCLUDED.reason",
    )
    .bind(ip)
    .bind(reason)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove(pool: &PgPool, ip: &str) -> Result<()> {
    sqlx::query("DELETE FROM ip_blacklist WHERE ip = $1")
        .bind(ip)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list(pool: &PgPool) -> Result<Vec<(String, String)>> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT ip, reason FROM ip_blacklist ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows)
}
