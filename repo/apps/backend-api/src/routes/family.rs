use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::family::service::{
    self, AddMemberInput, AddResidentInput, ConsentInput, WellnessActivityInput,
};

use crate::security::rbac::Principal;

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

#[derive(serde::Deserialize)]
pub struct WellnessQuery {
    pub institution_id: Uuid,
}

// GET /api/v1/family/groups
#[get("/family/groups")]
pub async fn list_my_groups(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let groups = service::list_my_groups(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(groups))
}

// POST /api/v1/family/groups
#[post("/family/groups")]
pub async fn create_group(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<service::CreateFamilyGroupInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let group = service::create_group(&pool, &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(group))
}

// POST /api/v1/family/groups/{id}/members
#[post("/family/groups/{id}/members")]
pub async fn add_member(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<AddMemberInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::add_member(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

// POST /api/v1/family/groups/{id}/residents
#[post("/family/groups/{id}/residents")]
pub async fn add_resident(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<AddResidentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::add_resident(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

// POST /api/v1/family/groups/{id}/consent
#[post("/family/groups/{id}/consent")]
pub async fn set_consent(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<ConsentInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let view = service::set_consent(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(view))
}

// GET /api/v1/family/groups/{id}/supply-summary
#[get("/family/groups/{id}/supply-summary")]
pub async fn get_supply_summary(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let summary = service::get_supply_summary(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(summary))
}

// GET /api/v1/family/groups/{id}/wellness-summary
#[get("/family/groups/{id}/wellness-summary")]
pub async fn get_wellness_summary(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let summary = service::get_wellness_summary(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(summary))
}

// GET /api/v1/family/groups/{id}
#[get("/family/groups/{id}")]
pub async fn get_group(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let group = service::get_group(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(group))
}

// GET /api/v1/family/groups/{id}/activity-log
#[get("/family/groups/{id}/activity-log")]
pub async fn get_activity_log(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let log = service::get_activity_log(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(log))
}

// POST /api/v1/family/wellness
#[post("/family/wellness")]
pub async fn log_wellness(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<WellnessQuery>,
    body: web::Json<WellnessActivityInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let row =
        service::log_wellness_activity(&pool, &p, query.institution_id, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(row))
}
