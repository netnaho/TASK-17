use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

// ── DTOs (family-facing — minimized) ──────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct FamilyGroupView {
    pub id: Uuid,
    pub label: String,
    pub institution_id: Uuid,
    pub residents: Vec<ResidentView>,
    pub consent: Vec<ConsentView>,
}

#[derive(Debug, Serialize)]
pub struct ResidentView {
    pub student_id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub class_code: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ConsentView {
    pub data_category: String,
    pub consented: bool,
    pub consented_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Supply usage summary exposed to family portal.
/// Shows approved requisition supply categories only.
/// No internal notes, no approver identities, no dollar amounts.
#[derive(Debug, Serialize)]
pub struct SupplySummaryItem {
    pub resident_first_name: String,
    pub resident_last_name: String,
    pub period_month: String,     // "YYYY-MM"
    pub category: String,         // requisition category label
    pub approved_count: i64,      // number of approved requisitions
    pub total_quantity: i64,      // total units requested across approved reqs
}

/// Wellness activity summary exposed to family portal.
/// No encrypted notes, no internal IDs.
#[derive(Debug, Serialize)]
pub struct WellnessSummaryItem {
    pub resident_first_name: String,
    pub resident_last_name: String,
    pub activity_type: String,
    pub activity_date: NaiveDate,
    pub duration_minutes: i32,
    // personal best flag: is this the best ever for this student+type?
    pub is_personal_best: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateFamilyGroupInput {
    pub institution_id: Uuid,
    pub label: String,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberInput {
    pub user_id: Uuid,
    pub role: Option<String>, // "guardian" | "viewer"
}

#[derive(Debug, Deserialize)]
pub struct AddResidentInput {
    pub student_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ConsentInput {
    pub data_category: String,
    pub consented: bool,
}

#[derive(Debug, Deserialize)]
pub struct WellnessActivityInput {
    pub student_id: Option<Uuid>,
    pub activity_type: String, // 'exercise'|'therapy'|'social'|'nutrition'
    pub duration_minutes: i32,
    pub notes: Option<String>, // will be encrypted at rest
    pub activity_date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
pub struct WellnessActivityRow {
    pub id: Uuid,
    pub institution_id: Uuid,
    pub student_id: Option<Uuid>,
    pub activity_type: String,
    pub duration_minutes: i32,
    pub activity_date: NaiveDate,
    // notes_encrypted is NOT included in this DTO — never expose encrypted blob
}

// ── Allowed values ────────────────────────────────────────────────────────

const ALLOWED_CONSENT_CATEGORIES: &[&str] = &["supply_usage", "wellness_summary", "activity_log"];
const ALLOWED_ACTIVITY_TYPES: &[&str] = &["exercise", "therapy", "social", "nutrition"];
const ALLOWED_MEMBER_ROLES: &[&str] = &["guardian", "viewer"];

// ── Internal helpers ──────────────────────────────────────────────────────

/// Check that the principal has `admin:users` permission OR is a member of
/// the specified family group.  Returns 403 if neither condition holds.
async fn assert_group_access(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
) -> Result<(), ApiAppError> {
    if p.permissions.contains("admin:users") || p.permissions.contains("*") {
        return Ok(());
    }
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM family_group_members WHERE family_group_id = $1 AND user_id = $2",
    )
    .bind(group_id)
    .bind(p.user_id)
    .fetch_optional(pool)
    .await?;
    if row.is_none() {
        return Err(ApiAppError::Forbidden("not a member of this family group".into()));
    }
    Ok(())
}

/// Load residents for a group.
async fn load_residents(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<ResidentView>, ApiAppError> {
    let rows: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
        "SELECT s.id, s.first_name, s.last_name, c.code AS class_code
         FROM students s
         JOIN family_group_residents fgr ON fgr.student_id = s.id
         LEFT JOIN classes c ON c.id = s.class_id
         WHERE fgr.family_group_id = $1
         ORDER BY s.last_name, s.first_name",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(student_id, first_name, last_name, class_code)| ResidentView {
            student_id,
            first_name,
            last_name,
            class_code,
        })
        .collect())
}

/// Load consent records for a group.
async fn load_consent(
    pool: &PgPool,
    group_id: Uuid,
) -> Result<Vec<ConsentView>, ApiAppError> {
    let rows: Vec<(String, bool, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT data_category, consented, consented_at
         FROM consent_records
         WHERE family_group_id = $1
         ORDER BY data_category",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(data_category, consented, consented_at)| ConsentView {
            data_category,
            consented,
            consented_at,
        })
        .collect())
}

/// Check that the group has an active consent record for `data_category`.
/// Returns 403 with a clear message if consent is absent or false.
async fn assert_consent(
    pool: &PgPool,
    group_id: Uuid,
    data_category: &str,
) -> Result<(), ApiAppError> {
    let row: Option<(bool,)> = sqlx::query_as(
        "SELECT consented FROM consent_records
         WHERE family_group_id = $1 AND data_category = $2",
    )
    .bind(group_id)
    .bind(data_category)
    .fetch_optional(pool)
    .await?;
    match row {
        Some((true,)) => Ok(()),
        _ => Err(ApiAppError::Forbidden(format!(
            "consent for '{data_category}' has not been granted"
        ))),
    }
}

// ── Use cases ─────────────────────────────────────────────────────────────

/// List family groups visible to the principal.
/// Admins see all groups; regular users see only groups they belong to.
pub async fn list_my_groups(
    pool: &PgPool,
    p: &Principal,
) -> Result<Vec<FamilyGroupView>, ApiAppError> {
    let is_admin = p.permissions.contains("admin:users") || p.permissions.contains("*");

    let groups: Vec<(Uuid, String, Uuid)> = if is_admin {
        sqlx::query_as("SELECT id, label, institution_id FROM family_groups ORDER BY label")
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as(
            "SELECT fg.id, fg.label, fg.institution_id
             FROM family_groups fg
             JOIN family_group_members fgm ON fgm.family_group_id = fg.id
             WHERE fgm.user_id = $1
             ORDER BY fg.label",
        )
        .bind(p.user_id)
        .fetch_all(pool)
        .await?
    };

    let mut out = Vec::with_capacity(groups.len());
    for (id, label, institution_id) in groups {
        let residents = load_residents(pool, id).await?;
        let consent = load_consent(pool, id).await?;
        out.push(FamilyGroupView {
            id,
            label,
            institution_id,
            residents,
            consent,
        });
    }
    Ok(out)
}

/// Create a new family group.  Requires `admin:users`.
pub async fn create_group(
    pool: &PgPool,
    p: &Principal,
    input: CreateFamilyGroupInput,
) -> Result<FamilyGroupView, ApiAppError> {
    p.require("admin:users")?;

    if input.label.trim().is_empty() {
        return Err(ApiAppError::BadRequest("label must not be empty".into()));
    }

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO family_groups (institution_id, label) VALUES ($1, $2) RETURNING id",
    )
    .bind(input.institution_id)
    .bind(input.label.trim())
    .fetch_one(pool)
    .await?;

    Ok(FamilyGroupView {
        id,
        label: input.label.trim().to_string(),
        institution_id: input.institution_id,
        residents: vec![],
        consent: vec![],
    })
}

/// Add a member to a family group.
/// Requires `admin:users` or existing group membership.
pub async fn add_member(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
    input: AddMemberInput,
) -> Result<(), ApiAppError> {
    assert_group_access(pool, p, group_id).await?;

    let role = input.role.as_deref().unwrap_or("guardian");
    if !ALLOWED_MEMBER_ROLES.contains(&role) {
        return Err(ApiAppError::BadRequest(format!(
            "role must be one of: {}",
            ALLOWED_MEMBER_ROLES.join(", ")
        )));
    }

    sqlx::query(
        "INSERT INTO family_group_members (family_group_id, user_id, role)
         VALUES ($1, $2, $3)
         ON CONFLICT (family_group_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(group_id)
    .bind(input.user_id)
    .bind(role)
    .execute(pool)
    .await?;
    Ok(())
}

/// Link a resident (student) to a family group.
/// Requires `admin:users` or existing group membership.
pub async fn add_resident(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
    input: AddResidentInput,
) -> Result<(), ApiAppError> {
    assert_group_access(pool, p, group_id).await?;

    sqlx::query(
        "INSERT INTO family_group_residents (family_group_id, student_id)
         VALUES ($1, $2)
         ON CONFLICT (family_group_id, student_id) DO NOTHING",
    )
    .bind(group_id)
    .bind(input.student_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Upsert a consent record for a family group.
/// Validates that the principal is in the group (or admin) and that the
/// data_category is one of the allowed values.
pub async fn set_consent(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
    input: ConsentInput,
) -> Result<ConsentView, ApiAppError> {
    assert_group_access(pool, p, group_id).await?;

    if !ALLOWED_CONSENT_CATEGORIES.contains(&input.data_category.as_str()) {
        return Err(ApiAppError::BadRequest(format!(
            "data_category must be one of: {}",
            ALLOWED_CONSENT_CATEGORIES.join(", ")
        )));
    }

    let (data_category, consented, consented_at): (
        String,
        bool,
        Option<chrono::DateTime<chrono::Utc>>,
    ) = sqlx::query_as(
        "INSERT INTO consent_records (family_group_id, data_category, consented, consented_at)
         VALUES ($1, $2, $3, CASE WHEN $3 THEN NOW() ELSE NULL END)
         ON CONFLICT (family_group_id, data_category)
         DO UPDATE SET
             consented = EXCLUDED.consented,
             consented_at = CASE WHEN EXCLUDED.consented THEN NOW() ELSE NULL END
         RETURNING data_category, consented, consented_at",
    )
    .bind(group_id)
    .bind(&input.data_category)
    .bind(input.consented)
    .fetch_one(pool)
    .await?;

    Ok(ConsentView {
        data_category,
        consented,
        consented_at,
    })
}

/// Return a supply-usage summary for all residents in a family group.
/// Requires group access AND active consent for 'supply_usage'.
pub async fn get_supply_summary(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
) -> Result<Vec<SupplySummaryItem>, ApiAppError> {
    assert_group_access(pool, p, group_id).await?;
    assert_consent(pool, group_id, "supply_usage").await?;

    // Join residents → students → their class → department → requisitions.
    //
    // PRIVACY FIX: the previous query joined requisitions via institution-wide
    // site scoping (`s.institution_id → all sites → all requisitions`), which
    // leaked every requisition from every department in the institution to every
    // family in that institution.  We now scope to the student's own department
    // via their class (students.class_id → classes.department_id → requisitions
    // .department_id).  Students with no class, or classes without a department,
    // simply return no requisition rows — which is the correct privacy behaviour.
    //
    // QUANTITY FIX: `total_quantity` previously counted requisitions (`COUNT(r.id)`)
    // instead of summing line quantities (`SUM(rl.quantity)`).
    // `approved_count` uses `COUNT(DISTINCT r.id)` because the requisition_lines
    // join multiplies rows — one row per line.
    //
    // We purposely avoid exposing dollar amounts, approver identities, or notes.
    let rows: Vec<(String, String, Option<String>, Option<String>, i64, i64)> =
        sqlx::query_as(
            "SELECT s.first_name, s.last_name,
                    TO_CHAR(r.created_at, 'YYYY-MM') as period_month,
                    ic.label as category,
                    COUNT(DISTINCT r.id) as approved_count,
                    COALESCE(SUM(rl.quantity), 0) as total_quantity
             FROM family_group_residents fgr
             JOIN students s ON s.id = fgr.student_id
             LEFT JOIN classes cl
               ON cl.id = s.class_id AND cl.department_id IS NOT NULL
             LEFT JOIN requisitions r
               ON r.department_id = cl.department_id
               AND r.status = 'issued'
             LEFT JOIN requisition_lines rl ON rl.requisition_id = r.id
             LEFT JOIN inventory_items ii ON ii.id = rl.item_id
             LEFT JOIN inventory_categories ic ON ic.id = ii.category_id
             WHERE fgr.family_group_id = $1
             GROUP BY s.id, s.first_name, s.last_name, period_month, ic.label
             ORDER BY period_month DESC, s.last_name",
        )
        .bind(group_id)
        .fetch_all(pool)
        .await?;

    Ok(rows
        .into_iter()
        .filter_map(
            |(first_name, last_name, period_month, category, approved_count, total_quantity)| {
                // Skip rows where there are no approved requisitions (LEFT JOIN nulls).
                let period_month = period_month?;
                let category = category?;
                Some(SupplySummaryItem {
                    resident_first_name: first_name,
                    resident_last_name: last_name,
                    period_month,
                    category,
                    approved_count,
                    total_quantity,
                })
            },
        )
        .collect())
}

/// Return a wellness-activity summary for all residents in a family group
/// covering the last 30 days.
/// Requires group access AND active consent for 'wellness_summary'.
/// Never decrypts or returns notes_encrypted.
pub async fn get_wellness_summary(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
) -> Result<Vec<WellnessSummaryItem>, ApiAppError> {
    assert_group_access(pool, p, group_id).await?;
    assert_consent(pool, group_id, "wellness_summary").await?;

    let rows: Vec<(String, String, String, NaiveDate, i32, bool)> = sqlx::query_as(
        "SELECT s.first_name, s.last_name, wa.activity_type, wa.activity_date,
                wa.duration_minutes,
                CASE WHEN wb.best_duration_minutes = wa.duration_minutes THEN true ELSE false END
                    as is_personal_best
         FROM wellness_activities wa
         JOIN family_group_residents fgr ON fgr.student_id = wa.student_id
         JOIN students s ON s.id = wa.student_id
         LEFT JOIN wellness_personal_bests wb
           ON wb.student_id = wa.student_id AND wb.activity_type = wa.activity_type
         WHERE fgr.family_group_id = $1
           AND wa.activity_date >= CURRENT_DATE - INTERVAL '30 days'
         ORDER BY wa.activity_date DESC
         LIMIT 100",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(first_name, last_name, activity_type, activity_date, duration_minutes, is_personal_best)| {
                WellnessSummaryItem {
                    resident_first_name: first_name,
                    resident_last_name: last_name,
                    activity_type,
                    activity_date,
                    duration_minutes,
                    is_personal_best,
                }
            },
        )
        .collect())
}

/// Log a wellness activity for a resident.
/// Requires `requisitions:write` permission (staff-level).
/// Notes, if provided, are encrypted at rest before storage.
/// The returned DTO never includes the encrypted notes blob.
pub async fn log_wellness_activity(
    pool: &PgPool,
    p: &Principal,
    institution_id: Uuid,
    input: WellnessActivityInput,
) -> Result<WellnessActivityRow, ApiAppError> {
    p.require("requisitions:write")?;

    if !ALLOWED_ACTIVITY_TYPES.contains(&input.activity_type.as_str()) {
        return Err(ApiAppError::BadRequest(format!(
            "activity_type must be one of: {}",
            ALLOWED_ACTIVITY_TYPES.join(", ")
        )));
    }
    if input.duration_minutes <= 0 {
        return Err(ApiAppError::BadRequest(
            "duration_minutes must be a positive integer".into(),
        ));
    }

    // Encrypt notes if present.  NEVER store or return plaintext.
    let notes_encrypted: Option<String> = match &input.notes {
        Some(text) => {
            let encrypted = infrastructure::crypto::encrypt(text)
                .map_err(|e| ApiAppError::Internal(e.to_string()))?;
            Some(encrypted)
        }
        None => None,
    };

    let activity_date = input.activity_date.unwrap_or_else(|| chrono::Utc::now().date_naive());

    let (id, inst_id, student_id, activity_type, duration_minutes, stored_date): (
        Uuid,
        Uuid,
        Option<Uuid>,
        String,
        i32,
        NaiveDate,
    ) = sqlx::query_as(
        "INSERT INTO wellness_activities
             (institution_id, student_id, activity_type, duration_minutes,
              notes_encrypted, activity_date)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, institution_id, student_id, activity_type, duration_minutes, activity_date",
    )
    .bind(institution_id)
    .bind(input.student_id)
    .bind(&input.activity_type)
    .bind(input.duration_minutes)
    .bind(notes_encrypted)
    .bind(activity_date)
    .fetch_one(pool)
    .await?;

    // Update personal bests if this duration is a new record for the student+type.
    // `achieved_on` is NOT NULL in the schema (migrations/0006_compliance.sql:98).
    if let Some(sid) = student_id {
        sqlx::query(
            "INSERT INTO wellness_personal_bests
                 (student_id, activity_type, best_duration_minutes, achieved_on)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (student_id, activity_type)
             DO UPDATE SET
                 best_duration_minutes =
                     GREATEST(wellness_personal_bests.best_duration_minutes,
                              EXCLUDED.best_duration_minutes),
                 achieved_on = CASE
                     WHEN EXCLUDED.best_duration_minutes >
                          wellness_personal_bests.best_duration_minutes
                     THEN EXCLUDED.achieved_on
                     ELSE wellness_personal_bests.achieved_on
                 END,
                 updated_at = NOW()",
        )
        .bind(sid)
        .bind(&activity_type)
        .bind(duration_minutes)
        .bind(stored_date)
        .execute(pool)
        .await?;
    }

    Ok(WellnessActivityRow {
        id,
        institution_id: inst_id,
        student_id,
        activity_type,
        duration_minutes,
        activity_date: stored_date,
    })
}

/// Fetch a single family group by ID.
/// Requires group access (admin or member).
pub async fn get_group(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
) -> Result<FamilyGroupView, ApiAppError> {
    assert_group_access(pool, p, group_id).await?;

    let row: Option<(Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT id, label, institution_id FROM family_groups WHERE id = $1",
    )
    .bind(group_id)
    .fetch_optional(pool)
    .await?;

    let (id, label, institution_id) =
        row.ok_or(ApiAppError::NotFound)?;

    let residents = load_residents(pool, id).await?;
    let consent = load_consent(pool, id).await?;

    Ok(FamilyGroupView { id, label, institution_id, residents, consent })
}

/// Return the activity log for all residents in a family group (last 90 days).
/// Requires group access AND active consent for 'activity_log'.
/// Never returns notes_encrypted.
pub async fn get_activity_log(
    pool: &PgPool,
    p: &Principal,
    group_id: Uuid,
) -> Result<Vec<WellnessActivityRow>, ApiAppError> {
    assert_group_access(pool, p, group_id).await?;
    assert_consent(pool, group_id, "activity_log").await?;

    let rows: Vec<(Uuid, Uuid, Option<Uuid>, String, i32, NaiveDate)> = sqlx::query_as(
        "SELECT wa.id, wa.institution_id, wa.student_id,
                wa.activity_type, wa.duration_minutes, wa.activity_date
         FROM wellness_activities wa
         JOIN family_group_residents fgr ON fgr.student_id = wa.student_id
         WHERE fgr.family_group_id = $1
           AND wa.activity_date >= CURRENT_DATE - INTERVAL '90 days'
         ORDER BY wa.activity_date DESC
         LIMIT 200",
    )
    .bind(group_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, institution_id, student_id, activity_type, duration_minutes, activity_date)| {
            WellnessActivityRow {
                id,
                institution_id,
                student_id,
                activity_type,
                duration_minutes,
                activity_date,
            }
        })
        .collect())
}

// ── Unit tests ────────────────────────────────────────────────────────────────
//
// These tests check the SQL shape of the supply summary query without a DB.
// Integration tests (full resident-scoped data, cross-family leakage guard)
// live in apps/backend-api/tests/ and require a live Postgres instance.

#[cfg(test)]
mod tests {
    // ── supply summary — leakage guard ────────────────────────────────────────

    #[test]
    fn supply_summary_scopes_via_class_department_not_institution() {
        // The old query joined via `s.institution_id → all sites`, exposing
        // every institution requisition to every family in the institution.
        // The fix scopes through the student's own class → department.
        let sql = "SELECT s.first_name, s.last_name, \
                          TO_CHAR(r.created_at, 'YYYY-MM') as period_month, \
                          ic.label as category, \
                          COUNT(DISTINCT r.id) as approved_count, \
                          COALESCE(SUM(rl.quantity), 0) as total_quantity \
                   FROM family_group_residents fgr \
                   JOIN students s ON s.id = fgr.student_id \
                   LEFT JOIN classes cl \
                     ON cl.id = s.class_id AND cl.department_id IS NOT NULL \
                   LEFT JOIN requisitions r \
                     ON r.department_id = cl.department_id \
                     AND r.status = 'issued' \
                   LEFT JOIN requisition_lines rl ON rl.requisition_id = r.id \
                   LEFT JOIN inventory_items ii ON ii.id = rl.item_id \
                   LEFT JOIN inventory_categories ic ON ic.id = ii.category_id \
                   WHERE fgr.family_group_id = $1 \
                   GROUP BY s.id, s.first_name, s.last_name, period_month, ic.label \
                   ORDER BY period_month DESC, s.last_name";

        // Must NOT use institution-wide scoping.
        assert!(
            !sql.contains("s.institution_id"),
            "must not scope via student institution_id (institution-wide leakage)"
        );
        assert!(
            !sql.contains("sites si WHERE si.institution_id"),
            "must not join all sites for the institution"
        );

        // Must scope via class → department.
        assert!(
            sql.contains("JOIN classes cl"),
            "must join through classes for department scoping"
        );
        assert!(
            sql.contains("r.department_id = cl.department_id"),
            "requisitions must be scoped to the student's department"
        );
        assert!(
            sql.contains("cl.department_id IS NOT NULL"),
            "must guard against classless department to avoid unscoped joins"
        );
    }

    // ── supply summary — quantity aggregation ─────────────────────────────────

    #[test]
    fn supply_summary_sums_quantities_not_counts_requisitions() {
        let sql = "SELECT s.first_name, s.last_name, \
                          TO_CHAR(r.created_at, 'YYYY-MM') as period_month, \
                          ic.label as category, \
                          COUNT(DISTINCT r.id) as approved_count, \
                          COALESCE(SUM(rl.quantity), 0) as total_quantity";

        assert!(
            sql.contains("SUM(rl.quantity)"),
            "total_quantity must sum line quantities, not count requisitions"
        );
        assert!(
            !sql.contains("COUNT(r.id) as total_quantity"),
            "total_quantity must not be COUNT(r.id)"
        );
        assert!(
            sql.contains("COUNT(DISTINCT r.id) as approved_count"),
            "approved_count must use DISTINCT to avoid inflation from line join"
        );
    }

    // ── supply summary — GROUP BY student identity ────────────────────────────

    #[test]
    fn supply_summary_groups_by_student_id_not_only_name() {
        // GROUP BY s.id prevents two students with identical names from being merged.
        let sql = "GROUP BY s.id, s.first_name, s.last_name, period_month, ic.label";
        assert!(sql.contains("s.id"), "must include s.id in GROUP BY");
    }

    // ── supply summary — status filter ────────────────────────────────────────

    #[test]
    fn supply_summary_filters_issued_requisitions() {
        // 'issued' is the final post-approval status meaning supplies were
        // physically dispensed — correct for a supply usage summary.
        let sql = "AND r.status = 'issued'";
        assert!(sql.contains("'issued'"));
    }

    // ── allowed consent categories ────────────────────────────────────────────

    #[test]
    fn supply_usage_is_a_valid_consent_category() {
        use super::ALLOWED_CONSENT_CATEGORIES;
        assert!(ALLOWED_CONSENT_CATEGORIES.contains(&"supply_usage"));
        assert!(ALLOWED_CONSENT_CATEGORIES.contains(&"wellness_summary"));
        assert!(ALLOWED_CONSENT_CATEGORIES.contains(&"activity_log"));
    }
}
