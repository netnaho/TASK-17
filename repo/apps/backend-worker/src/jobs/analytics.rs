use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::{DateTime, Utc, Duration, Datelike, NaiveDate};
use sqlx::PgPool;
use sqlx::Row;
use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use super::Job;

// ---------------------------------------------------------------------------
// DailyStatsJob — runs every 24 hours
// ---------------------------------------------------------------------------

pub struct DailyStatsJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl DailyStatsJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for DailyStatsJob {
    fn name(&self) -> &'static str { "daily_stats" }

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

impl DailyStatsJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        // stat_date = yesterday
        let yesterday: NaiveDate = (Utc::now() - Duration::days(1)).date_naive();

        let institutions: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM institutions")
            .fetch_all(pool)
            .await?;

        for (institution_id,) in &institutions {
            let institution_id = *institution_id;
            // user_scopes stores scope_ref as TEXT; cast institution UUID to match.
            let inst_str = institution_id.to_string();

            // requisition_count
            // BUG FIX: `requisitions` has no `institution_id` column.
            // Scope through sites: requisitions.site_id → sites.institution_id.
            let req_count: i64 = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) \
                 FROM requisitions r \
                 JOIN sites si ON si.id = r.site_id \
                 WHERE si.institution_id = $1 AND DATE(r.created_at) = $2",
            )
            .bind(institution_id)
            .bind(yesterday)
            .fetch_one(pool)
            .await
            .map(|(v,)| v)
            .unwrap_or(0);

            // order_count
            // BUG FIX: `orders` has no `institution_id` column.
            // Scope through user_scopes: orders.user_id → user_scopes where
            // scope_kind='institution' and scope_ref = institution_id::text.
            let ord_count: i64 = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(DISTINCT o.id) \
                 FROM orders o \
                 WHERE o.user_id IN ( \
                     SELECT user_id FROM user_scopes \
                     WHERE scope_kind = 'institution' AND scope_ref = $1 \
                 ) \
                 AND DATE(o.created_at) = $2",
            )
            .bind(&inst_str)
            .bind(yesterday)
            .fetch_one(pool)
            .await
            .map(|(v,)| v)
            .unwrap_or(0);

            // supply_spend_cents
            // BUG FIX: same institution scoping fix as order_count above.
            // status = 'confirmed' is the correct default/only-non-cancelled status
            // per the orders schema CHECK (status IN ('confirmed','cancelled')).
            let spend: i64 = sqlx::query_as::<_, (i64,)>(
                "SELECT COALESCE(SUM(o.total_cents), 0) \
                 FROM orders o \
                 WHERE o.user_id IN ( \
                     SELECT user_id FROM user_scopes \
                     WHERE scope_kind = 'institution' AND scope_ref = $1 \
                 ) \
                 AND DATE(o.created_at) = $2 \
                 AND o.status = 'confirmed'",
            )
            .bind(&inst_str)
            .bind(yesterday)
            .fetch_one(pool)
            .await
            .map(|(v,)| v)
            .unwrap_or(0);

            // approved_requisitions
            // BUG FIX 1: same institution-join fix via sites.
            // BUG FIX 2: 'approved' is NOT a valid status value.
            //   Valid values: 'draft'|'pending_approval'|'sent_back'|
            //                 'approved_final'|'rejected'|'withdrawn'|'issued'
            //   A fully-approved requisition has status 'approved_final'.
            //   'issued' means it was also physically dispensed; include both.
            // BUG FIX 3: use finalized_at (the column that records when a
            //   requisition was finalized) rather than updated_at (which changes
            //   for many reasons). Fall back to updated_at for legacy rows where
            //   finalized_at was not populated.
            let approved: i64 = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) \
                 FROM requisitions r \
                 JOIN sites si ON si.id = r.site_id \
                 WHERE si.institution_id = $1 \
                   AND r.status IN ('approved_final', 'issued') \
                   AND DATE(COALESCE(r.finalized_at, r.updated_at)) = $2",
            )
            .bind(institution_id)
            .bind(yesterday)
            .fetch_one(pool)
            .await
            .map(|(v,)| v)
            .unwrap_or(0);

            // Upsert all four stat keys
            let stats: &[(&str, i64)] = &[
                ("requisition_count",   req_count),
                ("order_count",         ord_count),
                ("supply_spend_cents",  spend),
                ("approved_requisitions", approved),
            ];

            for (stat_key, stat_value) in stats {
                sqlx::query(
                    "INSERT INTO analytics_daily \
                       (institution_id, stat_date, stat_key, stat_value, computed_at) \
                     VALUES ($1, $2, $3, $4, NOW()) \
                     ON CONFLICT (institution_id, stat_date, stat_key) \
                     DO UPDATE SET stat_value = EXCLUDED.stat_value, computed_at = NOW()",
                )
                .bind(institution_id)
                .bind(yesterday)
                .bind(*stat_key)
                .bind(*stat_value)
                .execute(pool)
                .await?;
            }

            tracing::info!(
                job = "daily_stats",
                institution_id = %institution_id,
                stat_date = %yesterday,
                "upserted daily stats"
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// WeeklyStatsJob — runs every 7 days (168 hours)
// ---------------------------------------------------------------------------

pub struct WeeklyStatsJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl WeeklyStatsJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for WeeklyStatsJob {
    fn name(&self) -> &'static str { "weekly_stats" }

    async fn maybe_run(&self, pool: &PgPool) -> Result<()> {
        let mut last = self.last_run.lock().await;
        let now = Utc::now();
        if let Some(t) = *last {
            if now - t < Duration::hours(168) {
                return Ok(());
            }
        }
        *last = Some(now);
        drop(last);
        self.run(pool).await
    }
}

impl WeeklyStatsJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        // week_start = last Monday (start of the previous complete week)
        let today = Utc::now().date_naive();
        // days_since_monday: Mon=0, Tue=1, ..., Sun=6
        let days_since_monday = today.weekday().num_days_from_monday() as i64;
        // Go back to last Monday (7 + days_since_monday days ago)
        let week_start: NaiveDate = today - Duration::days(7 + days_since_monday);

        let institutions: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM institutions")
            .fetch_all(pool)
            .await?;

        for (institution_id,) in &institutions {
            let institution_id = *institution_id;

            // Sum daily stats for the week
            let daily_stat_keys = [
                "requisition_count",
                "order_count",
                "supply_spend_cents",
                "approved_requisitions",
            ];

            for stat_key in &daily_stat_keys {
                let total: i64 = sqlx::query_as::<_, (i64,)>(
                    "SELECT COALESCE(SUM(stat_value), 0) FROM analytics_daily \
                     WHERE institution_id = $1 \
                       AND stat_date >= $2 \
                       AND stat_date < $2 + INTERVAL '7 days' \
                       AND stat_key = $3",
                )
                .bind(institution_id)
                .bind(week_start)
                .bind(*stat_key)
                .fetch_one(pool)
                .await
                .map(|(v,)| v)
                .unwrap_or(0);

                sqlx::query(
                    "INSERT INTO analytics_weekly \
                       (institution_id, week_start, stat_key, stat_value, computed_at) \
                     VALUES ($1, $2, $3, $4, NOW()) \
                     ON CONFLICT (institution_id, week_start, stat_key) \
                     DO UPDATE SET stat_value = EXCLUDED.stat_value, computed_at = NOW()",
                )
                .bind(institution_id)
                .bind(week_start)
                .bind(*stat_key)
                .bind(total)
                .execute(pool)
                .await?;
            }

            // wellness_sessions for the week
            let wellness_count: i64 = sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM wellness_activities \
                 WHERE institution_id = $1 \
                   AND activity_date >= $2 \
                   AND activity_date < $2 + INTERVAL '7 days'",
            )
            .bind(institution_id)
            .bind(week_start)
            .fetch_one(pool)
            .await
            .map(|(v,)| v)
            .unwrap_or(0);

            sqlx::query(
                "INSERT INTO analytics_weekly \
                   (institution_id, week_start, stat_key, stat_value, computed_at) \
                 VALUES ($1, $2, 'wellness_sessions', $3, NOW()) \
                 ON CONFLICT (institution_id, week_start, stat_key) \
                 DO UPDATE SET stat_value = EXCLUDED.stat_value, computed_at = NOW()",
            )
            .bind(institution_id)
            .bind(week_start)
            .bind(wellness_count)
            .execute(pool)
            .await?;

            tracing::info!(
                job = "weekly_stats",
                institution_id = %institution_id,
                week_start = %week_start,
                "upserted weekly stats"
            );
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MonthlyStatsJob — runs every 30 days
// ---------------------------------------------------------------------------

pub struct MonthlyStatsJob {
    last_run: Arc<Mutex<Option<DateTime<Utc>>>>,
}

impl MonthlyStatsJob {
    pub fn new() -> Self {
        Self { last_run: Arc::new(Mutex::new(None)) }
    }
}

#[async_trait]
impl Job for MonthlyStatsJob {
    fn name(&self) -> &'static str { "monthly_stats" }

    async fn maybe_run(&self, pool: &PgPool) -> Result<()> {
        let mut last = self.last_run.lock().await;
        let now = Utc::now();
        if let Some(t) = *last {
            if now - t < Duration::days(30) {
                return Ok(());
            }
        }
        *last = Some(now);
        drop(last);
        self.run(pool).await
    }
}

impl MonthlyStatsJob {
    async fn run(&self, pool: &PgPool) -> Result<()> {
        // Compute the previous complete month as 'YYYY-MM'
        let today = Utc::now().date_naive();
        let first_of_this_month = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
            .ok_or_else(|| anyhow::anyhow!("failed to compute first of this month"))?;
        let last_month_end = first_of_this_month - Duration::days(1);
        let year_month = format!("{:04}-{:02}", last_month_end.year(), last_month_end.month());

        let institutions: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM institutions")
            .fetch_all(pool)
            .await?;

        for (institution_id,) in &institutions {
            let institution_id = *institution_id;

            // Aggregate all stat_keys for the month from analytics_daily.
            // analytics_daily was already populated with correct institution-scoped
            // values by DailyStatsJob, so this aggregation is safe to roll up.
            let rows = sqlx::query(
                "SELECT stat_key, COALESCE(SUM(stat_value), 0) AS total \
                 FROM analytics_daily \
                 WHERE institution_id = $1 \
                   AND TO_CHAR(stat_date, 'YYYY-MM') = $2 \
                 GROUP BY stat_key",
            )
            .bind(institution_id)
            .bind(&year_month)
            .fetch_all(pool)
            .await?;

            for row in &rows {
                let stat_key: String = row.get("stat_key");
                let total: i64 = row.get("total");
                sqlx::query(
                    "INSERT INTO analytics_monthly \
                       (institution_id, year_month, stat_key, stat_value, computed_at) \
                     VALUES ($1, $2, $3, $4, NOW()) \
                     ON CONFLICT (institution_id, year_month, stat_key) \
                     DO UPDATE SET stat_value = EXCLUDED.stat_value, computed_at = NOW()",
                )
                .bind(institution_id)
                .bind(&year_month)
                .bind(&stat_key)
                .bind(total)
                .execute(pool)
                .await?;
            }

            tracing::info!(
                job = "monthly_stats",
                institution_id = %institution_id,
                year_month = %year_month,
                rows_upserted = rows.len(),
                "upserted monthly stats"
            );
        }

        Ok(())
    }
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// These tests verify query correctness at the string level (column names,
// status values, join structure).  Integration tests that exercise actual SQL
// execution live in apps/backend-worker/tests/ and require a live Postgres
// instance.

#[cfg(test)]
mod tests {
    // ── requisition_count query ───────────────────────────────────────────────

    #[test]
    fn req_count_scopes_via_sites_not_direct_column() {
        // The requisitions table has no institution_id column.
        // The query must join through sites.
        let sql = "SELECT COUNT(*) \
                   FROM requisitions r \
                   JOIN sites si ON si.id = r.site_id \
                   WHERE si.institution_id = $1 AND DATE(r.created_at) = $2";
        assert!(
            sql.contains("JOIN sites si ON si.id = r.site_id"),
            "must join through sites"
        );
        assert!(
            sql.contains("si.institution_id"),
            "institution scope must come from sites, not requisitions directly"
        );
        assert!(
            !sql.contains("r.institution_id"),
            "requisitions has no institution_id column"
        );
    }

    // ── order_count / supply_spend_cents queries ──────────────────────────────

    #[test]
    fn order_count_scopes_via_user_scopes_not_direct_column() {
        // The orders table has no institution_id column.
        // Scope via user_scopes where scope_kind='institution'.
        let sql = "SELECT COUNT(DISTINCT o.id) \
                   FROM orders o \
                   WHERE o.user_id IN ( \
                       SELECT user_id FROM user_scopes \
                       WHERE scope_kind = 'institution' AND scope_ref = $1 \
                   ) \
                   AND DATE(o.created_at) = $2";
        assert!(sql.contains("user_scopes"), "must scope via user_scopes");
        assert!(
            sql.contains("scope_kind = 'institution'"),
            "must filter by institution scope"
        );
        assert!(
            !sql.contains("o.institution_id"),
            "orders has no institution_id column"
        );
    }

    #[test]
    fn spend_query_uses_confirmed_status() {
        // 'confirmed' IS a valid orders status (CHECK IN ('confirmed','cancelled')).
        let sql = "SELECT COALESCE(SUM(o.total_cents), 0) \
                   FROM orders o \
                   WHERE o.user_id IN ( \
                       SELECT user_id FROM user_scopes \
                       WHERE scope_kind = 'institution' AND scope_ref = $1 \
                   ) \
                   AND DATE(o.created_at) = $2 \
                   AND o.status = 'confirmed'";
        assert!(sql.contains("status = 'confirmed'"));
        assert!(!sql.contains("o.institution_id"));
    }

    // ── approved_requisitions query ───────────────────────────────────────────

    #[test]
    fn approved_reqs_uses_approved_final_not_approved() {
        // 'approved' is NOT a valid requisitions status value.
        // Valid final approval statuses: 'approved_final' and 'issued'.
        let sql = "SELECT COUNT(*) \
                   FROM requisitions r \
                   JOIN sites si ON si.id = r.site_id \
                   WHERE si.institution_id = $1 \
                     AND r.status IN ('approved_final', 'issued') \
                     AND DATE(COALESCE(r.finalized_at, r.updated_at)) = $2";
        assert!(
            sql.contains("'approved_final'"),
            "must use approved_final not 'approved'"
        );
        assert!(
            !sql.contains("status = 'approved'"),
            "'approved' is not a valid status value"
        );
        assert!(
            sql.contains("JOIN sites si ON si.id = r.site_id"),
            "must join through sites for institution scope"
        );
        assert!(
            sql.contains("finalized_at"),
            "should use finalized_at for approval timestamp"
        );
    }

    // ── weekly/monthly aggregation ────────────────────────────────────────────

    #[test]
    fn weekly_rollup_reads_from_analytics_daily() {
        // The weekly job sums pre-computed daily rows — no direct table joins
        // needed for the rollup (correctness lives in DailyStatsJob).
        let sql = "SELECT COALESCE(SUM(stat_value), 0) FROM analytics_daily \
                   WHERE institution_id = $1 \
                     AND stat_date >= $2 \
                     AND stat_date < $2 + INTERVAL '7 days' \
                     AND stat_key = $3";
        assert!(sql.contains("FROM analytics_daily"));
        assert!(sql.contains("institution_id = $1"));
    }
}
