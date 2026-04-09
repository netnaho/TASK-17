/// CRUD service layer for master-data reference tables.
///
/// All mutations are scoped to an institution_id. Departments are read through
/// the `sites` join because the `departments` table pre-dates Phase 5 and
/// does not carry an institution_id column directly.
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::security::rbac::Principal;

// ── pagination ─────────────────────────────────────────────────────────────
#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub page:     Option<i64>,
    pub per_page: Option<i64>,
    pub filter:   Option<String>,
    pub sort:     Option<String>,
    pub order:    Option<String>,
}

impl ListParams {
    pub fn offset(&self) -> i64 { (self.page().saturating_sub(1)) * self.per_page() }
    pub fn page(&self) -> i64   { self.page.unwrap_or(1).max(1) }
    pub fn per_page(&self) -> i64 { self.per_page.unwrap_or(20).clamp(1, 200) }
    pub fn order_dir(&self) -> &str {
        match self.order.as_deref() { Some("desc") => "DESC", _ => "ASC" }
    }
}

#[derive(Debug, Serialize)]
pub struct Page<T> {
    pub items:    Vec<T>,
    pub total:    i64,
    pub page:     i64,
    pub per_page: i64,
}

// ── authorization helpers ──────────────────────────────────────────────────
pub fn assert_admin(p: &Principal) -> Result<(), ApiAppError> {
    p.require("admin:users")  // admin:users implies institution-admin level
}

/// Verify that the institution_id is accessible to the principal (either
/// they hold scope:any, or they have an explicit institution scope for it).
pub fn assert_institution_scope(p: &Principal, institution_id: Uuid) -> Result<(), ApiAppError> {
    if p.permissions.contains("scope:any") { return Ok(()); }
    let ok = p.scopes.iter().any(|s| {
        s.kind == crate::security::rbac::ScopeKind::Institution
            && s.reference == institution_id.to_string()
    });
    if ok { Ok(()) } else {
        Err(ApiAppError::Forbidden("institution scope required".into()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SEMESTERS
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SemesterRow {
    pub id: Uuid,
    pub institution_id: Uuid,
    pub code: String,
    pub label: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct SemesterInput {
    pub code: String,
    pub label: String,
    pub starts_on: NaiveDate,
    pub ends_on: NaiveDate,
    #[serde(default = "bool_true")]
    pub is_active: bool,
}

fn bool_true() -> bool { true }

pub async fn list_semesters(
    pool: &PgPool, p: &Principal, institution_id: Uuid, params: &ListParams,
) -> Result<Page<SemesterRow>, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    let filter = params.filter.as_deref().unwrap_or("");
    let like = format!("%{filter}%");
    let sort = match params.sort.as_deref() {
        Some("code") => "code", Some("starts_on") => "starts_on",
        Some("ends_on") => "ends_on", _ => "code",
    };
    let rows: Vec<(Uuid, Uuid, String, String, NaiveDate, NaiveDate, bool)> = sqlx::query_as(
        &format!(
            "SELECT id, institution_id, code, label, starts_on, ends_on, is_active
             FROM semesters
             WHERE institution_id=$1 AND (code ILIKE $2 OR label ILIKE $2)
             ORDER BY {sort} {dir} LIMIT $3 OFFSET $4",
            dir = params.order_dir()
        )
    )
    .bind(institution_id).bind(&like)
    .bind(params.per_page()).bind(params.offset())
    .fetch_all(pool).await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semesters WHERE institution_id=$1 AND (code ILIKE $2 OR label ILIKE $2)",
    ).bind(institution_id).bind(&like).fetch_one(pool).await?;

    Ok(Page {
        items: rows.into_iter().map(|r| SemesterRow {
            id: r.0, institution_id: r.1, code: r.2, label: r.3,
            starts_on: r.4, ends_on: r.5, is_active: r.6,
        }).collect(),
        total: total.0, page: params.page(), per_page: params.per_page(),
    })
}

pub async fn get_semester(pool: &PgPool, p: &Principal, id: Uuid) -> Result<SemesterRow, ApiAppError> {
    assert_admin(p)?;
    let r: Option<(Uuid, Uuid, String, String, NaiveDate, NaiveDate, bool)> = sqlx::query_as(
        "SELECT id, institution_id, code, label, starts_on, ends_on, is_active FROM semesters WHERE id=$1",
    ).bind(id).fetch_optional(pool).await?;
    let r = r.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, r.1)?;
    Ok(SemesterRow { id: r.0, institution_id: r.1, code: r.2, label: r.3, starts_on: r.4, ends_on: r.5, is_active: r.6 })
}

pub async fn create_semester(
    pool: &PgPool, p: &Principal, institution_id: Uuid, input: SemesterInput,
) -> Result<SemesterRow, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    validate_semester_input(&input)?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO semesters (institution_id, code, label, starts_on, ends_on, is_active)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(institution_id).bind(input.code.trim()).bind(input.label.trim())
    .bind(input.starts_on).bind(input.ends_on).bind(input.is_active)
    .fetch_one(pool).await
    .map_err(unique_err("code"))?;
    get_semester(pool, p, row.0).await
}

pub async fn update_semester(
    pool: &PgPool, p: &Principal, id: Uuid, input: SemesterInput,
) -> Result<SemesterRow, ApiAppError> {
    assert_admin(p)?;
    let sem = get_semester(pool, p, id).await?;
    assert_institution_scope(p, sem.institution_id)?;
    validate_semester_input(&input)?;
    sqlx::query(
        "UPDATE semesters SET code=$2, label=$3, starts_on=$4, ends_on=$5, is_active=$6, updated_at=NOW() WHERE id=$1",
    )
    .bind(id).bind(input.code.trim()).bind(input.label.trim())
    .bind(input.starts_on).bind(input.ends_on).bind(input.is_active)
    .execute(pool).await.map_err(unique_err("code"))?;
    get_semester(pool, p, id).await
}

pub async fn delete_semester(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    assert_admin(p)?;
    let sem = get_semester(pool, p, id).await?;
    assert_institution_scope(p, sem.institution_id)?;
    // Guard: block if any class references this semester
    let refs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM classes WHERE semester_id=$1")
        .bind(id).fetch_one(pool).await?;
    if refs.0 > 0 {
        return Err(ApiAppError::BadRequest(format!(
            "cannot delete: {} class(es) reference this semester", refs.0
        )));
    }
    sqlx::query("DELETE FROM semesters WHERE id=$1").bind(id).execute(pool).await?;
    Ok(())
}

fn validate_semester_input(i: &SemesterInput) -> Result<(), ApiAppError> {
    if i.code.trim().is_empty() { return Err(ApiAppError::BadRequest("code required".into())); }
    if i.label.trim().is_empty() { return Err(ApiAppError::BadRequest("label required".into())); }
    if i.ends_on <= i.starts_on {
        return Err(ApiAppError::BadRequest("ends_on must be after starts_on".into()));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// CLASSES
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ClassRow {
    pub id: Uuid,
    pub institution_id: Uuid,
    pub code: String,
    pub label: String,
    pub semester_id: Option<Uuid>,
    pub semester_code: Option<String>,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct ClassInput {
    pub code: String,
    pub label: String,
    pub semester_id: Option<Uuid>,
    pub department_id: Option<Uuid>,
    #[serde(default = "bool_true")]
    pub is_active: bool,
}

pub async fn list_classes(
    pool: &PgPool, p: &Principal, institution_id: Uuid, params: &ListParams,
) -> Result<Page<ClassRow>, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    let like = format!("%{}%", params.filter.as_deref().unwrap_or(""));
    let sort = match params.sort.as_deref() {
        Some("code") => "cl.code", Some("label") => "cl.label", _ => "cl.code",
    };
    let rows: Vec<(Uuid, Uuid, String, String, Option<Uuid>, Option<String>, Option<Uuid>, Option<String>, bool)> =
        sqlx::query_as(&format!(
            "SELECT cl.id, cl.institution_id, cl.code, cl.label,
                    cl.semester_id, s.code, cl.department_id, d.name, cl.is_active
             FROM classes cl
             LEFT JOIN semesters s ON s.id = cl.semester_id
             LEFT JOIN departments d ON d.id = cl.department_id
             WHERE cl.institution_id=$1 AND (cl.code ILIKE $2 OR cl.label ILIKE $2)
             ORDER BY {sort} {dir} LIMIT $3 OFFSET $4",
            dir = params.order_dir()
        ))
        .bind(institution_id).bind(&like)
        .bind(params.per_page()).bind(params.offset())
        .fetch_all(pool).await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM classes WHERE institution_id=$1 AND (code ILIKE $2 OR label ILIKE $2)",
    ).bind(institution_id).bind(&like).fetch_one(pool).await?;

    Ok(Page {
        items: rows.into_iter().map(|r| ClassRow {
            id: r.0, institution_id: r.1, code: r.2, label: r.3,
            semester_id: r.4, semester_code: r.5,
            department_id: r.6, department_name: r.7, is_active: r.8,
        }).collect(),
        total: total.0, page: params.page(), per_page: params.per_page(),
    })
}

pub async fn get_class(pool: &PgPool, p: &Principal, id: Uuid) -> Result<ClassRow, ApiAppError> {
    assert_admin(p)?;
    let r: Option<(Uuid, Uuid, String, String, Option<Uuid>, Option<String>, Option<Uuid>, Option<String>, bool)> =
        sqlx::query_as(
            "SELECT cl.id, cl.institution_id, cl.code, cl.label,
                    cl.semester_id, s.code, cl.department_id, d.name, cl.is_active
             FROM classes cl
             LEFT JOIN semesters s ON s.id = cl.semester_id
             LEFT JOIN departments d ON d.id = cl.department_id
             WHERE cl.id=$1",
        ).bind(id).fetch_optional(pool).await?;
    let r = r.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, r.1)?;
    Ok(ClassRow {
        id: r.0, institution_id: r.1, code: r.2, label: r.3,
        semester_id: r.4, semester_code: r.5,
        department_id: r.6, department_name: r.7, is_active: r.8,
    })
}

pub async fn create_class(
    pool: &PgPool, p: &Principal, institution_id: Uuid, input: ClassInput,
) -> Result<ClassRow, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    if input.code.trim().is_empty() { return Err(ApiAppError::BadRequest("code required".into())); }
    if input.label.trim().is_empty() { return Err(ApiAppError::BadRequest("label required".into())); }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO classes (institution_id, code, label, semester_id, department_id, is_active)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(institution_id).bind(input.code.trim()).bind(input.label.trim())
    .bind(input.semester_id).bind(input.department_id).bind(input.is_active)
    .fetch_one(pool).await.map_err(unique_err("code"))?;
    get_class(pool, p, row.0).await
}

pub async fn update_class(
    pool: &PgPool, p: &Principal, id: Uuid, input: ClassInput,
) -> Result<ClassRow, ApiAppError> {
    assert_admin(p)?;
    let cls = get_class(pool, p, id).await?;
    assert_institution_scope(p, cls.institution_id)?;
    if input.code.trim().is_empty() { return Err(ApiAppError::BadRequest("code required".into())); }
    sqlx::query(
        "UPDATE classes SET code=$2, label=$3, semester_id=$4, department_id=$5, is_active=$6, updated_at=NOW() WHERE id=$1",
    )
    .bind(id).bind(input.code.trim()).bind(input.label.trim())
    .bind(input.semester_id).bind(input.department_id).bind(input.is_active)
    .execute(pool).await.map_err(unique_err("code"))?;
    get_class(pool, p, id).await
}

pub async fn delete_class(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    assert_admin(p)?;
    let cls = get_class(pool, p, id).await?;
    assert_institution_scope(p, cls.institution_id)?;
    let refs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM students WHERE class_id=$1")
        .bind(id).fetch_one(pool).await?;
    if refs.0 > 0 {
        return Err(ApiAppError::BadRequest(format!("{} student(s) reference this class", refs.0)));
    }
    sqlx::query("DELETE FROM classes WHERE id=$1").bind(id).execute(pool).await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// COURSES
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CourseRow {
    pub id: Uuid,
    pub institution_id: Uuid,
    pub code: String,
    pub title: String,
    pub department_id: Option<Uuid>,
    pub department_name: Option<String>,
    pub credits: i32,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct CourseInput {
    pub code: String,
    pub title: String,
    pub department_id: Option<Uuid>,
    #[serde(default)]
    pub credits: i32,
    #[serde(default = "bool_true")]
    pub is_active: bool,
}

pub async fn list_courses(
    pool: &PgPool, p: &Principal, institution_id: Uuid, params: &ListParams,
) -> Result<Page<CourseRow>, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    let like = format!("%{}%", params.filter.as_deref().unwrap_or(""));
    let sort = match params.sort.as_deref() {
        Some("code") => "c.code", Some("title") => "c.title",
        Some("credits") => "c.credits", _ => "c.code",
    };
    let rows: Vec<(Uuid, Uuid, String, String, Option<Uuid>, Option<String>, i32, bool)> = sqlx::query_as(
        &format!(
            "SELECT c.id, c.institution_id, c.code, c.title,
                    c.department_id, d.name, c.credits, c.is_active
             FROM courses c LEFT JOIN departments d ON d.id = c.department_id
             WHERE c.institution_id=$1 AND (c.code ILIKE $2 OR c.title ILIKE $2)
             ORDER BY {sort} {dir} LIMIT $3 OFFSET $4",
            dir = params.order_dir()
        )
    )
    .bind(institution_id).bind(&like)
    .bind(params.per_page()).bind(params.offset())
    .fetch_all(pool).await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM courses WHERE institution_id=$1 AND (code ILIKE $2 OR title ILIKE $2)",
    ).bind(institution_id).bind(&like).fetch_one(pool).await?;

    Ok(Page {
        items: rows.into_iter().map(|r| CourseRow {
            id: r.0, institution_id: r.1, code: r.2, title: r.3,
            department_id: r.4, department_name: r.5, credits: r.6, is_active: r.7,
        }).collect(),
        total: total.0, page: params.page(), per_page: params.per_page(),
    })
}

pub async fn get_course(pool: &PgPool, p: &Principal, id: Uuid) -> Result<CourseRow, ApiAppError> {
    assert_admin(p)?;
    let r: Option<(Uuid, Uuid, String, String, Option<Uuid>, Option<String>, i32, bool)> = sqlx::query_as(
        "SELECT c.id, c.institution_id, c.code, c.title,
                c.department_id, d.name, c.credits, c.is_active
         FROM courses c LEFT JOIN departments d ON d.id = c.department_id
         WHERE c.id=$1",
    ).bind(id).fetch_optional(pool).await?;
    let r = r.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, r.1)?;
    Ok(CourseRow {
        id: r.0, institution_id: r.1, code: r.2, title: r.3,
        department_id: r.4, department_name: r.5, credits: r.6, is_active: r.7,
    })
}

pub async fn create_course(
    pool: &PgPool, p: &Principal, institution_id: Uuid, input: CourseInput,
) -> Result<CourseRow, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    if input.code.trim().is_empty() { return Err(ApiAppError::BadRequest("code required".into())); }
    if input.title.trim().is_empty() { return Err(ApiAppError::BadRequest("title required".into())); }
    if input.credits < 0 { return Err(ApiAppError::BadRequest("credits must be ≥ 0".into())); }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO courses (institution_id, code, title, department_id, credits, is_active)
         VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
    )
    .bind(institution_id).bind(input.code.trim()).bind(input.title.trim())
    .bind(input.department_id).bind(input.credits).bind(input.is_active)
    .fetch_one(pool).await.map_err(unique_err("code"))?;
    get_course(pool, p, row.0).await
}

pub async fn update_course(
    pool: &PgPool, p: &Principal, id: Uuid, input: CourseInput,
) -> Result<CourseRow, ApiAppError> {
    assert_admin(p)?;
    let crs = get_course(pool, p, id).await?;
    assert_institution_scope(p, crs.institution_id)?;
    if input.credits < 0 { return Err(ApiAppError::BadRequest("credits must be ≥ 0".into())); }
    sqlx::query(
        "UPDATE courses SET code=$2, title=$3, department_id=$4, credits=$5, is_active=$6, updated_at=NOW() WHERE id=$1",
    )
    .bind(id).bind(input.code.trim()).bind(input.title.trim())
    .bind(input.department_id).bind(input.credits).bind(input.is_active)
    .execute(pool).await.map_err(unique_err("code"))?;
    get_course(pool, p, id).await
}

pub async fn delete_course(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    assert_admin(p)?;
    let crs = get_course(pool, p, id).await?;
    assert_institution_scope(p, crs.institution_id)?;
    sqlx::query("DELETE FROM courses WHERE id=$1").bind(id).execute(pool).await?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// STUDENTS
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StudentRow {
    pub id: Uuid,
    pub institution_id: Uuid,
    pub student_number: String,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub class_id: Option<Uuid>,
    pub class_code: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
pub struct StudentInput {
    pub student_number: String,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub class_id: Option<Uuid>,
    #[serde(default = "bool_true")]
    pub is_active: bool,
}

pub async fn list_students(
    pool: &PgPool, p: &Principal, institution_id: Uuid, params: &ListParams,
) -> Result<Page<StudentRow>, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    let like = format!("%{}%", params.filter.as_deref().unwrap_or(""));
    let sort = match params.sort.as_deref() {
        Some("student_number") => "s.student_number",
        Some("last_name") => "s.last_name",
        Some("first_name") => "s.first_name",
        _ => "s.student_number",
    };
    let rows: Vec<(Uuid, Uuid, String, String, String, Option<String>, Option<NaiveDate>, Option<Uuid>, Option<String>, bool)> =
        sqlx::query_as(&format!(
            "SELECT s.id, s.institution_id, s.student_number, s.first_name, s.last_name,
                    s.email, s.date_of_birth, s.class_id, cl.code, s.is_active
             FROM students s LEFT JOIN classes cl ON cl.id = s.class_id
             WHERE s.institution_id=$1
               AND (s.student_number ILIKE $2 OR s.first_name ILIKE $2
                    OR s.last_name ILIKE $2 OR COALESCE(s.email,'') ILIKE $2)
             ORDER BY {sort} {dir} LIMIT $3 OFFSET $4",
            dir = params.order_dir()
        ))
        .bind(institution_id).bind(&like)
        .bind(params.per_page()).bind(params.offset())
        .fetch_all(pool).await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM students WHERE institution_id=$1
         AND (student_number ILIKE $2 OR first_name ILIKE $2
              OR last_name ILIKE $2 OR COALESCE(email,'') ILIKE $2)",
    ).bind(institution_id).bind(&like).fetch_one(pool).await?;

    Ok(Page {
        items: rows.into_iter().map(|r| StudentRow {
            id: r.0, institution_id: r.1, student_number: r.2,
            first_name: r.3, last_name: r.4, email: r.5, date_of_birth: r.6,
            class_id: r.7, class_code: r.8, is_active: r.9,
        }).collect(),
        total: total.0, page: params.page(), per_page: params.per_page(),
    })
}

pub async fn get_student(pool: &PgPool, p: &Principal, id: Uuid) -> Result<StudentRow, ApiAppError> {
    assert_admin(p)?;
    let r: Option<(Uuid, Uuid, String, String, String, Option<String>, Option<NaiveDate>, Option<Uuid>, Option<String>, bool)> =
        sqlx::query_as(
            "SELECT s.id, s.institution_id, s.student_number, s.first_name, s.last_name,
                    s.email, s.date_of_birth, s.class_id, cl.code, s.is_active
             FROM students s LEFT JOIN classes cl ON cl.id = s.class_id
             WHERE s.id=$1",
        ).bind(id).fetch_optional(pool).await?;
    let r = r.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, r.1)?;
    Ok(StudentRow {
        id: r.0, institution_id: r.1, student_number: r.2,
        first_name: r.3, last_name: r.4, email: r.5, date_of_birth: r.6,
        class_id: r.7, class_code: r.8, is_active: r.9,
    })
}

pub async fn create_student(
    pool: &PgPool, p: &Principal, institution_id: Uuid, input: StudentInput,
) -> Result<StudentRow, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    validate_student_input(&input)?;
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO students (institution_id, student_number, first_name, last_name, email, date_of_birth, class_id, is_active)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id",
    )
    .bind(institution_id).bind(input.student_number.trim())
    .bind(input.first_name.trim()).bind(input.last_name.trim())
    .bind(input.email.as_deref()).bind(input.date_of_birth)
    .bind(input.class_id).bind(input.is_active)
    .fetch_one(pool).await.map_err(unique_err("student_number"))?;
    get_student(pool, p, row.0).await
}

pub async fn update_student(
    pool: &PgPool, p: &Principal, id: Uuid, input: StudentInput,
) -> Result<StudentRow, ApiAppError> {
    assert_admin(p)?;
    let stu = get_student(pool, p, id).await?;
    assert_institution_scope(p, stu.institution_id)?;
    validate_student_input(&input)?;
    sqlx::query(
        "UPDATE students SET student_number=$2, first_name=$3, last_name=$4, email=$5,
         date_of_birth=$6, class_id=$7, is_active=$8, updated_at=NOW() WHERE id=$1",
    )
    .bind(id).bind(input.student_number.trim())
    .bind(input.first_name.trim()).bind(input.last_name.trim())
    .bind(input.email.as_deref()).bind(input.date_of_birth)
    .bind(input.class_id).bind(input.is_active)
    .execute(pool).await.map_err(unique_err("student_number"))?;
    get_student(pool, p, id).await
}

pub async fn delete_student(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    assert_admin(p)?;
    let stu = get_student(pool, p, id).await?;
    assert_institution_scope(p, stu.institution_id)?;
    sqlx::query("DELETE FROM students WHERE id=$1").bind(id).execute(pool).await?;
    Ok(())
}

fn validate_student_input(i: &StudentInput) -> Result<(), ApiAppError> {
    if i.student_number.trim().is_empty() {
        return Err(ApiAppError::BadRequest("student_number required".into()));
    }
    if i.first_name.trim().is_empty() {
        return Err(ApiAppError::BadRequest("first_name required".into()));
    }
    if i.last_name.trim().is_empty() {
        return Err(ApiAppError::BadRequest("last_name required".into()));
    }
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// DEPARTMENTS  (read-through existing table, scope via sites join)
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DepartmentRow {
    pub id: Uuid,
    pub site_id: Uuid,
    pub institution_id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct DepartmentInput {
    pub site_id: Uuid,
    pub name: String,
}

pub async fn list_departments(
    pool: &PgPool, p: &Principal, institution_id: Uuid, params: &ListParams,
) -> Result<Page<DepartmentRow>, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    let like = format!("%{}%", params.filter.as_deref().unwrap_or(""));
    let rows: Vec<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        &format!(
            "SELECT d.id, d.site_id, si.institution_id, d.name
             FROM departments d JOIN sites si ON si.id = d.site_id
             WHERE si.institution_id=$1 AND d.name ILIKE $2
             ORDER BY d.name {dir} LIMIT $3 OFFSET $4",
            dir = params.order_dir()
        )
    )
    .bind(institution_id).bind(&like)
    .bind(params.per_page()).bind(params.offset())
    .fetch_all(pool).await?;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM departments d JOIN sites si ON si.id=d.site_id
         WHERE si.institution_id=$1 AND d.name ILIKE $2",
    ).bind(institution_id).bind(&like).fetch_one(pool).await?;

    Ok(Page {
        items: rows.into_iter().map(|r| DepartmentRow {
            id: r.0, site_id: r.1, institution_id: r.2, name: r.3,
        }).collect(),
        total: total.0, page: params.page(), per_page: params.per_page(),
    })
}

pub async fn get_department_admin(pool: &PgPool, p: &Principal, id: Uuid) -> Result<DepartmentRow, ApiAppError> {
    assert_admin(p)?;
    let r: Option<(Uuid, Uuid, Uuid, String)> = sqlx::query_as(
        "SELECT d.id, d.site_id, si.institution_id, d.name
         FROM departments d JOIN sites si ON si.id=d.site_id WHERE d.id=$1",
    ).bind(id).fetch_optional(pool).await?;
    let r = r.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, r.2)?;
    Ok(DepartmentRow { id: r.0, site_id: r.1, institution_id: r.2, name: r.3 })
}

pub async fn create_department(
    pool: &PgPool, p: &Principal, institution_id: Uuid, input: DepartmentInput,
) -> Result<DepartmentRow, ApiAppError> {
    assert_admin(p)?;
    assert_institution_scope(p, institution_id)?;
    if input.name.trim().is_empty() { return Err(ApiAppError::BadRequest("name required".into())); }
    // Verify the site belongs to the institution
    let site_ok: Option<(bool,)> = sqlx::query_as(
        "SELECT true FROM sites WHERE id=$1 AND institution_id=$2",
    ).bind(input.site_id).bind(institution_id).fetch_optional(pool).await?;
    if site_ok.is_none() {
        return Err(ApiAppError::BadRequest("site does not belong to this institution".into()));
    }
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO departments (site_id, name) VALUES ($1,$2) RETURNING id",
    )
    .bind(input.site_id).bind(input.name.trim())
    .fetch_one(pool).await.map_err(unique_err("name"))?;
    get_department_admin(pool, p, row.0).await
}

pub async fn update_department(
    pool: &PgPool, p: &Principal, id: Uuid, input: DepartmentInput,
) -> Result<DepartmentRow, ApiAppError> {
    assert_admin(p)?;
    let dept = get_department_admin(pool, p, id).await?;
    assert_institution_scope(p, dept.institution_id)?;
    if input.name.trim().is_empty() { return Err(ApiAppError::BadRequest("name required".into())); }
    sqlx::query("UPDATE departments SET name=$2 WHERE id=$1")
        .bind(id).bind(input.name.trim())
        .execute(pool).await.map_err(unique_err("name"))?;
    get_department_admin(pool, p, id).await
}

pub async fn delete_department(pool: &PgPool, p: &Principal, id: Uuid) -> Result<(), ApiAppError> {
    assert_admin(p)?;
    let dept = get_department_admin(pool, p, id).await?;
    assert_institution_scope(p, dept.institution_id)?;
    // Guard: classes and courses reference departments
    let class_refs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM classes WHERE department_id=$1")
        .bind(id).fetch_one(pool).await?;
    let course_refs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM courses WHERE department_id=$1")
        .bind(id).fetch_one(pool).await?;
    let total = class_refs.0 + course_refs.0;
    if total > 0 {
        return Err(ApiAppError::BadRequest(format!(
            "cannot delete: {} class(es)/course(s) reference this department", total
        )));
    }
    sqlx::query("DELETE FROM departments WHERE id=$1").bind(id).execute(pool).await?;
    Ok(())
}

// ── institutions list ──────────────────────────────────────────────────────
#[derive(Debug, Serialize)]
pub struct InstitutionRow {
    pub id: Uuid,
    pub name: String,
    pub code: String,
}

pub async fn list_institutions(pool: &PgPool, p: &Principal) -> Result<Vec<InstitutionRow>, ApiAppError> {
    assert_admin(p)?;
    let rows: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, name, code FROM institutions ORDER BY name",
    ).fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| InstitutionRow { id: r.0, name: r.1, code: r.2 }).collect())
}

// ── import job status ──────────────────────────────────────────────────────
#[derive(Debug, Serialize)]
pub struct ImportJobDetail {
    pub id: Uuid,
    pub entity_type: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub file_fingerprint: String,
    pub mime_type: String,
    pub status: String,
    pub total_rows: i32,
    pub accepted_rows: i32,
    pub rejected_rows: i32,
    pub error_message: Option<String>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub rows: Vec<ImportJobRowDetail>,
}

#[derive(Debug, Serialize)]
pub struct ImportJobRowDetail {
    pub row_number: i32,
    pub status: String,
    pub raw_data: serde_json::Value,
    pub rejection_reason: Option<String>,
    pub entity_id: Option<Uuid>,
}

pub async fn get_import_job(pool: &PgPool, p: &Principal, job_id: Uuid) -> Result<ImportJobDetail, ApiAppError> {
    assert_admin(p)?;
    let job: Option<(Uuid, String, Uuid, String, i64, String, String, String, i32, i32, i32, Option<String>, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, entity_type, institution_id, file_name, file_size_bytes, file_fingerprint,
                    mime_type, status, total_rows, accepted_rows, rejected_rows,
                    error_message, completed_at, created_at
             FROM import_jobs WHERE id=$1",
        ).bind(job_id).fetch_optional(pool).await?;
    let j = job.ok_or(ApiAppError::NotFound)?;
    assert_institution_scope(p, j.2)?;

    let rows: Vec<(i32, String, serde_json::Value, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT row_number, status, raw_data, rejection_reason, entity_id
         FROM import_job_rows WHERE job_id=$1 ORDER BY row_number",
    ).bind(job_id).fetch_all(pool).await?;

    Ok(ImportJobDetail {
        id: j.0, entity_type: j.1, file_name: j.3,
        file_size_bytes: j.4, file_fingerprint: j.5, mime_type: j.6,
        status: j.7, total_rows: j.8, accepted_rows: j.9, rejected_rows: j.10,
        error_message: j.11, completed_at: j.12, created_at: j.13,
        rows: rows.into_iter().map(|r| ImportJobRowDetail {
            row_number: r.0, status: r.1, raw_data: r.2,
            rejection_reason: r.3, entity_id: r.4,
        }).collect(),
    })
}

// ── error mapping helper ──────────────────────────────────────────────────
fn unique_err(field: &'static str) -> impl Fn(sqlx::Error) -> ApiAppError {
    move |e: sqlx::Error| {
        if let sqlx::Error::Database(db) = &e {
            if db.constraint().is_some() {
                return ApiAppError::BadRequest(format!("{field} already exists"));
            }
        }
        ApiAppError::Internal(e.to_string())
    }
}
