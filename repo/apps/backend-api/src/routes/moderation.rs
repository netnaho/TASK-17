use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::moderation::service::{self, CreatePolicyInput, ReviewInput};
use crate::security::rbac::Principal;

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

#[derive(Deserialize)]
struct QueueQuery {
    status: Option<String>,
}

// GET /api/v1/moderation/queue?status=
#[get("/moderation/queue")]
pub async fn list_queue(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<QueueQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let items = service::list_queue(&pool, &p, query.status.as_deref()).await?;
    Ok(HttpResponse::Ok().json(items))
}

// POST /api/v1/moderation/queue/{id}/review
#[post("/moderation/queue/{id}/review")]
pub async fn review_item(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<ReviewInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::review_item(&pool, &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

// GET /api/v1/moderation/policies
#[get("/moderation/policies")]
pub async fn list_policies(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let policies = service::list_policies(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(policies))
}

// POST /api/v1/moderation/policies
#[post("/moderation/policies")]
pub async fn create_policy(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    body: web::Json<CreatePolicyInput>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let policy = service::create_policy(&pool, &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(policy))
}

// POST /api/v1/moderation/policies/{id}/delete
#[post("/moderation/policies/{id}/delete")]
pub async fn delete_policy(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::delete_policy(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}
