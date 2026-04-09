use actix_web::{get, web, HttpMessage, HttpRequest, HttpResponse};
use serde::Deserialize;
use uuid::Uuid;

use crate::analytics::service;
use crate::error::ApiAppError;
use crate::security::rbac::Principal;

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

#[derive(Deserialize)]
struct DashboardQuery {
    institution_id: Uuid,
}

#[derive(Deserialize)]
struct DailyQuery {
    institution_id: Uuid,
    date: chrono::NaiveDate,
}

#[derive(Deserialize)]
struct WeeklyQuery {
    institution_id: Uuid,
    week_start: chrono::NaiveDate,
}

#[derive(Deserialize)]
struct MonthlyQuery {
    institution_id: Uuid,
    year_month: String,
}

// GET /api/v1/analytics/dashboard?institution_id=UUID
#[get("/analytics/dashboard")]
pub async fn dashboard(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<DashboardQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let view = service::get_dashboard(&pool, &p, query.institution_id).await?;
    Ok(HttpResponse::Ok().json(view))
}

// GET /api/v1/analytics/daily?institution_id=UUID&date=YYYY-MM-DD
#[get("/analytics/daily")]
pub async fn daily_stats(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<DailyQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let stats = service::get_daily_stats(&pool, &p, query.institution_id, query.date).await?;
    Ok(HttpResponse::Ok().json(stats))
}

// GET /api/v1/analytics/weekly?institution_id=UUID&week_start=YYYY-MM-DD
#[get("/analytics/weekly")]
pub async fn weekly_stats(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<WeeklyQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let stats =
        service::get_weekly_stats(&pool, &p, query.institution_id, query.week_start).await?;
    Ok(HttpResponse::Ok().json(stats))
}

// GET /api/v1/analytics/monthly?institution_id=UUID&year_month=YYYY-MM
#[get("/analytics/monthly")]
pub async fn monthly_stats(
    req: HttpRequest,
    pool: web::Data<sqlx::PgPool>,
    query: web::Query<MonthlyQuery>,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let stats =
        service::get_monthly_stats(&pool, &p, query.institution_id, &query.year_month).await?;
    Ok(HttpResponse::Ok().json(stats))
}
