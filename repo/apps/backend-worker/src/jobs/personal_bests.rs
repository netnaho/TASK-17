use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use anyhow::Result;
use async_trait::async_trait;

use super::Job;

pub struct PersonalBestsJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl PersonalBestsJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for PersonalBestsJob {
    fn name(&self) -> &'static str { "personal_bests" }

    async fn maybe_run(&self, pool: &PgPool) -> Result<()> {
        let mut last = self.last_run.lock().await;
        let now = Utc::now();
        if let Some(t) = *last {
            if now - t < Duration::hours(6) {
                return Ok(());
            }
        }
        *last = Some(now);
        drop(last);
        self.run(pool).await
    }
}

impl PersonalBestsJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        // DISTINCT ON selects the row with the highest duration_minutes per
        // (student_id, activity_type) pair; ties broken by earliest activity_date.
        let result = sqlx::query(
            r#"
            INSERT INTO wellness_personal_bests
                (student_id, activity_type, best_duration_minutes, achieved_on)
            SELECT DISTINCT ON (student_id, activity_type)
                student_id,
                activity_type,
                duration_minutes,
                activity_date
            FROM wellness_activities
            WHERE student_id IS NOT NULL
            ORDER BY student_id, activity_type, duration_minutes DESC, activity_date ASC
            ON CONFLICT (student_id, activity_type) DO UPDATE
                SET best_duration_minutes = EXCLUDED.best_duration_minutes,
                    achieved_on           = EXCLUDED.achieved_on,
                    updated_at            = NOW()
            WHERE EXCLUDED.best_duration_minutes >= wellness_personal_bests.best_duration_minutes
            "#,
        )
        .execute(pool)
        .await?;

        tracing::info!(
            job = "personal_bests",
            rows_affected = result.rows_affected(),
            "upserted wellness personal bests"
        );

        Ok(())
    }
}
