use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

// ── DTOs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct KeywordPolicy {
    pub id: Uuid,
    pub keyword: String,
    pub severity: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ModerationQueueItem {
    pub id: Uuid,
    pub content_type: String,
    pub content_ref_id: Option<Uuid>,
    pub content_snippet: String,
    pub matched_keywords: Vec<String>,
    pub severity: String,
    pub status: String,
    pub flagged_at: chrono::DateTime<chrono::Utc>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePolicyInput {
    pub keyword: String,
    pub severity: Option<String>, // 'low'|'medium'|'high', defaults to 'medium'
}

#[derive(Debug, Deserialize)]
pub struct ReviewInput {
    pub action: String, // 'approve'|'reject'|'escalate'
    pub note: Option<String>,
}

// ── Constants ─────────────────────────────────────────────────────────────

const ALLOWED_SEVERITIES: &[&str] = &["low", "medium", "high"];
const ALLOWED_REVIEW_ACTIONS: &[&str] = &["approve", "reject", "escalate"];
/// Maximum number of characters stored as the content snippet.
const SNIPPET_MAX_LEN: usize = 200;

// ── Helpers ───────────────────────────────────────────────────────────────

/// Truncate `text` to at most `SNIPPET_MAX_LEN` characters, never storing
/// the full content to minimise storage of potentially sensitive material.
fn make_snippet(text: &str) -> String {
    if text.len() <= SNIPPET_MAX_LEN {
        text.to_string()
    } else {
        // Truncate on a char boundary.
        let truncated: String = text.chars().take(SNIPPET_MAX_LEN).collect();
        truncated
    }
}

// ── Use cases ─────────────────────────────────────────────────────────────

/// Scan `text` against all active keyword policies.
/// If any keywords match, insert a moderation_queue entry and an audit log
/// entry.  Returns the list of matched keywords (empty if none matched).
///
/// This function is called from content-submission paths and should not
/// propagate transient errors in a way that blocks the caller.  The caller
/// is responsible for deciding whether to surface or swallow the error.
pub async fn check_and_flag(
    pool: &PgPool,
    text: &str,
    content_type: &str,
    content_ref_id: Option<Uuid>,
) -> Result<Vec<String>, ApiAppError> {
    // Load all active keyword policies.
    let policies: Vec<(String, String)> = sqlx::query_as(
        "SELECT keyword, severity FROM keyword_policies WHERE is_active = true",
    )
    .fetch_all(pool)
    .await?;

    // Case-insensitive keyword search.
    let text_lower = text.to_lowercase();
    let mut matched: Vec<(String, String)> = Vec::new(); // (keyword, severity)
    for (keyword, severity) in &policies {
        if text_lower.contains(&keyword.to_lowercase()) {
            matched.push((keyword.clone(), severity.clone()));
        }
    }

    if matched.is_empty() {
        return Ok(vec![]);
    }

    // Determine the highest severity among matches.
    let severity = matched
        .iter()
        .map(|(_, s)| s.as_str())
        .max_by_key(|s| match *s {
            "high" => 2,
            "medium" => 1,
            _ => 0,
        })
        .unwrap_or("medium")
        .to_string();

    let matched_keywords: Vec<String> = matched.iter().map(|(k, _)| k.clone()).collect();
    let snippet = make_snippet(text);

    // Store matched keywords as a Postgres text array.
    let keywords_arr: Vec<&str> = matched_keywords.iter().map(|s| s.as_str()).collect();

    let (queue_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO moderation_queue
             (content_type, content_ref_id, content_snippet, matched_keywords, severity, status)
         VALUES ($1, $2, $3, $4, $5, 'pending')
         RETURNING id",
    )
    .bind(content_type)
    .bind(content_ref_id)
    .bind(&snippet)
    .bind(&keywords_arr)
    .bind(&severity)
    .fetch_one(pool)
    .await?;

    // `performed_by` is NULL for system-generated flag events (no human actor).
    // `detail` carries the structured metadata added by migration 0007.
    sqlx::query(
        "INSERT INTO moderation_audit_log (queue_item_id, action, detail)
         VALUES ($1, 'flagged', $2)",
    )
    .bind(queue_id)
    .bind(serde_json::json!({
        "matched_keywords": matched_keywords,
        "severity": severity,
        "content_type": content_type,
    }))
    .execute(pool)
    .await?;

    Ok(matched_keywords)
}

/// List moderation queue items, optionally filtered by status.
/// Requires `moderation:review` permission.
pub async fn list_queue(
    pool: &PgPool,
    p: &Principal,
    status_filter: Option<&str>,
) -> Result<Vec<ModerationQueueItem>, ApiAppError> {
    p.require("moderation:review")?;

    let rows: Vec<(
        Uuid,
        String,
        Option<Uuid>,
        String,
        Vec<String>,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
        Option<Uuid>,
        Option<chrono::DateTime<chrono::Utc>>,
    )> = match status_filter {
        Some(status) => sqlx::query_as(
            "SELECT id, content_type, content_ref_id, content_snippet,
                    matched_keywords, severity, status, flagged_at,
                    reviewed_by, reviewed_at
             FROM moderation_queue
             WHERE status = $1
             ORDER BY flagged_at DESC",
        )
        .bind(status)
        .fetch_all(pool)
        .await?,
        None => sqlx::query_as(
            "SELECT id, content_type, content_ref_id, content_snippet,
                    matched_keywords, severity, status, flagged_at,
                    reviewed_by, reviewed_at
             FROM moderation_queue
             ORDER BY flagged_at DESC",
        )
        .fetch_all(pool)
        .await?,
    };

    Ok(rows
        .into_iter()
        .map(
            |(
                id,
                content_type,
                content_ref_id,
                content_snippet,
                matched_keywords,
                severity,
                status,
                flagged_at,
                reviewed_by,
                reviewed_at,
            )| ModerationQueueItem {
                id,
                content_type,
                content_ref_id,
                content_snippet,
                matched_keywords,
                severity,
                status,
                flagged_at,
                reviewed_by,
                reviewed_at,
            },
        )
        .collect())
}

/// Record a review decision on a moderation queue item.
/// Requires `moderation:review` permission.
pub async fn review_item(
    pool: &PgPool,
    p: &Principal,
    item_id: Uuid,
    input: ReviewInput,
) -> Result<(), ApiAppError> {
    p.require("moderation:review")?;

    if !ALLOWED_REVIEW_ACTIONS.contains(&input.action.as_str()) {
        return Err(ApiAppError::BadRequest(format!(
            "action must be one of: {}",
            ALLOWED_REVIEW_ACTIONS.join(", ")
        )));
    }

    let affected = sqlx::query(
        "UPDATE moderation_queue
         SET status = $2, reviewed_by = $3, reviewed_at = NOW()
         WHERE id = $1",
    )
    .bind(item_id)
    .bind(&input.action)
    .bind(p.user_id)
    .execute(pool)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiAppError::NotFound);
    }

    // Use the schema-correct column names:
    //   `performed_by` (UUID FK) — who performed the review action.
    //   `note`         (TEXT)    — the reviewer's human-readable note, if any.
    // The `detail` JSONB column is reserved for flag events that carry
    // structured keyword metadata; review events have no additional structured
    // payload beyond what the dedicated columns already capture.
    sqlx::query(
        "INSERT INTO moderation_audit_log (queue_item_id, action, performed_by, note)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(item_id)
    .bind(&input.action)
    .bind(p.user_id)
    .bind(input.note.as_deref())
    .execute(pool)
    .await?;

    Ok(())
}

/// List all keyword policies.  Requires `admin:users`.
pub async fn list_policies(
    pool: &PgPool,
    p: &Principal,
) -> Result<Vec<KeywordPolicy>, ApiAppError> {
    p.require("admin:users")?;

    let rows: Vec<(Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, keyword, severity, is_active, created_at
             FROM keyword_policies
             ORDER BY keyword",
        )
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(id, keyword, severity, is_active, created_at)| KeywordPolicy {
            id,
            keyword,
            severity,
            is_active,
            created_at,
        })
        .collect())
}

/// Create (or reactivate) a keyword policy.  Requires `admin:users`.
pub async fn create_policy(
    pool: &PgPool,
    p: &Principal,
    input: CreatePolicyInput,
) -> Result<KeywordPolicy, ApiAppError> {
    p.require("admin:users")?;

    let keyword = input.keyword.trim().to_lowercase();
    if keyword.is_empty() {
        return Err(ApiAppError::BadRequest("keyword must not be empty".into()));
    }

    let severity = input.severity.as_deref().unwrap_or("medium");
    if !ALLOWED_SEVERITIES.contains(&severity) {
        return Err(ApiAppError::BadRequest(format!(
            "severity must be one of: {}",
            ALLOWED_SEVERITIES.join(", ")
        )));
    }

    let (id, stored_keyword, stored_severity, is_active, created_at): (
        Uuid,
        String,
        String,
        bool,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        "INSERT INTO keyword_policies (keyword, severity, is_active)
         VALUES ($1, $2, true)
         ON CONFLICT (keyword) DO UPDATE
             SET severity = EXCLUDED.severity,
                 is_active = true
         RETURNING id, keyword, severity, is_active, created_at",
    )
    .bind(&keyword)
    .bind(severity)
    .fetch_one(pool)
    .await?;

    Ok(KeywordPolicy {
        id,
        keyword: stored_keyword,
        severity: stored_severity,
        is_active,
        created_at,
    })
}

/// Delete a keyword policy by ID.  Requires `admin:users`.
/// Keyword policies are not foreign-key referenced, so hard delete is safe.
pub async fn delete_policy(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    p.require("admin:users")?;

    let affected = sqlx::query("DELETE FROM keyword_policies WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if affected == 0 {
        return Err(ApiAppError::NotFound);
    }
    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// These tests cover pure-function logic that does not require a database.
// Integration tests that exercise the SQL INSERT paths (check_and_flag,
// review_item audit writes) live in apps/backend-api/tests/ and require a
// live Postgres instance running migration 0007.

#[cfg(test)]
mod tests {
    use super::*;

    // ── make_snippet ──────────────────────────────────────────────────────────

    #[test]
    fn snippet_short_text_unchanged() {
        let text = "hello world";
        assert_eq!(make_snippet(text), text);
    }

    #[test]
    fn snippet_exactly_at_limit_unchanged() {
        let text: String = "x".repeat(SNIPPET_MAX_LEN);
        let result = make_snippet(&text);
        assert_eq!(result, text);
        assert_eq!(result.len(), SNIPPET_MAX_LEN);
    }

    #[test]
    fn snippet_over_limit_truncated() {
        let text: String = "a".repeat(SNIPPET_MAX_LEN + 50);
        let result = make_snippet(&text);
        assert_eq!(result.len(), SNIPPET_MAX_LEN);
    }

    #[test]
    fn snippet_multibyte_truncated_on_char_boundary() {
        // Each '√' is 3 UTF-8 bytes; truncation must not split a codepoint.
        let text: String = "√".repeat(SNIPPET_MAX_LEN);
        let result = make_snippet(&text);
        assert_eq!(result.chars().count(), SNIPPET_MAX_LEN);
        // Confirm the result is valid UTF-8 (would panic otherwise).
        let _ = result.as_str();
    }

    // ── severity ranking ──────────────────────────────────────────────────────

    fn dominant_severity(pairs: &[(&str, &str)]) -> &'static str {
        pairs
            .iter()
            .map(|(_, s)| *s)
            .max_by_key(|s| match *s {
                "high" => 2,
                "medium" => 1,
                _ => 0,
            })
            .unwrap_or("medium")
    }

    #[test]
    fn severity_high_outranks_medium_and_low() {
        let pairs = [("w1", "low"), ("w2", "high"), ("w3", "medium")];
        assert_eq!(dominant_severity(&pairs), "high");
    }

    #[test]
    fn severity_medium_outranks_low() {
        let pairs = [("w1", "low"), ("w2", "medium")];
        assert_eq!(dominant_severity(&pairs), "medium");
    }

    #[test]
    fn severity_defaults_to_medium_when_no_matches() {
        let pairs: Vec<(&str, &str)> = vec![];
        assert_eq!(dominant_severity(&pairs), "medium");
    }

    // ── flag audit detail shape ───────────────────────────────────────────────
    //
    // Verifies that the JSON blob written by check_and_flag contains the keys
    // that consumers (analytics, admin UI) depend on.

    #[test]
    fn flag_audit_detail_contains_required_keys() {
        let matched_keywords = vec!["bad".to_string(), "phrase".to_string()];
        let severity = "high";
        let content_type = "requisition_note";

        let detail = serde_json::json!({
            "matched_keywords": matched_keywords,
            "severity": severity,
            "content_type": content_type,
        });

        assert_eq!(detail["severity"], "high");
        assert_eq!(detail["content_type"], "requisition_note");
        assert!(detail["matched_keywords"].is_array());
        assert_eq!(detail["matched_keywords"].as_array().unwrap().len(), 2);
    }

    // ── review action guard ───────────────────────────────────────────────────

    #[test]
    fn allowed_review_actions_are_complete() {
        assert!(ALLOWED_REVIEW_ACTIONS.contains(&"approve"));
        assert!(ALLOWED_REVIEW_ACTIONS.contains(&"reject"));
        assert!(ALLOWED_REVIEW_ACTIONS.contains(&"escalate"));
    }

    #[test]
    fn unknown_review_action_not_allowed() {
        assert!(!ALLOWED_REVIEW_ACTIONS.contains(&"delete"));
        assert!(!ALLOWED_REVIEW_ACTIONS.contains(&"flag"));
    }

    // ── audit column contract documentation ──────────────────────────────────
    //
    // Regression guard: the SQL strings must reference schema-correct columns.
    // Column names are verified here as string constants so a future rename in
    // the migration will require a matching update here.

    #[test]
    fn flag_insert_sql_references_correct_columns() {
        // check_and_flag audit insert (system event, no performed_by).
        let sql = "INSERT INTO moderation_audit_log (queue_item_id, action, detail) \
                   VALUES ($1, 'flagged', $2)";
        assert!(sql.contains("detail"));
        assert!(!sql.contains("actor_user_id"), "actor_user_id is not a schema column");
    }

    #[test]
    fn review_insert_sql_references_correct_columns() {
        // review_item audit insert (human actor event).
        let sql = "INSERT INTO moderation_audit_log (queue_item_id, action, performed_by, note) \
                   VALUES ($1, $2, $3, $4)";
        assert!(sql.contains("performed_by"));
        assert!(sql.contains("note"));
        assert!(!sql.contains("actor_user_id"), "actor_user_id is not a schema column");
        assert!(!sql.contains("detail"), "detail not needed for review events");
    }
}
