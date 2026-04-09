use actix_web::{get, post, web, HttpMessage, HttpRequest, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiAppError;
use crate::requisitions::service::{
    self, CreateRequisitionInput, DecisionInput,
};
use crate::security::rbac::{Principal, ScopeKind};

fn principal(req: &HttpRequest) -> Result<Principal, ApiAppError> {
    req.extensions()
        .get::<Principal>()
        .cloned()
        .ok_or_else(|| ApiAppError::Unauthorized("no principal".into()))
}

// ---- catalog ----
#[derive(Serialize)]
struct CatalogItem {
    id: Uuid,
    sku: String,
    name: String,
    unit: String,
    unit_price_cents: i64,
    on_hand: i32,
    controlled: bool,
    category: String,
}

#[get("/inventory/catalog")]
pub async fn catalog(pool: web::Data<PgPool>, req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    p.require("inventory:read").or_else(|_| p.require("requisitions:write"))?;
    // Show stock at the FIRST department-scoped site (Phase 3 demo).
    // The site picker is wired in a later phase.
    let scope_any = p.permissions.contains("scope:any");
    let dept_refs: Vec<String> = p.scopes.iter()
        .filter(|s| s.kind == ScopeKind::Department)
        .map(|s| s.reference.clone()).collect();
    let site_id: Option<(Uuid,)> = if scope_any {
        sqlx::query_as("SELECT id FROM sites ORDER BY name LIMIT 1").fetch_optional(pool.get_ref()).await?
    } else if let Some(d) = dept_refs.first() {
        sqlx::query_as("SELECT site_id FROM departments WHERE id = $1::uuid")
            .bind(d).fetch_optional(pool.get_ref()).await?
    } else { None };
    let site_id = site_id.map(|s| s.0);

    let rows: Vec<(Uuid, String, String, String, i64, bool, String, Option<i32>)> = sqlx::query_as(
        "SELECT i.id, i.sku, i.name, i.unit, i.unit_price_cents, c.controlled, c.label,
                (SELECT on_hand FROM inventory_stock_by_site s WHERE s.item_id=i.id AND s.site_id=$1)
         FROM inventory_items i JOIN inventory_categories c ON c.id=i.category_id
         WHERE i.is_active ORDER BY i.name",
    ).bind(site_id).fetch_all(pool.get_ref()).await?;

    let out: Vec<CatalogItem> = rows.into_iter().map(|r| CatalogItem {
        id: r.0, sku: r.1, name: r.2, unit: r.3,
        unit_price_cents: r.4, controlled: r.5, category: r.6,
        on_hand: r.7.unwrap_or(0),
    }).collect();
    Ok(HttpResponse::Ok().json(out))
}

// ---- requisitions ----
#[post("/requisitions")]
pub async fn create(
    pool: web::Data<PgPool>,
    body: web::Json<CreateRequisitionInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::create_draft(pool.get_ref(), &p, body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[post("/requisitions/{id}/submit")]
pub async fn submit(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::submit(pool.get_ref(), &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[post("/requisitions/{id}/withdraw")]
pub async fn withdraw(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::withdraw(pool.get_ref(), &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[post("/requisitions/{id}/approve")]
pub async fn approve(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::approve(pool.get_ref(), &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[post("/requisitions/{id}/reject")]
pub async fn reject(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::reject(pool.get_ref(), &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[post("/requisitions/{id}/send-back")]
pub async fn send_back(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<DecisionInput>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::send_back(pool.get_ref(), &p, path.into_inner(), body.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[get("/requisitions/{id}")]
pub async fn get_one(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let r = service::load_detail(pool.get_ref(), &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(r))
}

#[get("/requisitions/mine/list")]
pub async fn list_mine(pool: web::Data<PgPool>, req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    Ok(HttpResponse::Ok().json(service::list_for_requester(pool.get_ref(), &p).await?))
}

#[get("/approvals/inbox")]
pub async fn inbox(pool: web::Data<PgPool>, req: HttpRequest) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    Ok(HttpResponse::Ok().json(service::approver_inbox(pool.get_ref(), &p).await?))
}

#[get("/requisitions/{id}/audit")]
pub async fn audit_timeline(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    let detail = service::load_detail(pool.get_ref(), &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(detail.audit))
}

#[get("/issue-records/{req_id}")]
pub async fn issue_record(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    req: HttpRequest,
) -> Result<HttpResponse, ApiAppError> {
    let p = principal(&req)?;
    // Authorization and data loading delegated to service layer.
    // `load_issue_record` applies the same read policy as `load_detail`:
    //   owner | (requisitions:approve AND dept-scoped) | scope:any
    let view = service::load_issue_record(pool.get_ref(), &p, path.into_inner()).await?;
    Ok(HttpResponse::Ok().json(view))
}
