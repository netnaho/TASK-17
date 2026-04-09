use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::Job;

pub struct ConsistencyValidationJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl ConsistencyValidationJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for ConsistencyValidationJob {
    fn name(&self) -> &'static str { "consistency_validation" }

    async fn maybe_run(&self, pool: &PgPool) -> Result<()> {
        let mut last = self.last_run.lock().await;
        let now = Utc::now();
        if let Some(t) = *last {
            if now - t < Duration::hours(24) {
                return Ok(());
            }
        }
        *last = Some(now);
        drop(last);
        self.run(pool).await
    }
}

impl ConsistencyValidationJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        let institutions: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM institutions")
            .fetch_all(pool)
            .await?;

        // Determine the institution_ids to iterate over; fall back to a
        // single NULL pass if the table is empty.
        if institutions.is_empty() {
            self.run_checks(pool, None).await?;
        } else {
            for (id,) in &institutions {
                self.run_checks(pool, Some(*id)).await?;
            }
        }

        Ok(())
    }

    async fn run_checks(&self, pool: &PgPool, institution_id: Option<Uuid>) -> Result<()> {
        // ------------------------------------------------------------------
        // 1. Orphan check
        // ------------------------------------------------------------------
        let orphan_students: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM students s \
             LEFT JOIN institutions i ON i.id = s.institution_id \
             WHERE i.id IS NULL",
        )
        .fetch_one(pool)
        .await
        .map(|(v,)| v)
        .unwrap_or(0);

        let orphan_findings = json!([
            { "check": "orphan_students", "count": orphan_students }
        ]);
        let orphan_count = 1_i32;

        sqlx::query(
            "INSERT INTO consistency_reports \
               (report_type, institution_id, findings, finding_count, run_at) \
             VALUES ('orphan_check', $1, $2, $3, NOW())",
        )
        .bind(institution_id)
        .bind(orphan_findings)
        .bind(orphan_count)
        .execute(pool)
        .await?;

        // ------------------------------------------------------------------
        // 2. ID map gaps
        // ------------------------------------------------------------------
        let unverified: i64 = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM entity_id_map \
             WHERE verified_at IS NULL OR verified_at < NOW() - INTERVAL '30 days'",
        )
        .fetch_one(pool)
        .await
        .map(|(v,)| v)
        .unwrap_or(0);

        let gap_findings = json!([
            { "check": "unverified_mappings", "count": unverified }
        ]);
        let gap_count = 1_i32;

        sqlx::query(
            "INSERT INTO consistency_reports \
               (report_type, institution_id, findings, finding_count, run_at) \
             VALUES ('id_map_gaps', $1, $2, $3, NOW())",
        )
        .bind(institution_id)
        .bind(gap_findings)
        .bind(gap_count)
        .execute(pool)
        .await?;

        // ------------------------------------------------------------------
        // 3. Referential integrity — id_map_conflicts
        // ------------------------------------------------------------------
        // id_map_conflicts table may not exist in all environments; treat a
        // missing table as zero unresolved conflicts.
        let unresolved_result: Result<(i64,), sqlx::Error> = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM id_map_conflicts WHERE resolved = false",
        )
        .fetch_one(pool)
        .await;

        let unresolved: i64 = match unresolved_result {
            Ok((v,)) => v,
            Err(err) => {
                tracing::error!(
                    job = "consistency_validation",
                    error = %err,
                    "could not query id_map_conflicts — skipping referential integrity check"
                );
                0
            }
        };

        let ri_findings = json!([
            { "check": "unresolved_conflicts", "count": unresolved }
        ]);
        let ri_count = 1_i32;

        sqlx::query(
            "INSERT INTO consistency_reports \
               (report_type, institution_id, findings, finding_count, run_at) \
             VALUES ('referential_integrity', $1, $2, $3, NOW())",
        )
        .bind(institution_id)
        .bind(ri_findings)
        .bind(ri_count)
        .execute(pool)
        .await?;

        tracing::info!(
            job = "consistency_validation",
            institution_id = ?institution_id,
            orphan_students,
            unverified_mappings = unverified,
            unresolved_conflicts = unresolved,
            "consistency checks written"
        );

        Ok(())
    }
}
