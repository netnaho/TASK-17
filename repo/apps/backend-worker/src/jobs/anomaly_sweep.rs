use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use sqlx::Row;
use anyhow::Result;
use async_trait::async_trait;

use super::Job;

pub struct AnomalySweepJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl AnomalySweepJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for AnomalySweepJob {
    fn name(&self) -> &'static str { "anomaly_sweep" }

    async fn maybe_run(&self, pool: &PgPool) -> Result<()> {
        let mut last = self.last_run.lock().await;
        let now = Utc::now();
        if let Some(t) = *last {
            if now - t < Duration::hours(1) {
                return Ok(());
            }
        }
        *last = Some(now);
        drop(last);
        self.run(pool).await
    }
}

impl AnomalySweepJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        // Auto-acknowledge stale low-severity events older than 7 days
        let acked = sqlx::query(
            "UPDATE anomaly_events \
             SET acknowledged = true, acknowledged_at = NOW() \
             WHERE severity = 'low' \
               AND acknowledged = false \
               AND detected_at < NOW() - INTERVAL '7 days'",
        )
        .execute(pool)
        .await?;

        tracing::info!(
            job = "anomaly_sweep",
            auto_acknowledged = acked.rows_affected(),
            "auto-acknowledged stale low-severity anomaly events"
        );

        // Summarise unacknowledged events by severity
        let summary = sqlx::query(
            "SELECT severity, COUNT(*) AS event_count \
             FROM anomaly_events \
             WHERE acknowledged = false \
             GROUP BY severity \
             ORDER BY severity",
        )
        .fetch_all(pool)
        .await?;

        for row in &summary {
            let severity: String = row.get("severity");
            let event_count: i64 = row.get("event_count");
            tracing::info!(
                job = "anomaly_sweep",
                severity = %severity,
                unacknowledged_count = event_count,
                "unacknowledged anomaly events summary"
            );
        }

        Ok(())
    }
}
