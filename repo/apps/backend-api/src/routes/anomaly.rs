use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::anomaly::service;
use crate::error::ApiAppError;
use crate::security::rbac::Principal;

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

#[derive(Deserialize)]
struct EventsQuery {
    unacked: Option<bool>,
}

// GET /api/v1/anomaly/events?unacked=bool
#[get("/anomaly/events")]
pub async fn list_events(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<EventsQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let unacked_only = query.unacked.unwrap_or(false);
    let events = service::list_events(&pool, &p, unacked_only).await?;
    Ok(HttpResponse::Ok().json(events))
}

// POST /api/v1/anomaly/events/{id}/acknowledge
#[post("/anomaly/events/{id}/acknowledge")]
pub async fn acknowledge_event(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    service::acknowledge_event(&pool, &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().finish())
}

// GET /api/v1/anomaly/rules
#[get("/anomaly/rules")]
pub async fn list_rules(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let rules = service::list_rules(&pool, &p).await?;
    Ok(HttpResponse::Ok().json(rules))
}
