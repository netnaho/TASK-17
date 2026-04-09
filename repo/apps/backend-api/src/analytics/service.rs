use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::{Principal, ScopeKind};

#[derive(Debug, Serialize)]
pub struct StatRow {
    pub stat_key: String,
    pub stat_value: f64,
}

#[derive(Debug, Serialize)]
pub struct DailyStats {
    pub institution_id: Uuid,
    pub stat_date: String,
    pub stats: Vec<StatRow>,
}

#[derive(Debug, Serialize)]
pub struct WeeklyStats {
    pub institution_id: Uuid,
    pub week_start: String,
    pub stats: Vec<StatRow>,
}

#[derive(Debug, Serialize)]
pub struct MonthlyStats {
    pub institution_id: Uuid,
    pub year_month: String,
    pub stats: Vec<StatRow>,
}

#[derive(Debug, Serialize)]
pub struct DashboardView {
    pub institution_id: Uuid,
    /// Most recent 7 days of daily stats
    pub recent_daily: Vec<DailyStats>,
    /// Most recent 4 weeks
    pub recent_weekly: Vec<WeeklyStats>,
    /// Most recent 3 months
    pub recent_monthly: Vec<MonthlyStats>,
    /// Live counts (computed on the fly, not from analytics tables)
    pub live: LiveCounts,
}

#[derive(Debug, Serialize)]
pub struct LiveCounts {
    pub pending_requisitions: i64,
    pub open_orders: i64,
    pub wellness_sessions_this_week: i64,
    pub moderation_queue_pending: i64,
    pub anomaly_events_unacked: i64,
}

// ── Authorization ─────────────────────────────────────────────────────────────
//
// Analytics data is partitioned by institution_id.  Every entry point must call
// `assert_institution_scope` after the permission check to prevent a caller
// scoped to institution A from reading institution B's analytics by simply
// passing a different UUID in the query string.
//
// Allowed principals
//   • `scope:any`                     — global admin; bypassed inside require_scope
//   • ScopeKind::Institution matching — caller explicitly granted this institution
//
// The `scope:any` bypass is handled inside `p.require_scope`, so this helper
// covers both branches with a single call.
fn assert_institution_scope(p: &Principal, institution_id: Uuid) -> Result<(), ApiAppError> {
    p.require_scope(ScopeKind::Institution, &institution_id.to_string())
}

pub async fn get_daily_stats(
    pool: &PgPool,
    p: &Principal,
    institution_id: Uuid,
    stat_date: chrono::NaiveDate,
) -> Result<DailyStats, ApiAppError> {
    p.require("analytics:read")?;
    assert_institution_scope(p, institution_id)?;

    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT stat_key, stat_value::float8
         FROM analytics_daily
         WHERE institution_id = $1 AND stat_date = $2
         ORDER BY stat_key",
    )
    .bind(institution_id)
    .bind(stat_date)
    .fetch_all(pool)
    .await?;

    Ok(DailyStats {
        institution_id,
        stat_date: stat_date.to_string(),
        stats: rows
            .into_iter()
            .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
            .collect(),
    })
}

pub async fn get_weekly_stats(
    pool: &PgPool,
    p: &Principal,
    institution_id: Uuid,
    week_start: chrono::NaiveDate,
) -> Result<WeeklyStats, ApiAppError> {
    p.require("analytics:read")?;
    assert_institution_scope(p, institution_id)?;

    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT stat_key, stat_value::float8
         FROM analytics_weekly
         WHERE institution_id = $1 AND week_start = $2
         ORDER BY stat_key",
    )
    .bind(institution_id)
    .bind(week_start)
    .fetch_all(pool)
    .await?;

    Ok(WeeklyStats {
        institution_id,
        week_start: week_start.to_string(),
        stats: rows
            .into_iter()
            .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
            .collect(),
    })
}

pub async fn get_monthly_stats(
    pool: &PgPool,
    p: &Principal,
    institution_id: Uuid,
    year_month: &str,
) -> Result<MonthlyStats, ApiAppError> {
    p.require("analytics:read")?;
    assert_institution_scope(p, institution_id)?;

    if year_month.len() != 7 || !year_month.contains('-') {
        return Err(ApiAppError::BadRequest(
            "year_month must be in YYYY-MM format".into(),
        ));
    }

    let rows: Vec<(String, f64)> = sqlx::query_as(
        "SELECT stat_key, stat_value::float8
         FROM analytics_monthly
         WHERE institution_id = $1 AND year_month = $2
         ORDER BY stat_key",
    )
    .bind(institution_id)
    .bind(year_month)
    .fetch_all(pool)
    .await?;

    Ok(MonthlyStats {
        institution_id,
        year_month: year_month.to_string(),
        stats: rows
            .into_iter()
            .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
            .collect(),
    })
}

pub async fn get_dashboard(
    pool: &PgPool,
    p: &Principal,
    institution_id: Uuid,
) -> Result<DashboardView, ApiAppError> {
    p.require("analytics:read")?;
    assert_institution_scope(p, institution_id)?;

    // --- recent_daily: last 7 distinct dates ---
    let daily_dates: Vec<(chrono::NaiveDate,)> = sqlx::query_as(
        "SELECT DISTINCT stat_date
         FROM analytics_daily
         WHERE institution_id = $1
         ORDER BY stat_date DESC
         LIMIT 7",
    )
    .bind(institution_id)
    .fetch_all(pool)
    .await?;

    let mut recent_daily = Vec::new();
    for (date,) in daily_dates {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT stat_key, stat_value::float8
             FROM analytics_daily
             WHERE institution_id = $1 AND stat_date = $2
             ORDER BY stat_key",
        )
        .bind(institution_id)
        .bind(date)
        .fetch_all(pool)
        .await?;

        recent_daily.push(DailyStats {
            institution_id,
            stat_date: date.to_string(),
            stats: rows
                .into_iter()
                .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
                .collect(),
        });
    }

    // --- recent_weekly: last 4 week_starts ---
    let weekly_starts: Vec<(chrono::NaiveDate,)> = sqlx::query_as(
        "SELECT DISTINCT week_start
         FROM analytics_weekly
         WHERE institution_id = $1
         ORDER BY week_start DESC
         LIMIT 4",
    )
    .bind(institution_id)
    .fetch_all(pool)
    .await?;

    let mut recent_weekly = Vec::new();
    for (week_start,) in weekly_starts {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT stat_key, stat_value::float8
             FROM analytics_weekly
             WHERE institution_id = $1 AND week_start = $2
             ORDER BY stat_key",
        )
        .bind(institution_id)
        .bind(week_start)
        .fetch_all(pool)
        .await?;

        recent_weekly.push(WeeklyStats {
            institution_id,
            week_start: week_start.to_string(),
            stats: rows
                .into_iter()
                .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
                .collect(),
        });
    }

    // --- recent_monthly: last 3 year_months ---
    let monthly_periods: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT year_month
         FROM analytics_monthly
         WHERE institution_id = $1
         ORDER BY year_month DESC
         LIMIT 3",
    )
    .bind(institution_id)
    .fetch_all(pool)
    .await?;

    let mut recent_monthly = Vec::new();
    for (year_month,) in monthly_periods {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT stat_key, stat_value::float8
             FROM analytics_monthly
             WHERE institution_id = $1 AND year_month = $2
             ORDER BY stat_key",
        )
        .bind(institution_id)
        .bind(&year_month)
        .fetch_all(pool)
        .await?;

        recent_monthly.push(MonthlyStats {
            institution_id,
            year_month,
            stats: rows
                .into_iter()
                .map(|(stat_key, stat_value)| StatRow { stat_key, stat_value })
                .collect(),
        });
    }

    // --- live counts ---
    let (pending_requisitions,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM requisitions r
         JOIN sites s ON s.id = r.site_id
         WHERE s.institution_id = $1 AND r.status = 'pending_approval'",
    )
    .bind(institution_id)
    .fetch_one(pool)
    .await?;

    let (open_orders,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM orders WHERE status = 'confirmed'",
    )
    .fetch_one(pool)
    .await?;

    let (wellness_sessions_this_week,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wellness_activities
         WHERE institution_id = $1 AND activity_date >= date_trunc('week', CURRENT_DATE)",
    )
    .bind(institution_id)
    .fetch_one(pool)
    .await?;

    let (moderation_queue_pending,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM moderation_queue WHERE status = 'pending'",
    )
    .fetch_one(pool)
    .await?;

    let (anomaly_events_unacked,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM anomaly_events WHERE acknowledged = false",
    )
    .fetch_one(pool)
    .await?;

    Ok(DashboardView {
        institution_id,
        recent_daily,
        recent_weekly,
        recent_monthly,
        live: LiveCounts {
            pending_requisitions,
            open_orders,
            wellness_sessions_this_week,
            moderation_queue_pending,
            anomaly_events_unacked,
        },
    })
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// `assert_institution_scope` is a pure sync function — no DB needed.
// Each test constructs a Principal directly and asserts the expected outcome.
// Integration tests that exercise the full SQL path live in
// apps/backend-api/tests/.
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::security::rbac::{Principal, Scope};

    fn make_principal(perms: &[&str], institution_scopes: &[Uuid]) -> Principal {
        Principal {
            user_id: Uuid::new_v4(),
            email: "test@example.com".into(),
            roles: vec![],
            permissions: perms.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
            scopes: institution_scopes
                .iter()
                .map(|id| Scope {
                    kind: ScopeKind::Institution,
                    reference: id.to_string(),
                })
                .collect(),
        }
    }

    // ── assert_institution_scope ───────────────────────────────────────────

    #[test]
    fn correct_institution_scope_allowed() {
        let inst = Uuid::new_v4();
        let p = make_principal(&["analytics:read"], &[inst]);
        assert!(assert_institution_scope(&p, inst).is_ok());
    }

    #[test]
    fn wrong_institution_scope_forbidden() {
        let inst_a = Uuid::new_v4();
        let inst_b = Uuid::new_v4();
        let p = make_principal(&["analytics:read"], &[inst_a]);
        let err = assert_institution_scope(&p, inst_b).unwrap_err();
        assert!(
            matches!(err, ApiAppError::Forbidden(_)),
            "expected Forbidden, got {err:?}"
        );
    }

    #[test]
    fn no_institution_scope_forbidden() {
        // Principal has the permission but zero institution scopes.
        let p = make_principal(&["analytics:read"], &[]);
        let err = assert_institution_scope(&p, Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }

    #[test]
    fn scope_any_bypasses_institution_check() {
        // scope:any is the global admin bypass — no explicit institution scope needed.
        let p = make_principal(&["analytics:read", "scope:any"], &[]);
        assert!(assert_institution_scope(&p, Uuid::new_v4()).is_ok());
    }

    #[test]
    fn scope_any_works_across_multiple_arbitrary_institutions() {
        let p = make_principal(&["scope:any"], &[]);
        for _ in 0..3 {
            assert!(assert_institution_scope(&p, Uuid::new_v4()).is_ok());
        }
    }

    #[test]
    fn multiple_institution_scopes_correct_one_allowed() {
        let inst_a = Uuid::new_v4();
        let inst_b = Uuid::new_v4();
        let p = make_principal(&["analytics:read"], &[inst_a, inst_b]);
        // Both scoped institutions are accessible.
        assert!(assert_institution_scope(&p, inst_a).is_ok());
        assert!(assert_institution_scope(&p, inst_b).is_ok());
    }

    #[test]
    fn multiple_institution_scopes_unscoped_one_forbidden() {
        let inst_a = Uuid::new_v4();
        let inst_b = Uuid::new_v4();
        let inst_c = Uuid::new_v4(); // not in scope
        let p = make_principal(&["analytics:read"], &[inst_a, inst_b]);
        let err = assert_institution_scope(&p, inst_c).unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }

    #[test]
    fn missing_analytics_read_permission_rejected_before_scope_check() {
        // p.require("analytics:read") fires before assert_institution_scope, so
        // a missing permission always returns Forbidden regardless of scope.
        let inst = Uuid::new_v4();
        let p = make_principal(&[], &[inst]); // no analytics:read
        let err = p.require("analytics:read").unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }
}
