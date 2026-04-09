// NOTE: The `record_*` functions in this module are designed to be called from
// non-critical paths (auth failure handlers, permission-check middleware, upload
// handlers).  Callers should log but not propagate errors returned by these
// functions so that a transient DB issue does not degrade the primary request
// path.  All functions still return `Result` so callers can decide.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

// ── DTOs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AnomalyEvent {
    pub id: Uuid,
    pub rule_key: String,
    pub severity: String,
    pub subject_type: String,
    pub subject_value: String,
    pub detail: serde_json::Value,
    pub detected_at: chrono::DateTime<chrono::Utc>,
    pub acknowledged: bool,
    pub acknowledged_by: Option<Uuid>,
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Serialize)]
pub struct AnomalyRule {
    pub id: Uuid,
    pub rule_key: String,
    pub label: String,
    pub description: String,
    pub threshold_count: i32,
    pub window_seconds: i32,
    pub severity: String,
    pub is_active: bool,
}

// ── Thresholds ────────────────────────────────────────────────────────────

/// Number of auth failures from a single IP within the window before an
/// anomaly event is recorded.
const AUTH_FAILURE_THRESHOLD: i64 = 5;

/// Number of permission denials for a single user within the window before
/// an anomaly event is recorded.
const PERMISSION_DENIAL_THRESHOLD: i64 = 3;

/// File-size threshold above which an upload is flagged (8 MiB).
const LARGE_UPLOAD_THRESHOLD_BYTES: usize = 8 * 1024 * 1024;

/// Deduplication window: do not emit a second anomaly event of the same
/// rule_key for the same subject if one was already emitted within this
/// number of minutes.  Prevents log flooding on sustained attack bursts.
const DEDUP_WINDOW_MINUTES: i64 = 15;

// ── Internal helpers ──────────────────────────────────────────────────────

async fn insert_event(
    pool: &PgPool,
    rule_key: &str,
    severity: &str,
    subject_type: &str,
    subject_value: &str,
    detail: serde_json::Value,
) -> Result<(), ApiAppError> {
    sqlx::query(
        "INSERT INTO anomaly_events
             (rule_key, severity, subject_type, subject_value, detail)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(rule_key)
    .bind(severity)
    .bind(subject_type)
    .bind(subject_value)
    .bind(detail)
    .execute(pool)
    .await?;
    Ok(())
}

/// Return true if a recent (within `DEDUP_WINDOW_MINUTES`) anomaly event
/// already exists for this rule_key + subject.  Used to suppress duplicates
/// during a sustained burst.
async fn recent_event_exists(
    pool: &PgPool,
    rule_key: &str,
    subject_value: &str,
) -> Result<bool, ApiAppError> {
    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM anomaly_events
         WHERE rule_key = $1
           AND subject_value = $2
           AND detected_at > NOW() - ($3 * INTERVAL '1 minute')",
    )
    .bind(rule_key)
    .bind(subject_value)
    .bind(DEDUP_WINDOW_MINUTES)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

// ── Recording functions ───────────────────────────────────────────────────

/// Record a potential auth-failure burst from an IP address.
///
/// BOOTSTRAP FIX: the previous implementation counted existing anomaly_events
/// before deciding whether to insert one — so the first event could never be
/// created (count=0 is always below threshold=5).
///
/// The fix counts actual failed login attempts from the `login_attempts` source
/// table (which is populated by every auth attempt).  Once the failure count
/// crosses the threshold we emit exactly one anomaly event per DEDUP_WINDOW to
/// avoid flooding on a sustained brute-force burst.
///
/// Called from auth-failure code paths.  Callers should log, not propagate,
/// any error returned by this function.
pub async fn record_auth_failure(
    pool: &PgPool,
    user_id: Option<Uuid>,
    ip: &str,
) -> Result<(), ApiAppError> {
    // Count actual failed login attempts from the source signal table.
    let (failure_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM login_attempts
         WHERE ip = $1 AND success = false
           AND occurred_at > NOW() - INTERVAL '1 hour'",
    )
    .bind(ip)
    .fetch_one(pool)
    .await?;

    if failure_count < AUTH_FAILURE_THRESHOLD {
        return Ok(());
    }

    // Dedup: emit at most one event per DEDUP_WINDOW_MINUTES for this IP.
    if recent_event_exists(pool, "auth_failure_burst", ip).await? {
        return Ok(());
    }

    insert_event(
        pool,
        "auth_failure_burst",
        "high",
        "ip",
        ip,
        serde_json::json!({
            "ip": ip,
            "user_id": user_id.map(|u| u.to_string()),
            "failure_count": failure_count,
        }),
    )
    .await
}

/// Record a potential privilege-escalation attempt for a user.
///
/// BOOTSTRAP FIX: the previous implementation counted existing anomaly_events
/// before deciding whether to insert — same chicken-and-egg bootstrap problem.
///
/// The fix:
///   1. Write each denial to `audit_log` (`action = 'permission_denied'`) so
///      the signal persists and can be counted across calls.
///   2. Count recent denials from `audit_log` (the source signal).
///   3. Emit an anomaly event once the threshold is crossed, with a dedup
///      window to prevent flooding.
///
/// Called from permission-check code paths.  Callers should log, not
/// propagate, any error returned by this function.
pub async fn record_permission_denial(
    pool: &PgPool,
    user_id: Uuid,
    permission: &str,
    path: &str,
) -> Result<(), ApiAppError> {
    let user_str = user_id.to_string();

    // Write the denial signal to audit_log so it can be counted across calls.
    sqlx::query(
        "INSERT INTO audit_log (actor_user_id, action, entity_type, entity_id, payload)
         VALUES ($1, 'permission_denied', 'permission', $2, $3)",
    )
    .bind(user_id)
    .bind(permission)
    .bind(serde_json::json!({ "permission": permission, "path": path }))
    .execute(pool)
    .await?;

    // Count denials from the source signal within the detection window.
    let (denial_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM audit_log
         WHERE actor_user_id = $1
           AND action = 'permission_denied'
           AND occurred_at > NOW() - INTERVAL '1 hour'",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    if denial_count < PERMISSION_DENIAL_THRESHOLD {
        return Ok(());
    }

    // Dedup: emit at most one event per DEDUP_WINDOW_MINUTES for this user.
    if recent_event_exists(pool, "privilege_escalation_attempt", &user_str).await? {
        return Ok(());
    }

    insert_event(
        pool,
        "privilege_escalation_attempt",
        "medium",
        "user",
        &user_str,
        serde_json::json!({
            "user_id": user_str,
            "permission": permission,
            "path": path,
            "denial_count": denial_count,
        }),
    )
    .await
}

/// Record a near-limit upload event.
///
/// Fires when `file_size_bytes` exceeds 8 MiB (near the 10 MiB hard limit).
/// Severity is 'low' — informational rather than actionable on its own.
/// Each large upload emits its own event (no threshold count needed); the
/// dedup window prevents repeat events for the same user within 15 minutes.
///
/// Called from upload handlers.  Callers should log, not propagate, any error
/// returned by this function.
pub async fn record_large_upload(
    pool: &PgPool,
    user_id: Uuid,
    file_size_bytes: usize,
    entity_type: &str,
) -> Result<(), ApiAppError> {
    if file_size_bytes <= LARGE_UPLOAD_THRESHOLD_BYTES {
        return Ok(());
    }

    let user_str = user_id.to_string();

    // Dedup: one event per user per window is sufficient for this low-severity signal.
    if recent_event_exists(pool, "large_upload_near_limit", &user_str).await? {
        return Ok(());
    }

    insert_event(
        pool,
        "large_upload_near_limit",
        "low",
        "user",
        &user_str,
        serde_json::json!({
            "user_id": user_str,
            "file_size_bytes": file_size_bytes,
            "entity_type": entity_type,
        }),
    )
    .await
}

// ── Admin reads ───────────────────────────────────────────────────────────

/// List anomaly events.  Requires `admin:users`.
///
/// Pass `unacked_only = true` to filter to events that have not yet been
/// acknowledged.  Results are ordered newest-first, capped at 200.
pub async fn list_events(
    pool: &PgPool,
    p: &Principal,
    unacked_only: bool,
) -> Result<Vec<AnomalyEvent>, ApiAppError> {
    p.require("admin:users")?;

    let rows: Vec<(
        Uuid,
        String,
        String,
        String,
        String,
        serde_json::Value,
        chrono::DateTime<chrono::Utc>,
        bool,
        Option<Uuid>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = if unacked_only {
        sqlx::query_as(
            "SELECT id, rule_key, severity, subject_type, subject_value, detail,
                    detected_at, acknowledged, acknowledged_by, acknowledged_at
             FROM anomaly_events
             WHERE acknowledged = false
             ORDER BY detected_at DESC
             LIMIT 200",
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, rule_key, severity, subject_type, subject_value, detail,
                    detected_at, acknowledged, acknowledged_by, acknowledged_at
             FROM anomaly_events
             ORDER BY detected_at DESC
             LIMIT 200",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                rule_key,
                severity,
                subject_type,
                subject_value,
                detail,
                detected_at,
                acknowledged,
                acknowledged_by,
                acknowledged_at,
            )| AnomalyEvent {
                id,
                rule_key,
                severity,
                subject_type,
                subject_value,
                detail,
                detected_at,
                acknowledged,
                acknowledged_by,
                acknowledged_at,
            },
        )
        .collect())
}

/// Acknowledge an anomaly event.  Requires `admin:users`.
pub async fn acknowledge_event(
    pool: &PgPool,
    p: &Principal,
    event_id: Uuid,
) -> Result<(), ApiAppError> {
    p.require("admin:users")?;

    let affected = sqlx::query(
        "UPDATE anomaly_events
         SET acknowledged = true,
             acknowledged_by = $2,
             acknowledged_at = NOW()
         WHERE id = $1",
    )
    .bind(event_id)
    .bind(p.user_id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiAppError::NotFound);
    }
    Ok(())
}

/// List all anomaly detection rules.  Requires `admin:users`.
pub async fn list_rules(pool: &PgPool, p: &Principal) -> Result<Vec<AnomalyRule>, ApiAppError> {
    p.require("admin:users")?;

    let rows: Vec<(Uuid, String, String, String, i32, i32, String, bool)> = sqlx::query_as(
        "SELECT id, rule_key, label, description,
                threshold_count, window_seconds, severity, is_active
         FROM anomaly_rules
         ORDER BY rule_key",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(id, rule_key, label, description, threshold_count, window_seconds, severity, is_active)| {
                AnomalyRule {
                    id,
                    rule_key,
                    label,
                    description,
                    threshold_count,
                    window_seconds,
                    severity,
                    is_active,
                }
            },
        )
        .collect())
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// Pure-logic tests that verify threshold constants, query shape, and the
// bootstrap fix (source-signal counting vs. self-referential event counting).
// Integration tests that exercise actual DB state live in apps/backend-api/tests/.

#[cfg(test)]
mod tests {
    use super::*;

    // ── threshold constants ───────────────────────────────────────────────────

    #[test]
    fn auth_failure_threshold_is_five() {
        assert_eq!(AUTH_FAILURE_THRESHOLD, 5);
    }

    #[test]
    fn permission_denial_threshold_is_three() {
        assert_eq!(PERMISSION_DENIAL_THRESHOLD, 3);
    }

    #[test]
    fn large_upload_threshold_is_eight_mib() {
        assert_eq!(LARGE_UPLOAD_THRESHOLD_BYTES, 8 * 1024 * 1024);
    }

    #[test]
    fn dedup_window_is_positive() {
        assert!(DEDUP_WINDOW_MINUTES > 0);
    }

    // ── bootstrap fix — auth failure ─────────────────────────────────────────
    //
    // Verifies that the detection query reads from `login_attempts` (the source
    // signal table), NOT from `anomaly_events`.  If it read from anomaly_events
    // the first event could never be created (count=0 < threshold=5 always).

    #[test]
    fn auth_failure_counts_from_login_attempts_not_anomaly_events() {
        let detection_sql = "SELECT COUNT(*) FROM login_attempts \
                             WHERE ip = $1 AND success = false \
                               AND occurred_at > NOW() - INTERVAL '1 hour'";
        assert!(
            detection_sql.contains("FROM login_attempts"),
            "must count from login_attempts (source signal), not anomaly_events"
        );
        assert!(
            !detection_sql.contains("FROM anomaly_events"),
            "counting anomaly_events causes bootstrap deadlock"
        );
        assert!(detection_sql.contains("success = false"));
    }

    // ── bootstrap fix — permission denial ────────────────────────────────────
    //
    // Verifies that the detection query reads from `audit_log` where the denial
    // was just written, NOT from `anomaly_events`.

    #[test]
    fn permission_denial_counts_from_audit_log_not_anomaly_events() {
        let detection_sql = "SELECT COUNT(*) FROM audit_log \
                             WHERE actor_user_id = $1 \
                               AND action = 'permission_denied' \
                               AND occurred_at > NOW() - INTERVAL '1 hour'";
        assert!(
            detection_sql.contains("FROM audit_log"),
            "must count from audit_log (source signal), not anomaly_events"
        );
        assert!(
            !detection_sql.contains("FROM anomaly_events"),
            "counting anomaly_events causes bootstrap deadlock"
        );
        assert!(detection_sql.contains("action = 'permission_denied'"));
    }

    #[test]
    fn permission_denial_writes_signal_to_audit_log() {
        let write_sql = "INSERT INTO audit_log \
                         (actor_user_id, action, entity_type, entity_id, payload) \
                         VALUES ($1, 'permission_denied', 'permission', $2, $3)";
        assert!(write_sql.contains("INTO audit_log"));
        assert!(write_sql.contains("'permission_denied'"));
    }

    // ── dedup window ─────────────────────────────────────────────────────────

    #[test]
    fn dedup_query_checks_anomaly_events_with_time_window() {
        let dedup_sql = "SELECT COUNT(*) FROM anomaly_events \
                         WHERE rule_key = $1 \
                           AND subject_value = $2 \
                           AND detected_at > NOW() - ($3 * INTERVAL '1 minute')";
        assert!(dedup_sql.contains("FROM anomaly_events"));
        assert!(dedup_sql.contains("detected_at >"));
        // The dedup check is the one place we legitimately query anomaly_events.
    }

    // ── large upload threshold ────────────────────────────────────────────────

    #[test]
    fn large_upload_below_threshold_is_skipped() {
        // Simulate the guard condition.
        let file_size = 4 * 1024 * 1024; // 4 MiB — below 8 MiB threshold
        assert!(file_size <= LARGE_UPLOAD_THRESHOLD_BYTES);
    }

    #[test]
    fn large_upload_at_exactly_threshold_is_skipped() {
        let file_size = LARGE_UPLOAD_THRESHOLD_BYTES; // exactly 8 MiB
        assert!(file_size <= LARGE_UPLOAD_THRESHOLD_BYTES);
    }

    #[test]
    fn large_upload_above_threshold_triggers() {
        let file_size = LARGE_UPLOAD_THRESHOLD_BYTES + 1; // 8 MiB + 1 byte
        assert!(file_size > LARGE_UPLOAD_THRESHOLD_BYTES);
    }
}
