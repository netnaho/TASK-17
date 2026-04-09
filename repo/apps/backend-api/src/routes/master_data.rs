use actix_multipart::Multipart;
use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::master_data::{export, import, service};
use crate::security::rbac::Principal;

// ── helpers ────────────────────────────────────────────────────────────────
fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

// ── institutions ────────────────────────────────────────────────────────────
// GET /api/v1/master-data/institutions
#[get("/master-data/institutions")]
pub async fn list_institutions(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let rows = service::list_institutions(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(rows))
}

// ═══════════════════════════════════════════════════════════════════════════
// SEMESTERS  /api/v1/master-data/institutions/{iid}/semesters
// ═══════════════════════════════════════════════════════════════════════════
#[get("/master-data/institutions/{iid}/semesters")]
pub async fn list_semesters(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<service::ListParams>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let page = service::list_semesters(&pool, &p, path.into_inner(), &query).await?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/master-data/semesters/{id}")]
pub async fn get_semester(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::get_semester(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/institutions/{iid}/semesters")]
pub async fn create_semester(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::SemesterInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::create_semester(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(row))
}

#[post("/master-data/semesters/{id}/update")]
pub async fn update_semester(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::SemesterInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::update_semester(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/semesters/{id}/delete")]
pub async fn delete_semester(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_semester(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ═══════════════════════════════════════════════════════════════════════════
// CLASSES
// ═══════════════════════════════════════════════════════════════════════════
#[get("/master-data/institutions/{iid}/classes")]
pub async fn list_classes(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<service::ListParams>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let page = service::list_classes(&pool, &p, path.into_inner(), &query).await?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/master-data/classes/{id}")]
pub async fn get_class(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::get_class(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/institutions/{iid}/classes")]
pub async fn create_class(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::ClassInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::create_class(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(row))
}

#[post("/master-data/classes/{id}/update")]
pub async fn update_class(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::ClassInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::update_class(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/classes/{id}/delete")]
pub async fn delete_class(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_class(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ═══════════════════════════════════════════════════════════════════════════
// COURSES
// ═══════════════════════════════════════════════════════════════════════════
#[get("/master-data/institutions/{iid}/courses")]
pub async fn list_courses(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<service::ListParams>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let page = service::list_courses(&pool, &p, path.into_inner(), &query).await?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/master-data/courses/{id}")]
pub async fn get_course(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::get_course(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/institutions/{iid}/courses")]
pub async fn create_course(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::CourseInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::create_course(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(row))
}

#[post("/master-data/courses/{id}/update")]
pub async fn update_course(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::CourseInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::update_course(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/courses/{id}/delete")]
pub async fn delete_course(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_course(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ═══════════════════════════════════════════════════════════════════════════
// STUDENTS
// ═══════════════════════════════════════════════════════════════════════════
#[get("/master-data/institutions/{iid}/students")]
pub async fn list_students(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<service::ListParams>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let page = service::list_students(&pool, &p, path.into_inner(), &query).await?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/master-data/students/{id}")]
pub async fn get_student(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::get_student(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/institutions/{iid}/students")]
pub async fn create_student(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::StudentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::create_student(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(row))
}

#[post("/master-data/students/{id}/update")]
pub async fn update_student(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::StudentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::update_student(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/students/{id}/delete")]
pub async fn delete_student(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_student(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ═══════════════════════════════════════════════════════════════════════════
// DEPARTMENTS
// ═══════════════════════════════════════════════════════════════════════════
#[get("/master-data/institutions/{iid}/departments")]
pub async fn list_departments(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<service::ListParams>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let page = service::list_departments(&pool, &p, path.into_inner(), &query).await?;
    Ok(HttpResponse::Ok().json(page))
}

#[get("/master-data/departments/{id}")]
pub async fn get_department(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::get_department_admin(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/institutions/{iid}/departments")]
pub async fn create_department(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::DepartmentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::create_department(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Created().json(row))
}

#[post("/master-data/departments/{id}/update")]
pub async fn update_department(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<service::DepartmentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row = service::update_department(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}

#[post("/master-data/departments/{id}/delete")]
pub async fn delete_department(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_department(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::NoContent().finish())
}

// ═══════════════════════════════════════════════════════════════════════════
// IMPORT  POST /api/v1/master-data/institutions/{iid}/import/{entity_type}
// ═══════════════════════════════════════════════════════════════════════════
#[post("/master-data/institutions/{iid}/import/{entity_type}")]
pub async fn run_import(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<(Uuid, String)>,
    payload: Multipart,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let (institution_id, entity_type) = path.into_inner();
    let result = import::run_import(&pool, &p, entity_type, institution_id, payload).await?;
    let status = if result.rejected_rows == 0 {
        actix_web::http::StatusCode::OK
    } else {
        actix_web::http::StatusCode::MULTI_STATUS
    };
    Ok(HttpResponse::build(status).json(result))
}

// GET /api/v1/master-data/import-jobs/{job_id}
#[get("/master-data/import-jobs/{job_id}")]
pub async fn get_import_job(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let job = service::get_import_job(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(job))
}

// ═══════════════════════════════════════════════════════════════════════════
// EXPORT  GET /api/v1/master-data/institutions/{iid}/export/{entity_type}
// ═══════════════════════════════════════════════════════════════════════════
#[derive(serde::Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
}

#[get("/master-data/institutions/{iid}/export/{entity_type}")]
pub async fn export_entity(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<(Uuid, String)>,
    query: web::Query<ExportQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let (institution_id, entity_type) = path.into_inner();
    let format = query.format.as_deref().unwrap_or("csv");
    let out = export::export_entity(&pool, &p, &entity_type, institution_id, format).await?;
    Ok(HttpResponse::Ok()
        .content_type(out.content_type)
        .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", out.file_name)))
        .insert_header(("X-Row-Count", out.row_count.to_string()))
        .body(out.bytes))
}
