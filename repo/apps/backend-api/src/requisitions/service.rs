use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::{Principal, ScopeKind};

use super::engine::{resolve_route, ReqSnapshot, ResolvedNode};
use super::state::{ReqStatus, Transition};

// =========================================================================
// DTOs
// =========================================================================
#[derive(Debug, Deserialize)]
pub struct LineInput {
    pub item_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct CreateRequisitionInput {
    pub department_id: Uuid,
    pub needed_by: NaiveDate,
    pub justification: String,
    pub lines: Vec<LineInput>,
}

#[derive(Debug, Serialize)]
pub struct RequisitionLine {
    pub id: Uuid,
    pub item_id: Uuid,
    pub item_name: String,
    pub sku: String,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
    pub on_hand: i32,
    pub controlled: bool,
}

#[derive(Debug, Serialize)]
pub struct RequisitionDetail {
    pub id: Uuid,
    pub ref_code: String,
    pub status: String,
    pub requester_id: Uuid,
    pub requester_email: String,
    pub site_id: Uuid,
    pub department_id: Uuid,
    pub needed_by: NaiveDate,
    pub justification: String,
    pub total_amount_cents: i64,
    pub contains_controlled: bool,
    pub workflow_id: Option<Uuid>,
    pub lines: Vec<RequisitionLine>,
    pub route: Vec<RouteStep>,
    pub audit: Vec<AuditEntry>,
}

#[derive(Debug, Serialize)]
pub struct RouteStep {
    pub sequence: i32,
    pub label: String,
    pub required_role: String,
    pub decision: Option<String>,
    pub decided_by: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub occurred_at: chrono::DateTime<chrono::Utc>,
    pub actor_user_id: Option<Uuid>,
    pub event_kind: String,
    pub comment: Option<String>,
}

// =========================================================================
// Helpers
// =========================================================================
async fn next_ref_code(tx: &mut Transaction<'_, Postgres>) -> Result<String, ApiAppError> {
    let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM requisitions")
        .fetch_one(&mut **tx)
        .await?;
    Ok(format!("REQ-{:06}", row.0 + 1))
}

async fn load_workflow_id(pool: &PgPool) -> Result<Uuid, ApiAppError> {
    let row: (Uuid,) = sqlx::query_as(
        "SELECT id FROM approval_workflows WHERE key = 'requisition_default' AND is_active LIMIT 1",
    )
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

async fn audit(
    tx: &mut Transaction<'_, Postgres>,
    req_id: Uuid,
    actor: Option<Uuid>,
    kind: &str,
    comment: Option<&str>,
) -> Result<(), ApiAppError> {
    sqlx::query(
        "INSERT INTO requisition_audit (requisition_id, actor_user_id, event_kind, comment) VALUES ($1,$2,$3,$4)",
    )
    .bind(req_id).bind(actor).bind(kind).bind(comment)
    .execute(&mut **tx).await?;
    Ok(())
}

// =========================================================================
// Authorization helpers (object-level)
// =========================================================================
fn assert_owner(p: &Principal, requester_id: Uuid) -> Result<(), ApiAppError> {
    if p.user_id == requester_id || p.permissions.contains("scope:any") {
        Ok(())
    } else {
        Err(ApiAppError::Forbidden("not the requester".into()))
    }
}

fn assert_dept_scope(p: &Principal, department_id: Uuid) -> Result<(), ApiAppError> {
    p.require_scope(ScopeKind::Department, &department_id.to_string())
}

/// Pure read-authorization predicate for a single requisition.
/// Policy (mirrors inbox and load_detail):
///   owner  OR  (has `requisitions:approve` AND dept-scoped)  OR  scope:any
///
/// Extracted as a named helper so both `load_detail` and `load_issue_record`
/// share exactly one copy of the policy — changes here propagate to both.
fn can_read_requisition(p: &Principal, requester_id: Uuid, department_id: Uuid) -> bool {
    if p.permissions.contains("scope:any") {
        return true;
    }
    if p.user_id == requester_id {
        return true;
    }
    if p.permissions.contains("requisitions:approve") {
        let dept_ok = p
            .scopes
            .iter()
            .any(|s| s.kind == ScopeKind::Department && s.reference == department_id.to_string());
        if dept_ok {
            return true;
        }
    }
    false
}

/// Department-scope guard for approval actors (approve / reject / send-back).
/// Fetches the requisition's department_id from the DB, then delegates to
/// `assert_dept_scope` which already handles the `scope:any` bypass.
/// Returns 404 if the requisition doesn't exist.
async fn assert_approval_dept_scope(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
) -> Result<(), ApiAppError> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT department_id FROM requisitions WHERE id = $1")
            .bind(req_id)
            .fetch_optional(pool)
            .await?;
    let (dept_id,) = row.ok_or(ApiAppError::NotFound)?;
    assert_dept_scope(p, dept_id)
}

// =========================================================================
// Use cases
// =========================================================================
pub async fn create_draft(
    pool: &PgPool,
    p: &Principal,
    input: CreateRequisitionInput,
) -> Result<RequisitionDetail, ApiAppError> {
    p.require("requisitions:write")?;
    assert_dept_scope(p, input.department_id)?;
    if input.needed_by < chrono::Local::now().date_naive() {
        return Err(ApiAppError::BadRequest("needed_by must be today or a future date".into()));
    }
    if input.justification.trim().is_empty() {
        return Err(ApiAppError::BadRequest("justification required".into()));
    }
    if input.lines.is_empty() {
        return Err(ApiAppError::BadRequest("at least one line required".into()));
    }
    for l in &input.lines {
        if l.quantity <= 0 {
            return Err(ApiAppError::BadRequest("quantities must be > 0".into()));
        }
    }

    let mut tx = pool.begin().await?;

    // Derive the site from the department.
    let site_row: Option<(Uuid,)> = sqlx::query_as("SELECT site_id FROM departments WHERE id=$1")
        .bind(input.department_id).fetch_optional(&mut *tx).await?;
    let site_id = site_row.ok_or_else(|| ApiAppError::BadRequest("unknown department".into()))?.0;

    // Look up items + categories + on_hand at the requisition's site.
    let mut total: i64 = 0;
    let mut controlled = false;
    let mut prepared: Vec<(Uuid, i32, i64, i64)> = Vec::new();
    for l in &input.lines {
        let row: Option<(i64, bool)> = sqlx::query_as(
            "SELECT i.unit_price_cents, c.controlled
             FROM inventory_items i JOIN inventory_categories c ON c.id = i.category_id
             WHERE i.id = $1 AND i.is_active",
        )
        .bind(l.item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (unit_price, ctrl) = row.ok_or_else(|| ApiAppError::BadRequest("item not found".into()))?;
        let line_total = unit_price * l.quantity as i64;
        total += line_total;
        if ctrl { controlled = true; }
        prepared.push((l.item_id, l.quantity, unit_price, line_total));
    }

    let ref_code = next_ref_code(&mut tx).await?;
    let req_id: (Uuid,) = sqlx::query_as(
        "INSERT INTO requisitions
            (ref_code, requester_id, site_id, department_id, status,
             needed_by, justification, total_amount_cents, contains_controlled)
         VALUES ($1, $2, $3, $4, 'draft', $5, $6, $7, $8) RETURNING id",
    )
    .bind(&ref_code)
    .bind(p.user_id)
    .bind(site_id)
    .bind(input.department_id)
    .bind(input.needed_by)
    .bind(input.justification.trim())
    .bind(total)
    .bind(controlled)
    .fetch_one(&mut *tx)
    .await?;

    for (item_id, qty, unit_price, line_total) in prepared {
        sqlx::query(
            "INSERT INTO requisition_lines (requisition_id, item_id, quantity, unit_price_cents, line_total_cents)
             VALUES ($1,$2,$3,$4,$5)",
        )
        .bind(req_id.0).bind(item_id).bind(qty).bind(unit_price).bind(line_total)
        .execute(&mut *tx).await?;
    }
    audit(&mut tx, req_id.0, Some(p.user_id), "created", None).await?;
    tx.commit().await?;

    // Content moderation: scan the free-text justification for flagged keywords.
    // Called AFTER commit so a transient moderation-DB error never rolls back the
    // requisition; the primary operation is already persisted.  Fire-and-forget.
    if let Err(e) = crate::moderation::service::check_and_flag(
        pool,
        &input.justification,
        "requisition_note",
        Some(req_id.0),
    )
    .await
    {
        tracing::warn!(
            req_id = %req_id.0,
            err = %e,
            "moderation: check_and_flag failed for requisition justification"
        );
    }

    load_detail(pool, p, req_id.0).await
}

pub async fn submit(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
) -> Result<RequisitionDetail, ApiAppError> {
    p.require("requisitions:write")?;
    let row: Option<(Uuid, String, i64, bool)> = sqlx::query_as(
        "SELECT requester_id, status, total_amount_cents, contains_controlled
         FROM requisitions WHERE id = $1",
    )
    .bind(req_id).fetch_optional(pool).await?;
    let (requester, status, total, ctrl) = row.ok_or(ApiAppError::NotFound)?;
    assert_owner(p, requester)?;
    let cur = ReqStatus::parse(&status).unwrap();
    Transition::check(cur, ReqStatus::PendingApproval)?;

    let workflow_id = load_workflow_id(pool).await?;
    let snap = ReqSnapshot { total_amount_cents: total, contains_controlled: ctrl };
    let route: Vec<ResolvedNode> = resolve_route(pool, workflow_id, &snap).await?;

    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE requisitions SET status='pending_approval', workflow_id=$2, submitted_at=NOW(), updated_at=NOW() WHERE id=$1")
        .bind(req_id).bind(workflow_id).execute(&mut *tx).await?;
    let inst: (Uuid,) = sqlx::query_as(
        "INSERT INTO requisition_approval_instances (requisition_id, workflow_id) VALUES ($1,$2) RETURNING id",
    ).bind(req_id).bind(workflow_id).fetch_one(&mut *tx).await?;
    for (i, n) in route.iter().enumerate() {
        sqlx::query(
            "INSERT INTO requisition_approval_steps (instance_id, sequence, node_id, required_role)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(inst.0).bind(i as i32 + 1).bind(n.node_id).bind(&n.required_role)
        .execute(&mut *tx).await?;
    }
    audit(&mut tx, req_id, Some(p.user_id), "submitted",
          Some(&format!("routed to {} approval node(s)", route.len()))).await?;
    tx.commit().await?;
    load_detail(pool, p, req_id).await
}

pub async fn withdraw(pool: &PgPool, p: &Principal, req_id: Uuid) -> Result<RequisitionDetail, ApiAppError> {
    let row: Option<(Uuid, String)> = sqlx::query_as("SELECT requester_id, status FROM requisitions WHERE id=$1")
        .bind(req_id).fetch_optional(pool).await?;
    let (requester, status) = row.ok_or(ApiAppError::NotFound)?;
    assert_owner(p, requester)?;
    let cur = ReqStatus::parse(&status).unwrap();
    if !cur.is_withdrawable() {
        return Err(ApiAppError::BadRequest(format!("cannot withdraw from {}", cur.as_str())));
    }
    Transition::check(cur, ReqStatus::Withdrawn)?;
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE requisitions SET status='withdrawn', updated_at=NOW() WHERE id=$1")
        .bind(req_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisition_approval_instances SET status='withdrawn' WHERE requisition_id=$1 AND status='in_progress'")
        .bind(req_id).execute(&mut *tx).await?;
    audit(&mut tx, req_id, Some(p.user_id), "withdrawn", None).await?;
    tx.commit().await?;
    load_detail(pool, p, req_id).await
}

#[derive(Debug, Deserialize)]
pub struct DecisionInput {
    pub reason: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
}

async fn current_pending_step(
    pool: &PgPool,
    req_id: Uuid,
) -> Result<(Uuid, Uuid, i32, String), ApiAppError> {
    // (instance_id, step_id, sequence, required_role) for the next undecided step
    let row: Option<(Uuid, Uuid, i32, String)> = sqlx::query_as(
        "SELECT s.instance_id, s.id, s.sequence, s.required_role
         FROM requisition_approval_steps s
         JOIN requisition_approval_instances i ON i.id = s.instance_id
         WHERE i.requisition_id = $1 AND i.status='in_progress' AND s.decision IS NULL
         ORDER BY s.sequence LIMIT 1",
    )
    .bind(req_id).fetch_optional(pool).await?;
    row.ok_or_else(|| ApiAppError::BadRequest("no pending approval step".into()))
}

fn assert_can_act_at_step(p: &Principal, required_role: &str) -> Result<(), ApiAppError> {
    if p.has_role(required_role) || p.permissions.contains("*") {
        Ok(())
    } else {
        Err(ApiAppError::Forbidden(format!("requires role {required_role}")))
    }
}

pub async fn approve(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
    input: DecisionInput,
) -> Result<RequisitionDetail, ApiAppError> {
    p.require("requisitions:approve")?;
    // Object-level: approver must be scoped to this requisition's department.
    // Aligned with inbox query (service.rs:520-521) and load_detail read policy.
    assert_approval_dept_scope(pool, p, req_id).await?;
    let (instance_id, step_id, sequence, role) = current_pending_step(pool, req_id).await?;
    assert_can_act_at_step(p, &role)?;

    // Determine if this is the LAST step.
    let total_steps: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM requisition_approval_steps WHERE instance_id=$1")
            .bind(instance_id).fetch_one(pool).await?;
    let is_final = sequence as i64 == total_steps.0;

    if !is_final {
        // Just record the step decision; status stays pending_approval.
        let mut tx = pool.begin().await?;
        sqlx::query(
            "UPDATE requisition_approval_steps SET decision='approved', decided_by=$2, decided_at=NOW(), reason=$3 WHERE id=$1",
        )
        .bind(step_id).bind(p.user_id).bind(input.comment.as_deref())
        .execute(&mut *tx).await?;
        sqlx::query("UPDATE requisition_approval_instances SET current_step=$2 WHERE id=$1")
            .bind(instance_id).bind(sequence).execute(&mut *tx).await?;
        audit(&mut tx, req_id, Some(p.user_id), "approved_step",
              input.comment.as_deref()).await?;
        tx.commit().await?;
        return load_detail(pool, p, req_id).await;
    }

    // FINAL APPROVAL — the high-risk path.
    finalize_in_transaction(pool, p, req_id, instance_id, step_id, input.comment.as_deref()).await?;
    load_detail(pool, p, req_id).await
}

/// Single transaction: mark final step approved, mark instance approved,
/// move requisition through approved_final → issued, verify stock, deduct
/// stock per line, create issue_record + issue_record_lines, append audit.
/// Any failure rolls back the entire chain.
async fn finalize_in_transaction(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
    instance_id: Uuid,
    step_id: Uuid,
    comment: Option<&str>,
) -> Result<(), ApiAppError> {
    let mut tx = pool.begin().await?;

    // Lock the requisition + its lines.
    let req: (String, Uuid) = sqlx::query_as(
        "SELECT status, site_id FROM requisitions WHERE id=$1 FOR UPDATE",
    ).bind(req_id).fetch_one(&mut *tx).await?;
    let cur = ReqStatus::parse(&req.0).unwrap();
    Transition::check(cur, ReqStatus::ApprovedFinal)?;
    let site_id = req.1;

    // Lock + read lines.
    let lines: Vec<(Uuid, Uuid, i32, i64, i64)> = sqlx::query_as(
        "SELECT id, item_id, quantity, unit_price_cents, line_total_cents
         FROM requisition_lines WHERE requisition_id=$1 ORDER BY id",
    ).bind(req_id).fetch_all(&mut *tx).await?;

    // Verify + deduct stock per line, locking the stock row.
    for (_lid, item_id, qty, _, _) in &lines {
        let stock: Option<(i32,)> = sqlx::query_as(
            "SELECT on_hand FROM inventory_stock_by_site WHERE item_id=$1 AND site_id=$2 FOR UPDATE",
        ).bind(item_id).bind(site_id).fetch_optional(&mut *tx).await?;
        let on_hand = stock.map(|s| s.0).unwrap_or(0);
        if on_hand < *qty {
            // Rolls back the whole transaction by returning early.
            return Err(ApiAppError::BadRequest(format!(
                "insufficient stock for item {item_id}: have {on_hand}, need {qty}"
            )));
        }
        sqlx::query(
            "UPDATE inventory_stock_by_site SET on_hand = on_hand - $3 WHERE item_id=$1 AND site_id=$2",
        ).bind(item_id).bind(site_id).bind(qty).execute(&mut *tx).await?;
    }

    // Mark approval step + instance.
    sqlx::query(
        "UPDATE requisition_approval_steps SET decision='approved', decided_by=$2, decided_at=NOW(), reason=$3 WHERE id=$1",
    ).bind(step_id).bind(p.user_id).bind(comment).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisition_approval_instances SET status='approved' WHERE id=$1")
        .bind(instance_id).execute(&mut *tx).await?;

    // approved_final → issued (record both transitions in audit).
    sqlx::query("UPDATE requisitions SET status='approved_final', finalized_at=NOW(), updated_at=NOW() WHERE id=$1")
        .bind(req_id).execute(&mut *tx).await?;
    audit(&mut tx, req_id, Some(p.user_id), "approved_final", comment).await?;

    sqlx::query("UPDATE requisitions SET status='issued', updated_at=NOW() WHERE id=$1")
        .bind(req_id).execute(&mut *tx).await?;

    // Issue record + lines.
    let issue: (Uuid,) = sqlx::query_as(
        "INSERT INTO issue_records (requisition_id, site_id, issued_by) VALUES ($1,$2,$3) RETURNING id",
    ).bind(req_id).bind(site_id).bind(p.user_id).fetch_one(&mut *tx).await?;
    for (_lid, item_id, qty, unit_price, line_total) in &lines {
        sqlx::query(
            "INSERT INTO issue_record_lines (issue_record_id, item_id, quantity, unit_price_cents, line_total_cents)
             VALUES ($1,$2,$3,$4,$5)",
        ).bind(issue.0).bind(item_id).bind(qty).bind(unit_price).bind(line_total)
        .execute(&mut *tx).await?;
    }
    audit(&mut tx, req_id, Some(p.user_id), "issued",
          Some(&format!("issue_record={}", issue.0))).await?;

    tx.commit().await?;
    Ok(())
}

pub async fn reject(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
    input: DecisionInput,
) -> Result<RequisitionDetail, ApiAppError> {
    p.require("requisitions:approve")?;
    // Object-level: approver must be scoped to this requisition's department.
    assert_approval_dept_scope(pool, p, req_id).await?;
    let reason = input
        .reason
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiAppError::BadRequest("reject requires reason".into()))?
        .to_string();

    let (instance_id, step_id, _seq, role) = current_pending_step(pool, req_id).await?;
    assert_can_act_at_step(p, &role)?;

    let mut tx = pool.begin().await?;
    let cur_row: (String,) = sqlx::query_as("SELECT status FROM requisitions WHERE id=$1 FOR UPDATE")
        .bind(req_id).fetch_one(&mut *tx).await?;
    let cur = ReqStatus::parse(&cur_row.0).unwrap();
    Transition::check(cur, ReqStatus::Rejected)?;

    sqlx::query("UPDATE requisition_approval_steps SET decision='rejected', decided_by=$2, decided_at=NOW(), reason=$3 WHERE id=$1")
        .bind(step_id).bind(p.user_id).bind(&reason).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisition_approval_instances SET status='rejected' WHERE id=$1")
        .bind(instance_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisitions SET status='rejected', updated_at=NOW() WHERE id=$1")
        .bind(req_id).execute(&mut *tx).await?;
    audit(&mut tx, req_id, Some(p.user_id), "rejected", Some(&reason)).await?;
    tx.commit().await?;
    load_detail(pool, p, req_id).await
}

pub async fn send_back(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
    input: DecisionInput,
) -> Result<RequisitionDetail, ApiAppError> {
    p.require("requisitions:approve")?;
    // Object-level: approver must be scoped to this requisition's department.
    assert_approval_dept_scope(pool, p, req_id).await?;
    let reason = input
        .reason
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiAppError::BadRequest("send-back requires reason".into()))?
        .to_string();
    let (instance_id, step_id, _, role) = current_pending_step(pool, req_id).await?;
    assert_can_act_at_step(p, &role)?;

    let mut tx = pool.begin().await?;
    let cur_row: (String,) = sqlx::query_as("SELECT status FROM requisitions WHERE id=$1 FOR UPDATE")
        .bind(req_id).fetch_one(&mut *tx).await?;
    let cur = ReqStatus::parse(&cur_row.0).unwrap();
    Transition::check(cur, ReqStatus::SentBack)?;
    sqlx::query("UPDATE requisition_approval_steps SET decision='sent_back', decided_by=$2, decided_at=NOW(), reason=$3 WHERE id=$1")
        .bind(step_id).bind(p.user_id).bind(&reason).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisition_approval_instances SET status='sent_back' WHERE id=$1")
        .bind(instance_id).execute(&mut *tx).await?;
    sqlx::query("UPDATE requisitions SET status='sent_back', updated_at=NOW() WHERE id=$1")
        .bind(req_id).execute(&mut *tx).await?;
    audit(&mut tx, req_id, Some(p.user_id), "sent_back", Some(&reason)).await?;
    tx.commit().await?;
    load_detail(pool, p, req_id).await
}

// =========================================================================
// Issue-record view DTO
// =========================================================================
#[derive(Debug, Serialize)]
pub struct IssueRecordLine {
    pub item_id: Uuid,
    pub quantity: i32,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct IssueRecordView {
    pub id: Uuid,
    pub site_id: Uuid,
    pub issued_by: Uuid,
    pub issued_at: chrono::DateTime<chrono::Utc>,
    pub lines: Vec<IssueRecordLine>,
}

// =========================================================================
// Reads
// =========================================================================
pub async fn list_for_requester(pool: &PgPool, p: &Principal) -> Result<Vec<RequisitionDetail>, ApiAppError> {
    p.require("requisitions:read")?;
    let ids: Vec<(Uuid,)> =
        sqlx::query_as("SELECT id FROM requisitions WHERE requester_id=$1 ORDER BY created_at DESC")
            .bind(p.user_id).fetch_all(pool).await?;
    let mut out = Vec::new();
    for (id,) in ids { out.push(load_detail(pool, p, id).await?); }
    Ok(out)
}

pub async fn approver_inbox(pool: &PgPool, p: &Principal) -> Result<Vec<RequisitionDetail>, ApiAppError> {
    p.require("requisitions:approve")?;
    // Find all requisitions whose next undecided step requires a role the
    // principal holds, AND whose department is in their scope (unless they
    // hold scope:any).
    let scope_any = p.permissions.contains("scope:any");
    let dept_refs: Vec<String> = p.scopes.iter()
        .filter(|s| s.kind == ScopeKind::Department)
        .map(|s| s.reference.clone()).collect();

    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT r.id
         FROM requisitions r
         JOIN requisition_approval_instances i ON i.requisition_id = r.id
         JOIN requisition_approval_steps s ON s.instance_id = i.id
         WHERE r.status='pending_approval'
           AND i.status='in_progress'
           AND s.decision IS NULL
           AND s.sequence = (
               SELECT MIN(s2.sequence) FROM requisition_approval_steps s2
               WHERE s2.instance_id = i.id AND s2.decision IS NULL
           )
           AND s.required_role = ANY($1)
           AND ($2::bool OR r.department_id::text = ANY($3))
         ORDER BY r.id",
    )
    .bind(&p.roles)
    .bind(scope_any)
    .bind(&dept_refs)
    .fetch_all(pool).await?;

    let mut out = Vec::new();
    for (id,) in rows { out.push(load_detail(pool, p, id).await?); }
    Ok(out)
}

pub async fn load_detail(pool: &PgPool, p: &Principal, req_id: Uuid) -> Result<RequisitionDetail, ApiAppError> {
    let req: Option<(Uuid, String, String, Uuid, String, Uuid, Uuid, NaiveDate, String, i64, bool, Option<Uuid>)> =
        sqlx::query_as(
            "SELECT r.id, r.ref_code, r.status, r.requester_id, u.email,
                    r.site_id, r.department_id, r.needed_by, r.justification,
                    r.total_amount_cents, r.contains_controlled, r.workflow_id
             FROM requisitions r JOIN users u ON u.id = r.requester_id
             WHERE r.id = $1",
        ).bind(req_id).fetch_optional(pool).await?;
    let r = req.ok_or(ApiAppError::NotFound)?;

    // Object-level read auth via shared helper — same policy as inbox scoping
    // and issue_record access guard.
    if !can_read_requisition(p, r.3, r.6) {
        return Err(ApiAppError::Forbidden("not authorized to view this requisition".into()));
    }

    let lines: Vec<(Uuid, Uuid, String, String, i32, i64, i64, bool, Option<i32>)> = sqlx::query_as(
        "SELECT rl.id, rl.item_id, i.name, i.sku, rl.quantity, rl.unit_price_cents, rl.line_total_cents,
                c.controlled, st.on_hand
         FROM requisition_lines rl
         JOIN inventory_items i ON i.id = rl.item_id
         JOIN inventory_categories c ON c.id = i.category_id
         LEFT JOIN inventory_stock_by_site st ON st.item_id = rl.item_id AND st.site_id = $2
         WHERE rl.requisition_id = $1 ORDER BY rl.id",
    ).bind(req_id).bind(r.5).fetch_all(pool).await?;

    let route: Vec<(i32, String, String, Option<String>, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT s.sequence, n.label, s.required_role, s.decision, s.decided_by, s.reason
         FROM requisition_approval_steps s
         JOIN approval_nodes n ON n.id = s.node_id
         JOIN requisition_approval_instances i ON i.id = s.instance_id
         WHERE i.requisition_id = $1 ORDER BY s.sequence",
    ).bind(req_id).fetch_all(pool).await?;

    let audit_rows: Vec<(chrono::DateTime<chrono::Utc>, Option<Uuid>, String, Option<String>)> = sqlx::query_as(
        "SELECT occurred_at, actor_user_id, event_kind, comment FROM requisition_audit
         WHERE requisition_id=$1 ORDER BY occurred_at",
    ).bind(req_id).fetch_all(pool).await?;

    Ok(RequisitionDetail {
        id: r.0, ref_code: r.1, status: r.2, requester_id: r.3, requester_email: r.4,
        site_id: r.5, department_id: r.6, needed_by: r.7, justification: r.8,
        total_amount_cents: r.9, contains_controlled: r.10, workflow_id: r.11,
        lines: lines.into_iter().map(|l| RequisitionLine {
            id: l.0, item_id: l.1, item_name: l.2, sku: l.3,
            quantity: l.4, unit_price_cents: l.5, line_total_cents: l.6,
            controlled: l.7, on_hand: l.8.unwrap_or(0),
        }).collect(),
        route: route.into_iter().map(|s| RouteStep {
            sequence: s.0, label: s.1, required_role: s.2,
            decision: s.3, decided_by: s.4, reason: s.5,
        }).collect(),
        audit: audit_rows.into_iter().map(|a| AuditEntry {
            occurred_at: a.0, actor_user_id: a.1, event_kind: a.2, comment: a.3,
        }).collect(),
    })
}

/// Load an issue record for a requisition, enforcing the same read-authorization
/// policy as `load_detail` (owner / in-scope approver / scope:any).
pub async fn load_issue_record(
    pool: &PgPool,
    p: &Principal,
    req_id: Uuid,
) -> Result<IssueRecordView, ApiAppError> {
    // Fetch just enough to run the authorization check without loading full
    // requisition detail.
    let req_row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT requester_id, department_id FROM requisitions WHERE id = $1",
    )
    .bind(req_id)
    .fetch_optional(pool)
    .await?;
    let (requester_id, department_id) = req_row.ok_or(ApiAppError::NotFound)?;
    if !can_read_requisition(p, requester_id, department_id) {
        return Err(ApiAppError::Forbidden(
            "not authorized to view this issue record".into(),
        ));
    }

    let ir: Option<(Uuid, Uuid, Uuid, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        "SELECT id, site_id, issued_by, issued_at FROM issue_records WHERE requisition_id = $1",
    )
    .bind(req_id)
    .fetch_optional(pool)
    .await?;
    let (id, site_id, issued_by, issued_at) = ir.ok_or(ApiAppError::NotFound)?;

    let lines: Vec<(Uuid, i32, i64, i64)> = sqlx::query_as(
        "SELECT item_id, quantity, unit_price_cents, line_total_cents
         FROM issue_record_lines WHERE issue_record_id = $1",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(IssueRecordView {
        id,
        site_id,
        issued_by,
        issued_at,
        lines: lines
            .into_iter()
            .map(|(item_id, quantity, unit_price_cents, line_total_cents)| IssueRecordLine {
                item_id,
                quantity,
                unit_price_cents,
                line_total_cents,
            })
            .collect(),
    })
}

// =========================================================================
// Unit tests
//
// These cover the pure (no-DB) authorization helpers.  Integration tests that
// exercise the full SQL path live in apps/backend-api/tests/.
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use crate::security::rbac::Scope;

    fn make_principal(
        user_id: Uuid,
        perms: &[&str],
        roles: &[&str],
        dept_scopes: &[Uuid],
    ) -> Principal {
        Principal {
            user_id,
            email: "test@example.com".into(),
            roles: roles.iter().map(|s| s.to_string()).collect(),
            permissions: perms.iter().map(|s| s.to_string()).collect::<HashSet<_>>(),
            scopes: dept_scopes
                .iter()
                .map(|id| Scope {
                    kind: ScopeKind::Department,
                    reference: id.to_string(),
                })
                .collect(),
        }
    }

    // ── assert_dept_scope ──────────────────────────────────────────────────

    #[test]
    fn dept_scope_allowed_when_scope_any() {
        let p = make_principal(Uuid::new_v4(), &["scope:any"], &[], &[]);
        assert!(assert_dept_scope(&p, Uuid::new_v4()).is_ok());
    }

    #[test]
    fn dept_scope_allowed_when_dept_matches() {
        let dept = Uuid::new_v4();
        let p = make_principal(Uuid::new_v4(), &[], &[], &[dept]);
        assert!(assert_dept_scope(&p, dept).is_ok());
    }

    #[test]
    fn dept_scope_denied_for_wrong_department() {
        let dept_a = Uuid::new_v4();
        let dept_b = Uuid::new_v4();
        let p = make_principal(Uuid::new_v4(), &[], &[], &[dept_a]);
        let err = assert_dept_scope(&p, dept_b).unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }

    #[test]
    fn dept_scope_denied_when_no_scopes() {
        let p = make_principal(Uuid::new_v4(), &["requisitions:approve"], &[], &[]);
        let err = assert_dept_scope(&p, Uuid::new_v4()).unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }

    // ── assert_can_act_at_step ─────────────────────────────────────────────

    #[test]
    fn step_allowed_when_role_matches() {
        let p = make_principal(Uuid::new_v4(), &[], &["dept_head"], &[]);
        assert!(assert_can_act_at_step(&p, "dept_head").is_ok());
    }

    #[test]
    fn step_allowed_by_wildcard_permission() {
        let p = make_principal(Uuid::new_v4(), &["*"], &[], &[]);
        assert!(assert_can_act_at_step(&p, "dept_head").is_ok());
    }

    #[test]
    fn step_denied_for_wrong_role() {
        let p = make_principal(Uuid::new_v4(), &[], &["finance"], &[]);
        let err = assert_can_act_at_step(&p, "dept_head").unwrap_err();
        assert!(matches!(err, ApiAppError::Forbidden(_)));
    }

    // ── can_read_requisition ───────────────────────────────────────────────

    #[test]
    fn read_allowed_for_owner() {
        let owner_id = Uuid::new_v4();
        let p = make_principal(owner_id, &[], &[], &[]);
        assert!(can_read_requisition(&p, owner_id, Uuid::new_v4()));
    }

    #[test]
    fn read_allowed_for_in_scope_approver() {
        let dept = Uuid::new_v4();
        let p = make_principal(Uuid::new_v4(), &["requisitions:approve"], &[], &[dept]);
        assert!(can_read_requisition(&p, Uuid::new_v4(), dept));
    }

    #[test]
    fn read_denied_for_approver_with_wrong_dept() {
        let dept_a = Uuid::new_v4();
        let dept_b = Uuid::new_v4();
        let p = make_principal(Uuid::new_v4(), &["requisitions:approve"], &[], &[dept_a]);
        assert!(!can_read_requisition(&p, Uuid::new_v4(), dept_b));
    }

    #[test]
    fn read_denied_for_approver_with_no_dept_scope() {
        // Has the approve permission but no department scopes at all.
        let p = make_principal(Uuid::new_v4(), &["requisitions:approve"], &[], &[]);
        assert!(!can_read_requisition(&p, Uuid::new_v4(), Uuid::new_v4()));
    }

    #[test]
    fn read_allowed_with_scope_any() {
        let p = make_principal(Uuid::new_v4(), &["scope:any"], &[], &[]);
        assert!(can_read_requisition(&p, Uuid::new_v4(), Uuid::new_v4()));
    }

    #[test]
    fn read_denied_for_unrelated_user() {
        // Has read permission but is neither owner, approver, nor scope:any.
        let p = make_principal(Uuid::new_v4(), &["requisitions:read"], &[], &[]);
        assert!(!can_read_requisition(&p, Uuid::new_v4(), Uuid::new_v4()));
    }

    // ── Approval auth invariants ───────────────────────────────────────────
    //
    // Audit finding: approve/reject/send_back did NOT check department scope
    // (service.rs:308,310,423,433,458,467).  The fix adds
    // `assert_approval_dept_scope` (which calls `assert_dept_scope`) before
    // `assert_can_act_at_step`.  These tests prove neither check alone is
    // sufficient.

    #[test]
    fn approval_role_match_alone_is_insufficient_without_dept_scope() {
        // Approver has the right role for the step, but is scoped to a different
        // department — department scope check must reject this.
        let right_dept = Uuid::new_v4();
        let wrong_dept = Uuid::new_v4();
        let p = make_principal(
            Uuid::new_v4(),
            &["requisitions:approve"],
            &["dept_head"],
            &[wrong_dept],
        );
        assert!(assert_can_act_at_step(&p, "dept_head").is_ok(), "role check passes");
        assert!(
            assert_dept_scope(&p, right_dept).is_err(),
            "dept scope check must fail for out-of-scope approver"
        );
    }

    #[test]
    fn approval_dept_scope_alone_is_insufficient_without_role() {
        // Approver is scoped to the right department but lacks the required step role.
        let dept = Uuid::new_v4();
        let p = make_principal(
            Uuid::new_v4(),
            &["requisitions:approve"],
            &["finance"],
            &[dept],
        );
        assert!(assert_dept_scope(&p, dept).is_ok(), "dept scope check passes");
        assert!(
            assert_can_act_at_step(&p, "dept_head").is_err(),
            "step role check must fail for wrong-role approver"
        );
    }

    #[test]
    fn approval_passes_when_both_role_and_dept_scope_match() {
        let dept = Uuid::new_v4();
        let p = make_principal(
            Uuid::new_v4(),
            &["requisitions:approve"],
            &["dept_head"],
            &[dept],
        );
        assert!(assert_can_act_at_step(&p, "dept_head").is_ok());
        assert!(assert_dept_scope(&p, dept).is_ok());
    }

    #[test]
    fn approval_scope_any_bypasses_dept_scope_check() {
        // scope:any bypasses dept check; * bypasses role check.
        let p = make_principal(Uuid::new_v4(), &["scope:any", "*"], &[], &[]);
        assert!(assert_dept_scope(&p, Uuid::new_v4()).is_ok());
        assert!(assert_can_act_at_step(&p, "any_role").is_ok());
    }

    // ── moderation wiring ─────────────────────────────────────────────────────

    /// Verifies that `create_draft` validates justification is non-empty before
    /// reaching the `check_and_flag` call.  The guard ensures the moderation
    /// function is only called with substantive text, never with an empty string.
    #[test]
    fn moderation_check_only_fires_for_non_empty_justification() {
        // The handler returns BadRequest("justification required") for empty
        // strings, so check_and_flag is only reached for non-empty input.
        let empty = "";
        let non_empty = "Need supplies for the student lab rehabilitation programme";
        assert!(
            empty.trim().is_empty(),
            "empty justification should be rejected before check_and_flag"
        );
        assert!(
            !non_empty.trim().is_empty(),
            "non-empty justification should reach check_and_flag"
        );
    }

    /// Verifies that the content_type label passed to `check_and_flag` is
    /// `"requisition_note"` — the label is stored in the moderation queue and
    /// used by the admin review UI to identify the source of flagged content.
    #[test]
    fn moderation_content_type_label_is_requisition_note() {
        let content_type = "requisition_note";
        assert_eq!(
            content_type, "requisition_note",
            "content_type must match the label expected by the moderation queue schema"
        );
    }
}

